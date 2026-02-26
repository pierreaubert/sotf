// ============================================================================
// HAL Input Plugin - Reads audio from macOS HAL driver
// ============================================================================

use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;

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
pub struct HalInputPlugin {
    /// Number of output channels
    channels: usize,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    /// HAL input reader
    reader: Option<HalInputReader>,
}

impl HalInputPlugin {
    /// Create a new HAL input plugin
    pub fn new(channels: usize) -> Result<Self, String> {
        // Validate channels
        if channels == 0 || channels > 16 {
            return Err(format!(
                "Invalid channel count: {}. Must be between 1 and 16",
                channels
            ));
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            let reader = HalInputReader::new();

            if reader.is_none() {
                return Err(
                    "HAL driver not initialized. Ensure daemon initialized HAL before creating plugins".to_string()
                );
            }

            Ok(Self { channels, reader })
        }

        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            // On other platforms or when feature is disabled, return a "null" plugin
            // but return error on creation to avoid confusion
            Err(
                "HAL input plugin is only supported on macOS with 'hal' feature enabled"
                    .to_string(),
            )
        }
    }

    /// Create from configuration parameters
    pub fn from_params(params: HalInputPluginParams) -> Result<Self, String> {
        Self::new(params.channels)
    }
}

impl Plugin for HalInputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("HAL Input", "1.0.0", "SotF")
            .with_description("Reads audio from macOS apps via HAL driver")
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
                .with_description("Number of output channels")
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

    fn process(
        &mut self,
        _input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        // Verify output buffer size
        let expected_len = context.num_frames * self.channels;
        if output.len() != expected_len {
            return Err(format!(
                "Output buffer size mismatch: expected {}, got {}",
                expected_len,
                output.len()
            ));
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            // Try to read from HAL
            if let Some(ref mut reader) = self.reader {
                let samples_read = reader.read(output);

                // TRACE: Log frames consumed from HAL shared memory by daemon
                if samples_read > 0 {
                    log::debug!(
                        "[AUDIO FLOW] HAL->Daemon: consumed {} frames from shared memory (expected {})",
                        samples_read / self.channels,
                        context.num_frames
                    );
                } else {
                    log::trace!("[AUDIO FLOW] HAL->Daemon: no frames available (underrun)");
                }

                // Zero-fill any remaining samples if we didn't read enough
                if samples_read < output.len() {
                    output[samples_read..].fill(0.0);
                }
            } else {
                return Err("HAL reader not available".to_string());
            }
        }

        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            output.fill(0.0);
        }

        Ok(context.num_frames)
    }
}
