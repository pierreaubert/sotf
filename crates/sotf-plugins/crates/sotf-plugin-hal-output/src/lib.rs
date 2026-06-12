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

// Static error messages used on the audio hot path. Using constants avoids
// re-formatting a fresh `String` every time an error is reported.
const ERR_INVALID_CHANNEL_COUNT: &str =
    "Invalid channel count. Must be between 1 and 16";
#[cfg(all(target_os = "macos", feature = "hal"))]
const ERR_HAL_DAEMON_NOT_INITIALIZED: &str =
    "HAL driver not initialized. Ensure daemon initialized HAL before creating plugins";
const ERR_HAL_UNSUPPORTED_PLATFORM: &str =
    "HAL output plugin is only supported on macOS with 'hal' feature enabled";
const ERR_NO_ADJUSTABLE_PARAMETERS: &str = "HAL output has no adjustable parameters";
#[cfg(all(target_os = "macos", feature = "hal"))]
const ERR_HAL_WRITER_NOT_AVAILABLE: &str = "HAL writer not available";

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

    /// Cached HAL buffer capacity in samples.
    #[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
    buffer_capacity: usize,

    /// Cached HAL connection status.
    is_connected: bool,

    /// Block counter used to rate-limit partial-write log warnings.
    #[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
    last_warn_block: u64,

    /// Cached back-pressure state, useful as a diagnostic signal for downstream UI/host.
    is_backpressured: bool,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    /// HAL output writer
    writer: Option<HalOutputWriter>,
}

impl HalOutputPlugin {
    /// Create a new HAL output plugin
    pub fn new(channels: usize) -> Result<Self, String> {
        // Validate channels
        if channels == 0 || channels > 16 {
            return Err(ERR_INVALID_CHANNEL_COUNT.to_string());
        }

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            let writer = HalOutputWriter::new();

            if writer.is_none() {
                return Err(ERR_HAL_DAEMON_NOT_INITIALIZED.to_string());
            }

            Ok(Self {
                channels,
                underrun_counter: Arc::new(AtomicU64::new(0)),
                write_success_ratio: 100.0,
                buffer_capacity: writer
                    .as_ref()
                    .map(|w| w.buffer_frames() as usize)
                    .unwrap_or(0),
                is_connected: writer.as_ref().is_some_and(HalOutputWriter::is_connected),
                last_warn_block: 0,
                is_backpressured: false,
                writer,
            })
        }

        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            Err(ERR_HAL_UNSUPPORTED_PLATFORM.to_string())
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
            Parameter::new_bool("is_connected", "Connected", self.is_connected)
                .with_description("HAL output writer currently connected to shared memory")
                .with_group("Diagnostics")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("is_backpressured", "Backpressure", self.is_backpressured)
                .with_description("HAL output reported a partial write in the last process block")
                .with_group("Diagnostics")
                .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err(ERR_NO_ADJUSTABLE_PARAMETERS.to_string())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "underrun_count" {
            Some(ParameterValue::Int(
                self.underrun_counter.load(Ordering::Relaxed) as i32,
            ))
        } else if id.0 == "write_success_ratio" {
            Some(ParameterValue::Float(self.write_success_ratio))
        } else if id.0 == "is_connected" {
            Some(ParameterValue::Bool(self.is_connected))
        } else if id.0 == "is_backpressured" {
            Some(ParameterValue::Bool(self.is_backpressured))
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
                self.is_connected = writer.is_connected();
                self.buffer_capacity = writer.buffer_frames() as usize;
                let samples_written = writer.write(input);

                // Update write success ratio diagnostic (100% = all samples accepted).
                if !input.is_empty() {
                    self.write_success_ratio =
                        (samples_written as f32 / input.len() as f32) * 100.0;
                    self.is_backpressured = samples_written < input.len();
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
                return Err(ERR_HAL_WRITER_NOT_AVAILABLE.to_string());
            }
        }

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.buffer_capacity
        }
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            0
        }
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
    fn diagnostic_parameters_include_connection_and_backpressure() {
        let plugin = make_test_plugin();
        let ids: Vec<String> = plugin.parameters().iter().map(|p| p.id.0.clone()).collect();
        assert!(ids.contains(&"is_connected".to_string()));
        assert!(ids.contains(&"is_backpressured".to_string()));
    }

    #[test]
    fn get_parameter_connection_and_backpressure_defaults_to_expected_values() {
        let plugin = make_test_plugin();
        assert_eq!(
            plugin.get_parameter(&ParameterId("is_connected".to_string())),
            Some(ParameterValue::Bool(false))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId("is_backpressured".to_string())),
            Some(ParameterValue::Bool(false))
        );
    }

    #[test]
    fn latency_samples_reports_cached_buffer_capacity() {
        let plugin = make_test_plugin();
        assert_eq!(plugin.latency_samples(), 0);
    }

    #[test]
    fn process_rejects_mismatched_buffer() {
        let mut plugin = make_test_plugin();
        let ctx = ProcessContext::new(48000, 4);
        // 4 frames * 2 channels = 8 samples; supply 7 instead.
        let input = vec![0.0f32; 7];
        let mut output = vec![];
        let err = plugin.process(&input, &mut output, &ctx).unwrap_err();
        assert!(err.contains("mismatch"), "unexpected error message: {err}");
    }

    #[test]
    fn set_parameter_returns_static_no_adjustable_params_error() {
        let mut plugin = make_test_plugin();
        let err = plugin
            .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(0.0))
            .unwrap_err();
        assert_eq!(err, ERR_NO_ADJUSTABLE_PARAMETERS);
    }

    #[test]
    fn new_rejects_invalid_channel_count_with_static_error() {
        let err = match HalOutputPlugin::new(0) {
            Err(e) => e,
            Ok(_) => panic!("expected error for 0 channels"),
        };
        assert_eq!(err, ERR_INVALID_CHANNEL_COUNT);
    }

    /// Build a `HalOutputPlugin` directly (without the HAL writer) so tests
    /// can exercise the parameter and validation logic on any platform.
    fn make_test_plugin() -> HalOutputPlugin {
        HalOutputPlugin {
            channels: 2,
            underrun_counter: Arc::new(AtomicU64::new(0)),
            write_success_ratio: 100.0,
            buffer_capacity: 0,
            is_connected: false,
            last_warn_block: 0,
            is_backpressured: false,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            writer: None,
        }
    }
}
