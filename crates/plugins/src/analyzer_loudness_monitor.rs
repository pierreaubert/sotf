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
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::{Arc, Mutex};

/// Real-time loudness measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessInfo {
    /// Momentary loudness (M) - 400ms window, updated every 100ms
    /// Range: -inf to ~0 LUFS (typical: -40 to 0)
    pub momentary_lufs: f64,

    /// Short-term loudness (S) - 3 second window
    /// Range: -inf to ~0 LUFS (typical: -40 to 0)
    pub shortterm_lufs: f64,

    /// Integrated loudness (I) - whole program loudness
    /// Range: -inf to ~0 LUFS (typical: -40 to 0)
    pub integrated_lufs: f64,

    /// Current sample peak across all channels (0.0 to 1.0+)
    pub peak: f64,

    /// Per-channel sample peaks (0.0 to 1.0+)
    pub channel_peaks: Vec<f64>,

    /// Per-channel true peaks in dBTP (dB True Peak)
    pub true_peaks_dbtp: Vec<f64>,

    /// L/R correlation coefficient (only for stereo)
    pub correlation_lr: Option<f64>,
}

impl Default for LoudnessInfo {
    fn default() -> Self {
        Self {
            momentary_lufs: f64::NEG_INFINITY,
            shortterm_lufs: f64::NEG_INFINITY,
            integrated_lufs: f64::NEG_INFINITY,
            peak: 0.0,
            channel_peaks: Vec::new(),
            true_peaks_dbtp: Vec::new(),
            correlation_lr: None,
        }
    }
}

/// Thread-safe loudness monitor for real-time audio analysis
pub(crate) struct LoudnessMonitor {
    /// EBU R128 analyzer
    ebur128: Arc<Mutex<EbuR128>>,
    /// Number of channels
    channels: u32,
    /// Sample rate
    sample_rate: u32,
    /// Current measurements
    current_loudness: Arc<Mutex<LoudnessInfo>>,
    /// Per-channel peak trackers (separate from EBU R128 for meter display)
    channel_peak_trackers: Arc<Mutex<Vec<f64>>>,
    /// Peak decay rate per sample (for visual meter decay)
    peak_decay_per_sample: f64,
    /// Correlation computation buffer (for stereo L/R correlation)
    /// Stores recent samples for correlation calculation
    correlation_buffer_l: Arc<Mutex<Vec<f32>>>,
    correlation_buffer_r: Arc<Mutex<Vec<f32>>>,
    /// Maximum correlation buffer size (e.g., 1 second of audio)
    correlation_buffer_size: usize,
    /// Scratch buffer for true peaks calculation
    true_peaks_scratch: Arc<Mutex<Vec<f64>>>,
    /// Scratch buffer for new channel peaks calculation
    channel_peaks_scratch: Arc<Mutex<Vec<f64>>>,
}

impl LoudnessMonitor {
    /// Create a new loudness monitor
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub(crate) fn new(channels: u32, sample_rate: u32) -> Result<Self, String> {
        // Enable M (momentary), S (short-term), I (integrated), SAMPLE_PEAK, and TRUE_PEAK
        let ebur128 = EbuR128::new(
            channels,
            sample_rate,
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK | Mode::TRUE_PEAK,
        )
        .map_err(|e| format!("Failed to create EBU R128 analyzer: {:?}", e))?;

        // Calculate decay rate: decay to 0.0 over ~300ms (roughly -60 dB in 1.5 seconds)
        // Decay factor per sample = (1 - decay_time_samples)^(-1)
        // For 300ms hold + exponential decay: peak * (1 - decay_rate)^samples
        // We want to reach 0.01 (1%) in about 300ms
        // 0.01 = 1.0 * decay_rate^(sample_rate * 0.3)
        // decay_rate = 0.01^(1 / (sample_rate * 0.3))
        let decay_time_seconds = 0.3;
        let decay_samples = sample_rate as f64 * decay_time_seconds;
        // Use a linear decay for simplicity: subtract this much per sample
        let peak_decay_per_sample = 1.0 / decay_samples;

        // Correlation buffer: use 1 second of audio for correlation computation
        let correlation_buffer_size = sample_rate as usize;

        Ok(Self {
            ebur128: Arc::new(Mutex::new(ebur128)),
            channels,
            sample_rate,
            current_loudness: Arc::new(Mutex::new(LoudnessInfo::default())),
            channel_peak_trackers: Arc::new(Mutex::new(vec![0.0; channels as usize])),
            peak_decay_per_sample,
            correlation_buffer_l: Arc::new(Mutex::new(Vec::with_capacity(correlation_buffer_size))),
            correlation_buffer_r: Arc::new(Mutex::new(Vec::with_capacity(correlation_buffer_size))),
            correlation_buffer_size,
            true_peaks_scratch: Arc::new(Mutex::new(vec![f64::NEG_INFINITY; channels as usize])),
            channel_peaks_scratch: Arc::new(Mutex::new(vec![0.0; channels as usize])),
        })
    }

    /// Add audio frames to the analyzer
    ///
    /// # Arguments
    /// * `samples` - Interleaved f32 samples in range [-1.0, 1.0]
    ///
    /// # Returns
    /// Ok(()) if successful, Err if analysis fails
    pub(crate) fn add_frames(&self, samples: &[f32]) -> Result<(), String> {
        let mut ebur = self.ebur128.lock().unwrap();

        // Add frames to the analyzer
        ebur.add_frames_f32(samples)
            .map_err(|e| format!("Failed to add frames: {:?}", e))?;

        // Update measurements
        let momentary_lufs = ebur.loudness_momentary().unwrap_or(f64::NEG_INFINITY);
        let shortterm_lufs = ebur.loudness_shortterm().unwrap_or(f64::NEG_INFINITY);
        let integrated_lufs = ebur.loudness_global().unwrap_or(f64::NEG_INFINITY);

        // Get true peaks per channel from ebur128 - use scratch buffer
        let mut true_peaks_dbtp_guard = self.true_peaks_scratch.lock().unwrap();
        let true_peaks_dbtp = &mut *true_peaks_dbtp_guard;
        true_peaks_dbtp.fill(f64::NEG_INFINITY);

        for (ch, peak_db) in true_peaks_dbtp
            .iter_mut()
            .enumerate()
            .take(self.channels as usize)
        {
            match ebur.true_peak(ch as u32) {
                Ok(true_peak_linear) => {
                    // Convert to dBTP (dB True Peak)
                    // dBTP = 20 * log10(true_peak_linear)
                    // Use a small threshold to avoid log10(0) = -inf
                    if true_peak_linear >= 1e-10 {
                        *peak_db = 20.0 * true_peak_linear.log10();
                    } else {
                        // Very quiet or silent channel
                        *peak_db = f64::NEG_INFINITY;
                    }
                }
                Err(_e) => {
                    // Channel might not be available yet - ignore error in RT callback
                }
            }
        }

        // Calculate per-channel peaks from the current buffer with decay
        let num_frames = samples.len() / self.channels as usize;
        let mut peak = 0.0f64;

        let mut new_channel_peaks_guard = self.channel_peaks_scratch.lock().unwrap();
        let new_channel_peaks = &mut *new_channel_peaks_guard;
        new_channel_peaks.fill(0.0);

        // Get current peak levels by scanning the buffer
        for frame_idx in 0..num_frames {
            for (ch, channel_peak) in new_channel_peaks
                .iter_mut()
                .enumerate()
                .take(self.channels as usize)
            {
                let sample_idx = frame_idx * self.channels as usize + ch;
                let sample_abs = samples[sample_idx].abs() as f64;
                *channel_peak = f64::max(*channel_peak, sample_abs);
                peak = f64::max(peak, sample_abs);
            }
        }

        // Compute correlation for stereo signals
        let correlation_lr = if self.channels == 2 {
            self.compute_correlation_stereo(samples)
        } else {
            None
        };

        // Apply decay to existing peaks and take max with new peaks
        {
            let mut peak_trackers = self.channel_peak_trackers.lock().unwrap();

            // Decay existing peaks
            for tracker in peak_trackers.iter_mut() {
                *tracker = (*tracker - self.peak_decay_per_sample * num_frames as f64).max(0.0);
            }

            // Update with new peaks (take max of decayed and new)
            for (tracker, new_peak) in peak_trackers.iter_mut().zip(new_channel_peaks.iter()) {
                *tracker = f64::max(*tracker, *new_peak);
            }

            // Use the peak trackers as the channel peaks
            let channel_peaks = peak_trackers.clone();
            peak = channel_peaks.iter().cloned().fold(0.0, f64::max);

            // Update shared state
            let mut info = self.current_loudness.lock().unwrap();
            info.momentary_lufs = momentary_lufs;
            info.shortterm_lufs = shortterm_lufs;
            info.integrated_lufs = integrated_lufs;
            info.peak = peak;
            info.channel_peaks = channel_peaks;
            info.true_peaks_dbtp = true_peaks_dbtp.clone();
            info.correlation_lr = correlation_lr;
        }

        Ok(())
    }

    /// Compute L/R correlation for stereo signals
    ///
    /// # Arguments
    /// * `samples` - Interleaved stereo samples (L, R, L, R, ...)
    ///
    /// # Returns
    /// Correlation coefficient in range [-1.0, +1.0], or None if not enough data
    fn compute_correlation_stereo(&self, samples: &[f32]) -> Option<f64> {
        if self.channels != 2 {
            return None;
        }

        let num_frames = samples.len() / 2;

        // Update correlation buffers (rolling window)
        {
            let mut buf_l = self.correlation_buffer_l.lock().unwrap();
            let mut buf_r = self.correlation_buffer_r.lock().unwrap();

            // Extract L and R samples
            for frame_idx in 0..num_frames {
                let l = samples[frame_idx * 2];
                let r = samples[frame_idx * 2 + 1];

                buf_l.push(l);
                buf_r.push(r);
            }

            // Keep only last correlation_buffer_size samples
            if buf_l.len() > self.correlation_buffer_size {
                let excess = buf_l.len() - self.correlation_buffer_size;
                buf_l.drain(0..excess);
            }
            if buf_r.len() > self.correlation_buffer_size {
                let excess = buf_r.len() - self.correlation_buffer_size;
                buf_r.drain(0..excess);
            }

            // Need at least 100 samples for meaningful correlation
            if buf_l.len() < 100 {
                return None;
            }

            // Compute Pearson correlation coefficient
            let n = buf_l.len() as f64;
            let mean_l: f64 = buf_l.iter().map(|&x| x as f64).sum::<f64>() / n;
            let mean_r: f64 = buf_r.iter().map(|&x| x as f64).sum::<f64>() / n;

            let mut cov_lr = 0.0;
            let mut var_l = 0.0;
            let mut var_r = 0.0;

            for i in 0..buf_l.len() {
                let diff_l = buf_l[i] as f64 - mean_l;
                let diff_r = buf_r[i] as f64 - mean_r;
                cov_lr += diff_l * diff_r;
                var_l += diff_l * diff_l;
                var_r += diff_r * diff_r;
            }

            // Avoid division by zero
            if var_l < 1e-10 || var_r < 1e-10 {
                return Some(0.0);
            }

            let correlation = cov_lr / (var_l * var_r).sqrt();

            // Clamp to [-1, +1] to handle numerical errors
            Some(correlation.clamp(-1.0, 1.0))
        }
    }

    /// Get the current loudness measurements
    pub(crate) fn get_loudness(&self) -> LoudnessInfo {
        let info = self.current_loudness.lock().unwrap();
        info.clone()
    }

    /// Reset the monitor (clear all history)
    pub(crate) fn reset(&self) -> Result<(), String> {
        let mut ebur = self.ebur128.lock().unwrap();

        // Create a new EBU R128 instance to reset state
        let new_ebur = EbuR128::new(
            self.channels,
            self.sample_rate,
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK | Mode::TRUE_PEAK,
        )
        .map_err(|e| format!("Failed to reset analyzer: {:?}", e))?;

        *ebur = new_ebur;

        // Reset measurements
        {
            let mut info = self.current_loudness.lock().unwrap();
            *info = LoudnessInfo::default();
        }

        // Reset peak trackers
        {
            let mut peak_trackers = self.channel_peak_trackers.lock().unwrap();
            peak_trackers.fill(0.0);
        }

        // Reset correlation buffers
        {
            let mut buf_l = self.correlation_buffer_l.lock().unwrap();
            let mut buf_r = self.correlation_buffer_r.lock().unwrap();
            buf_l.clear();
            buf_r.clear();
        }

        Ok(())
    }
}

impl Clone for LoudnessMonitor {
    fn clone(&self) -> Self {
        Self {
            ebur128: Arc::clone(&self.ebur128),
            channels: self.channels,
            sample_rate: self.sample_rate,
            current_loudness: Arc::clone(&self.current_loudness),
            channel_peak_trackers: Arc::clone(&self.channel_peak_trackers),
            peak_decay_per_sample: self.peak_decay_per_sample,
            correlation_buffer_l: Arc::clone(&self.correlation_buffer_l),
            correlation_buffer_r: Arc::clone(&self.correlation_buffer_r),
            correlation_buffer_size: self.correlation_buffer_size,
            true_peaks_scratch: Arc::clone(&self.true_peaks_scratch),
            channel_peaks_scratch: Arc::clone(&self.channel_peaks_scratch),
        }
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
}

impl LoudnessMonitorPlugin {
    /// Create a new loudness monitor plugin
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let monitor = LoudnessMonitor::new(num_channels as u32, 48000)?;

        Ok(Self {
            monitor,
            num_channels,
        })
    }

    /// Get current loudness measurements
    pub fn get_loudness(&self) -> LoudnessInfo {
        self.monitor.get_loudness()
    }

    /// Convert LoudnessInfo to LoudnessData
    fn to_loudness_data(info: &LoudnessInfo) -> LoudnessData {
        LoudnessData {
            momentary_lufs: info.momentary_lufs,
            shortterm_lufs: info.shortterm_lufs,
            integrated_lufs: info.integrated_lufs,
            peak: info.peak,
            channel_peaks: info.channel_peaks.clone(),
            true_peaks_dbtp: info.true_peaks_dbtp.clone(),
            correlation_lr: info.correlation_lr,
        }
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
        // Recreate the monitor with the new sample rate
        self.monitor = LoudnessMonitor::new(self.num_channels as u32, sample_rate)
            .map_err(|e| format!("Failed to initialize loudness monitor: {}", e))?;

        Ok(())
    }

    fn reset(&mut self) {
        self.monitor.reset().ok();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify input/output size
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

        Ok(())
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let info = self.monitor.get_loudness();
        let data = Self::to_loudness_data(&info);
        Some(Arc::new(data))
    }

    fn latency_samples(&self) -> usize {
        // EBU R128 has some latency due to the windowing, but it's minimal
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plugin;

    #[test]
    fn test_loudness_monitor_plugin_creation() {
        let plugin = LoudnessMonitorPlugin::new(2).unwrap();
        assert_eq!(plugin.input_channels(), 2);
    }

    #[test]
    fn test_loudness_monitor_plugin_processing() {
        let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Create test signal: 1kHz sine wave at -20dBFS
        let num_frames = 4800; // 100ms at 48kHz
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.1; // -20dBFS
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let mut output = vec![0.0_f32; num_frames * 2];

        // Process
        plugin.process(&input, &mut output, &context).unwrap();

        // Get measurements
        let data = plugin.get_data().unwrap();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        log::info!("Momentary LUFS: {:.1}", loudness_data.momentary_lufs);
        log::info!("Short-term LUFS: {:.1}", loudness_data.shortterm_lufs);
        log::info!("Peak: {:.3}", loudness_data.peak);

        // Peak should be around 0.1
        assert!(
            loudness_data.peak > 0.05 && loudness_data.peak < 0.15,
            "Peak should be around 0.1, got {}",
            loudness_data.peak
        );
    }

    #[test]
    fn test_loudness_monitor_plugin_reset() {
        let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let num_frames = 1024;
        let input = vec![0.5_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let mut output = vec![0.0_f32; num_frames * 2];
        plugin.process(&input, &mut output, &context).unwrap();

        // Reset
        plugin.reset();

        // Measurements should be reset
        let data = plugin.get_data().unwrap();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        // After reset, values should be back to default (negative infinity for LUFS)
        log::info!(
            "After reset - Momentary: {:.1}, Peak: {:.3}",
            loudness_data.momentary_lufs,
            loudness_data.peak
        );
    }

    #[test]
    fn test_loudness_monitor_plugin_multichannel() {
        // Test with 5 channels (5.0 surround)
        let mut plugin = LoudnessMonitorPlugin::new(5).unwrap();
        plugin.initialize(48000).unwrap();

        let num_frames = 1024;
        let mut input = vec![0.0_f32; num_frames * 5];

        // Different signal on each channel
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            input[i * 5 + 0] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.1;
            input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 550.0 * t).sin() * 0.1;
            input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.1;
            input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 770.0 * t).sin() * 0.1;
            input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.1;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let mut output = vec![0.0_f32; num_frames * 5];
        plugin.process(&input, &mut output, &context).unwrap();

        let data = plugin.get_data().unwrap();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        log::info!(
            "5-channel loudness: {:.1} LUFS, peak: {:.3}",
            loudness_data.momentary_lufs,
            loudness_data.peak
        );

        assert!(loudness_data.peak > 0.0, "Peak should be non-zero");
    }
}
