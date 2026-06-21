// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================

use crate::analyzer::{LoudnessData, RealTimeCache};
use crate::analyzer_channel_correlation::ChannelCorrelationMonitor;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use math_audio_dsp::ebur128::{EbuR128, Mode};
use math_audio_dsp::fast_math::fast_log10;
use rtrb::{Consumer, RingBuffer};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

/// L/R correlation EMA window. Matches `analyzer_channel_correlation::WINDOW_SECONDS`
/// (400 ms — EBU R128 momentary block) so the L/R EMA and the full
/// per-pair correlation matrix share the same time-response characteristics
/// regardless of buffer size.
const CORRELATION_WINDOW_SECONDS: f64 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessInfo {
    pub momentary_lufs: f64,
    pub shortterm_lufs: f64,
    pub integrated_lufs: f64,
    pub peak: f64,
}

pub struct LoudnessMonitor {
    ebur128: EbuR128,
    channels: u32,
    /// Sample rate, used to scale the L/R correlation EMA so the smoothing
    /// time-constant is buffer-size-independent and consistent with the
    /// sliding-EMA used by `ChannelCorrelationMonitor`.
    sample_rate: u32,
    /// Running L/R correlation (Pearson) for stereo width.
    /// Smoothed with a buffer-size-aware EMA (alpha derived from `samples.len()
    /// / (sample_rate * WINDOW_SECONDS)`) so the time response matches the
    /// full correlation-matrix EMA at any block size.
    correlation_lr: Option<f64>,
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
}

impl LoudnessMonitor {
    pub fn new(channels: u32, sr: u32) -> Result<Self, String> {
        let ebur = EbuR128::new(
            channels,
            sr,
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK | Mode::TRUE_PEAK,
        )
        .map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            ebur128: ebur,
            channels,
            sample_rate: sr,
            correlation_lr: None,
            spatial_enabled: false,
            correlation_matrix: ChannelCorrelationMonitor::new(channels as usize, sr),
            matrix_scratch: crate::analyzer::CorrelationData::new(channels as usize),
            peaks_buf: vec![0.0; channels as usize],
            true_peaks_buf: vec![0.0; channels as usize],
        })
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
        // Compute L/R correlation for stereo signals using a sliding-EMA that
        // matches the full inter-channel matrix's time response: the per-block
        // alpha scales with the block's fraction of the 400 ms window, so the
        // smoothing time-constant is buffer-size-independent.
        if self.channels == 2 {
            let frame_corr = compute_correlation_interleaved(samples, 2);
            if let Some(c) = frame_corr {
                let frames = (samples.len() / 2) as f64;
                let window_samples =
                    (self.sample_rate as f64 * CORRELATION_WINDOW_SECONDS).max(1.0);
                let alpha = (frames / window_samples).clamp(0.0, 1.0);
                self.correlation_lr = Some(match self.correlation_lr {
                    Some(prev) => prev * (1.0 - alpha) + c * alpha,
                    None => c,
                });
            }
        }

        // Full inter-channel Pearson r matrix for the spatial-spider widget,
        // gated by an explicit opt-in so default consumers don't pay the cost.
        if self.spatial_enabled {
            self.correlation_matrix.add_frames(samples);
        }

        self.ebur128
            .add_frames_f32(samples)
            .map_err(|_| "EBU".into())
    }

    /// Update LoudnessData in-place to avoid allocations
    pub fn update_loudness_data(&mut self, d: &mut LoudnessData) {
        d.momentary_lufs = self.ebur128.loudness_momentary().unwrap_or(-120.0);
        d.shortterm_lufs = self.ebur128.loudness_shortterm().unwrap_or(-120.0);
        d.integrated_lufs = self.ebur128.loudness_global().unwrap_or(-120.0);

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
            peaks[ch] = self.ebur128.prev_sample_peak(ch as u32).unwrap_or(0.0);
            let tp_linear = self.ebur128.prev_true_peak(ch as u32).unwrap_or(0.0);
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
        d.correlation_lr = self.correlation_lr;

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
        self.correlation_lr = None;
        self.correlation_matrix.reset();
        Ok(())
    }
}

/// Compute Pearson correlation between channel 0 and channel 1 from interleaved samples.
/// Returns None if fewer than 2 frames or zero variance.
fn compute_correlation_interleaved(samples: &[f32], channels: usize) -> Option<f64> {
    if channels < 2 {
        return None;
    }
    let num_frames = samples.len() / channels;
    if num_frames < 2 {
        return None;
    }

    let mut sum_lr: f64 = 0.0;
    let mut sum_l2: f64 = 0.0;
    let mut sum_r2: f64 = 0.0;

    // Use chunks_exact(2) for common stereo case to help compiler auto-vectorize
    if channels == 2 {
        for chunk in samples.chunks_exact(2) {
            let l = chunk[0] as f64;
            let r = chunk[1] as f64;
            sum_lr += l * r;
            sum_l2 += l * l;
            sum_r2 += r * r;
        }
    } else {
        for i in 0..num_frames {
            let l = samples[i * channels] as f64;
            let r = samples[i * channels + 1] as f64;
            sum_lr += l * r;
            sum_l2 += l * l;
            sum_r2 += r * r;
        }
    }

    let denom = (sum_l2 * sum_r2).sqrt();
    if denom < 1e-12 {
        return None; // silence or zero variance
    }
    Some((sum_lr / denom).clamp(-1.0, 1.0))
}

pub struct LoudnessMonitorPlugin {
    num_channels: usize,
    sample_rate: u32,
    enabled: bool,
    producer: rtrb::Producer<f32>,
    consumer: Consumer<f32>,
    cache: RealTimeCache<LoudnessData>,
    monitor: LoudnessMonitor,
    cached_parameters: Vec<Parameter>,
}

impl LoudnessMonitorPlugin {
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let sr = 48000;
        let (p, c) = RingBuffer::new(sr as usize * 2);
        let monitor = LoudnessMonitor::new(num_channels as u32, sr)?;
        let cache = RealTimeCache::new(LoudnessData::new(num_channels));
        let mut p = Self {
            num_channels,
            sample_rate: sr,
            enabled: true,
            producer: p,
            consumer: c,
            cache,
            monitor,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![Parameter::new_bool("enabled", "Enabled", self.enabled)];
    }

    /// Toggle the inter-channel correlation matrix on the embedded monitor.
    ///
    /// Off by default. The audio engine flips this on for the output-side
    /// LoudnessMonitor so the spatial-spider widget has data to display; all
    /// other LoudnessMonitor instances (input-side, per-meter, CLI, ad-hoc)
    /// stay off and pay zero overhead.
    pub fn set_spatial_enabled(&mut self, enabled: bool) {
        self.monitor.set_spatial_enabled(enabled);
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
        self.validate_parameter(&id, &value)?;
        if id.as_str() == "enabled" {
            self.enabled = value.as_bool().unwrap_or(true);
            self.rebuild_cached_parameters();
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
        self.sample_rate = sr;
        // Preserve the spatial-enable bit across reinitialisation so callers
        // that opted in once don't silently lose the matrix after a sample-
        // rate or channel-count change.
        let spatial = self.monitor.spatial_enabled();
        self.monitor = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        self.monitor.set_spatial_enabled(spatial);
        Ok(())
    }
    fn reset(&mut self) {
        if let Err(e) = self.monitor.reset() {
            crate::rate_limited_log!(warn, 5, "loudness monitor reset failed: {e}");
        }
        self.cache.update(|d| {
            *d = LoudnessData::new(self.num_channels);
        });
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        if !self.enabled {
            return Ok(context.num_frames);
        }
        let mut dropped = 0usize;
        for &s in input {
            if self.producer.push(s).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            crate::rate_limited_log!(
                warn,
                5,
                "loudness ring buffer full, dropped {dropped} samples"
            );
        }
        let slots = self.consumer.slots();
        if let Ok(chunk) = self.consumer.read_chunk(slots) {
            let (s1, s2) = chunk.as_slices();
            if let Err(e) = self.monitor.add_frames(s1) {
                crate::rate_limited_log!(warn, 5, "loudness add_frames failed: {e}");
            }
            if let Err(e) = self.monitor.add_frames(s2) {
                crate::rate_limited_log!(warn, 5, "loudness add_frames failed: {e}");
            }
            chunk.commit_all();

            // Update cache: read loudness data then swap into cache.
            // Split borrows to avoid &mut self.cache + &mut self.monitor conflict.
            let monitor = &mut self.monitor;
            self.cache.update(|d| {
                monitor.update_loudness_data(d);
            });
        }
        Ok(context.num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        self.cache.take_contention_stats()
    }
}
