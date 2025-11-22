// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================
//
// Wraps the LoudnessMonitor as an AnalyzerPlugin.
// Provides real-time EBU R128 loudness measurements.

use super::analyzer::{AnalyzerPlugin, LoudnessData};
use super::plugin::{PluginInfo, PluginResult, ProcessContext};
use std::any::Any;

// ============================================================================
// Core Loudness Monitor Implementation
// ============================================================================

use ebur128::{EbuR128, Mode};
use serde::{Deserialize, Serialize};
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

    /// Current sample peak across all channels (0.0 to 1.0+)
    pub peak: f64,

    /// Per-channel peaks (0.0 to 1.0+)
    pub channel_peaks: Vec<f64>,
}

impl Default for LoudnessInfo {
    fn default() -> Self {
        Self {
            momentary_lufs: f64::NEG_INFINITY,
            shortterm_lufs: f64::NEG_INFINITY,
            peak: 0.0,
            channel_peaks: Vec::new(),
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
}

impl LoudnessMonitor {
    /// Create a new loudness monitor
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub(crate) fn new(channels: u32, sample_rate: u32) -> Result<Self, String> {
        let ebur128 = EbuR128::new(channels, sample_rate, Mode::M | Mode::S | Mode::SAMPLE_PEAK)
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

        Ok(Self {
            ebur128: Arc::new(Mutex::new(ebur128)),
            channels,
            sample_rate,
            current_loudness: Arc::new(Mutex::new(LoudnessInfo::default())),
            channel_peak_trackers: Arc::new(Mutex::new(vec![0.0; channels as usize])),
            peak_decay_per_sample,
        })
    }

    /// Add audio frames to the analyzer
    ///
    /// # Arguments
    /// * `samples` - Interleaved f32 samples in range [-1.0, 1.0]
    ///
    /// # Returns
    /// Ok(()) if successful, Err if analysis fails
    fn add_frames(&self, samples: &[f32]) -> Result<(), String> {
        let mut ebur = self.ebur128.lock().unwrap();

        // Add frames to the analyzer
        ebur.add_frames_f32(samples)
            .map_err(|e| format!("Failed to add frames: {:?}", e))?;

        // Update measurements
        let momentary_lufs = ebur.loudness_momentary().unwrap_or(f64::NEG_INFINITY);
        let shortterm_lufs = ebur.loudness_shortterm().unwrap_or(f64::NEG_INFINITY);

        // Calculate per-channel peaks from the current buffer with decay
        let num_frames = samples.len() / self.channels as usize;
        let mut peak = 0.0f64;
        let mut new_channel_peaks = vec![0.0; self.channels as usize];

        // Get current peak levels by scanning the buffer
        for frame_idx in 0..num_frames {
            for ch in 0..self.channels as usize {
                let sample_idx = frame_idx * self.channels as usize + ch;
                let sample_abs = samples[sample_idx].abs() as f64;
                new_channel_peaks[ch] = f64::max(new_channel_peaks[ch], sample_abs);
                peak = f64::max(peak, sample_abs);
            }
        }

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
            info.peak = peak;
            info.channel_peaks = channel_peaks;
        }

        Ok(())
    }

    /// Get the current loudness measurements
    fn get_loudness(&self) -> LoudnessInfo {
        let info = self.current_loudness.lock().unwrap();
        info.clone()
    }

    /// Reset the monitor (clear all history)
    fn reset(&self) -> Result<(), String> {
        let mut ebur = self.ebur128.lock().unwrap();

        // Create a new EBU R128 instance to reset state
        let new_ebur = EbuR128::new(
            self.channels,
            self.sample_rate,
            Mode::M | Mode::S | Mode::SAMPLE_PEAK,
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
            peak: info.peak,
            channel_peaks: info.channel_peaks.clone(),
        }
    }
}

impl AnalyzerPlugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Loudness Monitor".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Real-time EBU R128 loudness monitoring (LUFS, peaks)".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
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

    fn process(&mut self, input: &[f32], context: &ProcessContext) -> PluginResult<()> {
        // Verify input size
        let expected_samples = context.num_frames * self.num_channels;
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }

        // Add frames to the monitor
        self.monitor
            .add_frames(input)
            .map_err(|e| format!("Failed to add frames to loudness monitor: {}", e))?;

        Ok(())
    }

    fn get_data(&self) -> Box<dyn Any + Send> {
        let info = self.monitor.get_loudness();
        Box::new(Self::to_loudness_data(&info))
    }

    fn latency_samples(&self) -> usize {
        // EBU R128 has some latency due to the windowing, but it's minimal
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Process
        plugin.process(&input, &context).unwrap();

        // Get measurements
        let data = plugin.get_data();
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

        plugin.process(&input, &context).unwrap();

        // Reset
        plugin.reset();

        // Measurements should be reset
        let data = plugin.get_data();
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

        plugin.process(&input, &context).unwrap();

        let data = plugin.get_data();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        log::info!(
            "5-channel loudness: {:.1} LUFS, peak: {:.3}",
            loudness_data.momentary_lufs,
            loudness_data.peak
        );

        assert!(loudness_data.peak > 0.0, "Peak should be non-zero");
    }
}
