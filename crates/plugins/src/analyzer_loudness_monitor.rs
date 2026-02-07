//! ============================================================================
//! Loudness Monitor Analyzer Plugin
//! ============================================================================
//!
//! Wraps the LoudnessMonitor as an AnalyzerPlugin.
//! Provides real-time EBU R128 loudness measurements.

use super::analyzer::LoudnessData;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use ebur128::{EbuR128, Mode};
use std::any::Any;
use std::sync::Arc;

/// Type alias for backward compatibility — `LoudnessInfo` is now `LoudnessData`.
pub type LoudnessInfo = LoudnessData;

/// Loudness monitor for real-time audio analysis.
///
/// All access is serialized on the processing thread — no locks needed.
pub(crate) struct LoudnessMonitor {
    /// EBU R128 analyzer
    ebur128: EbuR128,
    /// Number of channels
    channels: u32,
    /// Sample rate
    sample_rate: u32,
    /// Current measurements (pre-allocated with correct channel count)
    current_loudness: LoudnessData,
    /// Per-channel peak trackers (separate from EBU R128 for meter display)
    channel_peak_trackers: Vec<f64>,
    /// Peak decay rate per sample (for visual meter decay)
    peak_decay_per_sample: f64,
    /// Ring buffer for L correlation samples (pre-allocated to correlation_buffer_size)
    correlation_ring_l: Vec<f32>,
    /// Ring buffer for R correlation samples (pre-allocated to correlation_buffer_size)
    correlation_ring_r: Vec<f32>,
    /// Current write position in the ring buffers
    correlation_write_pos: usize,
    /// Number of valid samples in the ring buffers (up to correlation_buffer_size)
    correlation_count: usize,
    /// Maximum correlation buffer size (1 second of audio)
    correlation_buffer_size: usize,
    /// Frames since last metrics update
    frames_since_metrics_update: usize,
    /// How often to recompute expensive metrics (sample_rate / 10 = every 100ms)
    metrics_update_interval: usize,
    /// Last computed correlation value
    cached_correlation: Option<f64>,
    /// Scratch buffer for true peaks calculation (placeholders)
    true_peaks_scratch: Vec<f64>,
    /// Scratch buffer for new channel peaks calculation
    channel_peaks_scratch: Vec<f64>,
    /// Last number of frames processed
    last_num_frames: usize,
}

impl LoudnessMonitor {
    /// Create a new loudness monitor
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub(crate) fn new(channels: u32, sample_rate: u32) -> Result<Self, String> {
        let ebur128 = EbuR128::new(
            channels,
            sample_rate,
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK,
        )
        .map_err(|e| format!("Failed to create EBU R128 analyzer: {:?}", e))?;

        // Decay to ~1% in 300ms (linear approximation)
        let decay_time_seconds = 0.3;
        let decay_samples = sample_rate as f64 * decay_time_seconds;
        let peak_decay_per_sample = 1.0 / decay_samples;

        let correlation_buffer_size = sample_rate as usize;
        let metrics_update_interval = sample_rate as usize / 10; // every 100ms

        Ok(Self {
            ebur128,
            channels,
            sample_rate,
            current_loudness: LoudnessData::new(channels as usize),
            channel_peak_trackers: vec![0.0; channels as usize],
            peak_decay_per_sample,
            correlation_ring_l: vec![0.0; correlation_buffer_size],
            correlation_ring_r: vec![0.0; correlation_buffer_size],
            correlation_write_pos: 0,
            correlation_count: 0,
            correlation_buffer_size,
            frames_since_metrics_update: 0,
            metrics_update_interval,
            cached_correlation: None,
            true_peaks_scratch: vec![-120.0; channels as usize],
            channel_peaks_scratch: vec![0.0; channels as usize],
            last_num_frames: 0,
        })
    }

    /// Add audio frames to the analyzer
    pub(crate) fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        // ebur128::add_frames_f32 is the most intensive part, but unavoidable for LUFS
        self.ebur128
            .add_frames_f32(samples)
            .map_err(|_| "Failed to add frames to EBU R128 analyzer".to_string())?;

        let num_frames = samples.len() / self.channels as usize;
        self.last_num_frames = num_frames;
        let channels = self.channels as usize;
        self.frames_since_metrics_update += num_frames;

        // Throttle expensive loudness queries
        if self.frames_since_metrics_update >= self.metrics_update_interval {
            self.frames_since_metrics_update = 0;

            self.current_loudness.momentary_lufs = self.ebur128.loudness_momentary().unwrap_or(-120.0);
            self.current_loudness.shortterm_lufs = self.ebur128.loudness_shortterm().unwrap_or(-120.0);
            self.current_loudness.integrated_lufs = self.ebur128.loudness_global().unwrap_or(-120.0);

            // Update correlation for stereo
            if self.channels == 2 {
                self.current_loudness.correlation_lr = self.calculate_correlation_stereo();
            }
        }

        // We still need to fill correlation ring buffers every block to avoid missing data
        if self.channels == 2 {
            self.fill_correlation_buffers(samples);
        }

        // Calculate per-channel sample peaks from the current buffer (cheap)
        self.channel_peaks_scratch.fill(0.0);
        for frame in samples.chunks_exact(channels) {
            for (ch, &sample) in frame.iter().enumerate() {
                let sample_abs = sample.abs() as f64;
                if sample_abs > self.channel_peaks_scratch[ch] {
                    self.channel_peaks_scratch[ch] = sample_abs;
                }
            }
        }

        // Apply decay to existing peaks and take max with new peaks
        let decay = self.peak_decay_per_sample * num_frames as f64;
        for (tracker, &new_peak) in self
            .channel_peak_trackers
            .iter_mut()
            .zip(self.channel_peaks_scratch.iter())
        {
            *tracker = (*tracker - decay).max(new_peak);
        }

        // Compute overall peak from trackers
        let peak = self
            .channel_peak_trackers
            .iter()
            .copied()
            .fold(0.0, f64::max);

        // Update basic peak info every block for responsive meters
        self.current_loudness.peak = peak;
        self.current_loudness
            .channel_peaks
            .copy_from_slice(&self.channel_peak_trackers);
        self.current_loudness
            .true_peaks_dbtp
            .copy_from_slice(&self.true_peaks_scratch);

        Ok(())
    }

    /// Fill the correlation ring buffers with new samples
    fn fill_correlation_buffers(&mut self, samples: &[f32]) {
        let mut write_pos = self.correlation_write_pos;
        let num_frames = samples.len() / 2;

        for frame in samples.chunks_exact(2) {
            self.correlation_ring_l[write_pos] = frame[0];
            self.correlation_ring_r[write_pos] = frame[1];

            write_pos += 1;
            if write_pos >= self.correlation_buffer_size {
                write_pos = 0;
            }
        }
        self.correlation_write_pos = write_pos;
        self.correlation_count =
            (self.correlation_count + num_frames).min(self.correlation_buffer_size);
    }

    /// Recompute correlation from ring buffer
    fn calculate_correlation_stereo(&mut self) -> Option<f64> {
        // Need at least 100 samples for meaningful correlation
        if self.correlation_count < 100 {
            return None;
        }

        let n = self.correlation_count as f64;
        let valid_len = self.correlation_count;

        // Compute means in one pass
        let (sum_l, sum_r) = self.correlation_ring_l[..valid_len]
            .iter()
            .zip(self.correlation_ring_r[..valid_len].iter())
            .fold((0.0, 0.0), |acc, (&l, &r)| {
                (acc.0 + l as f64, acc.1 + r as f64)
            });

        let mean_l = sum_l / n;
        let mean_r = sum_r / n;

        // Compute covariance and variance in one pass
        let (cov_lr, var_l, var_r) = self.correlation_ring_l[..valid_len]
            .iter()
            .zip(self.correlation_ring_r[..valid_len].iter())
            .fold((0.0, 0.0, 0.0), |acc, (&l, &r)| {
                let diff_l = l as f64 - mean_l;
                let diff_r = r as f64 - mean_r;
                (
                    acc.0 + diff_l * diff_r,
                    acc.1 + diff_l * diff_l,
                    acc.2 + diff_r * diff_r,
                )
            });

        let correlation = if var_l < 1e-10 || var_r < 1e-10 {
            0.0
        } else {
            (cov_lr / (var_l * var_r).sqrt()).clamp(-1.0, 1.0)
        };

        self.cached_correlation = Some(correlation);
        Some(correlation)
    }

    /// Get the current loudness measurements (zero-copy reference)
    pub(crate) fn get_loudness(&self) -> &LoudnessData {
        &self.current_loudness
    }

    /// Get the last number of frames processed
    pub(crate) fn last_num_frames(&self) -> usize {
        self.last_num_frames
    }

    /// Reset the monitor (clear all history)
    pub(crate) fn reset(&mut self) -> Result<(), String> {
        let new_ebur = EbuR128::new(
            self.channels,
            self.sample_rate,
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK,
        )
        .map_err(|e| format!("Failed to reset analyzer: {:?}", e))?;

        self.ebur128 = new_ebur;
        self.current_loudness = LoudnessData::new(self.channels as usize);
        self.channel_peak_trackers.fill(0.0);
        self.correlation_ring_l.fill(0.0);
        self.correlation_ring_r.fill(0.0);
        self.correlation_write_pos = 0;
        self.correlation_count = 0;
        self.frames_since_metrics_update = 0;
        self.cached_correlation = None;

        Ok(())
    }
}

// ============================================================================
// Plugin Wrapper
// ============================================================================

/// Loudness monitor analyzer plugin
pub struct LoudnessMonitorPlugin {
    /// Underlying loudness monitor
    monitor: LoudnessMonitor,
    /// Number of channels
    num_channels: usize,
    /// Sample rate
    sample_rate: u32,
    /// Cached Arc for zero-alloc get_data()
    cached_data: Option<Arc<dyn Any + Send + Sync>>,
    /// Samples since last metadata update (throttling)
    samples_since_update: usize,
    /// How often to update metadata (sample_rate / 10 = every 100ms)
    update_interval_samples: usize,
}

impl LoudnessMonitorPlugin {
    /// Create a new loudness monitor plugin
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let sample_rate = 48000; // Default until initialize()
        let monitor = LoudnessMonitor::new(num_channels as u32, sample_rate)?;

        Ok(Self {
            monitor,
            num_channels,
            sample_rate,
            cached_data: None,
            samples_since_update: 0,
            update_interval_samples: sample_rate as usize / 10,
        })
    }

    /// Get current loudness measurements (zero-copy reference)
    pub fn get_loudness(&self) -> &LoudnessData {
        self.monitor.get_loudness()
    }

    /// Rebuild the cached Arc from current monitor state.
    /// Tries Arc::get_mut() for in-place update (zero alloc when refcount == 1),
    /// falls back to new allocation when the old Arc is still held externally.
    fn rebuild_cached_data(&mut self) {
        let source = self.monitor.get_loudness();

        if let Some(ref mut arc) = self.cached_data {
            // Try to get mutable access (works when refcount == 1, i.e. nobody else holds a ref)
            if let Some(inner) = Arc::get_mut(arc)
                .and_then(|any| any.downcast_mut::<LoudnessData>())
            {
                // Zero-allocation in-place update
                inner.update_from(source);
                return;
            }
        }

        // First call or refcount > 1: allocate new Arc
        self.cached_data = Some(Arc::new(source.clone()));
    }
}

impl Plugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Monitor", "1.0.0", "SotF")
            .with_description("Real-time EBU R128 loudness monitoring (LUFS, peaks)")
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("Loudness monitor has no parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_interval_samples = sample_rate as usize / 10;
        self.samples_since_update = 0;
        self.monitor = LoudnessMonitor::new(self.num_channels as u32, sample_rate)
            .map_err(|e| format!("Failed to initialize loudness monitor: {}", e))?;
        self.rebuild_cached_data();

        Ok(())
    }

    fn reset(&mut self) {
        self.monitor.reset().ok();
        self.samples_since_update = 0;
        self.rebuild_cached_data();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let expected_samples = context.num_frames * self.num_channels;
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }
        if output.len() != expected_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                expected_samples,
                output.len()
            ));
        }

        // Pass-through: copy input to output
        output.copy_from_slice(input);

        // Add frames to the monitor
        self.monitor
            .add_frames(input)
            .map_err(|e| format!("Failed to add frames to loudness monitor: {}", e))?;

        // Rebuild cached Arc for zero-alloc get_data().
        // Throttled to every ~100ms to reduce overhead.
        self.samples_since_update += context.num_frames;
        if self.samples_since_update >= self.update_interval_samples {
            self.samples_since_update = 0;
            self.rebuild_cached_data();
        }

        Ok(context.num_frames)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        // Just a refcount bump — no heap allocation
        self.cached_data.clone()
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn last_output_frames(&self) -> Option<usize> {
        // Always pass through the frame count
        Some(self.monitor.last_num_frames())
    }
}
