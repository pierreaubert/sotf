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
            let writer = HalOutputWriter::new();

            if writer.is_none() {
                return Err(
                    "HAL driver not initialized. Ensure daemon initialized HAL before creating plugins".to_string()
                );
            }

            Ok(Self { channels, writer })
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err("HAL output plugin is only supported on macOS".to_string())
        }
    }

    /// Create from configuration parameters
    ///
    /// # Arguments
    /// * `channels` - Number of channels (from plugin chain, may differ from params)
    /// * `params` - Plugin parameters (currently unused, kept for API consistency)
    ///
    /// # Returns
    /// - `Ok(plugin)` if successful
    /// - `Err(msg)` if validation fails or HAL is not initialized
    pub fn from_params(channels: usize, _params: HalOutputPluginParams) -> Result<Self, String> {
        Self::new(channels)
    }
}

impl InPlacePlugin for HalOutputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("HAL Output", "1.0.0", "SotF")
            .with_description("Writes audio to macOS HAL driver for loopback")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_int("channels", "Channels", 2, 1, 16)
                .with_description("Number of audio channels")
                .with_group("Configuration")
                .with_importance(ParameterImportance::Critical),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let param_channels = ParameterId::from("channels");

        if id == param_channels {
            if let Some(channels) = value.as_int() {
                if !(1..=16).contains(&channels) {
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
            } else {
                // Should never happen since new() checks for writer
                return Err("HAL writer not available".to_string());
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Should never happen since new() fails on non-macOS
            return Err("HAL output plugin is only supported on macOS".to_string());
        }

        // Audio passes through unmodified
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hal_output_plugin_validation() {
        // Invalid channel counts should fail
        assert!(HalOutputPlugin::new(0).is_err());
        assert!(HalOutputPlugin::new(17).is_err());

        // Valid channel counts (will still fail without HAL initialized)
        #[cfg(target_os = "macos")]
        {
            // This will fail because HAL isn't initialized in tests
            assert!(HalOutputPlugin::new(2).is_err());
            assert!(HalOutputPlugin::new(5).is_err());
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Should fail on non-macOS platforms
            assert!(HalOutputPlugin::new(2).is_err());
        }
    }

    #[test]
    fn test_hal_output_plugin_from_params() {
        let params = HalOutputPluginParams { channels: 2 };
        // Will fail without HAL initialized
        let result = HalOutputPlugin::from_params(2, params);

        #[cfg(target_os = "macos")]
        assert!(result.is_err());

        #[cfg(not(target_os = "macos"))]
        assert!(result.is_err());
    }

    #[test]
    fn test_hal_output_plugin_invalid_params() {
        let params = HalOutputPluginParams { channels: 2 };

        assert!(HalOutputPlugin::from_params(0, params.clone()).is_err());
        assert!(HalOutputPlugin::from_params(20, params).is_err());
    }
}
