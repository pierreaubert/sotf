//! Platform-agnostic audio driver abstraction.
//!
//! Defines the [`AudioDriver`] trait that all platform-specific audio capture
//! drivers must implement (macOS HAL, Linux PipeWire, Windows APO).
//!
//! Also provides [`NullDriver`] as a fallback when no platform driver is available.

use serde::{Deserialize, Serialize};

/// Status information from the audio driver.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub driver_name: String,
}

/// Configuration request for the audio driver.
#[derive(Debug, Clone)]
pub struct DriverConfig {
    /// Requested sample rate in Hz.
    pub sample_rate: u32,
    /// Requested buffer size in frames.
    pub buffer_frames: u32,
}

/// Result of a configuration request.
#[derive(Debug, Clone)]
pub enum ConfigResult {
    /// The exact requested configuration was accepted.
    Accepted,
    /// The driver negotiated different values than requested.
    Negotiated {
        actual_rate: u32,
        actual_frames: u32,
    },
    /// The configuration request failed.
    Error(String),
}

/// Platform audio capture abstraction.
///
/// Each platform implements this trait differently:
/// - **macOS**: Reads from CoreAudio HAL shared memory (`driver-hal`)
/// - **Linux**: PipeWire filter node with ring buffer (`driver-linux`, future)
/// - **Windows**: APO + shared memory (`driver-windows`, future)
///
/// The daemon holds a `Box<dyn AudioDriver>` and uses it uniformly
/// regardless of platform.
pub trait AudioDriver: Send + 'static {
    /// Initialize the driver and connect to the audio source.
    ///
    /// Returns `Ok(())` if the driver is ready, or `Err` with a description.
    /// Non-fatal issues (e.g. driver not installed) should still return `Ok(())`
    /// with `status().driver_installed == false`.
    fn initialize(&mut self) -> Result<(), String>;

    /// Shut down the driver and release resources.
    fn shutdown(&mut self);

    /// Get current driver status.
    fn status(&self) -> DriverStatus;

    /// Read interleaved audio samples into `buffer`.
    ///
    /// Returns the number of *samples* (not frames) actually read.
    /// The caller provides a buffer of `frame_count * channel_count` floats.
    fn read_audio(&mut self, buffer: &mut [f32]) -> usize;

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
    fn set_engine_ready(&mut self, ready: bool);
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
    fn initialize(&mut self) -> Result<(), String> {
        log::info!("[NullDriver] No platform audio driver available");
        Ok(())
    }

    fn shutdown(&mut self) {}

    fn status(&self) -> DriverStatus {
        DriverStatus {
            platform_supported: false,
            driver_installed: false,
            capture_active: false,
            sample_rate: 0,
            channel_count: 0,
            buffer_frames: 0,
            driver_name: "None".to_string(),
        }
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
        ConfigResult::Error("No driver available".to_string())
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
        assert_eq!(&status.driver_name, "None");
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
    }

    #[test]
    fn test_null_driver_available_frames() {
        let driver = NullDriver::new();
        assert_eq!(driver.available_frames(), 0);
    }

    #[test]
    fn test_null_driver_request_config_fails() {
        let mut driver = NullDriver::new();
        let result = driver.request_config(DriverConfig {
            sample_rate: 48000,
            buffer_frames: 1024,
        });
        assert!(matches!(result, ConfigResult::Error(_)));
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
}
