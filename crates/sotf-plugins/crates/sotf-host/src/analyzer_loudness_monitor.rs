// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================

use crate::analyzer::{LoudnessData, RealTimeCache};
use crate::analyzer_channel_correlation::ChannelCorrelationMonitor;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use crate::speaker_config::ChannelLayout;
use math_audio_dsp::ebur128::{EbuR128, Mode};
use math_audio_dsp::fast_math::fast_log10;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessInfo {
    pub momentary_lufs: f64,
    pub shortterm_lufs: f64,
    pub integrated_lufs: f64,
    pub peak: f64,
}

pub struct LoudnessMonitor {
    ebur128: EbuR128,
    /// Raw, unscaled meter used only for sample/true peaks when an explicit
    /// layout requires role-dependent loudness scaling.
    peak_meter: Option<EbuR128>,
    channels: u32,
    sample_rate: u32,
    channel_layout: Option<ChannelLayout>,
    loudness_channels: usize,
    loudness_gains: Vec<f32>,
    weighted_scratch: Vec<f32>,
    /// When true, also maintain a full inter-channel Pearson r matrix and
    /// write it into `LoudnessData.correlation_matrix` on each update.
    /// Off by default — only the output-side LoudnessMonitor that feeds the
    /// spatial-spider widget needs to opt in. CLI tools, JSON dumps, and
    /// per-meter LoudnessMonitors keep the field empty for zero cost.
    spatial_enabled: bool,
    /// Full inter-channel correlation matrix accumulator. Lazily exercised:
    /// when `spatial_enabled == false`, `add_frames` skips it entirely and
    /// `update_loudness_data` leaves `LoudnessData.correlation_matrix` as the
    /// empty `Arc<Vec<f32>>` the caller constructed.
    correlation_matrix: ChannelCorrelationMonitor,
    /// Scratch buffer used to read the matrix into a contiguous slice for
    /// `LoudnessData::update_correlation_matrix`. Reused across calls to
    /// keep the audio-thread allocation count at zero.
    matrix_scratch: crate::analyzer::CorrelationData,
    /// Pre-allocated per-channel peak buffers sized to `channels`. Reused on
    /// every `update_loudness_data` call so >32-channel layouts (22.2, Atmos
    /// beds) do not silently truncate.
    peaks_buf: Vec<f64>,
    true_peaks_buf: Vec<f64>,
    query_error_generation: u64,
    frames_seen: u64,
}

impl LoudnessMonitor {
    pub fn new(channels: u32, sr: u32) -> Result<Self, String> {
        Self::new_inner(channels, sr, None)
    }

    pub fn new_with_layout(
        channels: u32,
        sr: u32,
        channel_layout: ChannelLayout,
    ) -> Result<Self, String> {
        Self::new_inner(channels, sr, Some(channel_layout))
    }

    fn new_inner(
        channels: u32,
        sr: u32,
        channel_layout: Option<ChannelLayout>,
    ) -> Result<Self, String> {
        if channels == 0 {
            return Err("loudness monitor requires at least one channel".to_string());
        }
        if sr == 0 {
            return Err("loudness monitor sample rate must be non-zero".to_string());
        }
        if let Some(layout) = &channel_layout {
            layout.validate_for_width(channels as usize)?;
        }
        // math-dsp's 5/6-channel meters embed a fixed assumed order. Use a
        // seven-channel unity-weight meter for explicit 5.0/5.1 layouts, then
        // supply role weights through sample scaling. Other widths already
        // have unity internal weights.
        let loudness_channels = if channel_layout.is_some() && matches!(channels, 5 | 6) {
            7
        } else {
            channels as usize
        };
        let loudness_mode = if channel_layout.is_some() {
            Mode::M | Mode::S | Mode::I
        } else {
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK | Mode::TRUE_PEAK
        };
        let ebur = EbuR128::new(loudness_channels as u32, sr, loudness_mode)
            .map_err(|e| format!("{:?}", e))?;
        let peak_meter = channel_layout
            .as_ref()
            .map(|_| EbuR128::new(channels, sr, Mode::SAMPLE_PEAK | Mode::TRUE_PEAK))
            .transpose()
            .map_err(|e| format!("{e:?}"))?;
        let loudness_gains = channel_layout
            .as_ref()
            .map(|layout| {
                (0..channels as usize)
                    .map(|index| {
                        layout
                            .role_at(index)
                            .expect("validated channel layout covers every index")
                            .bs1770_weight()
                            .sqrt()
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            ebur128: ebur,
            peak_meter,
            channels,
            sample_rate: sr,
            channel_layout,
            loudness_channels,
            loudness_gains,
            // Fixed-size chunking keeps arbitrary callback sizes allocation
            // free while bounding control-time scratch allocation.
            weighted_scratch: vec![0.0; loudness_channels * 256],
            spatial_enabled: false,
            correlation_matrix: ChannelCorrelationMonitor::new(channels as usize, sr),
            matrix_scratch: crate::analyzer::CorrelationData::new(channels as usize),
            peaks_buf: vec![0.0; channels as usize],
            true_peaks_buf: vec![0.0; channels as usize],
            query_error_generation: 0,
            frames_seen: 0,
        })
    }

    pub fn channel_layout(&self) -> Option<&ChannelLayout> {
        self.channel_layout.as_ref()
    }

    /// Enable / disable the inter-channel Pearson r matrix.
    ///
    /// Default is `false`. When enabled, `add_frames` accumulates correlation
    /// state and `update_loudness_data` writes the matrix into
    /// `LoudnessData.correlation_matrix`. When disabled, both paths skip the
    /// extra work and the matrix stays empty.
    pub fn set_spatial_enabled(&mut self, enabled: bool) {
        if !enabled && self.spatial_enabled {
            // Leaving the on-state: clear so the next enable starts fresh.
            self.correlation_matrix.reset();
        }
        self.spatial_enabled = enabled;
    }

    /// Builder-style helper for `set_spatial_enabled(true)`.
    pub fn with_spatial(mut self) -> Self {
        self.set_spatial_enabled(true);
        self
    }

    /// True when the spatial correlation matrix is being maintained.
    pub fn spatial_enabled(&self) -> bool {
        self.spatial_enabled
    }

    pub fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        if !samples.len().is_multiple_of(self.channels as usize) {
            return Err(format!(
                "loudness input has {} samples, not a whole number of {}-channel frames",
                samples.len(),
                self.channels
            ));
        }
        // Stereo width and the optional spatial matrix share one centered,
        // per-frame EMA accumulator, so callback partitioning cannot change
        // the published result.
        if self.channels == 2 || self.spatial_enabled {
            self.correlation_matrix.add_frames(samples);
        }

        if let Some(peak_meter) = &mut self.peak_meter {
            peak_meter
                .add_frames_f32(samples)
                .map_err(|error| format!("EBU R128 peak add_frames failed: {error:?}"))?;
            let input_channels = self.channels as usize;
            for input in samples.chunks(256 * input_channels) {
                let frames = input.len() / input_channels;
                let scratch_len = frames * self.loudness_channels;
                let scratch = &mut self.weighted_scratch[..scratch_len];
                scratch.fill(0.0);
                for (input_frame, output_frame) in input
                    .chunks_exact(input_channels)
                    .zip(scratch.chunks_exact_mut(self.loudness_channels))
                {
                    for channel in 0..input_channels {
                        output_frame[channel] = input_frame[channel] * self.loudness_gains[channel];
                    }
                }
                self.ebur128
                    .add_frames_f32(scratch)
                    .map_err(|error| format!("EBU R128 loudness add_frames failed: {error:?}"))?;
            }
        } else {
            self.ebur128
                .add_frames_f32(samples)
                .map_err(|error| format!("EBU R128 add_frames failed: {error:?}"))?;
        }
        self.frames_seen = self
            .frames_seen
            .saturating_add((samples.len() / self.channels as usize) as u64);
        Ok(())
    }

    /// Update LoudnessData in-place to avoid allocations
    pub fn update_loudness_data(&mut self, d: &mut LoudnessData) {
        let momentary = self.ebur128.loudness_momentary();
        let shortterm = self.ebur128.loudness_shortterm();
        let integrated = self.ebur128.loudness_global();
        let mut valid = momentary.is_ok()
            && shortterm.is_ok()
            && integrated.is_ok()
            && self.frames_seen >= self.sample_rate as u64 * 3;
        d.momentary_lufs = momentary.unwrap_or(f64::NEG_INFINITY);
        d.shortterm_lufs = shortterm.unwrap_or(f64::NEG_INFINITY);
        d.integrated_lufs = integrated.unwrap_or(f64::NEG_INFINITY);

        // Use the pre-allocated per-channel buffers (no stack-array channel
        // limit, so 22.2 / Atmos beds are not silently truncated).
        let nc = self.channels as usize;
        if self.peaks_buf.len() < nc {
            self.peaks_buf.resize(nc, 0.0);
            self.true_peaks_buf.resize(nc, 0.0);
        }
        let peaks = &mut self.peaks_buf[..nc];
        let tps = &mut self.true_peaks_buf[..nc];

        for ch in 0..nc {
            let peak_meter = self.peak_meter.as_mut().unwrap_or(&mut self.ebur128);
            let sample_peak = peak_meter.prev_sample_peak(ch as u32);
            let true_peak = peak_meter.prev_true_peak(ch as u32);
            valid &= sample_peak.is_ok() && true_peak.is_ok();
            peaks[ch] = sample_peak.unwrap_or(0.0);
            let tp_linear = true_peak.unwrap_or(0.0);
            tps[ch] = if tp_linear > 0.0 {
                // Use fast math for true peak dB conversion
                20.0 * fast_log10(tp_linear as f32) as f64
            } else {
                f64::NEG_INFINITY
            };
        }

        d.update_peaks(peaks);
        d.update_true_peaks(tps);

        d.peak = d.channel_peaks.iter().copied().fold(0.0, f64::max);
        if !valid {
            self.query_error_generation = self.query_error_generation.saturating_add(1);
        }
        d.measurement_valid = valid;
        d.measurement_enabled = true;
        d.channel_layout_is_compliant = self.channel_layout.is_some() || self.channels <= 2;
        d.query_error_generation = self.query_error_generation;
        d.true_peak_is_compliant = self.sample_rate == 48_000;
        d.integrated_window_seconds = 3_600;
        if self.channels == 2 {
            self.correlation_matrix
                .update_correlation_data(&mut self.matrix_scratch);
            d.correlation_lr = (self.matrix_scratch.samples_seen >= 2)
                .then(|| self.matrix_scratch.matrix[1] as f64);
        } else {
            d.correlation_lr = None;
        }

        if self.spatial_enabled {
            // Refresh the inter-channel correlation matrix. We write into a
            // re-used scratch CorrelationData so the matrix Vec is allocated
            // exactly once per LoudnessMonitor instance, then copy the slice
            // into LoudnessData.
            self.correlation_matrix
                .update_correlation_data(&mut self.matrix_scratch);
            d.update_correlation_matrix(&self.matrix_scratch.matrix);
            d.correlation_samples_seen = self.correlation_matrix.samples_seen();
        } else {
            // Spatial off → emit an empty matrix so downstream consumers can
            // unambiguously detect "feature disabled" via `is_empty()`.
            d.update_correlation_matrix(&[]);
            d.correlation_samples_seen = 0;
        }
    }

    pub fn get_loudness(&mut self) -> LoudnessData {
        let mut d = LoudnessData::new(self.channels as usize);
        self.update_loudness_data(&mut d);
        d
    }

    pub fn reset(&mut self) -> Result<(), String> {
        self.ebur128.reset();
        if let Some(peak_meter) = &mut self.peak_meter {
            peak_meter.reset();
        }
        self.correlation_matrix.reset();
        self.query_error_generation = 0;
        self.frames_seen = 0;
        Ok(())
    }
}

pub struct LoudnessMonitorPlugin {
    num_channels: usize,
    sample_rate: u32,
    initialized: bool,
    enabled: bool,
    cache: RealTimeCache<LoudnessData>,
    monitor: LoudnessMonitor,
    channel_layout: Option<ChannelLayout>,
    cached_parameters: Vec<Parameter>,
}

impl LoudnessMonitorPlugin {
    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::new_inner(num_channels, None)
    }

    pub fn with_channel_layout(channel_layout: ChannelLayout) -> Result<Self, String> {
        Self::new_inner(channel_layout.channels.len(), Some(channel_layout))
    }

    pub fn new_with_layout(
        num_channels: usize,
        channel_layout: ChannelLayout,
    ) -> Result<Self, String> {
        Self::new_inner(num_channels, Some(channel_layout))
    }

    fn new_inner(
        num_channels: usize,
        channel_layout: Option<ChannelLayout>,
    ) -> Result<Self, String> {
        if num_channels == 0 {
            return Err("loudness monitor requires at least one channel".to_string());
        }
        if let Some(layout) = &channel_layout {
            layout.validate_for_width(num_channels)?;
        }
        let sr = 48000;
        let monitor = if let Some(layout) = &channel_layout {
            LoudnessMonitor::new_with_layout(num_channels as u32, sr, layout.clone())?
        } else {
            LoudnessMonitor::new(num_channels as u32, sr)?
        };
        let layout_compliant = channel_layout.is_some() || num_channels <= 2;
        let cache = new_loudness_cache(num_channels, false, layout_compliant);
        let mut p = Self {
            num_channels,
            sample_rate: sr,
            initialized: false,
            enabled: true,
            cache,
            monitor,
            channel_layout,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn channel_layout(&self) -> Option<&ChannelLayout> {
        self.channel_layout.as_ref()
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![Parameter::new_bool("enabled", "Enabled", self.enabled)];
    }

    fn clear_measurement(&mut self) -> Result<(), String> {
        self.monitor.reset()?;
        let channels = self.num_channels;
        let spatial = self.monitor.spatial_enabled();
        // Reset both cache slots. Updating only the published half lets a
        // subsequent enable/update swap stale pre-reset measurements back in.
        for _ in 0..2 {
            self.cache.update(|data| {
                reset_loudness_data(
                    data,
                    channels,
                    spatial,
                    self.channel_layout.is_some() || channels <= 2,
                )
            });
        }
        Ok(())
    }

    /// Toggle the inter-channel correlation matrix on the embedded monitor.
    ///
    /// Off by default. The audio engine flips this on for the output-side
    /// LoudnessMonitor so the spatial-spider widget has data to display; all
    /// other LoudnessMonitor instances (input-side, per-meter, CLI, ad-hoc)
    /// stay off and pay zero overhead.
    pub fn set_spatial_enabled(&mut self, enabled: bool) {
        self.monitor.set_spatial_enabled(enabled);
        // Enabling spatial data is a control-thread structural operation.
        // Rebuild both cache slots now so the first audio callback only copies.
        self.cache = new_loudness_cache(
            self.num_channels,
            enabled,
            self.channel_layout.is_some() || self.num_channels <= 2,
        );
    }

    /// Builder-style helper.
    pub fn with_spatial(mut self) -> Self {
        self.monitor.set_spatial_enabled(true);
        self
    }
}

impl Plugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Monitor", "1.1.0", "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Analyzer
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::analyzer(Some(PluginCompiledOp::AnalyzerTap))
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let parameter = self
            .cached_parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .ok_or_else(|| format!("Unknown parameter: {id}"))?;
        parameter
            .validate(&value)
            .map_err(|error| format!("{id}: {error}"))?;
        if id.as_str() == "enabled" {
            let enabled = value.as_bool().unwrap_or(true);
            if self.enabled != enabled {
                self.enabled = enabled;
                if !enabled {
                    self.clear_measurement()?;
                } else {
                    for _ in 0..2 {
                        self.cache.update(|data| data.measurement_enabled = true);
                    }
                }
                if let Some(parameter) = self.cached_parameters.first_mut() {
                    parameter.default_value = ParameterValue::Bool(enabled);
                }
            }
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "enabled" {
            Some(ParameterValue::Bool(self.enabled))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        if sr == 0 {
            return Err("loudness monitor sample rate must be non-zero".to_string());
        }
        self.sample_rate = sr;
        // Preserve the spatial-enable bit across reinitialisation so callers
        // that opted in once don't silently lose the matrix after a sample-
        // rate or channel-count change.
        let spatial = self.monitor.spatial_enabled();
        self.monitor = if let Some(layout) = &self.channel_layout {
            LoudnessMonitor::new_with_layout(self.num_channels as u32, sr, layout.clone())?
        } else {
            LoudnessMonitor::new(self.num_channels as u32, sr)?
        };
        self.monitor.set_spatial_enabled(spatial);
        self.cache = new_loudness_cache(
            self.num_channels,
            spatial,
            self.channel_layout.is_some() || self.num_channels <= 2,
        );
        self.cache
            .update(|data| data.measurement_enabled = self.enabled);
        self.initialized = true;
        Ok(())
    }
    fn reset(&mut self) {
        if let Err(e) = self.monitor.reset() {
            crate::rate_limited_log!(warn, 5, "loudness monitor reset failed: {e}");
        }
        let channels = self.num_channels;
        let spatial = self.monitor.spatial_enabled();
        for _ in 0..2 {
            self.cache.update(|data| {
                reset_loudness_data(
                    data,
                    channels,
                    spatial,
                    self.channel_layout.is_some() || channels <= 2,
                )
            });
        }
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        if !self.initialized {
            return Err("loudness monitor must be initialized before processing".to_string());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "loudness monitor initialized at {} Hz but received {} Hz context",
                self.sample_rate, context.sample_rate
            ));
        }
        let expected_samples = context
            .num_frames
            .checked_mul(self.num_channels)
            .ok_or_else(|| "loudness monitor frame/channel count overflow".to_string())?;
        if input.len() != expected_samples || output.len() != expected_samples {
            return Err(format!(
                "loudness monitor expected {expected_samples} samples for {} frames x {} channels, got input={} output={}",
                context.num_frames,
                self.num_channels,
                input.len(),
                output.len()
            ));
        }
        output.copy_from_slice(input);
        if !self.enabled {
            return Ok(context.num_frames);
        }
        self.monitor.add_frames(input)?;

        // Update cache: read loudness data then swap into cache.
        // Split borrows to avoid &mut self.cache + &mut self.monitor conflict.
        let monitor = &mut self.monitor;
        self.cache.update(|d| {
            monitor.update_loudness_data(d);
        });
        Ok(context.num_frames)
    }
    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::AnalyzerTap {
            return None;
        }
        Some(self.process(input, output, context))
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        self.cache.take_contention_stats()
    }
}

fn new_loudness_data(channels: usize, spatial: bool, layout_compliant: bool) -> LoudnessData {
    let mut data = LoudnessData::new(channels);
    data.channel_layout_is_compliant = layout_compliant;
    if spatial {
        data.update_correlation_matrix(&vec![0.0; channels.saturating_mul(channels)]);
    }
    data
}

fn new_loudness_cache(
    channels: usize,
    spatial: bool,
    layout_compliant: bool,
) -> RealTimeCache<LoudnessData> {
    RealTimeCache::new_pair(
        new_loudness_data(channels, spatial, layout_compliant),
        new_loudness_data(channels, spatial, layout_compliant),
    )
}

fn reset_loudness_data(
    data: &mut LoudnessData,
    channels: usize,
    spatial: bool,
    layout_compliant: bool,
) {
    data.measurement_valid = false;
    data.query_error_generation = 0;
    data.measurement_enabled = false;
    data.channel_layout_is_compliant = layout_compliant;
    data.momentary_lufs = f64::NEG_INFINITY;
    data.shortterm_lufs = f64::NEG_INFINITY;
    data.integrated_lufs = f64::NEG_INFINITY;
    data.peak = 0.0;
    data.correlation_lr = None;
    data.correlation_samples_seen = 0;
    data.true_peak_is_compliant = false;
    data.integrated_window_seconds = 3_600;

    if let Some(peaks) = Arc::get_mut(&mut data.channel_peaks) {
        peaks.fill(0.0);
    }
    if let Some(true_peaks) = Arc::get_mut(&mut data.true_peaks_dbtp) {
        true_peaks.fill(f64::NEG_INFINITY);
    }
    if let Some(matrix) = Arc::get_mut(&mut data.correlation_matrix) {
        if spatial && matrix.len() == channels.saturating_mul(channels) {
            matrix.fill(0.0);
        } else if !spatial {
            matrix.clear();
        }
    }
}
