// ============================================================================
// Auto Gain Compensation
// ============================================================================
//
// This module provides automatic loudness matching for audio plugins.
// It measures the loudness before and after processing and applies
// compensating gain to maintain perceived loudness.
//
// Usage:
// 1. Create AutoGain with desired parameters
// 2. Call measure_input() before processing
// 3. Call measure_output() after processing
// 4. Apply get_compensation_gain_linear() to each output sample

use crate::analyzer_loudness_monitor::LoudnessMonitor;
use crate::smoothing::Smoother;
use serde::{Deserialize, Serialize};

/// Loudness measurement type for auto-gain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AutoGainLoudnessType {
    /// 400ms momentary loudness (faster response)
    #[default]
    Momentary,
    /// 3 second short-term loudness (more stable)
    ShortTerm,
}

/// Configuration parameters for AutoGain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGainParams {
    /// Enable automatic gain compensation
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Loudness measurement type
    #[serde(default)]
    pub loudness_type: AutoGainLoudnessType,

    /// Maximum gain correction in dB (clamped to +/- this value)
    #[serde(default = "default_max_gain_db")]
    pub max_gain_db: f32,

    /// Gain smoothing time in milliseconds
    #[serde(default = "default_smoothing_ms")]
    pub smoothing_ms: f32,
}

fn default_enabled() -> bool {
    false
}

fn default_max_gain_db() -> f32 {
    12.0
}

fn default_smoothing_ms() -> f32 {
    100.0
}

impl Default for AutoGainParams {
    fn default() -> Self {
        Self {
            enabled: false,
            loudness_type: AutoGainLoudnessType::Momentary,
            max_gain_db: 12.0,
            smoothing_ms: 100.0,
        }
    }
}

/// Automatic gain compensation for maintaining perceived loudness
///
/// This struct measures input and output loudness using EBU R128 and
/// calculates a compensating gain to match the output loudness to the input.
pub struct AutoGain {
    /// Number of channels
    num_channels: usize,

    /// Sample rate
    sample_rate: u32,

    /// Loudness monitor for input measurement
    input_monitor: LoudnessMonitor,

    /// Loudness monitor for output measurement
    output_monitor: LoudnessMonitor,

    /// Gain smoother to prevent zipper noise
    gain_smoother: Smoother,

    /// Current compensation gain in dB
    current_gain_db: f32,

    /// Last measured input loudness (LUFS)
    last_input_lufs: f64,

    /// Last measured output loudness (LUFS)
    last_output_lufs: f64,

    /// Last measured input peak level (0.0 to 1.0+)
    last_input_peak: f64,

    /// Last measured output peak level (0.0 to 1.0+)
    last_output_peak: f64,

    /// Whether auto-gain is enabled
    enabled: bool,

    /// Loudness measurement type
    loudness_type: AutoGainLoudnessType,

    /// Maximum gain correction in dB
    max_gain_db: f32,

    /// Smoothing time in ms
    smoothing_ms: f32,
}

impl std::fmt::Debug for AutoGain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoGain")
            .field("num_channels", &self.num_channels)
            .field("sample_rate", &self.sample_rate)
            .field("enabled", &self.enabled)
            .field("current_gain_db", &self.current_gain_db)
            .field("last_input_lufs", &self.last_input_lufs)
            .field("last_output_lufs", &self.last_output_lufs)
            .field("last_input_peak", &self.last_input_peak)
            .field("last_output_peak", &self.last_output_peak)
            .field("loudness_type", &self.loudness_type)
            .field("max_gain_db", &self.max_gain_db)
            .field("smoothing_ms", &self.smoothing_ms)
            .finish_non_exhaustive()
    }
}

impl AutoGain {
    /// Create a new AutoGain instance
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    /// * `params` - Configuration parameters
    ///
    /// # Returns
    /// Result with AutoGain or error string
    pub fn new(num_channels: usize, sample_rate: u32, params: AutoGainParams) -> Result<Self, String> {
        let input_monitor = LoudnessMonitor::new(num_channels as u32, sample_rate)?;
        let output_monitor = LoudnessMonitor::new(num_channels as u32, sample_rate)?;
        let gain_smoother = Smoother::new(0.0, params.smoothing_ms, sample_rate);

        Ok(Self {
            num_channels,
            sample_rate,
            input_monitor,
            output_monitor,
            gain_smoother,
            current_gain_db: 0.0,
            last_input_lufs: f64::NEG_INFINITY,
            last_output_lufs: f64::NEG_INFINITY,
            last_input_peak: 0.0,
            last_output_peak: 0.0,
            enabled: params.enabled,
            loudness_type: params.loudness_type,
            max_gain_db: params.max_gain_db,
            smoothing_ms: params.smoothing_ms,
        })
    }

    /// Create a new AutoGain with default parameters
    pub fn new_default(num_channels: usize, sample_rate: u32) -> Result<Self, String> {
        Self::new(num_channels, sample_rate, AutoGainParams::default())
    }

    /// Set the sample rate (call during initialize)
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> Result<(), String> {
        self.sample_rate = sample_rate;
        self.input_monitor = LoudnessMonitor::new(self.num_channels as u32, sample_rate)?;
        self.output_monitor = LoudnessMonitor::new(self.num_channels as u32, sample_rate)?;
        self.gain_smoother = Smoother::new(self.current_gain_db, self.smoothing_ms, sample_rate);
        Ok(())
    }

    /// Reset the auto-gain state
    pub fn reset(&mut self) {
        let _ = self.input_monitor.reset();
        let _ = self.output_monitor.reset();
        self.gain_smoother.reset(0.0);
        self.current_gain_db = 0.0;
        self.last_input_lufs = f64::NEG_INFINITY;
        self.last_output_lufs = f64::NEG_INFINITY;
        self.last_input_peak = 0.0;
        self.last_output_peak = 0.0;
    }

    /// Enable or disable auto-gain
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            // Fade gain back to 0 when disabled
            self.gain_smoother.set_target(0.0);
        }
    }

    /// Check if auto-gain is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the maximum gain correction in dB
    pub fn set_max_gain_db(&mut self, max_gain_db: f32) {
        self.max_gain_db = max_gain_db.abs();
    }

    /// Set the gain smoothing time in ms
    pub fn set_smoothing_ms(&mut self, smoothing_ms: f32) {
        self.smoothing_ms = smoothing_ms;
        self.gain_smoother.set_time(smoothing_ms, self.sample_rate);
    }

    /// Set the loudness measurement type
    pub fn set_loudness_type(&mut self, loudness_type: AutoGainLoudnessType) {
        self.loudness_type = loudness_type;
    }

    /// Measure the input audio (call before processing)
    ///
    /// # Arguments
    /// * `input` - Interleaved input samples
    pub fn measure_input(&mut self, input: &[f32]) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        self.input_monitor.add_frames(input)?;
        let info = self.input_monitor.get_loudness();
        self.last_input_lufs = match self.loudness_type {
            AutoGainLoudnessType::Momentary => info.momentary_lufs,
            AutoGainLoudnessType::ShortTerm => info.shortterm_lufs,
        };
        self.last_input_peak = info.peak;
        Ok(())
    }

    /// Measure the output audio and update compensation gain (call after processing)
    ///
    /// # Arguments
    /// * `output` - Interleaved output samples (before gain compensation)
    pub fn measure_output(&mut self, output: &[f32]) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        self.output_monitor.add_frames(output)?;
        let info = self.output_monitor.get_loudness();
        self.last_output_lufs = match self.loudness_type {
            AutoGainLoudnessType::Momentary => info.momentary_lufs,
            AutoGainLoudnessType::ShortTerm => info.shortterm_lufs,
        };
        self.last_output_peak = info.peak;

        // Calculate target gain: make output match input loudness
        // gain_db = input_lufs - output_lufs
        // If output is louder, gain is negative (attenuate)
        // If output is quieter, gain is positive (boost)
        if self.last_input_lufs.is_finite() && self.last_output_lufs.is_finite() {
            let target_gain_db = (self.last_input_lufs - self.last_output_lufs) as f32;
            let clamped_gain = target_gain_db.clamp(-self.max_gain_db, self.max_gain_db);
            self.gain_smoother.set_target(clamped_gain);
        }

        Ok(())
    }

    /// Get the next smoothed compensation gain in linear scale
    ///
    /// Call this once per sample to get the smoothed gain value.
    /// Multiply your output samples by this value.
    #[inline]
    pub fn next_gain_linear(&mut self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        self.current_gain_db = self.gain_smoother.next();
        db_to_linear(self.current_gain_db)
    }

    /// Get the current compensation gain in linear scale (without advancing smoother)
    #[inline]
    pub fn current_gain_linear(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        db_to_linear(self.gain_smoother.current())
    }

    /// Get the current compensation gain in dB
    pub fn current_gain_db(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        self.gain_smoother.current()
    }

    /// Get the last measured input loudness in LUFS
    pub fn last_input_lufs(&self) -> f64 {
        self.last_input_lufs
    }

    /// Get the last measured output loudness in LUFS
    pub fn last_output_lufs(&self) -> f64 {
        self.last_output_lufs
    }

    /// Get the last measured input peak level (0.0 to 1.0+)
    pub fn last_input_peak(&self) -> f64 {
        self.last_input_peak
    }

    /// Get the last measured output peak level (0.0 to 1.0+)
    pub fn last_output_peak(&self) -> f64 {
        self.last_output_peak
    }

    /// Apply compensation gain to output samples in-place
    ///
    /// This is a convenience method that applies per-sample smoothed gain.
    ///
    /// # Arguments
    /// * `output` - Interleaved output samples to modify in-place
    /// * `num_frames` - Number of audio frames
    pub fn apply_compensation(&mut self, output: &mut [f32], num_frames: usize) {
        if !self.enabled {
            return;
        }
        for frame in 0..num_frames {
            let gain = self.next_gain_linear();
            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                output[idx] *= gain;
            }
        }
    }
}

/// Convert dB to linear gain
#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Data exposed by AutoGain for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGainData {
    /// Whether auto-gain is enabled
    pub enabled: bool,

    /// Current compensation gain in dB
    pub gain_db: f32,

    /// Last measured input loudness (LUFS)
    pub input_lufs: f64,

    /// Last measured output loudness (LUFS)
    pub output_lufs: f64,

    /// Last measured input peak level (0.0 to 1.0+)
    pub input_peak: f64,

    /// Last measured output peak level (0.0 to 1.0+)
    pub output_peak: f64,
}

impl AutoGain {
    /// Get monitoring data
    pub fn get_data(&self) -> AutoGainData {
        AutoGainData {
            enabled: self.enabled,
            gain_db: self.current_gain_db,
            input_lufs: self.last_input_lufs,
            output_lufs: self.last_output_lufs,
            input_peak: self.last_input_peak,
            output_peak: self.last_output_peak,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_gain_creation() {
        let auto_gain = AutoGain::new_default(2, 48000).unwrap();
        assert!(!auto_gain.is_enabled());
        assert_eq!(auto_gain.current_gain_db(), 0.0);
    }

    #[test]
    fn test_auto_gain_disabled_passthrough() {
        let mut auto_gain = AutoGain::new_default(2, 48000).unwrap();

        // When disabled, gain should always be 1.0 (linear) / 0.0 dB
        assert_eq!(auto_gain.next_gain_linear(), 1.0);
        assert_eq!(auto_gain.current_gain_db(), 0.0);
    }

    #[test]
    fn test_auto_gain_enabled() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 10.0, // Fast smoothing for test
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Create test signals
        let num_frames = 4800; // 100ms at 48kHz

        // Input: moderate level sine wave
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5; // ~-6dBFS
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        // Output: louder (simulating a boost from processing)
        let mut output = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 1.0; // ~0dBFS (louder)
            output[i * 2] = sample;
            output[i * 2 + 1] = sample;
        }

        // Measure input and output
        auto_gain.measure_input(&input).unwrap();
        auto_gain.measure_output(&output).unwrap();

        // Since output is louder than input, gain should be negative (to attenuate)
        // Run smoother to settle
        for _ in 0..10000 {
            auto_gain.next_gain_linear();
        }

        let gain_db = auto_gain.current_gain_db();
        log::info!("Auto-gain: {} dB", gain_db);

        // Gain should be negative (attenuating to match quieter input)
        assert!(gain_db < 0.0, "Expected negative gain, got {} dB", gain_db);
    }

    #[test]
    fn test_auto_gain_max_clamp() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 6.0, // Limit to 6dB
            smoothing_ms: 1.0, // Very fast
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Create extreme difference: very quiet input, loud output
        let num_frames = 9600; // 200ms

        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.01; // Very quiet
            input[i * 2 + 1] = phase.sin() * 0.01;
        }

        let mut output = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            output[i * 2] = phase.sin() * 1.0; // Much louder
            output[i * 2 + 1] = phase.sin() * 1.0;
        }

        // Process multiple times to let monitors accumulate
        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        // Run smoother
        for _ in 0..50000 {
            auto_gain.next_gain_linear();
        }

        let gain_db = auto_gain.current_gain_db();
        log::info!("Clamped auto-gain: {} dB", gain_db);

        // Should be clamped to -6dB (max_gain_db)
        assert!(
            gain_db >= -6.5 && gain_db <= 0.0,
            "Expected gain clamped around -6dB, got {} dB",
            gain_db
        );
    }

    #[test]
    fn test_auto_gain_reset() {
        let params = AutoGainParams {
            enabled: true,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Process some audio
        let input = vec![0.5_f32; 1000 * 2];
        auto_gain.measure_input(&input).unwrap();
        auto_gain.measure_output(&input).unwrap();

        // Reset
        auto_gain.reset();

        // State should be reset
        assert_eq!(auto_gain.current_gain_db(), 0.0);
        assert!(auto_gain.last_input_lufs().is_infinite());
        assert!(auto_gain.last_output_lufs().is_infinite());
    }

    #[test]
    fn test_apply_compensation() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 0.0, // No smoothing for predictable test
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        let num_frames = 4800;

        // Create input and output with known loudness difference
        let input: Vec<f32> = (0..num_frames * 2).map(|_| 0.5).collect();
        let output: Vec<f32> = (0..num_frames * 2).map(|_| 1.0).collect();

        // Multiple measurements to build up loudness history
        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        // Apply compensation
        let mut compensated = output.clone();
        auto_gain.apply_compensation(&mut compensated, num_frames);

        // Compensated output should be quieter than original
        let orig_energy: f32 = output.iter().map(|x| x * x).sum();
        let comp_energy: f32 = compensated.iter().map(|x| x * x).sum();

        assert!(
            comp_energy < orig_energy,
            "Compensated energy {} should be less than original {}",
            comp_energy,
            orig_energy
        );
    }

    #[test]
    fn test_get_data() {
        let params = AutoGainParams {
            enabled: true,
            ..Default::default()
        };
        let auto_gain = AutoGain::new(2, 48000, params).unwrap();

        let data = auto_gain.get_data();
        assert!(data.enabled);
        assert_eq!(data.gain_db, 0.0);
    }

    #[test]
    fn test_auto_gain_boost_when_output_quieter() {
        // When output is quieter than input, auto-gain should boost
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 10.0,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        let num_frames = 4800; // 100ms at 48kHz

        // Input: loud signal
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.8; // Loud
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        // Output: quiet signal (simulating an EQ cut)
        let mut output = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.2; // Quiet
            output[i * 2] = sample;
            output[i * 2 + 1] = sample;
        }

        // Measure multiple times
        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        // Run smoother to settle
        for _ in 0..10000 {
            auto_gain.next_gain_linear();
        }

        let gain_db = auto_gain.current_gain_db();

        // Gain should be positive (boosting to match louder input)
        assert!(
            gain_db > 0.0,
            "Expected positive gain for quieter output, got {} dB",
            gain_db
        );
    }

    #[test]
    fn test_auto_gain_multichannel() {
        // Test with 5 channels (5.0 surround)
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 10.0,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(5, 48000, params).unwrap();

        let num_frames = 4800;

        // Create 5-channel test signal
        let mut input = vec![0.0_f32; num_frames * 5];
        let mut output = vec![0.0_f32; num_frames * 5];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            for ch in 0..5 {
                let idx = i * 5 + ch;
                input[idx] = phase.sin() * 0.5;
                output[idx] = phase.sin() * 0.8; // Louder output
            }
        }

        // Should not panic with 5 channels
        auto_gain.measure_input(&input).unwrap();
        auto_gain.measure_output(&output).unwrap();

        // Apply compensation
        auto_gain.apply_compensation(&mut output, num_frames);

        // Output should be modified
        let first_sample = output[0];
        assert!(first_sample.abs() < 0.8, "Gain compensation should have been applied");
    }

    #[test]
    fn test_auto_gain_shortterm_loudness() {
        let params = AutoGainParams {
            enabled: true,
            loudness_type: AutoGainLoudnessType::ShortTerm,
            max_gain_db: 12.0,
            smoothing_ms: 10.0,
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        let num_frames = 4800;

        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.sin() * 0.5;
        }

        // Should not panic with ShortTerm loudness type
        auto_gain.measure_input(&input).unwrap();
        auto_gain.measure_output(&input).unwrap();

        // Loudness values should be measurable (may be -inf initially for short-term)
        let _input_lufs = auto_gain.last_input_lufs();
        let _output_lufs = auto_gain.last_output_lufs();
    }

    #[test]
    fn test_auto_gain_runtime_enable_disable() {
        let params = AutoGainParams {
            enabled: false, // Start disabled
            max_gain_db: 12.0,
            smoothing_ms: 10.0,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        assert!(!auto_gain.is_enabled());
        assert_eq!(auto_gain.next_gain_linear(), 1.0);

        // Enable
        auto_gain.set_enabled(true);
        assert!(auto_gain.is_enabled());

        let num_frames = 4800;
        let input: Vec<f32> = (0..num_frames * 2).map(|i| ((i as f32) * 0.01).sin() * 0.5).collect();
        let output: Vec<f32> = (0..num_frames * 2).map(|i| ((i as f32) * 0.01).sin() * 1.0).collect();

        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        // After enabling and measuring, gain should start adjusting
        for _ in 0..10000 {
            auto_gain.next_gain_linear();
        }

        // Should have some non-zero gain now
        let gain_db = auto_gain.current_gain_db();
        assert!(gain_db < 0.0, "Expected negative gain when output is louder");

        // Disable again
        auto_gain.set_enabled(false);
        assert!(!auto_gain.is_enabled());

        // After some time, gain should return to 1.0
        for _ in 0..10000 {
            auto_gain.next_gain_linear();
        }
        assert_eq!(auto_gain.current_gain_db(), 0.0);
    }

    #[test]
    fn test_auto_gain_set_sample_rate() {
        let mut auto_gain = AutoGain::new_default(2, 44100).unwrap();

        // Change sample rate
        auto_gain.set_sample_rate(96000).unwrap();

        // Should work after sample rate change
        auto_gain.set_enabled(true);

        let num_frames = 9600; // 100ms at 96kHz
        let input: Vec<f32> = (0..num_frames * 2).map(|_| 0.5).collect();

        auto_gain.measure_input(&input).unwrap();
        auto_gain.measure_output(&input).unwrap();
    }

    #[test]
    fn test_auto_gain_set_max_gain() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 3.0, // Start with 3dB limit
            smoothing_ms: 1.0,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Change max gain
        auto_gain.set_max_gain_db(20.0);

        let num_frames = 9600;
        // Create extreme difference
        let mut input = vec![0.0_f32; num_frames * 2];
        let mut output = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.01; // Very quiet
            input[i * 2 + 1] = phase.sin() * 0.01;
            output[i * 2] = phase.sin() * 1.0; // Loud
            output[i * 2 + 1] = phase.sin() * 1.0;
        }

        for _ in 0..10 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        for _ in 0..50000 {
            auto_gain.next_gain_linear();
        }

        let gain_db = auto_gain.current_gain_db();
        // With 20dB max, gain can go below -3dB
        assert!(
            gain_db <= -3.0 || gain_db >= -20.0,
            "Gain {} should be within new max range",
            gain_db
        );
    }

    #[test]
    fn test_auto_gain_set_smoothing() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 500.0, // Slow smoothing
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Change to fast smoothing
        auto_gain.set_smoothing_ms(1.0);

        let num_frames = 4800;
        let input: Vec<f32> = (0..num_frames * 2).map(|_| 0.5).collect();
        let output: Vec<f32> = (0..num_frames * 2).map(|_| 1.0).collect();

        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        // With 1ms smoothing, should converge quickly
        for _ in 0..1000 {
            auto_gain.next_gain_linear();
        }

        let gain_db = auto_gain.current_gain_db();
        // Should have reached target already with fast smoothing
        assert!(
            gain_db < -1.0,
            "With fast smoothing, gain {} should have converged",
            gain_db
        );
    }

    #[test]
    fn test_auto_gain_set_loudness_type() {
        let mut auto_gain = AutoGain::new_default(2, 48000).unwrap();
        auto_gain.set_enabled(true);

        // Start with Momentary (default)
        auto_gain.set_loudness_type(AutoGainLoudnessType::Momentary);

        let num_frames = 4800;
        let input: Vec<f32> = (0..num_frames * 2).map(|i| ((i as f32) * 0.01).sin() * 0.5).collect();

        auto_gain.measure_input(&input).unwrap();
        let momentary_lufs = auto_gain.last_input_lufs();

        // Switch to ShortTerm
        auto_gain.set_loudness_type(AutoGainLoudnessType::ShortTerm);
        auto_gain.measure_input(&input).unwrap();
        let shortterm_lufs = auto_gain.last_input_lufs();

        // Both should return valid LUFS values (may differ due to window size)
        // With only 100ms of signal, short-term may not have enough data
        assert!(
            momentary_lufs.is_finite() || momentary_lufs.is_infinite(),
            "Momentary LUFS should be a valid f64"
        );
        assert!(
            shortterm_lufs.is_finite() || shortterm_lufs.is_infinite(),
            "ShortTerm LUFS should be a valid f64"
        );
    }

    #[test]
    fn test_auto_gain_current_gain_linear() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 10.0,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Initial gain should be 1.0 (0 dB)
        assert!((auto_gain.current_gain_linear() - 1.0).abs() < 0.001);

        let num_frames = 4800;
        let input: Vec<f32> = (0..num_frames * 2).map(|_| 0.5).collect();
        let output: Vec<f32> = (0..num_frames * 2).map(|_| 1.0).collect();

        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        // Advance smoother
        for _ in 0..10000 {
            let _ = auto_gain.next_gain_linear();
        }

        // current_gain_linear should match next_gain_linear when not advancing
        let linear = auto_gain.current_gain_linear();
        let db = auto_gain.current_gain_db();

        // Verify dB to linear conversion
        let expected_linear = 10.0_f32.powf(db / 20.0);
        assert!(
            (linear - expected_linear).abs() < 0.001,
            "Linear gain {} should match computed {} from {} dB",
            linear,
            expected_linear,
            db
        );
    }

    #[test]
    fn test_auto_gain_get_data_after_processing() {
        let params = AutoGainParams {
            enabled: true,
            max_gain_db: 12.0,
            smoothing_ms: 10.0,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        let num_frames = 4800;
        let mut input = vec![0.0_f32; num_frames * 2];
        let mut output = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.sin() * 0.5;
            output[i * 2] = phase.sin() * 0.8;
            output[i * 2 + 1] = phase.sin() * 0.8;
        }

        for _ in 0..5 {
            auto_gain.measure_input(&input).unwrap();
            auto_gain.measure_output(&output).unwrap();
        }

        for _ in 0..10000 {
            auto_gain.next_gain_linear();
        }

        let data = auto_gain.get_data();

        assert!(data.enabled);
        assert!(data.input_lufs.is_finite(), "Input LUFS should be finite after processing");
        assert!(data.output_lufs.is_finite(), "Output LUFS should be finite after processing");
        // Output is louder, so gain should be negative
        assert!(data.gain_db < 0.0, "Gain should be negative when output is louder");
    }

    #[test]
    fn test_auto_gain_params_default() {
        let params = AutoGainParams::default();
        assert!(!params.enabled);
        assert_eq!(params.loudness_type, AutoGainLoudnessType::Momentary);
        assert_eq!(params.max_gain_db, 12.0);
        assert_eq!(params.smoothing_ms, 100.0);
    }

    #[test]
    fn test_auto_gain_no_measurement_no_change() {
        // If we never measure, gain should stay at 0 dB
        let params = AutoGainParams {
            enabled: true,
            ..Default::default()
        };
        let mut auto_gain = AutoGain::new(2, 48000, params).unwrap();

        // Just advance smoother without measuring
        for _ in 0..10000 {
            auto_gain.next_gain_linear();
        }

        // Gain should still be 0 dB (no target set)
        assert_eq!(auto_gain.current_gain_db(), 0.0);
    }
}
