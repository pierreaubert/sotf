// ============================================================================
// Delay Plugin
// ============================================================================
//
// Simple delay effect with configurable delay time and feedback.
//
// Parameters:
// - delay_ms: Delay time in milliseconds
// - feedback: Feedback amount (0.0 to 1.0)
// - mix: Dry/wet mix (0.0 = all dry, 1.0 = all wet)

use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

fn default_delay_ms() -> f32 {
    100.0
}

fn default_feedback() -> f32 {
    0.3
}

fn default_mix() -> f32 {
    0.5
}

/// Configuration parameters for DelayPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayPluginParams {
    #[serde(default = "default_delay_ms")]
    pub delay_ms: f32,
    #[serde(default = "default_feedback")]
    pub feedback: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Simple delay effect with feedback
///
/// This plugin implements a circular buffer delay line with feedback.
/// Each channel has its own independent delay buffer.
///
/// # Example
/// ```
/// use sotf_plugins::DelayPlugin;
///
/// let mut delay = DelayPlugin::new(2, 250.0, 0.4, 0.5); // 250ms delay, 40% feedback, 50% mix
/// ```
pub struct DelayPlugin {
    /// Number of channels
    channels: usize,
    /// Sample rate
    sample_rate: u32,

    // Parameters
    param_delay_ms: ParameterId,
    delay_ms: f32,

    param_feedback: ParameterId,
    feedback: f32,

    param_mix: ParameterId,
    mix: f32,

    // Smoothed parameters to prevent clicks during parameter changes
    feedback_smoother: Smoother,
    mix_smoother: Smoother,

    // Delay buffers (one per channel)
    delay_buffers: Vec<Vec<f32>>,
    // Write positions in the circular buffers
    write_positions: Vec<usize>,
    // Current delay length in samples
    delay_samples: usize,
}

impl DelayPlugin {
    /// Create a new delay plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `delay_ms` - Delay time in milliseconds (default: 100.0)
    /// * `feedback` - Feedback amount 0.0-1.0 (default: 0.3)
    /// * `mix` - Dry/wet mix 0.0-1.0 (default: 0.5)
    pub fn new(channels: usize, delay_ms: f32, feedback: f32, mix: f32) -> Self {
        let sample_rate = 44100; // Updated in initialize()
        let delay_samples = Self::ms_to_samples(delay_ms, sample_rate);

        // Create delay buffers with enough space for max delay time (5 seconds)
        let max_delay_samples = Self::ms_to_samples(5000.0, sample_rate);
        let delay_buffers = vec![vec![0.0; max_delay_samples]; channels];
        let write_positions = vec![0; channels];

        Self {
            channels,
            sample_rate,

            param_delay_ms: ParameterId::from("delay_ms"),
            delay_ms,

            param_feedback: ParameterId::from("feedback"),
            feedback: feedback.clamp(0.0, 0.95),

            param_mix: ParameterId::from("mix"),
            mix: mix.clamp(0.0, 1.0),

            // Smoothed parameters (5ms time constant for fast but click-free changes)
            feedback_smoother: Smoother::new(feedback.clamp(0.0, 0.95), 5.0, sample_rate),
            mix_smoother: Smoother::new(mix.clamp(0.0, 1.0), 5.0, sample_rate),

            delay_buffers,
            write_positions,
            delay_samples,
        }
    }

    /// Create a new delay plugin from configuration parameters
    pub fn from_params(channels: usize, params: DelayPluginParams) -> Self {
        Self::new(channels, params.delay_ms, params.feedback, params.mix)
    }

    /// Convert milliseconds to samples
    fn ms_to_samples(ms: f32, sample_rate: u32) -> usize {
        ((ms * sample_rate as f32) / 1000.0).max(1.0) as usize
    }

    /// Update delay length in samples based on current delay_ms
    fn update_delay_length(&mut self) {
        self.delay_samples = Self::ms_to_samples(self.delay_ms, self.sample_rate);

        // Clamp to buffer size
        let max_delay_samples = self.delay_buffers[0].len();
        self.delay_samples = self.delay_samples.min(max_delay_samples);
    }

    /// Set delay time in milliseconds
    pub fn set_delay_ms(&mut self, delay_ms: f32) {
        self.delay_ms = delay_ms.clamp(0.1, 5000.0);
        self.update_delay_length();
    }

    /// Set feedback amount
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.95);
    }

    /// Set dry/wet mix
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Get current delay time in milliseconds
    pub fn delay_ms(&self) -> f32 {
        self.delay_ms
    }

    /// Get current feedback amount
    pub fn feedback(&self) -> f32 {
        self.feedback
    }

    /// Get current mix
    pub fn mix(&self) -> f32 {
        self.mix
    }
}

impl InPlacePlugin for DelayPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Delay", "1.0.0", "SotF")
            .with_description("Simple delay effect with feedback and mix control")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("delay_ms", "Delay Time", 100.0, 0.1, 5000.0)
                .with_description("Delay time in milliseconds")
                .with_group("Time")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("feedback", "Feedback", 0.3, 0.0, 0.95)
                .with_description("Feedback amount (0.0 = no feedback, 0.95 = maximum)")
                .with_group("Feedback")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("mix", "Mix", 0.5, 0.0, 1.0)
                .with_description("Dry/wet mix (0.0 = all dry, 1.0 = all wet)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_delay_ms {
            if let Some(delay_ms) = value.as_float() {
                self.set_delay_ms(delay_ms);
                Ok(())
            } else {
                Err("Delay time parameter must be a float".to_string())
            }
        } else if id == self.param_feedback {
            if let Some(feedback) = value.as_float() {
                self.feedback = feedback.clamp(0.0, 0.95);
                self.feedback_smoother.set_target(self.feedback);
                Ok(())
            } else {
                Err("Feedback parameter must be a float".to_string())
            }
        } else if id == self.param_mix {
            if let Some(mix) = value.as_float() {
                self.mix = mix.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
                Ok(())
            } else {
                Err("Mix parameter must be a float".to_string())
            }
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_delay_ms {
            Some(ParameterValue::Float(self.delay_ms))
        } else if id == &self.param_feedback {
            Some(ParameterValue::Float(self.feedback))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_delay_length();

        // Update smoother times for the new sample rate
        self.feedback_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);

        // Resize buffers if needed for the new sample rate
        let max_delay_samples = Self::ms_to_samples(5000.0, sample_rate);
        for buffer in &mut self.delay_buffers {
            buffer.resize(max_delay_samples, 0.0);
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Clear all delay buffers
        for buffer in &mut self.delay_buffers {
            buffer.fill(0.0);
        }
        self.write_positions.fill(0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Verify buffer size matches channel count
        if !buffer.len().is_multiple_of(self.channels) {
            return Err(format!(
                "Buffer size {} is not a multiple of channel count {}",
                buffer.len(),
                self.channels
            ));
        }

        let num_frames = context.num_frames;

        // Process each frame
        for frame in 0..num_frames {
            // Update smoothers per sample for smooth transitions
            let _ = self.feedback_smoother.next();
            let _ = self.mix_smoother.next();
            let current_feedback = self.feedback_smoother.current();
            let current_mix = self.mix_smoother.current();
            let current_dry_mix = 1.0 - current_mix;

            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;
                let input_sample = buffer[sample_idx];

                // Calculate read position (where we read the delayed signal from)
                let read_pos = if self.write_positions[ch] >= self.delay_samples {
                    self.write_positions[ch] - self.delay_samples
                } else {
                    self.delay_buffers[ch].len() - (self.delay_samples - self.write_positions[ch])
                };

                // Read delayed sample
                let delayed_sample = self.delay_buffers[ch][read_pos];

                // Write to delay buffer (input + feedback from delayed signal)
                let feedback_sample = input_sample + delayed_sample * current_feedback;
                self.delay_buffers[ch][self.write_positions[ch]] = feedback_sample;

                // Flush denormals to prevent CPU performance spikes and audio crackle
                // Feedback loops can accumulate denormal numbers from floating-point precision errors
                if feedback_sample.abs() < 1e-30 && feedback_sample != 0.0 {
                    self.delay_buffers[ch][self.write_positions[ch]] = 0.0;
                }

                // Advance write position (circular buffer)
                self.write_positions[ch] =
                    (self.write_positions[ch] + 1) % self.delay_buffers[ch].len();

                // Mix dry and wet signals using smoothed values
                buffer[sample_idx] = input_sample * current_dry_mix + delayed_sample * current_mix;
            }
        }

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0 // The delay is intentional, not latency
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_creation() {
        let delay = DelayPlugin::new(2, 100.0, 0.3, 0.5);
        assert_eq!(delay.channels(), 2);
        assert_eq!(delay.delay_ms(), 100.0);
        assert_eq!(delay.feedback(), 0.3);
        assert_eq!(delay.mix(), 0.5);
    }

    #[test]
    fn test_ms_to_samples() {
        let samples = DelayPlugin::ms_to_samples(100.0, 44100);
        assert_eq!(samples, 4410);

        let samples = DelayPlugin::ms_to_samples(1000.0, 48000);
        assert_eq!(samples, 48000);
    }

    #[test]
    fn test_parameter_clamping() {
        let mut delay = DelayPlugin::new(2, 100.0, 0.3, 0.5);

        // Feedback should be clamped to 0.95 max
        delay.set_feedback(1.5);
        assert_eq!(delay.feedback(), 0.95);

        delay.set_feedback(-0.1);
        assert_eq!(delay.feedback(), 0.0);

        // Mix should be clamped to 0.0-1.0
        delay.set_mix(1.5);
        assert_eq!(delay.mix(), 1.0);

        delay.set_mix(-0.1);
        assert_eq!(delay.mix(), 0.0);
    }

    #[test]
    fn test_no_delay_dry_signal() {
        let mut plugin = DelayPlugin::new(2, 100.0, 0.0, 0.0); // No feedback, all dry
        plugin.initialize(44100).unwrap();

        let mut buffer = vec![1.0, 1.0, 0.5, 0.5]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // With mix=0.0 (all dry), output should equal input
        assert_eq!(buffer, vec![1.0, 1.0, 0.5, 0.5]);
    }

    #[test]
    fn test_parameter_change() {
        let mut plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);

        // Set via parameter system
        plugin
            .set_parameter(ParameterId::from("delay_ms"), ParameterValue::Float(250.0))
            .unwrap();
        assert_eq!(plugin.delay_ms(), 250.0);

        plugin
            .set_parameter(ParameterId::from("feedback"), ParameterValue::Float(0.6))
            .unwrap();
        assert_eq!(plugin.feedback(), 0.6);

        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.8))
            .unwrap();
        assert_eq!(plugin.mix(), 0.8);
    }

    #[test]
    fn test_from_params() {
        let params = DelayPluginParams {
            delay_ms: 200.0,
            feedback: 0.4,
            mix: 0.6,
        };

        let plugin = DelayPlugin::from_params(2, params);
        assert_eq!(plugin.delay_ms(), 200.0);
        assert_eq!(plugin.feedback(), 0.4);
        assert_eq!(plugin.mix(), 0.6);
    }

    #[test]
    fn test_delay_various_sample_rates() {
        for &sample_rate in &[22050, 44100, 48000, 96000, 192000] {
            let mut plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);
            plugin.initialize(sample_rate).unwrap();

            let expected_samples = DelayPlugin::ms_to_samples(100.0, sample_rate);
            assert_eq!(plugin.delay_samples, expected_samples);

            let num_frames = 256;
            let mut buffer = vec![0.0_f32; num_frames * 2];
            buffer[0] = 1.0;
            buffer[1] = 1.0;

            let context = ProcessContext {
                sample_rate,
                num_frames,
            };

            plugin.process_in_place(&mut buffer, &context).unwrap();
        }
    }

    #[test]
    fn test_delay_wet_signal_appears_after_delay() {
        let mut plugin = DelayPlugin::new(1, 10.0, 0.0, 1.0); // 10ms, no feedback, all wet
        plugin.initialize(48000).unwrap();

        let delay_samples = DelayPlugin::ms_to_samples(10.0, 48000); // 480 samples
        let num_frames = 1024;
        let mut buffer = vec![0.0_f32; num_frames];
        buffer[0] = 1.0; // Impulse at sample 0

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // With 100% wet, the impulse should appear at the delay offset
        assert!(
            buffer[delay_samples].abs() > 0.5,
            "Delayed impulse expected at sample {}, got {}",
            delay_samples,
            buffer[delay_samples]
        );
        // Sample 0 should be zero (wet only, no dry signal and delay buffer was empty)
        assert!(
            buffer[0].abs() < 0.01,
            "No signal expected at sample 0 with full wet, got {}",
            buffer[0]
        );
    }

    #[test]
    fn test_delay_feedback_produces_echoes() {
        let mut plugin = DelayPlugin::new(1, 10.0, 0.5, 1.0); // 10ms, 50% feedback, all wet
        plugin.initialize(48000).unwrap();

        let delay_samples = DelayPlugin::ms_to_samples(10.0, 48000);
        let num_frames = delay_samples * 4; // Enough for several echoes
        let mut buffer = vec![0.0_f32; num_frames];
        buffer[0] = 1.0; // Impulse

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // First echo
        let first_echo = buffer[delay_samples].abs();
        assert!(first_echo > 0.3, "First echo should be present");

        // Second echo should be quieter due to feedback decay
        if delay_samples * 2 < num_frames {
            let second_echo = buffer[delay_samples * 2].abs();
            assert!(
                second_echo < first_echo,
                "Second echo should be quieter than first"
            );
        }
    }

    #[test]
    fn test_delay_reset_clears_buffers() {
        let mut plugin = DelayPlugin::new(1, 10.0, 0.5, 1.0);
        plugin.initialize(48000).unwrap();

        // Fill with data
        let num_frames = 256;
        let mut buffer = vec![1.0_f32; num_frames];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Reset
        plugin.reset();

        // After reset, processing silence should produce silence
        let mut buffer2 = vec![0.0_f32; num_frames];
        plugin.process_in_place(&mut buffer2, &context).unwrap();

        let energy: f32 = buffer2.iter().map(|s| s * s).sum();
        assert!(
            energy < 0.001,
            "After reset, processing silence should produce silence (energy: {})",
            energy
        );
    }

    #[test]
    fn test_delay_reinitialize_different_rate() {
        let mut plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);
        plugin.initialize(44100).unwrap();
        let delay_44k = plugin.delay_samples;

        plugin.initialize(96000).unwrap();
        let delay_96k = plugin.delay_samples;

        // 100ms at 96kHz should produce more samples than at 44.1kHz
        assert!(delay_96k > delay_44k);
    }
}
