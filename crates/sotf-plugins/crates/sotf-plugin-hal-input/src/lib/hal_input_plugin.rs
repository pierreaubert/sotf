use super::types::HalInputPluginParams;
#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// HAL Input Plugin - Source plugin that reads audio from macOS apps via HAL driver
pub struct HalInputPlugin {
    /// Number of output channels
    pub(super) channels: usize,

    /// Counter for buffer overruns (read returned fewer samples than expected)
    pub(super) underrun_counter: Arc<AtomicU64>,

    /// True if the HAL native sample rate differs from the engine sample rate
    pub(super) sample_rate_mismatch: bool,

    /// Cached HAL connection status.
    pub(super) is_connected: bool,

    /// Cached HAL buffer capacity in frames.
    #[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
    pub(super) buffer_capacity_frames: usize,

    #[cfg(all(target_os = "macos", feature = "hal"))]
    /// HAL input reader
    pub(super) reader: Option<HalInputReader>,
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

            Ok(Self {
                channels,
                underrun_counter: Arc::new(AtomicU64::new(0)),
                sample_rate_mismatch: false,
                is_connected: reader.as_ref().is_some_and(HalInputReader::is_connected),
                buffer_capacity_frames: reader
                    .as_ref()
                    .and_then(|r| r.current_format().ok())
                    .map(|(_, _, buffer_frames)| buffer_frames as usize)
                    .unwrap_or(0),
                reader,
            })
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

    /// Get the shared overrun counter (can be read from any thread)
    pub fn underrun_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.underrun_counter)
    }

    /// Get the current overrun count
    pub fn underrun_count(&self) -> u64 {
        self.underrun_counter.load(Ordering::Relaxed)
    }

    /// Zero-fill the tail of an interleaved output buffer after a partial read.
    ///
    /// Using `write_bytes` here gives us a low-level contiguous clear path for the
    /// remaining samples, which is typically faster than a hand-written scalar loop.
    pub(super) fn zero_fill_from(output: &mut [f32], start: usize) {
        if start >= output.len() {
            return;
        }

        let remaining = output.len() - start;
        unsafe {
            std::ptr::write_bytes(output.as_mut_ptr().add(start), 0, remaining);
        }
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
            Parameter::new_int(
                "input_channels",
                "Input Channels",
                self.channels as i32,
                1,
                16,
            )
                .with_description("Number of HAL input channels (structural — set at construction time)")
                .with_group("Configuration")
                .with_importance(ParameterImportance::Critical),
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
            Parameter::new_bool(
                "sample_rate_mismatch",
                "Sample Rate Mismatch",
                self.sample_rate_mismatch,
            )
            .with_description(
                "True if HAL native sample rate differs from engine sample rate (read-only diagnostic)",
            )
            .with_group("Diagnostics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "is_connected",
                "Connected",
                self.is_connected,
            )
            .with_description("HAL input reader currently connected to shared memory")
            .with_group("Diagnostics")
            .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate_mismatch = false;

        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            if let Some(ref reader) = self.reader {
                let hal_rate = reader.sample_rate();
                if hal_rate != sample_rate {
                    self.sample_rate_mismatch = true;
                    // No in-crate resampler is available; playing at mismatched rates
                    // produces incorrect pitch and duration.  Fail loudly so the caller
                    // can either configure the HAL to match the engine rate or insert a
                    // Resampler plugin upstream.
                    return Err(format!(
                        "[HAL Input] Sample rate mismatch: HAL native rate {} Hz != engine rate {} Hz. \
                         Configure the HAL device to {} Hz or insert a resampler plugin.",
                        hal_rate, sample_rate, sample_rate
                    ));
                }
            }
        }

        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            let _ = sample_rate;
        }

        Ok(())
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let param_input_channels = ParameterId::from("input_channels");

        if id == param_input_channels {
            // `input_channels` is a structural parameter: the HalInputReader is created
            // during new() with a fixed channel count.  Changing it at runtime would
            // desync the reader's channel count from the plugin's reported output_channels,
            // causing buffer-size mismatches or channel misalignment.
            //
            // Callers must construct a new plugin instance to change the channel count.
            let _ = value;
            Err(
                "input_channels is a structural parameter and cannot be changed after construction. \
                 Create a new HalInputPlugin with the desired channel count."
                    .to_string(),
            )
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let param_input_channels = ParameterId::from("input_channels");
        let param_underrun = ParameterId::from("underrun_count");
        let param_sr_mismatch = ParameterId::from("sample_rate_mismatch");
        let param_connected = ParameterId::from("is_connected");

        if id == &param_input_channels {
            Some(ParameterValue::Int(self.channels as i32))
        } else if id == &param_underrun {
            Some(ParameterValue::Int(
                self.underrun_counter.load(Ordering::Relaxed) as i32,
            ))
        } else if id == &param_sr_mismatch {
            Some(ParameterValue::Bool(self.sample_rate_mismatch))
        } else if id == &param_connected {
            Some(ParameterValue::Bool(self.is_connected))
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
                self.is_connected = reader.is_connected();
                self.buffer_capacity_frames = reader
                    .current_format()
                    .map(|(_, _, buffer_frames)| buffer_frames as usize)
                    .unwrap_or(0);
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

                // Zero-fill any remaining samples if we didn't read enough.
                // Only count as an underrun when we received *some* samples but fewer
                // than expected (partial read).  A fully empty read (0 samples) is
                // normal during device startup or device switching and must not pollute
                // the diagnostic counter — it is logged at trace level above.
                if samples_read < output.len() {
                    if samples_read > 0 {
                        self.underrun_counter.fetch_add(1, Ordering::Relaxed);
                        log::debug!(
                            "[HAL Input] Buffer underrun: read {} samples, expected {}",
                            samples_read,
                            output.len()
                        );
                    }
                    Self::zero_fill_from(output, samples_read);
                }
            } else {
                self.is_connected = false;
                Self::zero_fill_from(output, 0);
            }
        }

        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            Self::zero_fill_from(output, 0);
        }

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            self.buffer_capacity_frames
        }
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sotf_host::parameters::ParameterValue;
    use sotf_host::plugin::{Plugin, ProcessContext};

    // -----------------------------------------------------------------------
    // Helper: build a plugin struct directly without going through new() so
    // tests run on non-macOS / without the `hal` feature.
    // -----------------------------------------------------------------------
    fn stub_plugin(channels: usize) -> HalInputPlugin {
        HalInputPlugin {
            channels,
            underrun_counter: Arc::new(AtomicU64::new(0)),
            sample_rate_mismatch: false,
            is_connected: false,
            buffer_capacity_frames: 0,
            #[cfg(all(target_os = "macos", feature = "hal"))]
            reader: None,
        }
    }

    // -----------------------------------------------------------------------
    // Bug fix 1: parameter name mismatch
    // The registered parameter ID must be "input_channels" (matching params.rs),
    // NOT "channels".
    // -----------------------------------------------------------------------
    #[test]
    fn parameters_exposes_input_channels_id() {
        let plugin = stub_plugin(2);
        let params = plugin.parameters();
        let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
        assert!(
            ids.contains(&"input_channels"),
            "expected parameter id 'input_channels', got: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"channels"),
            "stale 'channels' id must not appear, got: {:?}",
            ids
        );
    }

    #[test]
    fn parameters_input_channels_reflects_constructed_channel_count() {
        let plugin = stub_plugin(6);
        let params = plugin.parameters();
        let input_channels = params
            .iter()
            .find(|p| p.id == ParameterId::from("input_channels"))
            .expect("input_channels parameter should exist");
        assert_eq!(
            input_channels.default_value,
            ParameterValue::Int(6),
            "input_channels metadata must reflect the plugin's structural channel count"
        );
    }

    #[test]
    fn get_parameter_input_channels_returns_value() {
        let plugin = stub_plugin(4);
        let id = ParameterId::from("input_channels");
        let val = plugin.get_parameter(&id);
        assert_eq!(val, Some(ParameterValue::Int(4)));
    }

    #[test]
    fn get_parameter_old_channels_id_returns_none() {
        let plugin = stub_plugin(2);
        let id = ParameterId::from("channels");
        let val = plugin.get_parameter(&id);
        assert_eq!(
            val, None,
            "old 'channels' id must return None after rename to 'input_channels'"
        );
    }

    #[test]
    fn parameters_exposes_connected_diagnostic() {
        let plugin = stub_plugin(2);
        let params = plugin.parameters();
        let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"is_connected"));
    }

    #[test]
    fn get_parameter_connected_returns_bool() {
        let plugin = stub_plugin(2);
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("is_connected")),
            Some(ParameterValue::Bool(false))
        );
    }

    #[test]
    fn latency_samples_reports_buffer_capacity() {
        let plugin = stub_plugin(2);
        assert_eq!(plugin.latency_samples(), 0);
    }

    #[test]
    fn set_parameter_input_channels_id_is_rejected_post_construction() {
        // After construction the channel count is structural — changing it
        // without reinitializing the reader would desync channel count vs
        // reader config. set_parameter must return Err.
        let mut plugin = stub_plugin(2);
        let id = ParameterId::from("input_channels");
        let result = plugin.set_parameter(id, ParameterValue::Int(6));
        assert!(
            result.is_err(),
            "set_parameter for 'input_channels' must return Err (read-only post-construction)"
        );
    }

    // -----------------------------------------------------------------------
    // Bug fix 2: underrun counter only on partial reads (samples_read > 0)
    // A fully empty read (0 samples) during startup must NOT increment.
    // -----------------------------------------------------------------------
    #[test]
    fn underrun_count_not_incremented_on_fully_empty_read() {
        // The non-hal path fills the buffer with silence (analogous to a
        // fully-empty HAL read).  The underrun counter must stay at 0.
        let ctx = ProcessContext::new(48000, 4);
        let input: Vec<f32> = vec![];
        let mut output: Vec<f32> = vec![0.0f32; ctx.num_frames * 2];
        let mut p = stub_plugin(2);
        let _ = p.process(&input, &mut output, &ctx);
        assert_eq!(
            p.underrun_count(),
            0,
            "empty read (non-hal silence fill) must not count as underrun"
        );
    }

    // -----------------------------------------------------------------------
    // Bug fix 3: initialize() returns Err on sample rate mismatch (hal only)
    // On non-hal builds initialize() must succeed regardless of sample_rate.
    // -----------------------------------------------------------------------
    #[test]
    fn initialize_succeeds_on_non_hal_build() {
        let mut p = stub_plugin(2);
        // On non-hal builds any sample rate is fine — no reader to check against.
        assert!(p.initialize(44100).is_ok());
        assert!(p.initialize(96000).is_ok());
    }

    // -----------------------------------------------------------------------
    // process() returns num_frames even on non-hal (silence) path
    // -----------------------------------------------------------------------
    #[test]
    fn process_returns_num_frames() {
        let ctx = ProcessContext::new(48000, 8);
        let input: Vec<f32> = vec![];
        let mut output: Vec<f32> = vec![0.0f32; ctx.num_frames * 2];
        let mut p = stub_plugin(2);
        let result = p.process(&input, &mut output, &ctx);
        assert_eq!(result, Ok(8));
    }

    #[test]
    fn process_rejects_wrong_output_buffer_size() {
        let ctx = ProcessContext::new(48000, 4);
        let input: Vec<f32> = vec![];
        // Buffer too small: 7 instead of 4*2=8
        let mut output: Vec<f32> = vec![0.0f32; 7];
        let mut p = stub_plugin(2);
        let result = p.process(&input, &mut output, &ctx);
        assert!(result.is_err(), "wrong buffer size must return Err");
    }

    #[test]
    fn zero_fill_from_clears_expected_tail() {
        let mut output: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        HalInputPlugin::zero_fill_from(&mut output, 3);
        assert_eq!(output, vec![1.0, 2.0, 3.0, 0.0, 0.0]);
    }
}
