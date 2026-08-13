use super::types::HalInputPluginParams;
#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalInputReader;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HalStreamFormat {
    sample_rate: u32,
    channels: usize,
    buffer_capacity_frames: usize,
}

trait HalAudioSource: Send {
    /// Fill interleaved output and return the number of complete frames read.
    fn read_frames(&mut self, output: &mut [f32]) -> usize;
    fn is_connected(&self) -> bool;
    fn current_format(&self) -> Result<HalStreamFormat, String>;
    fn transport_available_frames(&self) -> usize {
        0
    }
    fn needs_control_recovery(&self) -> bool {
        false
    }
}

#[cfg(all(target_os = "macos", feature = "hal"))]
impl HalAudioSource for HalInputReader {
    fn read_frames(&mut self, output: &mut [f32]) -> usize {
        self.read(output)
    }

    fn is_connected(&self) -> bool {
        HalInputReader::is_connected(self)
    }

    fn current_format(&self) -> Result<HalStreamFormat, String> {
        let (sample_rate, channels, buffer_capacity_frames) =
            HalInputReader::current_format(self).map_err(|error| error.to_string())?;
        Ok(HalStreamFormat {
            sample_rate,
            channels: channels as usize,
            buffer_capacity_frames: buffer_capacity_frames as usize,
        })
    }

    fn needs_control_recovery(&self) -> bool {
        self.needs_cipher_reload()
    }

    fn transport_available_frames(&self) -> usize {
        self.available_read_frames()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalInputErrorKind {
    None,
    Disconnected,
    CipherReloadRequired,
    FormatMismatch,
    TransportCorruption,
    Underrun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalInputDiagnostics {
    pub underrun_count: u64,
    pub missing_frames: u64,
    pub connection_generation: u64,
    pub recovery_generation: u64,
    pub format_change_generation: u64,
    pub connected: bool,
    pub sample_rate_mismatch: bool,
    pub needs_control_recovery: bool,
    pub format: Option<(u32, usize)>,
    pub buffer_capacity_frames: usize,
    pub available_frames: usize,
    pub last_error: HalInputErrorKind,
}

/// HAL Input Plugin - Source plugin that reads audio from macOS apps via HAL driver
pub struct HalInputPlugin {
    /// Number of output channels
    pub(super) channels: usize,

    /// Counter for buffer overruns (read returned fewer samples than expected)
    pub(super) underrun_counter: Arc<AtomicU64>,
    missing_frames: Arc<AtomicU64>,
    connection_generation: Arc<AtomicU64>,
    recovery_generation: Arc<AtomicU64>,
    format_change_generation: Arc<AtomicU64>,

    /// True if the HAL native sample rate differs from the engine sample rate
    pub(super) sample_rate_mismatch: bool,

    /// Cached HAL connection status.
    pub(super) is_connected: bool,

    /// Cached HAL buffer capacity in frames.
    #[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
    pub(super) buffer_capacity_frames: usize,

    /// HAL input reader
    reader: Option<Box<dyn HalAudioSource>>,

    /// Engine rate negotiated during initialization.
    initialized_sample_rate: Option<u32>,

    /// True after at least one complete or partial read while connected.
    stream_armed: bool,
    recovery_pending: bool,
    format_mismatch_pending: bool,
    last_error: HalInputErrorKind,
    cached_parameters: Vec<Parameter>,
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
                missing_frames: Arc::new(AtomicU64::new(0)),
                connection_generation: Arc::new(AtomicU64::new(0)),
                recovery_generation: Arc::new(AtomicU64::new(0)),
                format_change_generation: Arc::new(AtomicU64::new(0)),
                sample_rate_mismatch: false,
                is_connected: reader.as_ref().is_some_and(HalInputReader::is_connected),
                buffer_capacity_frames: reader
                    .as_ref()
                    .and_then(|r| r.current_format().ok())
                    .map(|(_, _, buffer_frames)| buffer_frames as usize)
                    .unwrap_or(0),
                reader: reader.map(|reader| Box::new(reader) as Box<dyn HalAudioSource>),
                initialized_sample_rate: None,
                stream_armed: false,
                recovery_pending: false,
                format_mismatch_pending: false,
                last_error: HalInputErrorKind::None,
                cached_parameters: Self::build_parameters(channels),
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
        Self::new(params.input_channels)
    }

    /// Get the shared overrun counter (can be read from any thread)
    pub fn underrun_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.underrun_counter)
    }

    /// Get the current overrun count
    pub fn underrun_count(&self) -> u64 {
        self.underrun_counter.load(Ordering::Relaxed)
    }

    pub fn diagnostics(&self) -> HalInputDiagnostics {
        let format = self
            .reader
            .as_ref()
            .and_then(|reader| reader.current_format().ok());
        HalInputDiagnostics {
            underrun_count: self.underrun_count(),
            missing_frames: self.missing_frames.load(Ordering::Relaxed),
            connection_generation: self.connection_generation.load(Ordering::Relaxed),
            recovery_generation: self.recovery_generation.load(Ordering::Relaxed),
            format_change_generation: self.format_change_generation.load(Ordering::Relaxed),
            connected: self.is_connected,
            sample_rate_mismatch: self.sample_rate_mismatch,
            needs_control_recovery: self.recovery_pending,
            format: format.map(|format| (format.sample_rate, format.channels)),
            buffer_capacity_frames: self.buffer_capacity_frames,
            available_frames: self
                .reader
                .as_ref()
                .map_or(0, |reader| reader.transport_available_frames()),
            last_error: self.last_error,
        }
    }

    fn build_parameters(channels: usize) -> Vec<Parameter> {
        vec![
            Parameter::new_int("input_channels", "Input Channels", channels as i32, 1, 16)
                .with_description(
                    "Number of HAL input channels (structural — set at construction time)",
                )
                .with_group("Configuration")
                .with_importance(ParameterImportance::Critical),
        ]
    }

    fn activate_reader(&mut self, reader: Box<dyn HalAudioSource>) -> Result<(), String> {
        let format = reader.current_format()?;
        if format.channels != self.channels {
            return Err(format!(
                "[HAL Input] Channel mismatch: HAL publishes {} channels but plugin is configured for {}",
                format.channels, self.channels
            ));
        }
        if let Some(sample_rate) = self.initialized_sample_rate
            && format.sample_rate != sample_rate
        {
            self.sample_rate_mismatch = true;
            return Err(format!(
                "[HAL Input] Sample rate mismatch: HAL native rate {} Hz != engine rate {} Hz",
                format.sample_rate, sample_rate
            ));
        }
        self.buffer_capacity_frames = format.buffer_capacity_frames;
        self.is_connected = reader.is_connected();
        self.reader = Some(reader);
        self.recovery_pending = false;
        self.format_mismatch_pending = false;
        self.stream_armed = false;
        self.last_error = HalInputErrorKind::None;
        Ok(())
    }

    /// Reopen shared memory and reload the session cipher on a control thread.
    /// Never call this from `process`: mapping and key loading may allocate and
    /// perform filesystem I/O. The existing reader remains active if validation
    /// of the replacement fails.
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub fn refresh_transport(&mut self) -> Result<(), String> {
        let mut replacement = HalInputReader::new()
            .ok_or_else(|| "HAL input shared memory is unavailable".to_string())?;
        if replacement.needs_cipher_reload() {
            replacement
                .reload_cipher()
                .map_err(|error| error.to_string())?;
        }
        self.activate_reader(Box::new(replacement))
    }

    pub(super) fn zero_fill_from(output: &mut [f32], start: usize) {
        if let Some(tail) = output.get_mut(start..) {
            tail.fill(0.0);
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

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::boundary(PluginCostClass::External, self.latency_samples())
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("HAL Input sample rate must be non-zero".to_string());
        }
        self.sample_rate_mismatch = false;

        if let Some(ref reader) = self.reader {
            let format = reader.current_format()?;
            if format.channels != self.channels {
                return Err(format!(
                    "[HAL Input] Channel mismatch: HAL publishes {} channels but plugin is configured for {}",
                    format.channels, self.channels
                ));
            }
            if format.sample_rate != sample_rate {
                self.sample_rate_mismatch = true;
                // No in-crate resampler is available; playing at mismatched rates
                // produces incorrect pitch and duration.  Fail loudly so the caller
                // can either configure the HAL to match the engine rate or insert a
                // Resampler plugin upstream.
                return Err(format!(
                    "[HAL Input] Sample rate mismatch: HAL native rate {} Hz != engine rate {} Hz. \
                         Configure the HAL device to {} Hz or insert a resampler plugin.",
                    format.sample_rate, sample_rate, sample_rate
                ));
            }
            self.buffer_capacity_frames = format.buffer_capacity_frames;
        }

        self.initialized_sample_rate = Some(sample_rate);
        self.recovery_pending = false;
        self.format_mismatch_pending = false;

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
        if id == &param_input_channels {
            Some(ParameterValue::Int(self.channels as i32))
        } else {
            None
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        if !input.is_empty() {
            return Err(format!(
                "HAL Input is a source and requires an empty input buffer, got {} samples",
                input.len()
            ));
        }
        let expected_len = context
            .num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "HAL Input frame/sample count overflow".to_string())?;
        if output.len() != expected_len {
            return Err(format!(
                "Output buffer size mismatch: expected {}, got {}",
                expected_len,
                output.len()
            ));
        }
        let Some(initialized_rate) = self.initialized_sample_rate else {
            Self::zero_fill_from(output, 0);
            return Err("HAL Input must be initialized before processing".to_string());
        };
        if context.sample_rate != initialized_rate {
            Self::zero_fill_from(output, 0);
            return Err(format!(
                "HAL Input context rate {} Hz differs from initialized rate {} Hz",
                context.sample_rate, initialized_rate
            ));
        }

        if let Some(ref mut reader) = self.reader {
            let was_connected = self.is_connected;
            self.is_connected = reader.is_connected();
            if self.is_connected != was_connected {
                self.connection_generation.fetch_add(1, Ordering::Relaxed);
            }
            let recovery_needed = !self.is_connected || reader.needs_control_recovery();
            if recovery_needed && !self.recovery_pending {
                self.recovery_generation.fetch_add(1, Ordering::Relaxed);
            }
            self.recovery_pending = recovery_needed;
            if !self.is_connected {
                self.last_error = HalInputErrorKind::Disconnected;
                self.stream_armed = false;
                Self::zero_fill_from(output, 0);
                return Ok(context.num_frames);
            }
            if reader.needs_control_recovery() {
                self.last_error = HalInputErrorKind::CipherReloadRequired;
                self.stream_armed = false;
                Self::zero_fill_from(output, 0);
                return Ok(context.num_frames);
            }
            let format = reader.current_format()?;
            if format.channels != self.channels
                || self
                    .initialized_sample_rate
                    .is_some_and(|rate| rate != format.sample_rate)
            {
                if !self.format_mismatch_pending {
                    self.format_change_generation
                        .fetch_add(1, Ordering::Relaxed);
                }
                self.format_mismatch_pending = true;
                self.last_error = HalInputErrorKind::FormatMismatch;
                Self::zero_fill_from(output, 0);
                self.stream_armed = false;
                return Err(format!(
                    "HAL Input stream format changed to {} Hz/{} ch; expected {:?} Hz/{} ch",
                    format.sample_rate,
                    format.channels,
                    self.initialized_sample_rate,
                    self.channels
                ));
            }
            self.format_mismatch_pending = false;
            self.buffer_capacity_frames = format.buffer_capacity_frames;
            let frames_read = reader.read_frames(output);
            if frames_read > context.num_frames {
                self.last_error = HalInputErrorKind::TransportCorruption;
                Self::zero_fill_from(output, 0);
                return Err(format!(
                    "HAL Input reader returned {frames_read} frames for a {}-frame callback",
                    context.num_frames
                ));
            }
            let samples_read = frames_read
                .checked_mul(self.channels)
                .ok_or_else(|| "HAL Input reader frame/sample count overflow".to_string())?;
            if frames_read > 0 {
                self.stream_armed = true;
            }
            if frames_read < context.num_frames {
                let missing = context.num_frames - frames_read;
                self.missing_frames
                    .fetch_add(missing as u64, Ordering::Relaxed);
                if frames_read > 0 || (self.stream_armed && self.is_connected) {
                    self.underrun_counter.fetch_add(1, Ordering::Relaxed);
                    self.last_error = HalInputErrorKind::Underrun;
                }
                Self::zero_fill_from(output, samples_read);
            }
            if !self.is_connected {
                self.stream_armed = false;
            }
        } else {
            if self.is_connected {
                self.connection_generation.fetch_add(1, Ordering::Relaxed);
            }
            self.is_connected = false;
            self.last_error = HalInputErrorKind::Disconnected;
            self.stream_armed = false;
            Self::zero_fill_from(output, 0);
        }

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        // Shared-memory capacity is not signal latency. Until the producer
        // maintains a measured target fill, report no graph compensation.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sotf_host::parameters::ParameterValue;
    use sotf_host::plugin::{Plugin, ProcessContext};
    use sotf_host::{CountingAlloc, assert_no_allocs};

    #[global_allocator]
    static ALLOCATOR: CountingAlloc = CountingAlloc;

    struct FakeReader {
        format: HalStreamFormat,
        connected: bool,
        needs_recovery: bool,
        frames_to_return: Vec<usize>,
        next_read: usize,
    }

    impl FakeReader {
        fn new(channels: usize, frames_to_return: Vec<usize>) -> Self {
            Self {
                format: HalStreamFormat {
                    sample_rate: 48_000,
                    channels,
                    buffer_capacity_frames: 1024,
                },
                connected: true,
                needs_recovery: false,
                frames_to_return,
                next_read: 0,
            }
        }
    }

    impl HalAudioSource for FakeReader {
        fn read_frames(&mut self, output: &mut [f32]) -> usize {
            let frames = self.frames_to_return[self.next_read.min(self.frames_to_return.len() - 1)];
            self.next_read += 1;
            let samples = frames
                .saturating_mul(self.format.channels)
                .min(output.len());
            for (index, sample) in output[..samples].iter_mut().enumerate() {
                *sample = (index + 1) as f32;
            }
            frames
        }

        fn is_connected(&self) -> bool {
            self.connected
        }

        fn current_format(&self) -> Result<HalStreamFormat, String> {
            Ok(self.format)
        }

        fn needs_control_recovery(&self) -> bool {
            self.needs_recovery
        }
    }

    fn fake_plugin(channels: usize, frames_to_return: Vec<usize>) -> HalInputPlugin {
        let mut plugin = stub_plugin(channels);
        plugin.reader = Some(Box::new(FakeReader::new(channels, frames_to_return)));
        plugin.initialize(48_000).unwrap();
        plugin
    }

    // -----------------------------------------------------------------------
    // Helper: build a plugin struct directly without going through new() so
    // tests run on non-macOS / without the `hal` feature.
    // -----------------------------------------------------------------------
    fn stub_plugin(channels: usize) -> HalInputPlugin {
        HalInputPlugin {
            channels,
            underrun_counter: Arc::new(AtomicU64::new(0)),
            missing_frames: Arc::new(AtomicU64::new(0)),
            connection_generation: Arc::new(AtomicU64::new(0)),
            recovery_generation: Arc::new(AtomicU64::new(0)),
            format_change_generation: Arc::new(AtomicU64::new(0)),
            sample_rate_mismatch: false,
            is_connected: false,
            buffer_capacity_frames: 0,
            reader: None,
            initialized_sample_rate: Some(48_000),
            stream_armed: false,
            recovery_pending: false,
            format_mismatch_pending: false,
            last_error: HalInputErrorKind::None,
            cached_parameters: HalInputPlugin::build_parameters(channels),
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
    fn diagnostics_are_typed_state_not_automatable_parameters() {
        let plugin = stub_plugin(2);
        let params = plugin.parameters();
        let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["input_channels"]);
        assert_eq!(
            plugin.diagnostics(),
            HalInputDiagnostics {
                underrun_count: 0,
                missing_frames: 0,
                connection_generation: 0,
                recovery_generation: 0,
                format_change_generation: 0,
                connected: false,
                sample_rate_mismatch: false,
                needs_control_recovery: false,
                format: None,
                buffer_capacity_frames: 0,
                available_frames: 0,
                last_error: HalInputErrorKind::None,
            }
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("underrun_count")),
            None
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

    #[test]
    fn full_reads_preserve_every_interleaved_sample_for_supported_layouts() {
        for channels in [1, 2, 6, 16] {
            let frames = 8;
            let mut plugin = fake_plugin(channels, vec![frames]);
            let mut output = vec![-1.0; frames * channels];
            plugin
                .process(&[], &mut output, &ProcessContext::new(48_000, frames))
                .unwrap();
            assert_eq!(
                output,
                (1..=frames * channels)
                    .map(|value| value as f32)
                    .collect::<Vec<_>>()
            );
            assert_eq!(plugin.underrun_count(), 0);
        }
    }

    #[test]
    fn partial_reads_zero_only_missing_complete_frames() {
        for channels in [1, 2, 6, 16] {
            let mut plugin = fake_plugin(channels, vec![3]);
            let mut output = vec![-1.0; 8 * channels];
            plugin
                .process(&[], &mut output, &ProcessContext::new(48_000, 8))
                .unwrap();
            assert_eq!(
                &output[..3 * channels],
                &(1..=3 * channels)
                    .map(|value| value as f32)
                    .collect::<Vec<_>>()
            );
            assert!(output[3 * channels..].iter().all(|sample| *sample == 0.0));
            assert_eq!(plugin.underrun_count(), 1);
            assert_eq!(plugin.diagnostics().missing_frames, 5);
            assert_eq!(plugin.diagnostics().last_error, HalInputErrorKind::Underrun);
        }
    }

    #[test]
    fn zero_read_counts_only_after_stream_is_armed() {
        let mut plugin = fake_plugin(2, vec![0, 4, 0]);
        let context = ProcessContext::new(48_000, 4);
        let mut output = vec![1.0; 8];
        plugin.process(&[], &mut output, &context).unwrap();
        assert_eq!(plugin.underrun_count(), 0);
        plugin.process(&[], &mut output, &context).unwrap();
        assert_eq!(plugin.underrun_count(), 0);
        plugin.process(&[], &mut output, &context).unwrap();
        assert_eq!(plugin.underrun_count(), 1);
    }

    #[test]
    fn initialize_rejects_channel_mismatch() {
        let mut plugin = stub_plugin(2);
        plugin.reader = Some(Box::new(FakeReader::new(6, vec![4])));
        assert!(
            plugin
                .initialize(48_000)
                .unwrap_err()
                .contains("Channel mismatch")
        );
    }

    #[test]
    fn replacement_reader_is_validated_before_activation() {
        let mut plugin = fake_plugin(2, vec![4]);
        assert!(
            plugin
                .activate_reader(Box::new(FakeReader::new(6, vec![4])))
                .is_err()
        );
        let mut output = vec![0.0; 8];
        plugin
            .process(&[], &mut output, &ProcessContext::new(48_000, 4))
            .unwrap();
        assert_eq!(
            output,
            (1..=8).map(|value| value as f32).collect::<Vec<_>>()
        );
    }

    #[test]
    fn process_rejects_context_rate_change_before_consuming_audio() {
        let mut plugin = fake_plugin(2, vec![4]);
        let mut output = vec![1.0; 8];
        assert!(
            plugin
                .process(&[], &mut output, &ProcessContext::new(44_100, 4))
                .unwrap_err()
                .contains("context rate")
        );
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn live_format_change_fails_before_consuming_audio() {
        let mut plugin = fake_plugin(2, vec![4]);
        let reader = plugin.reader.as_mut().unwrap();
        // Replace the negotiated reader with a changed native layout.
        *reader = Box::new(FakeReader::new(6, vec![4]));
        let mut output = vec![1.0; 8];
        assert!(
            plugin
                .process(&[], &mut output, &ProcessContext::new(48_000, 4))
                .is_err()
        );
        assert!(output.iter().all(|sample| *sample == 0.0));
        assert_eq!(plugin.diagnostics().format_change_generation, 1);
        assert!(
            plugin
                .process(&[], &mut output, &ProcessContext::new(48_000, 4))
                .is_err()
        );
        assert_eq!(plugin.diagnostics().format_change_generation, 1);
    }

    #[test]
    fn disconnect_and_key_rotation_raise_one_control_recovery_generation_per_episode() {
        let mut plugin = stub_plugin(2);
        let mut disconnected = FakeReader::new(2, vec![0]);
        disconnected.connected = false;
        plugin.reader = Some(Box::new(disconnected));
        plugin.initialize(48_000).unwrap();
        let mut output = vec![1.0; 8];
        let context = ProcessContext::new(48_000, 4);
        plugin.process(&[], &mut output, &context).unwrap();
        assert_eq!(plugin.diagnostics().recovery_generation, 1);
        assert!(plugin.diagnostics().needs_control_recovery);
        plugin.process(&[], &mut output, &context).unwrap();
        assert_eq!(plugin.diagnostics().recovery_generation, 1);

        let mut key_rotated = FakeReader::new(2, vec![0]);
        key_rotated.needs_recovery = true;
        plugin.reader = Some(Box::new(key_rotated));
        // Successful control-side replacement clears the prior episode.
        plugin.recovery_pending = false;
        plugin.process(&[], &mut output, &context).unwrap();
        assert_eq!(plugin.diagnostics().recovery_generation, 2);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn invalid_reader_frame_count_is_rejected() {
        let mut plugin = fake_plugin(2, vec![5]);
        let mut output = vec![1.0; 8];
        assert!(
            plugin
                .process(&[], &mut output, &ProcessContext::new(48_000, 4))
                .is_err()
        );
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn source_rejects_nonempty_input_and_reports_zero_graph_latency() {
        let mut plugin = fake_plugin(2, vec![4]);
        let mut output = vec![0.0; 8];
        assert!(
            plugin
                .process(&[1.0], &mut output, &ProcessContext::new(48_000, 4))
                .is_err()
        );
        assert_eq!(plugin.latency_samples(), 0);
    }

    #[test]
    fn successful_and_starved_process_paths_allocate_nothing() {
        let mut plugin = fake_plugin(16, vec![8, 0]);
        let context = ProcessContext::new(48_000, 8);
        let mut output = vec![0.0; 8 * 16];
        assert_no_allocs("HalInputPlugin full read", || {
            plugin.process(&[], &mut output, &context).unwrap();
        });
        assert_no_allocs("HalInputPlugin armed starvation", || {
            plugin.process(&[], &mut output, &context).unwrap();
        });
    }
}
