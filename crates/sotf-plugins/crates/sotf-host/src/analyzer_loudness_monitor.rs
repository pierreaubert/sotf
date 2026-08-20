// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================

use crate::analyzer::{IntegratedLoudnessMode, LoudnessData, LoudnessQueryError, RealTimeCache};
use crate::analyzer_channel_correlation::ChannelCorrelationMonitor;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use crate::speaker_config::{ChannelLayout, ChannelRole};
use math_audio_dsp::ebur128::{EbuR128, Mode};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

const TRUE_PEAK_TAPS: usize = 12;
pub const INTEGRATED_HISTORY_SECONDS: u32 = 3_600;
const EXACT_GATING_BLOCK_CAPACITY: usize = INTEGRATED_HISTORY_SECONDS as usize * 10;
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
const RELATIVE_GATE_DB: f64 = -10.0;

fn energy_to_loudness(energy: f64) -> f64 {
    -0.691 + 10.0 * energy.log10()
}

fn loudness_to_energy(loudness: f64) -> f64 {
    10.0_f64.powf((loudness + 0.691) / 10.0)
}

/// Exact two-pass BS.1770 gating over every retained overlapping 400 ms
/// block. Capacity is prepared before processing; history is never evicted.
struct WholeProgramIntegrated {
    gating_blocks: Vec<f64>,
    cached_loudness: f64,
    dirty: bool,
    capacity_exceeded: bool,
    capacity_error_published: bool,
}

impl WholeProgramIntegrated {
    fn new(capacity: usize) -> Self {
        Self {
            gating_blocks: Vec::with_capacity(capacity),
            cached_loudness: f64::NEG_INFINITY,
            dirty: false,
            capacity_exceeded: false,
            capacity_error_published: false,
        }
    }

    fn push(&mut self, energy: f64) {
        if self.capacity_exceeded {
            return;
        }
        if self.gating_blocks.len() == self.gating_blocks.capacity() {
            self.capacity_exceeded = true;
            self.cached_loudness = f64::NEG_INFINITY;
            self.dirty = false;
            return;
        }
        self.gating_blocks.push(energy);
        self.dirty = true;
    }

    fn refresh(&mut self) {
        if !self.dirty || self.capacity_exceeded {
            return;
        }
        self.cached_loudness = gated_loudness(&self.gating_blocks);
        self.dirty = false;
    }

    fn reset(&mut self) {
        self.gating_blocks.clear();
        self.cached_loudness = f64::NEG_INFINITY;
        self.dirty = false;
        self.capacity_exceeded = false;
        self.capacity_error_published = false;
    }
}

fn gated_loudness(blocks: &[f64]) -> f64 {
    let absolute_gate = loudness_to_energy(ABSOLUTE_GATE_LUFS);
    let (absolute_sum, absolute_count) = blocks
        .iter()
        .filter(|&&energy| energy > absolute_gate)
        .fold((0.0, 0_u64), |(sum, count), &energy| {
            (sum + energy, count + 1)
        });
    if absolute_count == 0 {
        return f64::NEG_INFINITY;
    }
    let relative_gate =
        absolute_sum / absolute_count as f64 * 10.0_f64.powf(RELATIVE_GATE_DB / 10.0);
    let (relative_sum, relative_count) = blocks
        .iter()
        .filter(|&&energy| energy > relative_gate)
        .fold((0.0, 0_u64), |(sum, count), &energy| {
            (sum + energy, count + 1)
        });
    if relative_count == 0 {
        f64::NEG_INFINITY
    } else {
        energy_to_loudness(relative_sum / relative_count as f64)
    }
}

// ITU-R BS.1770-4 Table 2. At 44.1/48 kHz all four phases provide the
// required 4x measurement rate. At 88.2/96 kHz phases 0 and 2 provide the
// required 2x measurement rate without changing the FIR's input-time basis.
const TRUE_PEAK_PHASES: [[f64; TRUE_PEAK_TAPS]; 4] = [
    [
        0.0017089843750,
        -0.0291748046875,
        -0.0189208984375,
        0.0776367187500,
        0.0983886718750,
        -0.1897583007813,
        -0.3953857421875,
        0.8893127441406,
        0.6444091796875,
        -0.0517578125000,
        -0.0245361328125,
        0.0015869140625,
    ],
    [
        -0.0291748046875,
        0.0017089843750,
        0.0776367187500,
        -0.0189208984375,
        -0.1897583007813,
        0.0983886718750,
        0.8893127441406,
        -0.3953857421875,
        -0.0517578125000,
        0.6444091796875,
        0.0015869140625,
        -0.0245361328125,
    ],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [
        -0.0245361328125,
        0.0015869140625,
        0.6444091796875,
        -0.0517578125000,
        -0.3953857421875,
        0.8893127441406,
        0.0983886718750,
        -0.1897583007813,
        -0.0189208984375,
        0.0776367187500,
        0.0017089843750,
        -0.0291748046875,
    ],
];

struct Bs1770TruePeakMeter {
    history: Vec<[f64; TRUE_PEAK_TAPS]>,
    interval_peaks: Vec<f64>,
    phase_indices: &'static [usize],
    compliant: bool,
}

impl Bs1770TruePeakMeter {
    fn new(channels: usize, sample_rate: u32) -> Self {
        const FOUR_PHASES: &[usize] = &[0, 1, 2, 3];
        const TWO_PHASES: &[usize] = &[0, 2];
        const UNSUPPORTED_PHASES: &[usize] = &[];
        let (phase_indices, compliant) = match sample_rate {
            44_100 | 48_000 => (FOUR_PHASES, true),
            88_200 | 96_000 => (TWO_PHASES, true),
            _ => (UNSUPPORTED_PHASES, false),
        };
        Self {
            history: vec![[0.0; TRUE_PEAK_TAPS]; channels],
            interval_peaks: vec![0.0; channels],
            phase_indices,
            compliant,
        }
    }

    fn add_frames(&mut self, samples: &[f32], channels: usize) {
        if !self.compliant {
            return;
        }
        for frame in samples.chunks_exact(channels) {
            for (channel, &sample) in frame.iter().enumerate() {
                let history = &mut self.history[channel];
                history.copy_within(1.., 0);
                history[TRUE_PEAK_TAPS - 1] = sample as f64;
                for &phase_index in self.phase_indices {
                    let interpolated = TRUE_PEAK_PHASES[phase_index]
                        .iter()
                        .zip(history.iter())
                        .map(|(coefficient, value)| coefficient * value)
                        .sum::<f64>()
                        .abs();
                    self.interval_peaks[channel] = self.interval_peaks[channel].max(interpolated);
                }
            }
        }
    }

    fn take_interval_peak(&mut self, channel: usize) -> Option<f64> {
        self.compliant
            .then(|| std::mem::take(&mut self.interval_peaks[channel]))
    }

    fn reset(&mut self) {
        self.history.fill([0.0; TRUE_PEAK_TAPS]);
        self.interval_peaks.fill(0.0);
    }
}

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
    loudness_channel_indices: Vec<usize>,
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
    true_peak_meter: Bs1770TruePeakMeter,
    query_error_generation: u64,
    frames_seen: u64,
    sub_block_frames: usize,
    frames_into_sub_block: usize,
    completed_sub_blocks: u64,
    integrated_mode: IntegratedLoudnessMode,
    whole_program_integrated: Option<WholeProgramIntegrated>,
}

impl LoudnessMonitor {
    pub fn new(channels: u32, sr: u32) -> Result<Self, String> {
        Self::new_inner(channels, sr, None, IntegratedLoudnessMode::Rolling)
    }

    pub fn new_with_integrated_mode(
        channels: u32,
        sr: u32,
        integrated_mode: IntegratedLoudnessMode,
    ) -> Result<Self, String> {
        Self::new_inner(channels, sr, None, integrated_mode)
    }

    pub fn new_with_layout(
        channels: u32,
        sr: u32,
        channel_layout: ChannelLayout,
    ) -> Result<Self, String> {
        Self::new_inner(
            channels,
            sr,
            Some(channel_layout),
            IntegratedLoudnessMode::Rolling,
        )
    }

    pub fn new_with_layout_and_integrated_mode(
        channels: u32,
        sr: u32,
        channel_layout: ChannelLayout,
        integrated_mode: IntegratedLoudnessMode,
    ) -> Result<Self, String> {
        Self::new_inner(channels, sr, Some(channel_layout), integrated_mode)
    }

    fn new_inner(
        channels: u32,
        sr: u32,
        channel_layout: Option<ChannelLayout>,
        integrated_mode: IntegratedLoudnessMode,
    ) -> Result<Self, String> {
        if channels == 0 {
            return Err("loudness monitor requires at least one channel".to_string());
        }
        if sr < 10 {
            return Err("loudness monitor sample rate must be at least 10 Hz".to_string());
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
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK
        };
        let ebur = EbuR128::new(loudness_channels as u32, sr, loudness_mode)
            .map_err(|e| format!("{:?}", e))?;
        let peak_meter = channel_layout
            .as_ref()
            .map(|_| EbuR128::new(channels, sr, Mode::SAMPLE_PEAK))
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
        let loudness_channel_indices = channel_layout
            .as_ref()
            .map(|layout| {
                (0..channels as usize)
                    .map(|index| {
                        if loudness_channels != 7 {
                            return index;
                        }
                        match layout.role_at(index) {
                            Some(ChannelRole::FrontLeft) => 0,
                            Some(ChannelRole::FrontRight) => 1,
                            Some(ChannelRole::FrontCenter) => 2,
                            Some(ChannelRole::Lfe) => 3,
                            Some(ChannelRole::SideLeft | ChannelRole::BackLeft) => 4,
                            Some(ChannelRole::SideRight | ChannelRole::BackRight) => 5,
                            _ => index.min(loudness_channels.saturating_sub(1)),
                        }
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
            loudness_channel_indices,
            // Fixed-size chunking keeps arbitrary callback sizes allocation
            // free while bounding control-time scratch allocation.
            weighted_scratch: vec![0.0; loudness_channels * 256],
            spatial_enabled: false,
            correlation_matrix: ChannelCorrelationMonitor::new(channels as usize, sr),
            matrix_scratch: crate::analyzer::CorrelationData::new(channels as usize),
            peaks_buf: vec![0.0; channels as usize],
            true_peaks_buf: vec![0.0; channels as usize],
            true_peak_meter: Bs1770TruePeakMeter::new(channels as usize, sr),
            query_error_generation: 0,
            frames_seen: 0,
            sub_block_frames: sr as usize / 10,
            frames_into_sub_block: 0,
            completed_sub_blocks: 0,
            integrated_mode,
            whole_program_integrated: (integrated_mode == IntegratedLoudnessMode::WholeProgram)
                .then(|| WholeProgramIntegrated::new(EXACT_GATING_BLOCK_CAPACITY)),
        })
    }

    pub fn integrated_mode(&self) -> IntegratedLoudnessMode {
        self.integrated_mode
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
        self.true_peak_meter
            .add_frames(samples, self.channels as usize);

        if let Some(peak_meter) = &mut self.peak_meter {
            peak_meter
                .add_frames_f32(samples)
                .map_err(|error| format!("EBU R128 peak add_frames failed: {error:?}"))?;
        }
        let input_channels = self.channels as usize;
        let total_frames = samples.len() / input_channels;
        let mut frame_offset = 0;
        while frame_offset < total_frames {
            let until_boundary = self.sub_block_frames - self.frames_into_sub_block;
            let mut chunk_frames = (total_frames - frame_offset).min(until_boundary);
            if self.peak_meter.is_some() {
                chunk_frames = chunk_frames.min(256);
            }
            let sample_start = frame_offset * input_channels;
            let sample_end = (frame_offset + chunk_frames) * input_channels;
            self.add_loudness_chunk(&samples[sample_start..sample_end])?;
            frame_offset += chunk_frames;
            self.frames_into_sub_block += chunk_frames;
            if self.frames_into_sub_block == self.sub_block_frames {
                self.frames_into_sub_block = 0;
                self.completed_sub_blocks = self.completed_sub_blocks.saturating_add(1);
                if self.completed_sub_blocks >= 4
                    && let Some(exact) = &mut self.whole_program_integrated
                {
                    let momentary = self.ebur128.loudness_momentary().map_err(|error| {
                        format!("EBU R128 integrated block query failed: {error:?}")
                    })?;
                    let energy = if momentary.is_finite() {
                        loudness_to_energy(momentary)
                    } else {
                        0.0
                    };
                    exact.push(energy);
                }
            }
        }
        if let Some(exact) = &mut self.whole_program_integrated {
            // At most one bounded two-pass scan per callback, irrespective of
            // how many 100 ms boundaries an offline block crossed.
            exact.refresh();
        }
        self.frames_seen = self
            .frames_seen
            .saturating_add((samples.len() / self.channels as usize) as u64);
        Ok(())
    }

    fn add_loudness_chunk(&mut self, input: &[f32]) -> Result<(), String> {
        let input_channels = self.channels as usize;
        if self.peak_meter.is_some() {
            let frames = input.len() / input_channels;
            let scratch_len = frames * self.loudness_channels;
            let scratch = &mut self.weighted_scratch[..scratch_len];
            scratch.fill(0.0);
            for (input_frame, output_frame) in input
                .chunks_exact(input_channels)
                .zip(scratch.chunks_exact_mut(self.loudness_channels))
            {
                for channel in 0..input_channels {
                    let output_channel = self.loudness_channel_indices[channel];
                    output_frame[output_channel] = if self.loudness_channels == 7 {
                        input_frame[channel]
                    } else {
                        input_frame[channel] * self.loudness_gains[channel]
                    };
                }
            }
            self.ebur128
                .add_frames_f32(scratch)
                .map_err(|error| format!("EBU R128 loudness add_frames failed: {error:?}"))
        } else {
            self.ebur128
                .add_frames_f32(input)
                .map_err(|error| format!("EBU R128 add_frames failed: {error:?}"))
        }
    }

    /// Update LoudnessData in-place to avoid allocations
    pub fn update_loudness_data(&mut self, d: &mut LoudnessData) {
        let momentary = self.ebur128.loudness_momentary();
        let shortterm = self.ebur128.loudness_shortterm();
        let integrated = match &self.whole_program_integrated {
            Some(exact) if exact.capacity_exceeded => Ok(f64::NEG_INFINITY),
            Some(exact) => Ok(exact.cached_loudness),
            None => self.ebur128.loudness_global(),
        };
        let momentary_ok = momentary.is_ok();
        let shortterm_ok = shortterm.is_ok();
        let integrated_ok = integrated.is_ok();
        let mut meter_query_failed = !momentary_ok || !shortterm_ok || !integrated_ok;
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
        let mut sample_peak_failed = false;

        for ch in 0..nc {
            let peak_meter = self.peak_meter.as_mut().unwrap_or(&mut self.ebur128);
            let sample_peak = peak_meter.prev_sample_peak(ch as u32);
            let true_peak = self.true_peak_meter.take_interval_peak(ch);
            sample_peak_failed |= sample_peak.is_err();
            peaks[ch] = sample_peak.unwrap_or(0.0);
            let tp_linear = true_peak.unwrap_or(0.0);
            tps[ch] = if tp_linear > 0.0 {
                20.0 * tp_linear.log10()
            } else {
                f64::NEG_INFINITY
            };
        }
        meter_query_failed |= sample_peak_failed;

        d.update_peaks(peaks);
        d.update_true_peaks(tps);

        d.peak = d.channel_peaks.iter().copied().fold(0.0, f64::max);
        let capacity_exceeded = self
            .whole_program_integrated
            .as_ref()
            .is_some_and(|exact| exact.capacity_exceeded);
        let newly_exceeded = self.whole_program_integrated.as_mut().is_some_and(|exact| {
            if exact.capacity_exceeded && !exact.capacity_error_published {
                exact.capacity_error_published = true;
                true
            } else {
                false
            }
        });
        if meter_query_failed || newly_exceeded {
            self.query_error_generation = self.query_error_generation.saturating_add(1);
        }
        d.query_error = if capacity_exceeded {
            Some(LoudnessQueryError::IntegratedProgramCapacityExceeded)
        } else if meter_query_failed {
            Some(LoudnessQueryError::MeterQueryFailed)
        } else {
            None
        };
        d.momentary_valid = momentary_ok && self.frames_seen >= (self.sample_rate as u64 * 4 / 10);
        d.shortterm_valid = shortterm_ok && self.frames_seen >= self.sample_rate as u64 * 3;
        d.integrated_valid = integrated_ok && !capacity_exceeded && self.completed_sub_blocks >= 4;
        d.sample_peak_valid = !sample_peak_failed;
        d.true_peak_valid = self.true_peak_meter.compliant;
        d.measurement_valid =
            d.momentary_valid && d.shortterm_valid && d.integrated_valid && d.sample_peak_valid;
        d.measurement_enabled = true;
        d.channel_layout_is_compliant = self.channel_layout.is_some() || self.channels <= 2;
        d.query_error_generation = self.query_error_generation;
        d.true_peak_is_compliant = self.true_peak_meter.compliant;
        d.integrated_mode = self.integrated_mode;
        d.integrated_window_seconds = INTEGRATED_HISTORY_SECONDS;
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
        self.true_peak_meter.reset();
        self.query_error_generation = 0;
        self.frames_seen = 0;
        self.frames_into_sub_block = 0;
        self.completed_sub_blocks = 0;
        if let Some(exact) = &mut self.whole_program_integrated {
            exact.reset();
        }
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
        let cache = new_loudness_cache(
            num_channels,
            false,
            layout_compliant,
            IntegratedLoudnessMode::Rolling,
        );
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
        for _ in 0..3 {
            self.cache.update(|data| {
                reset_loudness_data(
                    data,
                    channels,
                    spatial,
                    self.channel_layout.is_some() || channels <= 2,
                    self.monitor.integrated_mode(),
                    false,
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
            self.monitor.integrated_mode(),
        );
    }

    /// Builder-style helper.
    pub fn with_spatial(mut self) -> Self {
        self.monitor.set_spatial_enabled(true);
        self
    }

    /// Select integrated-history policy before realtime processing begins.
    /// Whole-program mode prepares its complete bounded store here, never in
    /// `process`.
    pub fn with_integrated_mode(mut self, mode: IntegratedLoudnessMode) -> Result<Self, String> {
        let spatial = self.monitor.spatial_enabled();
        self.monitor = if let Some(layout) = &self.channel_layout {
            LoudnessMonitor::new_with_layout_and_integrated_mode(
                self.num_channels as u32,
                self.sample_rate,
                layout.clone(),
                mode,
            )?
        } else {
            LoudnessMonitor::new_with_integrated_mode(
                self.num_channels as u32,
                self.sample_rate,
                mode,
            )?
        };
        self.monitor.set_spatial_enabled(spatial);
        self.cache = new_loudness_cache(
            self.num_channels,
            spatial,
            self.channel_layout.is_some() || self.num_channels <= 2,
            mode,
        );
        Ok(self)
    }
}

impl Plugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Monitor", "1.2.0", "Sotf")
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
                    for _ in 0..3 {
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
        if sr < 10 {
            return Err("loudness monitor sample rate must be at least 10 Hz".to_string());
        }
        self.sample_rate = sr;
        // Preserve the spatial-enable bit across reinitialisation so callers
        // that opted in once don't silently lose the matrix after a sample-
        // rate or channel-count change.
        let spatial = self.monitor.spatial_enabled();
        let integrated_mode = self.monitor.integrated_mode();
        self.monitor = if let Some(layout) = &self.channel_layout {
            LoudnessMonitor::new_with_layout_and_integrated_mode(
                self.num_channels as u32,
                sr,
                layout.clone(),
                integrated_mode,
            )?
        } else {
            LoudnessMonitor::new_with_integrated_mode(
                self.num_channels as u32,
                sr,
                integrated_mode,
            )?
        };
        self.monitor.set_spatial_enabled(spatial);
        self.cache = new_loudness_cache(
            self.num_channels,
            spatial,
            self.channel_layout.is_some() || self.num_channels <= 2,
            integrated_mode,
        );
        for _ in 0..3 {
            self.cache
                .update(|data| data.measurement_enabled = self.enabled);
        }
        self.initialized = true;
        Ok(())
    }
    fn reset(&mut self) {
        if let Err(e) = self.monitor.reset() {
            crate::rate_limited_log!(warn, 5, "loudness monitor reset failed: {e}");
        }
        let channels = self.num_channels;
        let spatial = self.monitor.spatial_enabled();
        for _ in 0..3 {
            self.cache.update(|data| {
                reset_loudness_data(
                    data,
                    channels,
                    spatial,
                    self.channel_layout.is_some() || channels <= 2,
                    self.monitor.integrated_mode(),
                    self.enabled,
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
        let expected_samples = self.validate_analyzer_input(input, context)?;
        if output.len() != expected_samples {
            return Err(format!(
                "loudness monitor expected {expected_samples} output samples for {} frames x {} channels, got output={}",
                context.num_frames,
                self.num_channels,
                output.len()
            ));
        }
        output.copy_from_slice(input);
        self.process_analyzer_input(input, context)
    }
    fn process_analyzer_tap_f32(
        &mut self,
        input: &[f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        Some(
            self.validate_analyzer_input(input, context)
                .and_then(|_| self.process_analyzer_input(input, context)),
        )
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

impl LoudnessMonitorPlugin {
    fn validate_analyzer_input(
        &self,
        input: &[f32],
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
        if input.len() != expected_samples {
            return Err(format!(
                "loudness monitor expected {expected_samples} input samples for {} frames x {} channels, got input={}",
                context.num_frames,
                self.num_channels,
                input.len()
            ));
        }
        Ok(expected_samples)
    }

    fn process_analyzer_input(
        &mut self,
        input: &[f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
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
}

fn new_loudness_data(
    channels: usize,
    spatial: bool,
    layout_compliant: bool,
    integrated_mode: IntegratedLoudnessMode,
) -> LoudnessData {
    let mut data = LoudnessData::new(channels);
    data.channel_layout_is_compliant = layout_compliant;
    data.integrated_mode = integrated_mode;
    if spatial {
        data.update_correlation_matrix(&vec![0.0; channels.saturating_mul(channels)]);
    }
    data
}

fn new_loudness_cache(
    channels: usize,
    spatial: bool,
    layout_compliant: bool,
    integrated_mode: IntegratedLoudnessMode,
) -> RealTimeCache<LoudnessData> {
    RealTimeCache::new_triplet(
        new_loudness_data(channels, spatial, layout_compliant, integrated_mode),
        new_loudness_data(channels, spatial, layout_compliant, integrated_mode),
        new_loudness_data(channels, spatial, layout_compliant, integrated_mode),
    )
}

fn reset_loudness_data(
    data: &mut LoudnessData,
    channels: usize,
    spatial: bool,
    layout_compliant: bool,
    integrated_mode: IntegratedLoudnessMode,
    enabled: bool,
) {
    data.measurement_valid = false;
    data.query_error_generation = 0;
    data.query_error = None;
    data.measurement_enabled = enabled;
    data.momentary_valid = false;
    data.shortterm_valid = false;
    data.integrated_valid = false;
    data.sample_peak_valid = false;
    data.true_peak_valid = false;
    data.channel_layout_is_compliant = layout_compliant;
    data.momentary_lufs = f64::NEG_INFINITY;
    data.shortterm_lufs = f64::NEG_INFINITY;
    data.integrated_lufs = f64::NEG_INFINITY;
    data.integrated_mode = integrated_mode;
    data.peak = 0.0;
    data.correlation_lr = None;
    data.correlation_samples_seen = 0;
    data.true_peak_is_compliant = false;
    data.integrated_window_seconds = INTEGRATED_HISTORY_SECONDS;

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

#[cfg(test)]
mod true_peak_tests {
    use super::*;

    // Independent test oracle copied directly from BS.1770-4 Table 2. Tests
    // deliberately do not call the production convolution or its constants.
    const ORACLE_PHASES: [[f64; 12]; 4] = [
        [
            0.0017089843750,
            -0.0291748046875,
            -0.0189208984375,
            0.0776367187500,
            0.0983886718750,
            -0.1897583007813,
            -0.3953857421875,
            0.8893127441406,
            0.6444091796875,
            -0.0517578125000,
            -0.0245361328125,
            0.0015869140625,
        ],
        [
            -0.0291748046875,
            0.0017089843750,
            0.0776367187500,
            -0.0189208984375,
            -0.1897583007813,
            0.0983886718750,
            0.8893127441406,
            -0.3953857421875,
            -0.0517578125000,
            0.6444091796875,
            0.0015869140625,
            -0.0245361328125,
        ],
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [
            -0.0245361328125,
            0.0015869140625,
            0.6444091796875,
            -0.0517578125000,
            -0.3953857421875,
            0.8893127441406,
            0.0983886718750,
            -0.1897583007813,
            -0.0189208984375,
            0.0776367187500,
            0.0017089843750,
            -0.0291748046875,
        ],
    ];

    fn oracle_peak(signal: &[f32], phases: &[usize]) -> f64 {
        let mut history = [0.0_f64; 12];
        let mut peak = 0.0_f64;
        for &sample in signal {
            history.copy_within(1.., 0);
            history[11] = sample as f64;
            for &phase in phases {
                let value = ORACLE_PHASES[phase]
                    .iter()
                    .zip(history)
                    .map(|(coefficient, sample)| coefficient * sample)
                    .sum::<f64>()
                    .abs();
                peak = peak.max(value);
            }
        }
        peak
    }

    fn fixture() -> Vec<f32> {
        let mut signal: Vec<f32> = (0..513)
            .map(|index| (std::f64::consts::TAU * 0.459 * index as f64).sin() as f32 * 0.91)
            .collect();
        // An impulse immediately on a likely callback boundary exercises FIR
        // history continuity rather than merely steady-state tone behavior.
        signal[64] = -0.97;
        signal
    }

    #[test]
    fn table_2_meter_matches_independent_oracle_at_supported_rates() {
        let signal = fixture();
        for (sample_rate, phases) in [
            (44_100, &[0, 1, 2, 3][..]),
            (48_000, &[0, 1, 2, 3][..]),
            (88_200, &[0, 2][..]),
            (96_000, &[0, 2][..]),
        ] {
            let mut meter = Bs1770TruePeakMeter::new(1, sample_rate);
            meter.add_frames(&signal, 1);
            let measured = meter.take_interval_peak(0).unwrap();
            let expected = oracle_peak(&signal, phases);
            assert!((measured - expected).abs() < 1.0e-14, "{sample_rate} Hz");
        }
    }

    #[test]
    fn table_2_meter_preserves_history_across_callback_boundaries() {
        let signal = fixture();
        for sample_rate in [44_100, 48_000, 96_000] {
            let mut whole = Bs1770TruePeakMeter::new(1, sample_rate);
            whole.add_frames(&signal, 1);
            let expected = whole.take_interval_peak(0).unwrap();

            let mut split = Bs1770TruePeakMeter::new(1, sample_rate);
            let mut measured = 0.0_f64;
            let mut offset = 0;
            for length in [1, 7, 56, 1, 113, 257, usize::MAX] {
                if offset == signal.len() {
                    break;
                }
                let end = offset.saturating_add(length).min(signal.len());
                split.add_frames(&signal[offset..end], 1);
                measured = measured.max(split.take_interval_peak(0).unwrap());
                offset = end;
            }
            assert_eq!(offset, signal.len());
            assert!((measured - expected).abs() < 1.0e-14, "{sample_rate} Hz");
        }
    }

    #[test]
    fn unsupported_rate_publishes_no_true_peak() {
        let mut meter = Bs1770TruePeakMeter::new(1, 192_000);
        meter.add_frames(&fixture(), 1);
        assert!(!meter.compliant);
        assert_eq!(meter.take_interval_peak(0), None);
    }

    #[test]
    fn whole_program_gate_matches_independent_two_level_reference() {
        let quiet = loudness_to_energy(-60.0);
        let loud = loudness_to_energy(-20.0);
        let measured = gated_loudness(&[quiet, loud, loud]);
        assert!((measured - -20.0).abs() < 1.0e-12);
    }

    #[test]
    fn whole_program_capacity_failure_is_explicit_and_generation_is_stable() {
        let mut monitor = LoudnessMonitor::new_with_integrated_mode(
            1,
            48_000,
            IntegratedLoudnessMode::WholeProgram,
        )
        .unwrap();
        monitor.whole_program_integrated = Some(WholeProgramIntegrated::new(2));
        monitor.add_frames(&vec![0.1; 48_000 * 6 / 10]).unwrap();

        let mut data = LoudnessData::new(1);
        monitor.update_loudness_data(&mut data);
        assert_eq!(
            data.query_error,
            Some(LoudnessQueryError::IntegratedProgramCapacityExceeded)
        );
        assert_eq!(data.query_error_generation, 1);
        assert!(!data.integrated_valid);
        assert!(data.integrated_lufs.is_infinite() && data.integrated_lufs.is_sign_negative());

        monitor.update_loudness_data(&mut data);
        assert_eq!(data.query_error_generation, 1);
        monitor.reset().unwrap();
        monitor.update_loudness_data(&mut data);
        assert!(data.query_error.is_none());
        assert_eq!(data.query_error_generation, 0);
    }
}
