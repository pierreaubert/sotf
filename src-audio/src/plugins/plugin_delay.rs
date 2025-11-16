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

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
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
/// use sotf_audio::DelayPlugin;
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
        PluginInfo {
            name: "Delay".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Simple delay effect with feedback and mix control".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("delay_ms", "Delay Time", 100.0, 0.1, 5000.0)
                .with_description("Delay time in milliseconds"),
            Parameter::new_float("feedback", "Feedback", 0.3, 0.0, 0.95)
                .with_description("Feedback amount (0.0 = no feedback, 0.95 = maximum)"),
            Parameter::new_float("mix", "Mix", 0.5, 0.0, 1.0)
                .with_description("Dry/wet mix (0.0 = all dry, 1.0 = all wet)"),
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
                self.set_feedback(feedback);
                Ok(())
            } else {
                Err("Feedback parameter must be a float".to_string())
            }
        } else if id == self.param_mix {
            if let Some(mix) = value.as_float() {
                self.set_mix(mix);
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
    ) -> PluginResult<()> {
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
                self.delay_buffers[ch][self.write_positions[ch]] =
                    input_sample + delayed_sample * self.feedback;

                // Advance write position (circular buffer)
                self.write_positions[ch] = (self.write_positions[ch] + 1) % self.delay_buffers[ch].len();

                // Mix dry and wet signals
                buffer[sample_idx] = input_sample * (1.0 - self.mix) + delayed_sample * self.mix;
            }
        }

        Ok(())
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
            .set_parameter(
                ParameterId::from("delay_ms"),
                ParameterValue::Float(250.0),
            )
            .unwrap();
        assert_eq!(plugin.delay_ms(), 250.0);

        plugin
            .set_parameter(
                ParameterId::from("feedback"),
                ParameterValue::Float(0.6),
            )
            .unwrap();
        assert_eq!(plugin.feedback(), 0.6);

        plugin
            .set_parameter(
                ParameterId::from("mix"),
                ParameterValue::Float(0.8),
            )
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
}
