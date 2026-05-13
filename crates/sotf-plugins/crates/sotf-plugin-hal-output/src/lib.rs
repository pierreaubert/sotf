// ============================================================================
// HAL Output Plugin - Writes audio to macOS HAL driver
// ============================================================================

pub mod params;

use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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

    /// Write success ratio for the last process block as a percentage (0.0–100.0).
    /// 100.0 means all samples were accepted; lower values indicate back-pressure.
    write_success_ratio: f32,

    /// Total buffer capacity in samples (for computing fill percentage).
    /// Only used on macOS with the HAL feature enabled.
    #[allow(dead_code)]
    buffer_capacity: usize,

    /// Block counter used to rate-limit partial-write log warnings.
    /// Only read inside the `hal`-feature block.
    #[allow(dead_code)]
    last_warn_block: u64,

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
                write_success_ratio: 100.0,
                buffer_capacity: 0,
                last_warn_block: 0,
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
        vec![
            Parameter::new_int(
                "underrun_count",
                "Underrun Count",
                self.underrun_counter.load(Ordering::Relaxed) as i32,
                0,
                i32::MAX,
            )
            .with_description("Number of buffer underruns detected (read-only diagnostic)")
            .with_group("Diagnostics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "write_success_ratio",
                "Write Success",
                self.write_success_ratio,
                0.0,
                100.0,
            )
            .with_description(
                "Percentage of samples accepted by the HAL writer in the last process block \
                 (100 = all accepted, <100 = back-pressure / partial write; read-only diagnostic)",
            )
            .with_group("Diagnostics")
            .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("HAL output has no adjustable parameters".to_string())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "underrun_count" {
            Some(ParameterValue::Int(
                self.underrun_counter.load(Ordering::Relaxed) as i32,
            ))
        } else if id.0 == "write_success_ratio" {
            Some(ParameterValue::Float(self.write_success_ratio))
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

                // Update write success ratio diagnostic (100% = all samples accepted).
                if !input.is_empty() {
                    self.write_success_ratio =
                        (samples_written as f32 / input.len() as f32) * 100.0;
                }

                if samples_written < input.len() {
                    let block = self.underrun_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    // Rate-limit: log on first underrun, then every 1000 blocks.
                    if block == 1 || block - self.last_warn_block >= 1000 {
                        self.last_warn_block = block;
                        log::warn!(
                            "[HAL Output] Partial write: wrote {} of {} samples \
                             ({} of {} frames) — possible underrun (total: {})",
                            samples_written,
                            input.len(),
                            samples_written / self.channels.max(1),
                            context.num_frames,
                            block,
                        );
                    }
                }
            } else {
                return Err("HAL writer not available".to_string());
            }
        }

        Ok(context.num_frames)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a minimal plugin without the HAL feature.
    // On non-macOS / non-hal builds new() returns Err, so we test the public
    // interface that is reachable regardless of the HAL feature gate.

    #[test]
    fn new_rejects_zero_channels() {
        match HalOutputPlugin::new(0) {
            Err(e) => assert!(e.contains("Invalid channel count"), "unexpected error: {e}"),
            Ok(_) => panic!("expected Err for 0 channels"),
        }
    }

    #[test]
    fn new_rejects_too_many_channels() {
        match HalOutputPlugin::new(17) {
            Err(e) => assert!(e.contains("Invalid channel count"), "unexpected error: {e}"),
            Ok(_) => panic!("expected Err for 17 channels"),
        }
    }

    #[test]
    fn diagnostic_parameter_id_is_write_success_ratio() {
        // Construct a bare struct (bypassing new() which requires HAL on macOS)
        // by directly testing that the old name is gone from the parameter list.
        // We do this via a non-hal build where new() returns Err but we can
        // still inspect the parameter names returned by a manually constructed value.
        //
        // On macOS+hal this test would need a running HAL daemon, so we skip the
        // Plugin::parameters() check and just verify the id string constants.
        assert_ne!(
            "buffer_fill_level", "write_success_ratio",
            "parameter was not renamed — this test itself is wrong"
        );

        // The get_parameter implementation must recognise "write_success_ratio"
        // and must NOT recognise the old name "buffer_fill_level".
        // We build a fake struct to drive the get_parameter logic directly.
        // Use a raw struct literal to avoid calling new() so this compiles on
        // any platform regardless of feature flags.
        let plugin = make_test_plugin();
        let old_id = ParameterId("buffer_fill_level".to_string());
        let new_id = ParameterId("write_success_ratio".to_string());

        assert!(
            plugin.get_parameter(&old_id).is_none(),
            "old parameter id 'buffer_fill_level' must no longer be recognised"
        );
        assert!(
            plugin.get_parameter(&new_id).is_some(),
            "new parameter id 'write_success_ratio' must be recognised"
        );
    }

    #[test]
    fn get_parameter_write_success_ratio_returns_float() {
        let plugin = make_test_plugin();
        let id = ParameterId("write_success_ratio".to_string());
        match plugin.get_parameter(&id) {
            Some(ParameterValue::Float(v)) => {
                assert!(
                    (0.0..=100.0).contains(&v),
                    "write_success_ratio {v} out of 0–100 range"
                );
            }
            other => panic!("expected ParameterValue::Float, got {other:?}"),
        }
    }

    #[test]
    fn get_parameter_underrun_count_returns_int() {
        let plugin = make_test_plugin();
        let id = ParameterId("underrun_count".to_string());
        assert!(matches!(
            plugin.get_parameter(&id),
            Some(ParameterValue::Int(_))
        ));
    }

    #[test]
    fn process_rejects_mismatched_buffer() {
        let mut plugin = make_test_plugin();
        let ctx = ProcessContext {
            num_frames: 4,
            sample_rate: 48000,
        };
        // 4 frames * 2 channels = 8 samples; supply 7 instead.
        let input = vec![0.0f32; 7];
        let mut output = vec![];
        let err = plugin.process(&input, &mut output, &ctx).unwrap_err();
        assert!(err.contains("mismatch"), "unexpected error message: {err}");
    }

    /// Build a `HalOutputPlugin` directly (without the HAL writer) so tests
    /// can exercise the parameter and validation logic on any platform.
    fn make_test_plugin() -> HalOutputPlugin {
        HalOutputPlugin {
            channels: 2,
            underrun_counter: Arc::new(AtomicU64::new(0)),
            write_success_ratio: 100.0,
            buffer_capacity: 0,
            last_warn_block: 0,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            writer: None,
        }
    }
}
