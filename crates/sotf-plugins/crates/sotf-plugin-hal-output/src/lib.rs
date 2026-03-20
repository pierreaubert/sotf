// ============================================================================
// HAL Output Plugin - Writes audio to macOS HAL driver
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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

    /// Counter for buffer underruns (partial write detected)
    underrun_counter: Arc<AtomicU64>,

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

            Ok(Self {
                channels,
                underrun_counter: Arc::new(AtomicU64::new(0)),
                writer,
            })
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

    /// Get the shared underrun counter (can be read from any thread)
    pub fn underrun_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.underrun_counter)
    }

    /// Get the current underrun count
    pub fn underrun_count(&self) -> u64 {
        self.underrun_counter.load(Ordering::Relaxed)
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
        vec![Parameter::new_int(
            "underrun_count",
            "Underrun Count",
            self.underrun_counter.load(Ordering::Relaxed) as i32,
            0,
            i32::MAX,
        )
        .with_description("Number of buffer underruns detected (read-only diagnostic)")
        .with_group("Diagnostics")
        .with_importance(ParameterImportance::FineTuning)]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("HAL output has no adjustable parameters".to_string())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "underrun_count" {
            Some(ParameterValue::Int(
                self.underrun_counter.load(Ordering::Relaxed) as i32,
            ))
        } else {
            None
        }
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
                let samples_written = writer.write(input);
                if samples_written < input.len() {
                    self.underrun_counter.fetch_add(1, Ordering::Relaxed);
                    log::warn!(
                        "[HAL Output] Partial write: wrote {} of {} samples ({} of {} frames) — possible underrun",
                        samples_written,
                        input.len(),
                        samples_written / self.channels.max(1),
                        context.num_frames,
                    );
                }
            } else {
                return Err("HAL writer not available".to_string());
            }
        }

        Ok(context.num_frames)
    }
}
