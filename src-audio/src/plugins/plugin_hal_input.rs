// ============================================================================
// HAL Input Plugin - Reads audio from macOS HAL driver
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use sotf_hal::HalInputReader;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration parameters for HalInputPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalInputPluginParams {
    /// Number of output channels (default: 2 for stereo)
    #[serde(default = "default_channels")]
    pub channels: usize,
}

fn default_channels() -> usize {
    2
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// HAL Input Plugin - Source plugin that reads audio from macOS apps via HAL driver
///
/// This is a source plugin (0 input channels → N output channels) that reads
/// audio from macOS applications through the HAL virtual audio device.
///
/// # Platform Support
/// - **macOS**: Fully supported via sotf_hal
/// - **Other platforms**: Stub implementation (always outputs silence)
///
/// # Example
/// ```json
/// {
///   "plugin_type": "hal_input",
///   "parameters": {
///     "channels": 2
///   }
/// }
/// ```
pub struct HalInputPlugin {
    /// Number of output channels
    channels: usize,

    #[cfg(target_os = "macos")]
    /// HAL input reader
    reader: Option<HalInputReader>,
}

impl HalInputPlugin {
    /// Create a new HAL input plugin
    ///
    /// # Arguments
    /// * `channels` - Number of output channels
    ///
    /// # Returns
    /// - `Ok(plugin)` if successful
    /// - `Err(msg)` if channels are invalid or HAL is not initialized
    pub fn new(channels: usize) -> Result<Self, String> {
        // Validate channels
        if channels == 0 || channels > 16 {
            return Err(format!(
                "Invalid channel count: {}. Must be between 1 and 16",
                channels
            ));
        }

        #[cfg(target_os = "macos")]
        {
            let reader = HalInputReader::new();

            if reader.is_none() {
                return Err(
                    "HAL driver not initialized. Ensure daemon initialized HAL before creating plugins".to_string()
                );
            }

            Ok(Self { channels, reader })
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err("HAL input plugin is only supported on macOS".to_string())
        }
    }

    /// Create from configuration parameters
    ///
    /// # Returns
    /// - `Ok(plugin)` if successful
    /// - `Err(msg)` if validation fails or HAL is not initialized
    pub fn from_params(params: HalInputPluginParams) -> Result<Self, String> {
        Self::new(params.channels)
    }
}

impl Plugin for HalInputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "HAL Input".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Reads audio from macOS apps via HAL driver".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        0 // Source plugin - no input
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_int("channels", "Output Channels", 2, 1, 16)
                .with_description("Number of output channels"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let param_channels = ParameterId::from("channels");

        if id == param_channels {
            if let Some(channels) = value.as_int() {
                if channels < 1 || channels > 16 {
                    return Err("Channels must be between 1 and 16".to_string());
                }
                self.channels = channels as usize;
                Ok(())
            } else {
                Err("Channels parameter must be an integer".to_string())
            }
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let param_channels = ParameterId::from("channels");

        if id == &param_channels {
            Some(ParameterValue::Int(self.channels as i32))
        } else {
            None
        }
    }

    fn process(
        &mut self,
        _input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify output buffer size
        let expected_len = context.num_frames * self.channels;
        if output.len() != expected_len {
            return Err(format!(
                "Output buffer size mismatch: expected {}, got {}",
                expected_len,
                output.len()
            ));
        }

        #[cfg(target_os = "macos")]
        {
            // Try to read from HAL
            if let Some(ref mut reader) = self.reader {
                let samples_read = reader.read(output);

                // Zero-fill any remaining samples if we didn't read enough
                if samples_read < output.len() {
                    log::trace!(
                        "HAL input underrun: read {}/{} samples, zero-filling remainder",
                        samples_read,
                        output.len()
                    );
                    output[samples_read..].fill(0.0);
                }
            } else {
                // Should never happen since new() checks for reader
                return Err("HAL reader not available".to_string());
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Should never happen since new() fails on non-macOS
            return Err("HAL input plugin is only supported on macOS".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hal_input_plugin_validation() {
        // Invalid channel counts should fail
        assert!(HalInputPlugin::new(0).is_err());
        assert!(HalInputPlugin::new(17).is_err());

        // Valid channel counts (will still fail without HAL initialized)
        #[cfg(target_os = "macos")]
        {
            // This will fail because HAL isn't initialized in tests
            assert!(HalInputPlugin::new(2).is_err());
            assert!(HalInputPlugin::new(5).is_err());
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Should fail on non-macOS platforms
            assert!(HalInputPlugin::new(2).is_err());
        }
    }

    #[test]
    fn test_hal_input_plugin_from_params() {
        let params = HalInputPluginParams { channels: 2 };
        // Will fail without HAL initialized
        let result = HalInputPlugin::from_params(params);

        #[cfg(target_os = "macos")]
        assert!(result.is_err());

        #[cfg(not(target_os = "macos"))]
        assert!(result.is_err());
    }

    #[test]
    fn test_hal_input_plugin_invalid_params() {
        let params = HalInputPluginParams { channels: 0 };
        assert!(HalInputPlugin::from_params(params).is_err());

        let params = HalInputPluginParams { channels: 20 };
        assert!(HalInputPlugin::from_params(params).is_err());
    }
}
