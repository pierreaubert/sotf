//! Platform-agnostic audio driver abstraction.
//!
//! Defines the [`AudioDriver`] trait that all platform-specific audio capture
//! drivers must implement (macOS HAL, Linux PipeWire, Windows APO).
//!
//! Also provides [`NullDriver`] as a fallback when no platform driver is available.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Status information from the audio driver.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DriverStatus {
    /// Whether the current platform has a supported driver implementation.
    pub platform_supported: bool,
    /// Whether the driver is installed on this system.
    pub driver_installed: bool,
    /// Whether audio capture is currently active.
    pub capture_active: bool,
    /// Current sample rate in Hz.
    pub sample_rate: u32,
    /// Current channel count.
    pub channel_count: u32,
    /// Current buffer size in frames.
    pub buffer_frames: u32,
    /// Human-readable driver name (e.g. "macOS CoreAudio HAL", "PipeWire", "Windows APO").
    /// Static platform label. Keeping this borrowed avoids allocating on
    /// every status poll; platform drivers use compile-time names.
    pub driver_name: &'static str,
    /// Whether the platform driver itself reports it is ready (kernel/HAL side initialised).
    /// On platforms with no driver concept this mirrors `driver_installed`.
    pub driver_ready: bool,
}

impl DriverStatus {
    /// Construct a status snapshot without exposing the struct's full field
    /// set to downstream driver implementations.
    pub const fn new(
        platform_supported: bool,
        driver_installed: bool,
        capture_active: bool,
        sample_rate: u32,
        channel_count: u32,
        buffer_frames: u32,
        driver_name: &'static str,
        driver_ready: bool,
    ) -> Self {
        Self {
            platform_supported,
            driver_installed,
            capture_active,
            sample_rate,
            channel_count,
            buffer_frames,
            driver_name,
            driver_ready,
        }
    }
}

/// Configuration request for the audio driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DriverConfig {
    /// Requested sample rate in Hz.
    ///
    /// A value of `0` means "keep current".
    pub sample_rate: u32,
    /// Requested buffer size in frames.
    ///
    /// A value of `0` means "keep current".
    pub buffer_frames: u32,
    /// Requested driver channel count.
    ///
    /// A value of `0` means "keep current".
    pub channel_count: u32,
}

impl DriverConfig {
    /// A request that leaves every driver setting unchanged.
    pub const fn keep_current() -> Self {
        Self {
            sample_rate: 0,
            buffer_frames: 0,
            channel_count: 0,
        }
    }

    /// Create a complete configuration request.
    pub const fn new(sample_rate: u32, buffer_frames: u32, channel_count: u32) -> Self {
        Self {
            sample_rate,
            buffer_frames,
            channel_count,
        }
    }

    /// Request a specific sample rate while preserving all other settings.
    pub const fn with_sample_rate(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..Self::keep_current()
        }
    }

    /// Request a specific buffer size while preserving all other settings.
    pub const fn with_buffer_frames(buffer_frames: u32) -> Self {
        Self {
            buffer_frames,
            ..Self::keep_current()
        }
    }

    /// Request a specific channel count while preserving all other settings.
    pub const fn with_channel_count(channel_count: u32) -> Self {
        Self {
            channel_count,
            ..Self::keep_current()
        }
    }

    /// Resolve the sample-rate request against the current value.
    pub const fn sample_rate_or(self, current: u32) -> u32 {
        if self.sample_rate == 0 {
            current
        } else {
            self.sample_rate
        }
    }

    /// Resolve the buffer-size request against the current value.
    pub const fn buffer_frames_or(self, current: u32) -> u32 {
        if self.buffer_frames == 0 {
            current
        } else {
            self.buffer_frames
        }
    }

    /// Resolve the channel-count request against the current value.
    pub const fn channel_count_or(self, current: u32) -> u32 {
        if self.channel_count == 0 {
            current
        } else {
            self.channel_count
        }
    }
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self::keep_current()
    }
}

/// Structured driver error.
///
/// This intentionally stores displayable details instead of platform-native
/// error values so it can cross the daemon IPC boundary without string
/// matching at call sites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DriverError {
    /// No driver implementation or shared transport is available.
    NotAvailable { message: String },
    /// The platform driver is not installed.
    NotInstalled { message: String },
    /// The current process lacks permission to use the driver.
    PermissionDenied { message: String },
    /// A requested driver setting is invalid.
    InvalidConfig { field: String, message: String },
    /// The requested operation timed out.
    Timeout { message: String },
    /// Filesystem or device I/O failed.
    Io { message: String },
    /// Fallback for errors that do not have a stable category yet.
    Other { message: String },
}

impl DriverError {
    pub fn not_available(message: impl Into<String>) -> Self {
        Self::NotAvailable {
            message: message.into(),
        }
    }

    pub fn not_installed(message: impl Into<String>) -> Self {
        Self::NotInstalled {
            message: message.into(),
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
        }
    }

    pub fn invalid_config(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAvailable { message }
            | Self::NotInstalled { message }
            | Self::PermissionDenied { message }
            | Self::Timeout { message }
            | Self::Io { message }
            | Self::Other { message } => f.write_str(message),
            Self::InvalidConfig { field, message } => write!(f, "{}: {}", field, message),
        }
    }
}

impl std::error::Error for DriverError {}

impl From<String> for DriverError {
    fn from(message: String) -> Self {
        Self::other(message)
    }
}

impl From<&str> for DriverError {
    fn from(message: &str) -> Self {
        Self::other(message)
    }
}

impl From<std::io::Error> for DriverError {
    fn from(error: std::io::Error) -> Self {
        let message = error.to_string();
        match error.kind() {
            std::io::ErrorKind::PermissionDenied => Self::permission_denied(message),
            std::io::ErrorKind::TimedOut => Self::timeout(message),
            _ => Self::io(message),
        }
    }
}

/// Result of a configuration request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ConfigResult {
    /// The exact requested configuration was accepted.
    Accepted,
    /// The driver negotiated different values than requested.
    Negotiated {
        actual_rate: u32,
        actual_frames: u32,
        actual_channels: u32,
    },
    /// The configuration request failed.
    Error(DriverError),
}

impl ConfigResult {
    pub const fn negotiated(actual_rate: u32, actual_frames: u32, actual_channels: u32) -> Self {
        Self::Negotiated {
            actual_rate,
            actual_frames,
            actual_channels,
        }
    }

    pub fn error(error: impl Into<DriverError>) -> Self {
        Self::Error(error.into())
    }
}

/// Platform audio capture abstraction.
///
/// Each platform implements this trait differently:
/// - **macOS**: Reads from CoreAudio HAL shared memory (`driver-hal`)
/// - **Linux**: PipeWire filter node with ring buffer (`driver-linux`, future)
/// - **Windows**: APO + shared memory (`driver-windows`, future)
///
/// The daemon holds a single-owner `Box<dyn AudioDriver>` and uses it
/// uniformly regardless of platform. The trait is `Send` so ownership can move
/// to the daemon thread; it is not `Sync` because drivers own realtime handles
/// and shared-memory cursors that should not be called concurrently.
pub trait AudioDriver: Send + 'static {
    /// Initialize the driver and connect to the audio source.
    ///
    /// Returns `Ok(())` if the driver is ready, or `Err` with a description.
    /// Non-fatal issues (e.g. driver not installed) should still return `Ok(())`
    /// with `status().driver_installed == false`.
    fn initialize(&mut self) -> Result<(), DriverError>;

    /// Shut down the driver and release resources.
    ///
    /// Implementors should also release kernel/shared-memory resources from
    /// `Drop` when possible; the daemon calls this explicitly during normal
    /// shutdown, but `Drop` is the backstop for abrupt owner teardown.
    fn shutdown(&mut self);

    /// Get current driver status.
    fn status(&self) -> DriverStatus;

    /// Read interleaved audio samples into `buffer`.
    ///
    /// Returns the number of complete *frames* actually read. The caller
    /// provides a buffer of `frame_count * channel_count()` floats, and an
    /// implementation must never consume a partial interleaved frame.
    fn read_audio(&mut self, buffer: &mut [f32]) -> usize;

    /// Read interleaved audio and return complete frames.
    ///
    /// This is an explicit alias for the frame-based `read_audio` contract.
    fn read_frames(&mut self, buffer: &mut [f32]) -> usize {
        self.read_audio(buffer)
    }

    /// Number of complete frames available for reading without blocking.
    fn available_frames(&self) -> usize;

    /// Current sample rate reported by the driver.
    fn sample_rate(&self) -> u32;

    /// Current channel count reported by the driver.
    fn channel_count(&self) -> u32;

    /// Request a configuration change (sample rate, buffer size).
    fn request_config(&mut self, config: DriverConfig) -> ConfigResult;

    /// Poll for driver-initiated configuration changes.
    ///
    /// Returns `Some(config)` if the driver (e.g. HAL, PipeWire) changed
    /// its configuration and the daemon needs to reconfigure.
    fn poll_config_change(&mut self) -> Option<DriverConfig>;

    /// Acknowledge a driver-initiated config change with the actual values used.
    fn acknowledge_config_change(&mut self, actual: DriverConfig, result: ConfigResult);

    /// Signal to the driver whether the engine is ready to process audio.
    ///
    /// Calls are idempotent. Implementations must publish the ready state
    /// before audio is considered live and must clear it during shutdown;
    /// repeated calls with the same value must not create duplicate worker
    /// threads or otherwise accumulate resources.
    fn set_engine_ready(&mut self, ready: bool);
}

/// Test helpers shared by each platform driver's conformance tests.
///
/// Keeping the behavioral assertions next to the trait prevents platform
/// tests from quietly drifting apart as new implementations are added.
#[doc(hidden)]
pub mod test_support {
    use super::AudioDriver;

    /// Check the driver invariants that do not require a real audio device.
    pub fn assert_audio_driver_contract<D: AudioDriver>(mut driver: D) -> Result<(), String> {
        driver
            .initialize()
            .map_err(|error| format!("audio driver contract fixture must initialize: {error}"))?;
        driver.set_engine_ready(true);
        driver.set_engine_ready(true);

        let channels = driver.channel_count() as usize;
        let sample_count = channels.saturating_mul(3).saturating_add(1).max(1);
        let max_frames = sample_count / channels.max(1);

        let mut buffer = vec![f32::NAN; sample_count];
        let frames = driver.read_audio(&mut buffer);
        if frames > max_frames {
            return Err(format!(
                "read_audio returned {frames} frames for {sample_count} samples and {channels} channels"
            ));
        }

        let frames = driver.read_frames(&mut buffer);
        if frames > max_frames {
            return Err(format!(
                "read_frames returned {frames} frames for {sample_count} samples and {channels} channels"
            ));
        }

        let _ = driver.available_frames();
        let _ = driver.sample_rate();
        let _ = driver.status();
        assert!(driver.poll_config_change().is_none());
        driver.set_engine_ready(false);
        driver.set_engine_ready(false);
        driver.shutdown();
        Ok(())
    }
}

/// Fallback driver when no platform driver is available.
///
/// Returns `platform_supported: false` and reads zero frames.
/// This allows the daemon to compile and run on any platform,
/// gracefully degrading when no capture driver exists.
#[derive(Debug)]
pub struct NullDriver;

impl NullDriver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDriver for NullDriver {
    fn initialize(&mut self) -> Result<(), DriverError> {
        log::debug!("[NullDriver] No platform audio driver available");
        Ok(())
    }

    fn shutdown(&mut self) {}

    fn status(&self) -> DriverStatus {
        DriverStatus::new(false, false, false, 0, 0, 0, "None", false)
    }

    fn read_audio(&mut self, _buffer: &mut [f32]) -> usize {
        0
    }

    fn available_frames(&self) -> usize {
        0
    }

    fn sample_rate(&self) -> u32 {
        0
    }

    fn channel_count(&self) -> u32 {
        0
    }

    fn request_config(&mut self, _config: DriverConfig) -> ConfigResult {
        // Unsupported platforms intentionally degrade to a silent no-op. The
        // null driver has no transport to negotiate, but rejecting a benign
        // configuration request would make the fallback unusable.
        ConfigResult::Accepted
    }

    fn poll_config_change(&mut self) -> Option<DriverConfig> {
        None
    }

    fn acknowledge_config_change(&mut self, _actual: DriverConfig, _result: ConfigResult) {}

    fn set_engine_ready(&mut self, _ready: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_driver_status() {
        let driver = NullDriver::new();
        let status = driver.status();
        assert!(!status.platform_supported);
        assert!(!status.driver_installed);
        assert!(!status.capture_active);
        assert_eq!(status.driver_name, "None");
    }

    #[test]
    fn test_null_driver_initialize() {
        let mut driver = NullDriver::new();
        assert!(driver.initialize().is_ok());
    }

    #[test]
    fn test_null_driver_reads_zero() {
        let mut driver = NullDriver::new();
        let mut buf = vec![1.0f32; 1024];
        let read = driver.read_audio(&mut buf);
        assert_eq!(read, 0);
        assert_eq!(driver.read_frames(&mut buf), 0);
    }

    #[test]
    fn test_null_driver_available_frames() {
        let driver = NullDriver::new();
        assert_eq!(driver.available_frames(), 0);
    }

    #[test]
    fn test_null_driver_request_config_is_a_noop() {
        let mut driver = NullDriver::new();
        let result = driver.request_config(DriverConfig::new(48000, 1024, 0));
        assert_eq!(result, ConfigResult::Accepted);
    }

    #[test]
    fn test_null_driver_no_config_changes() {
        let mut driver = NullDriver::new();
        assert!(driver.poll_config_change().is_none());
    }

    #[test]
    fn test_null_driver_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NullDriver>();
    }

    #[test]
    fn test_audio_driver_object_safety() {
        // Verify AudioDriver can be used as trait object
        let driver: Box<dyn AudioDriver> = Box::new(NullDriver::new());
        let status = driver.status();
        assert!(!status.platform_supported);
    }

    #[test]
    fn test_driver_config_keep_current_helpers() {
        let keep = DriverConfig::default();
        assert_eq!(keep, DriverConfig::keep_current());
        assert_eq!(keep.sample_rate_or(48_000), 48_000);
        assert_eq!(keep.buffer_frames_or(512), 512);
        assert_eq!(keep.channel_count_or(2), 2);

        let channels = DriverConfig::with_channel_count(8);
        assert_eq!(channels.sample_rate_or(44_100), 44_100);
        assert_eq!(channels.buffer_frames_or(256), 256);
        assert_eq!(channels.channel_count_or(2), 8);
    }

    #[test]
    fn test_config_result_serializes_negotiated_channels() {
        let result = ConfigResult::negotiated(48_000, 512, 8);
        let json = serde_json::to_value(&result).expect("ConfigResult should serialize");
        assert_eq!(json["Negotiated"]["actual_rate"], 48_000);
        assert_eq!(json["Negotiated"]["actual_frames"], 512);
        assert_eq!(json["Negotiated"]["actual_channels"], 8);
    }

    #[test]
    fn test_driver_error_display_for_invalid_config() {
        let error = DriverError::invalid_config("channel_count", "must be 1..=32");
        assert_eq!(error.to_string(), "channel_count: must be 1..=32");
    }

    #[test]
    fn test_driver_error_display_all_variants() {
        assert_eq!(DriverError::not_available("na").to_string(), "na");
        assert_eq!(DriverError::not_installed("ni").to_string(), "ni");
        assert_eq!(DriverError::permission_denied("pd").to_string(), "pd");
        assert_eq!(DriverError::timeout("to").to_string(), "to");
        assert_eq!(DriverError::io("io").to_string(), "io");
        assert_eq!(DriverError::other("ot").to_string(), "ot");
    }

    #[test]
    fn test_driver_error_from_string_and_str_and_io() {
        let from_string: DriverError = String::from("msg").into();
        assert_eq!(from_string.to_string(), "msg");

        let from_str: DriverError = "slice".into();
        assert_eq!(from_str.to_string(), "slice");

        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let from_io: DriverError = io.into();
        assert_eq!(from_io.to_string(), "gone");

        let permission = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "blocked");
        assert!(matches!(
            DriverError::from(permission),
            DriverError::PermissionDenied { .. }
        ));
    }

    #[test]
    fn test_driver_error_equality_and_serialization() {
        let a = DriverError::timeout("slow");
        let b = DriverError::timeout("slow");
        let c = DriverError::timeout("fast");
        assert_eq!(a, b);
        assert_ne!(a, c);

        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["Timeout"]["message"], "slow");
        let back: DriverError = serde_json::from_value(json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn test_driver_config_with_helpers() {
        let sr = DriverConfig::with_sample_rate(96_000);
        assert_eq!(sr.sample_rate, 96_000);
        assert_eq!(sr.buffer_frames, 0);
        assert_eq!(sr.channel_count, 0);
        assert_eq!(sr.sample_rate_or(48_000), 96_000);
        assert_eq!(sr.buffer_frames_or(512), 512);

        let bf = DriverConfig::with_buffer_frames(256);
        assert_eq!(bf.sample_rate, 0);
        assert_eq!(bf.buffer_frames, 256);
        assert_eq!(bf.buffer_frames_or(512), 256);

        let cc = DriverConfig::with_channel_count(8);
        assert_eq!(cc.sample_rate, 0);
        assert_eq!(cc.channel_count, 8);
        assert_eq!(cc.channel_count_or(2), 8);
    }

    #[test]
    fn test_driver_config_default_and_new() {
        assert_eq!(DriverConfig::default(), DriverConfig::keep_current());
        let full = DriverConfig::new(48_000, 512, 2);
        assert_eq!(full.sample_rate_or(44_100), 48_000);
        assert_eq!(full.buffer_frames_or(256), 512);
        assert_eq!(full.channel_count_or(1), 2);
    }

    #[test]
    fn test_null_driver_default_and_acknowledges() {
        let mut driver: NullDriver = Default::default();
        driver.shutdown();
        driver.set_engine_ready(true);
        driver.acknowledge_config_change(DriverConfig::keep_current(), ConfigResult::Accepted);
        assert_eq!(driver.available_frames(), 0);
    }

    #[test]
    fn test_config_result_accepted_error_and_serialization() {
        assert_eq!(ConfigResult::Accepted, ConfigResult::Accepted);
        assert_ne!(ConfigResult::Accepted, ConfigResult::error("boom"));

        let err = ConfigResult::error("boom");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["Error"]["Other"]["message"], "boom");

        let neg = ConfigResult::negotiated(48_000, 512, 2);
        assert!(matches!(neg, ConfigResult::Negotiated { .. }));
    }

    #[test]
    fn test_null_driver_read_frames_preserves_channels() {
        let mut driver = NullDriver::new();
        let mut buf = vec![1.0f32; 10];
        assert_eq!(driver.read_frames(&mut buf), 0);
    }

    #[test]
    fn test_null_driver_conforms_to_audio_driver_contract() {
        test_support::assert_audio_driver_contract(NullDriver::new()).expect("NullDriver contract");
    }
}
