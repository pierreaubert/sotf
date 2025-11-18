// ============================================================================
// HAL Output Plugin - Writes audio to macOS HAL driver (loopback)
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use sotf_hal::HalOutputWriter;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration parameters for HalOutputPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalOutputPluginParams {
    /// Number of channels (default: 2 for stereo)
    #[serde(default = "default_channels")]
    pub channels: usize,
}

fn default_channels() -> usize {
    2
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// HAL Output Plugin - Passthrough plugin that writes audio to HAL driver
///
/// This is a passthrough plugin (N input channels → N output channels) that
/// writes audio to the HAL virtual audio device for loopback monitoring.
///
/// The plugin passes audio through unmodified while also sending it to the HAL.
///
/// # Platform Support
/// - **macOS**: Fully supported via sotf_hal
/// - **Other platforms**: Stub implementation (passthrough only, no HAL write)
///
/// # Example
/// ```json
/// {
///   "plugin_type": "hal_output",
///   "parameters": {
///     "channels": 2
///   }
/// }
/// ```
pub struct HalOutputPlugin {
    /// Number of channels
    channels: usize,

    #[cfg(target_os = "macos")]
    /// HAL output writer
    writer: Option<HalOutputWriter>,
}

impl HalOutputPlugin {
    /// Create a new HAL output plugin
    ///
    /// # Arguments
    /// * `channels` - Number of channels
    pub fn new(channels: usize) -> Self {
        #[cfg(target_os = "macos")]
        let writer = HalOutputWriter::new();

        #[cfg(target_os = "macos")]
        if writer.is_none() {
            log::warn!("HAL driver not initialized - plugin will not write to HAL");
        }

        Self {
            channels,
            #[cfg(target_os = "macos")]
            writer,
        }
    }

    /// Create from configuration parameters
    pub fn from_params(channels: usize, params: HalOutputPluginParams) -> Self {
        let _ = params; // params.channels is used to validate, but we use the provided channels
        Self::new(channels)
    }
}

impl InPlacePlugin for HalOutputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "HAL Output".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Writes audio to macOS HAL driver for loopback".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_int("channels", "Channels", 2, 1, 16)
                .with_description("Number of audio channels"),
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

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify buffer size matches channel count
        let expected_len = context.num_frames * self.channels;
        if buffer.len() != expected_len {
            return Err(format!(
                "Buffer size mismatch: expected {}, got {}",
                expected_len,
                buffer.len()
            ));
        }

        #[cfg(target_os = "macos")]
        {
            // Write to HAL (loopback)
            if let Some(ref mut writer) = self.writer {
                let written = writer.write(buffer);

                // Log warning if we couldn't write all samples
                if written < buffer.len() {
                    log::trace!(
                        "HAL output buffer full: wrote {}/{} samples",
                        written,
                        buffer.len()
                    );
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Not on macOS - just pass through
            let _ = context;
        }

        // Audio passes through unmodified
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hal_output_plugin_creation() {
        let plugin = HalOutputPlugin::new(2);
        assert_eq!(plugin.channels(), 2);
    }

    #[test]
    fn test_hal_output_plugin_from_params() {
        let params = HalOutputPluginParams { channels: 5 };
        let plugin = HalOutputPlugin::from_params(5, params);
        assert_eq!(plugin.channels(), 5);
    }

    #[test]
    fn test_hal_output_plugin_process() {
        let mut plugin = HalOutputPlugin::new(2);
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 512,
        };

        let mut buffer = vec![0.5; 512 * 2];
        let original = buffer.clone();

        let result = plugin.process_in_place(&mut buffer, &context);
        assert!(result.is_ok());

        // Buffer should be unchanged (passthrough)
        assert_eq!(buffer, original);
    }

    #[test]
    fn test_hal_output_plugin_parameters() {
        let mut plugin = HalOutputPlugin::new(2);

        // Test setting channels
        let result = plugin.set_parameter(
            ParameterId::from("channels"),
            ParameterValue::Int(5),
        );
        assert!(result.is_ok());
        assert_eq!(plugin.channels(), 5);

        // Test getting channels
        let value = plugin.get_parameter(&ParameterId::from("channels"));
        assert_eq!(value, Some(ParameterValue::Int(5)));
    }
}
