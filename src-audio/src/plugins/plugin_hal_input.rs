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

    /// Buffer for zero-filling when no data available
    zero_buffer: Vec<f32>,
}

impl HalInputPlugin {
    /// Create a new HAL input plugin
    ///
    /// # Arguments
    /// * `channels` - Number of output channels
    pub fn new(channels: usize) -> Self {
        #[cfg(target_os = "macos")]
        let reader = HalInputReader::new();

        #[cfg(target_os = "macos")]
        if reader.is_none() {
            log::warn!("HAL driver not initialized - plugin will output silence");
        }

        Self {
            channels,
            #[cfg(target_os = "macos")]
            reader,
            zero_buffer: vec![0.0; 8192], // Pre-allocate buffer for zero-filling
        }
    }

    /// Create from configuration parameters
    pub fn from_params(params: HalInputPluginParams) -> Self {
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
                    output[samples_read..].fill(0.0);
                }
            } else {
                // No HAL driver available - output silence
                output.fill(0.0);
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Not on macOS - output silence
            output.fill(0.0);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hal_input_plugin_creation() {
        let plugin = HalInputPlugin::new(2);
        assert_eq!(plugin.input_channels(), 0);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_hal_input_plugin_from_params() {
        let params = HalInputPluginParams { channels: 5 };
        let plugin = HalInputPlugin::from_params(params);
        assert_eq!(plugin.output_channels(), 5);
    }

    #[test]
    fn test_hal_input_plugin_process() {
        let mut plugin = HalInputPlugin::new(2);
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 512,
        };

        let input = vec![];
        let mut output = vec![0.0; 512 * 2];

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());

        // Should output silence when HAL not available
        // (or actual data if HAL is initialized)
    }

    #[test]
    fn test_hal_input_plugin_parameters() {
        let mut plugin = HalInputPlugin::new(2);

        // Test setting channels
        let result = plugin.set_parameter(
            ParameterId::from("channels"),
            ParameterValue::Int(5),
        );
        assert!(result.is_ok());
        assert_eq!(plugin.output_channels(), 5);

        // Test getting channels
        let value = plugin.get_parameter(&ParameterId::from("channels"));
        assert_eq!(value, Some(ParameterValue::Int(5)));
    }
}
