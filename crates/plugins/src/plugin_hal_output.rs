// ============================================================================
// HAL Output Plugin - Writes audio to macOS HAL driver
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalOutputWriter;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration parameters for HalOutputPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalOutputPluginParams {
    /// Number of input channels (default: 2 for stereo)
    #[serde(default = "default_channels")]
    pub channels: usize,
}

fn default_channels() -> usize {
    2
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// HAL Output Plugin - Sink plugin that writes audio to macOS HAL driver
pub struct HalOutputPlugin {
    /// Number of input channels
    channels: usize,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    /// HAL output writer
    writer: Option<HalOutputWriter>,
}

impl HalOutputPlugin {
    /// Create a new HAL output plugin
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
            let writer = HalOutputWriter::new();

            if writer.is_none() {
                return Err(
                    "HAL driver not initialized. Ensure daemon initialized HAL before creating plugins".to_string()
                );
            }

            Ok(Self { channels, writer })
        }

        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            Err(
                "HAL output plugin is only supported on macOS with 'hal' feature enabled"
                    .to_string(),
            )
        }
    }

    /// Create from configuration parameters
    pub fn from_params(params: HalOutputPluginParams) -> Result<Self, String> {
        Self::new(params.channels)
    }
}

impl Plugin for HalOutputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("HAL Output", "1.0.0", "SotF")
            .with_description("Writes audio to macOS HAL driver")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        0 // Sink plugin - no output
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("HAL output has no adjustable parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        _output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        // Verify input buffer size
        let expected_len = context.num_frames * self.channels;
        if input.len() != expected_len {
            return Err(format!(
                "Input buffer size mismatch: expected {}, got {}",
                expected_len,
                input.len()
            ));
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            // Try to write to HAL
            if let Some(ref mut writer) = self.writer {
                writer.write(input);
            } else {
                return Err("HAL writer not available".to_string());
            }
        }

        Ok(context.num_frames)
    }
}
