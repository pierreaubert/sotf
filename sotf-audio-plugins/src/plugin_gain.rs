// ============================================================================
// Gain Plugin - Simple example plugin for testing
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

fn default_gain_db() -> f32 {
    0.0
}

/// Configuration parameters for GainPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GainPluginParams {
    #[serde(default = "default_gain_db")]
    pub gain_db: f32,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Simple gain plugin that multiplies all samples by a gain factor
///
/// The gain can be specified in dB or as a linear multiplier.
///
/// # Example
/// ```
/// use sotf_plugins::GainPlugin;
///
/// let mut gain = GainPlugin::new(2, -6.0); // -6dB gain on 2 channels
/// gain.set_gain_db(0.0); // Change to unity gain
/// ```
pub struct GainPlugin {
    /// Number of channels
    channels: usize,
    /// Current gain in dB
    gain_db: f32,
    /// Linear gain multiplier (cached from gain_db)
    gain_linear: f32,
    /// Parameter ID for gain
    param_gain_db: ParameterId,
}

impl GainPlugin {
    /// Create a new gain plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `gain_db` - Initial gain in dB (0.0 = unity, negative = attenuation, positive = boost)
    pub fn new(channels: usize, gain_db: f32) -> Self {
        let gain_linear = Self::db_to_linear(gain_db);
        Self {
            channels,
            gain_db,
            gain_linear,
            param_gain_db: ParameterId::from("gain_db"),
        }
    }

    /// Create a new gain plugin from configuration parameters
    pub fn from_params(channels: usize, params: GainPluginParams) -> Self {
        Self::new(channels, params.gain_db)
    }

    /// Set gain in dB
    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.gain_db = gain_db;
        self.gain_linear = Self::db_to_linear(gain_db);
    }

    /// Set gain as linear multiplier
    pub fn set_gain_linear(&mut self, gain: f32) {
        self.gain_linear = gain;
        self.gain_db = Self::linear_to_db(gain);
    }

    /// Get current gain in dB
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Get current gain as linear multiplier
    pub fn gain_linear(&self) -> f32 {
        self.gain_linear
    }

    /// Convert dB to linear gain
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    /// Convert linear gain to dB
    fn linear_to_db(linear: f32) -> f32 {
        20.0 * linear.log10()
    }
}

impl InPlacePlugin for GainPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Gain".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Simple gain/volume control plugin".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("gain_db", "Gain (dB)", 0.0, -60.0, 20.0).with_description(
                "Gain in decibels. 0dB = unity gain, negative = attenuation, positive = boost",
            ),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_gain_db {
            if let Some(gain_db) = value.as_float() {
                self.set_gain_db(gain_db);
                Ok(())
            } else {
                Err("Gain parameter must be a float".to_string())
            }
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_gain_db {
            Some(ParameterValue::Float(self.gain_db))
        } else {
            None
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        _context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify buffer size matches channel count
        if !buffer.len().is_multiple_of(self.channels) {
            return Err(format!(
                "Buffer size {} is not a multiple of channel count {}",
                buffer.len(),
                self.channels
            ));
        }

        // Apply gain to all samples
        for sample in buffer.iter_mut() {
            *sample *= self.gain_linear;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_conversion() {
        assert!((GainPlugin::db_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((GainPlugin::db_to_linear(-6.0) - 0.501).abs() < 0.01);
        assert!((GainPlugin::db_to_linear(-12.0) - 0.251).abs() < 0.01);
        assert!((GainPlugin::db_to_linear(6.0) - 1.995).abs() < 0.01);
    }

    #[test]
    fn test_unity_gain() {
        let mut plugin = GainPlugin::new(2, 0.0);
        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Should be unchanged at 0dB
        assert_eq!(buffer, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_attenuation() {
        let mut plugin = GainPlugin::new(2, -6.0);
        let mut buffer = vec![1.0, 2.0, 1.0, 2.0]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // -6dB ≈ 0.5x
        for &sample in &buffer {
            assert!((sample - 0.5).abs() < 0.01 || (sample - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_parameter_change() {
        let mut plugin = GainPlugin::new(2, 0.0);

        // Set via parameter system
        plugin
            .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-12.0))
            .unwrap();

        assert_eq!(plugin.gain_db(), -12.0);
        assert!((plugin.gain_linear() - 0.251).abs() < 0.01);

        // Get via parameter system
        let value = plugin.get_parameter(&ParameterId::from("gain_db"));
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_float(), Some(-12.0));
    }
}
