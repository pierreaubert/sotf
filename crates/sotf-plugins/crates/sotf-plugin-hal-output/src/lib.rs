// ============================================================================
// HAL Output Plugin - Writes audio to macOS HAL driver
// ============================================================================

pub mod params;

use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(all(target_os = "macos", feature = "hal"))]
use driver_hal::HalOutputWriter;

trait HalWriter: Send {
    fn is_connected(&self) -> bool;
    fn write(&mut self, buffer: &[f32]) -> usize;
    fn current_format(&self) -> Result<(u32, u32, u32), String>;
    fn config_changed(&self) -> bool;
    fn clear_config_changed(&self);
    fn set_engine_ready(&self, ready: bool);
    fn reconnect(&mut self) -> Result<(), String>;
    fn reload_cipher(&mut self) -> Result<(), String>;
    fn encryption_key_ready(&self) -> bool;
}

#[cfg(all(target_os = "macos", feature = "hal"))]
impl HalWriter for HalOutputWriter {
    fn is_connected(&self) -> bool {
        HalOutputWriter::is_connected(self)
    }

    fn write(&mut self, buffer: &[f32]) -> usize {
        HalOutputWriter::write(self, buffer)
    }

    fn current_format(&self) -> Result<(u32, u32, u32), String> {
        HalOutputWriter::current_format(self).map_err(|error| error.to_string())
    }

    fn config_changed(&self) -> bool {
        HalOutputWriter::config_changed(self)
    }

    fn clear_config_changed(&self) {
        HalOutputWriter::clear_config_changed(self);
    }

    fn set_engine_ready(&self, ready: bool) {
        HalOutputWriter::set_engine_ready(self, ready);
    }

    fn reconnect(&mut self) -> Result<(), String> {
        HalOutputWriter::reconnect(self).map_err(|error| error.to_string())
    }

    fn reload_cipher(&mut self) -> Result<(), String> {
        HalOutputWriter::reload_cipher(self).map_err(|error| error.to_string())
    }

    fn encryption_key_ready(&self) -> bool {
        HalOutputWriter::encryption_key_ready(self)
    }
}

// Static error messages used on the audio hot path. Using constants avoids
// re-formatting a fresh `String` every time an error is reported.
const ERR_INVALID_CHANNEL_COUNT: &str = "Invalid channel count. Must be between 1 and 16";
#[cfg(all(target_os = "macos", feature = "hal"))]
const ERR_HAL_DAEMON_NOT_INITIALIZED: &str =
    "HAL driver not initialized. Ensure daemon initialized HAL before creating plugins";
#[allow(
    dead_code,
    reason = "used on non-macOS / non-hal builds; dead on macOS+hal configurations"
)]
const ERR_HAL_UNSUPPORTED_PLATFORM: &str =
    "HAL output plugin is only supported on macOS with 'hal' feature enabled";
const ERR_NO_ADJUSTABLE_PARAMETERS: &str = "HAL output has no adjustable parameters";
const ERR_HAL_WRITER_NOT_AVAILABLE: &str = "HAL writer not available";

// ============================================================================
// Configuration
// ============================================================================

/// Configuration parameters for HalOutputPlugin
pub type HalOutputPluginParams = params::Params;

/// Non-automatable transport state reported by [`HalOutputPlugin::telemetry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalOutputTransportState {
    Uninitialized,
    Ready,
    Disconnected,
    KeyMismatch,
    Backpressured,
    ConfigurationChanged,
    FormatError,
}

/// Versioned, lossless transport diagnostics. Unlike plugin parameters these
/// values are not automatable and all lifetime counters retain 64-bit range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalOutputTelemetry {
    pub version: u32,
    pub state: HalOutputTransportState,
    pub requested_frames: u64,
    pub written_frames: u64,
    pub dropped_frames: u64,
    pub queued_frames: usize,
    pub queue_capacity_frames: usize,
    pub backpressure_events: u64,
    pub connected: bool,
    pub encryption_key_ready: bool,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// HAL Output Plugin - Sink plugin that writes audio to macOS HAL driver
pub struct HalOutputPlugin {
    /// Number of input channels
    channels: usize,

    /// Counter for transport backpressure events (legacy API name retained).
    underrun_counter: Arc<AtomicU64>,

    /// Write success ratio for the last process block as a percentage (0.0–100.0).
    /// 100.0 means all samples were accepted; lower values indicate back-pressure.
    write_success_ratio: f32,

    /// Cached HAL buffer capacity in samples.
    #[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
    buffer_capacity: usize,

    /// Cached HAL connection status.
    is_connected: bool,

    /// Cached back-pressure state, useful as a diagnostic signal for downstream UI/host.
    is_backpressured: bool,

    /// Negotiated transport rate. Zero until initialization succeeds.
    sample_rate: u32,

    /// Frame-aligned unwritten tail retained after transport backpressure.
    pending: VecDeque<f32>,
    pending_capacity_samples: usize,

    state: HalOutputTransportState,
    requested_frames: u64,
    written_frames: u64,
    dropped_frames: u64,

    /// HAL output writer
    writer: Option<Box<dyn HalWriter>>,
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
            let writer =
                HalOutputWriter::new().map(|writer| Box::new(writer) as Box<dyn HalWriter>);

            if writer.is_none() {
                return Err(ERR_HAL_DAEMON_NOT_INITIALIZED.to_string());
            }

            Ok(Self {
                channels,
                underrun_counter: Arc::new(AtomicU64::new(0)),
                write_success_ratio: 100.0,
                buffer_capacity: writer
                    .as_ref()
                    .and_then(|writer| writer.current_format().ok())
                    .map(|(_, _, buffer_frames)| buffer_frames as usize)
                    .unwrap_or(0),
                is_connected: writer.as_ref().is_some_and(|writer| writer.is_connected()),
                is_backpressured: false,
                sample_rate: 0,
                pending: VecDeque::new(),
                pending_capacity_samples: 0,
                state: HalOutputTransportState::Uninitialized,
                requested_frames: 0,
                written_frames: 0,
                dropped_frames: 0,
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

    pub fn telemetry(&self) -> HalOutputTelemetry {
        HalOutputTelemetry {
            version: 1,
            state: self.state,
            requested_frames: self.requested_frames,
            written_frames: self.written_frames,
            dropped_frames: self.dropped_frames,
            queued_frames: self.pending.len() / self.channels,
            queue_capacity_frames: self.pending_capacity_samples / self.channels,
            backpressure_events: self.underrun_count(),
            connected: self.is_connected,
            encryption_key_ready: self
                .writer
                .as_ref()
                .is_some_and(|writer| writer.encryption_key_ready()),
        }
    }

    /// Perform filesystem/remapping/key work on a control thread. Audio
    /// processing never calls this method implicitly.
    pub fn service_transport(&mut self) -> Result<(), String> {
        {
            let writer = self
                .writer
                .as_mut()
                .ok_or_else(|| ERR_HAL_WRITER_NOT_AVAILABLE.to_string())?;
            writer.set_engine_ready(false);
            if !writer.is_connected() {
                writer.reconnect()?;
            }
            writer.reload_cipher()?;
        }
        self.validate_transport_format(self.sample_rate)?;
        let writer = self
            .writer
            .as_ref()
            .ok_or_else(|| ERR_HAL_WRITER_NOT_AVAILABLE.to_string())?;
        writer.clear_config_changed();
        writer.set_engine_ready(true);
        self.is_connected = writer.is_connected();
        self.state = if !self.is_connected {
            HalOutputTransportState::Disconnected
        } else if !writer.encryption_key_ready() {
            HalOutputTransportState::KeyMismatch
        } else {
            HalOutputTransportState::Ready
        };
        Ok(())
    }

    fn validate_transport_format(&mut self, sample_rate: u32) -> Result<(), String> {
        let writer = self
            .writer
            .as_ref()
            .ok_or_else(|| ERR_HAL_WRITER_NOT_AVAILABLE.to_string())?;
        let (transport_rate, transport_channels, buffer_frames) = writer.current_format()?;
        if transport_rate != sample_rate || transport_channels as usize != self.channels {
            self.state = HalOutputTransportState::FormatError;
            return Err(format!(
                "HAL output format mismatch: plugin={sample_rate} Hz/{} channels, transport={transport_rate} Hz/{transport_channels} channels",
                self.channels
            ));
        }
        self.buffer_capacity = buffer_frames as usize;
        let pending_capacity = (buffer_frames as usize)
            .checked_mul(self.channels)
            .ok_or_else(|| "HAL output pending capacity overflow".to_string())?;
        if self.pending.capacity() < pending_capacity {
            self.pending
                .reserve_exact(pending_capacity - self.pending.capacity());
        }
        self.pending_capacity_samples = pending_capacity;
        Ok(())
    }

    fn append_pending(&mut self, samples: &[f32]) -> usize {
        debug_assert_eq!(samples.len() % self.channels, 0);
        let available = self
            .pending_capacity_samples
            .saturating_sub(self.pending.len());
        let accepted = samples.len().min(available) / self.channels * self.channels;
        self.pending.extend(samples[..accepted].iter().copied());
        let dropped_frames = (samples.len() - accepted) / self.channels;
        self.dropped_frames = self.dropped_frames.saturating_add(dropped_frames as u64);
        dropped_frames
    }

    fn flush_pending(
        pending: &mut VecDeque<f32>,
        writer: &mut dyn HalWriter,
        channels: usize,
    ) -> Result<usize, String> {
        let mut total_written = 0;
        for _ in 0..2 {
            let requested_samples = {
                let (front, _) = pending.as_slices();
                front.len() / channels * channels
            };
            if requested_samples == 0 {
                break;
            }
            let written = {
                let (front, _) = pending.as_slices();
                Self::write_frames(writer, &front[..requested_samples], channels)?
            };
            pending.drain(..written * channels);
            total_written += written;
            if written * channels < requested_samples {
                break;
            }
        }
        Ok(total_written)
    }

    fn write_frames(
        writer: &mut dyn HalWriter,
        samples: &[f32],
        channels: usize,
    ) -> Result<usize, String> {
        let requested_frames = samples.len() / channels;
        let written_frames = writer.write(samples);
        if written_frames > requested_frames {
            return Err(format!(
                "HAL output writer returned {written_frames} frames for {requested_frames}-frame request"
            ));
        }
        Ok(written_frames)
    }
}

impl Drop for HalOutputPlugin {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.as_ref() {
            writer.set_engine_ready(false);
        }
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

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::boundary(PluginCostClass::External, self.latency_samples())
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err(ERR_NO_ADJUSTABLE_PARAMETERS.to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("HAL output sample rate must be non-zero".to_string());
        }
        self.validate_transport_format(sample_rate)?;
        self.sample_rate = sample_rate;
        self.pending.clear();
        if let Some(writer) = self.writer.as_ref() {
            writer.clear_config_changed();
            writer.set_engine_ready(true);
            self.is_connected = writer.is_connected();
            self.state = if !self.is_connected {
                HalOutputTransportState::Disconnected
            } else if !writer.encryption_key_ready() {
                HalOutputTransportState::KeyMismatch
            } else {
                HalOutputTransportState::Ready
            };
        }
        Ok(())
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        if self.sample_rate == 0 {
            return Err("HAL output must be initialized before processing".to_string());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "HAL output initialized at {} Hz but received {} Hz context",
                self.sample_rate, context.sample_rate
            ));
        }
        if !output.is_empty() {
            return Err(format!(
                "HAL output is a sink and requires an empty output buffer, got {} samples",
                output.len()
            ));
        }
        // Verify input buffer size
        let expected_len = context
            .num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "HAL output frame/channel count overflow".to_string())?;
        if input.len() != expected_len {
            return Err(format!(
                "Input buffer size mismatch: expected {}, got {}",
                expected_len,
                input.len()
            ));
        }

        self.requested_frames = self
            .requested_frames
            .saturating_add(context.num_frames as u64);

        if self
            .writer
            .as_ref()
            .is_some_and(|writer| writer.config_changed())
        {
            self.state = HalOutputTransportState::ConfigurationChanged;
            self.is_backpressured = true;
            self.append_pending(input);
            return Ok(context.num_frames);
        }

        let key_ready = {
            let writer = self
                .writer
                .as_ref()
                .ok_or_else(|| ERR_HAL_WRITER_NOT_AVAILABLE.to_string())?;
            self.is_connected = writer.is_connected();
            writer.encryption_key_ready()
        };
        if !self.is_connected {
            self.state = HalOutputTransportState::Disconnected;
        } else if !key_ready {
            self.state = HalOutputTransportState::KeyMismatch;
        }
        if self.state == HalOutputTransportState::Disconnected
            || self.state == HalOutputTransportState::KeyMismatch
        {
            self.append_pending(input);
            self.is_backpressured = true;
            return Ok(context.num_frames);
        }
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| ERR_HAL_WRITER_NOT_AVAILABLE.to_string())?;

        if !self.pending.is_empty() {
            let written_frames =
                Self::flush_pending(&mut self.pending, writer.as_mut(), self.channels)?;
            self.written_frames = self.written_frames.saturating_add(written_frames as u64);
            if !self.pending.is_empty() {
                self.append_pending(input);
                self.write_success_ratio = 0.0;
                self.is_backpressured = true;
                self.underrun_counter.fetch_add(1, Ordering::Relaxed);
                self.state = HalOutputTransportState::Backpressured;
                return Ok(context.num_frames);
            }
        }

        let written_frames = Self::write_frames(writer.as_mut(), input, self.channels)?;
        self.written_frames = self.written_frames.saturating_add(written_frames as u64);
        self.write_success_ratio = if context.num_frames == 0 {
            100.0
        } else {
            written_frames as f32 / context.num_frames as f32 * 100.0
        };
        self.is_backpressured = written_frames < context.num_frames;
        if self.is_backpressured {
            self.append_pending(&input[written_frames * self.channels..]);
            self.underrun_counter.fetch_add(1, Ordering::Relaxed);
            self.state = HalOutputTransportState::Backpressured;
        } else {
            self.state = HalOutputTransportState::Ready;
        }

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::{CountingAlloc, assert_no_allocs};

    #[global_allocator]
    static ALLOCATOR: CountingAlloc = CountingAlloc;

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
    fn diagnostics_are_not_automatable_parameters() {
        let plugin = make_test_plugin();
        assert!(plugin.parameters().is_empty());
        assert!(
            plugin
                .get_parameter(&ParameterId::from("underrun_count"))
                .is_none()
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
            is_backpressured: false,
            sample_rate: 48_000,
            pending: VecDeque::new(),
            pending_capacity_samples: 0,
            state: HalOutputTransportState::Uninitialized,
            requested_frames: 0,
            written_frames: 0,
            dropped_frames: 0,
            writer: None,
        }
    }

    #[derive(Default)]
    struct FakeWriterState {
        writes: Vec<Vec<f32>>,
        ready: Vec<bool>,
        clear_count: usize,
        reconnect_count: usize,
        reload_count: usize,
    }

    struct FakeWriter {
        format: (u32, u32, u32),
        connected: bool,
        changed: bool,
        writes: Vec<usize>,
        state: Arc<std::sync::Mutex<FakeWriterState>>,
    }

    impl HalWriter for FakeWriter {
        fn is_connected(&self) -> bool {
            self.connected
        }

        fn write(&mut self, buffer: &[f32]) -> usize {
            self.state.lock().unwrap().writes.push(buffer.to_vec());
            if self.writes.is_empty() {
                buffer.len() / self.format.1 as usize
            } else {
                self.writes.remove(0)
            }
        }

        fn current_format(&self) -> Result<(u32, u32, u32), String> {
            Ok(self.format)
        }

        fn config_changed(&self) -> bool {
            self.changed
        }

        fn clear_config_changed(&self) {
            self.state.lock().unwrap().clear_count += 1;
        }

        fn set_engine_ready(&self, ready: bool) {
            self.state.lock().unwrap().ready.push(ready);
        }

        fn reconnect(&mut self) -> Result<(), String> {
            self.connected = true;
            self.state.lock().unwrap().reconnect_count += 1;
            Ok(())
        }

        fn reload_cipher(&mut self) -> Result<(), String> {
            self.state.lock().unwrap().reload_count += 1;
            Ok(())
        }

        fn encryption_key_ready(&self) -> bool {
            true
        }
    }

    fn make_plugin_with_writer(
        channels: usize,
        format: (u32, u32, u32),
        writes: Vec<usize>,
    ) -> (HalOutputPlugin, Arc<std::sync::Mutex<FakeWriterState>>) {
        make_plugin_with_writer_state(channels, format, writes, true, false)
    }

    fn make_plugin_with_writer_state(
        channels: usize,
        format: (u32, u32, u32),
        writes: Vec<usize>,
        connected: bool,
        changed: bool,
    ) -> (HalOutputPlugin, Arc<std::sync::Mutex<FakeWriterState>>) {
        let state = Arc::new(std::sync::Mutex::new(FakeWriterState::default()));
        let writer = FakeWriter {
            format,
            connected,
            changed,
            writes,
            state: Arc::clone(&state),
        };
        (
            HalOutputPlugin {
                channels,
                underrun_counter: Arc::new(AtomicU64::new(0)),
                write_success_ratio: 100.0,
                buffer_capacity: format.2 as usize,
                is_connected: true,
                is_backpressured: false,
                sample_rate: 0,
                pending: VecDeque::with_capacity(format.1 as usize * format.2 as usize),
                pending_capacity_samples: format.1 as usize * format.2 as usize,
                state: HalOutputTransportState::Uninitialized,
                requested_frames: 0,
                written_frames: 0,
                dropped_frames: 0,
                writer: Some(Box::new(writer)),
            },
            state,
        )
    }

    #[test]
    fn full_multichannel_writes_are_counted_in_frames() {
        for channels in [1, 2, 6, 16] {
            let frames = 32;
            let (mut plugin, _) =
                make_plugin_with_writer(channels, (48_000, channels as u32, 256), vec![frames]);
            plugin.initialize(48_000).unwrap();
            let input = vec![0.25; frames * channels];
            let mut output = Vec::new();
            assert_eq!(
                plugin
                    .process(&input, &mut output, &ProcessContext::new(48_000, frames),)
                    .unwrap(),
                frames
            );
            assert_eq!(plugin.write_success_ratio, 100.0);
            assert!(!plugin.is_backpressured);
            assert_eq!(plugin.underrun_count(), 0);
        }
    }

    #[test]
    fn partial_write_retries_tail_before_new_audio() {
        let (mut plugin, state) = make_plugin_with_writer(2, (48_000, 2, 8), vec![2, 2, 2]);
        plugin.initialize(48_000).unwrap();
        let mut output = Vec::new();
        let first: Vec<f32> = (0..8).map(|sample| sample as f32).collect();
        plugin
            .process(&first, &mut output, &ProcessContext::new(48_000, 4))
            .unwrap();
        assert_eq!(plugin.write_success_ratio, 50.0);
        assert!(plugin.is_backpressured);

        let second: Vec<f32> = (8..12).map(|sample| sample as f32).collect();
        plugin
            .process(&second, &mut output, &ProcessContext::new(48_000, 2))
            .unwrap();

        let state = state.lock().unwrap();
        assert_eq!(state.writes[0], first);
        assert_eq!(state.writes[1], vec![4.0, 5.0, 6.0, 7.0]);
        assert_eq!(state.writes[2], second);
    }

    #[test]
    fn bounded_queue_drops_newest_complete_frames_and_preserves_oldest_order() {
        let (mut plugin, state) = make_plugin_with_writer(2, (48_000, 2, 4), vec![0, 0, 4, 4]);
        plugin.initialize(48_000).unwrap();
        let mut output = Vec::new();
        let first: Vec<f32> = (0..8).map(|sample| sample as f32).collect();
        let second: Vec<f32> = (8..16).map(|sample| sample as f32).collect();
        let third: Vec<f32> = (16..24).map(|sample| sample as f32).collect();
        let context = ProcessContext::new(48_000, 4);
        plugin.process(&first, &mut output, &context).unwrap();
        plugin.process(&second, &mut output, &context).unwrap();
        let telemetry = plugin.telemetry();
        assert_eq!(telemetry.queued_frames, 4);
        assert_eq!(telemetry.dropped_frames, 4);
        plugin.process(&third, &mut output, &context).unwrap();

        let writes = &state.lock().unwrap().writes;
        assert_eq!(writes[0], first);
        assert_eq!(writes[1], first);
        assert_eq!(writes[2], first);
        assert_eq!(writes[3], third);
        assert_eq!(plugin.telemetry().queued_frames, 0);
    }

    #[test]
    fn telemetry_preserves_counters_past_i32_range() {
        let plugin = make_test_plugin();
        let value = i32::MAX as u64 + 42;
        plugin.underrun_counter.store(value, Ordering::Relaxed);
        assert_eq!(plugin.telemetry().backpressure_events, value);
    }

    #[test]
    fn control_thread_service_reconnects_reloads_key_and_reasserts_readiness() {
        let (mut plugin, state) =
            make_plugin_with_writer_state(2, (48_000, 2, 8), vec![], false, false);
        plugin.sample_rate = 48_000;
        plugin.service_transport().unwrap();
        let snapshot = state.lock().unwrap();
        assert_eq!(snapshot.reconnect_count, 1);
        assert_eq!(snapshot.reload_count, 1);
        assert_eq!(snapshot.ready, vec![false, true]);
        assert_eq!(plugin.telemetry().state, HalOutputTransportState::Ready);
    }

    #[test]
    fn config_change_is_quiesced_until_control_thread_service() {
        let (mut plugin, state) =
            make_plugin_with_writer_state(2, (48_000, 2, 8), vec![], true, true);
        plugin.initialize(48_000).unwrap();
        let input = vec![0.0; 8];
        plugin
            .process(&input, &mut [], &ProcessContext::new(48_000, 4))
            .unwrap();
        assert_eq!(
            plugin.telemetry().state,
            HalOutputTransportState::ConfigurationChanged
        );
        assert!(state.lock().unwrap().writes.is_empty());
        plugin.service_transport().unwrap();
        assert_eq!(plugin.telemetry().state, HalOutputTransportState::Ready);
    }

    #[test]
    fn invalid_transport_frame_count_is_rejected_without_losing_writer() {
        let (mut plugin, _) = make_plugin_with_writer(2, (48_000, 2, 8), vec![5, 4]);
        plugin.initialize(48_000).unwrap();
        let context = ProcessContext::new(48_000, 4);
        assert!(plugin.process(&[0.0; 8], &mut [], &context).is_err());
        assert!(plugin.process(&[0.0; 8], &mut [], &context).is_ok());
    }

    #[test]
    fn maximum_channel_full_and_backpressured_callbacks_allocate_nothing() {
        struct NoAllocWriter {
            writes: [usize; 4],
            next: usize,
        }
        impl HalWriter for NoAllocWriter {
            fn is_connected(&self) -> bool {
                true
            }
            fn write(&mut self, _buffer: &[f32]) -> usize {
                let value = self.writes[self.next];
                self.next += 1;
                value
            }
            fn current_format(&self) -> Result<(u32, u32, u32), String> {
                Ok((48_000, 16, 8))
            }
            fn config_changed(&self) -> bool {
                false
            }
            fn clear_config_changed(&self) {}
            fn set_engine_ready(&self, _ready: bool) {}
            fn reconnect(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn reload_cipher(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn encryption_key_ready(&self) -> bool {
                true
            }
        }
        let mut plugin = make_test_plugin();
        plugin.channels = 16;
        plugin.sample_rate = 0;
        plugin.writer = Some(Box::new(NoAllocWriter {
            writes: [8, 0, 8, 8],
            next: 0,
        }));
        plugin.initialize(48_000).unwrap();
        let input = [0.25; 8 * 16];
        let context = ProcessContext::new(48_000, 8);
        assert_no_allocs("HAL output full write", || {
            plugin.process(&input, &mut [], &context).unwrap();
        });
        assert_no_allocs("HAL output queued write", || {
            plugin.process(&input, &mut [], &context).unwrap();
        });
        assert_no_allocs("HAL output queue recovery", || {
            plugin.process(&input, &mut [], &context).unwrap();
        });
    }

    #[test]
    fn initialize_rejects_transport_rate_and_channel_mismatch() {
        let (mut wrong_rate, _) = make_plugin_with_writer(2, (44_100, 2, 256), vec![]);
        assert!(wrong_rate.initialize(48_000).is_err());

        let (mut wrong_channels, _) = make_plugin_with_writer(2, (48_000, 6, 256), vec![]);
        assert!(wrong_channels.initialize(48_000).is_err());
    }

    #[test]
    fn process_validates_initialization_context_overflow_and_sink_output() {
        let (mut plugin, _) = make_plugin_with_writer(2, (48_000, 2, 256), vec![]);
        let mut output = Vec::new();
        assert!(
            plugin
                .process(&[0.0; 8], &mut output, &ProcessContext::new(48_000, 4))
                .is_err()
        );

        plugin.initialize(48_000).unwrap();
        assert!(
            plugin
                .process(&[0.0; 8], &mut output, &ProcessContext::new(44_100, 4))
                .is_err()
        );
        assert!(
            plugin
                .process(&[], &mut output, &ProcessContext::new(48_000, usize::MAX))
                .is_err()
        );
        assert!(
            plugin
                .process(&[0.0; 8], &mut [0.0], &ProcessContext::new(48_000, 4))
                .is_err()
        );
    }

    #[test]
    fn ring_capacity_is_not_reported_as_latency() {
        let (plugin, _) = make_plugin_with_writer(2, (48_000, 2, 512), vec![]);
        assert_eq!(plugin.latency_samples(), 0);
    }
}
