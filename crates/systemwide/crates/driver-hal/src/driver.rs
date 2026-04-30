//! [`AudioDriver`] implementation wrapping the macOS CoreAudio HAL shared memory interface.
//!
//! This adapter connects the platform-agnostic [`AudioDriver`] trait to the existing
//! [`HalInputReader`] and [`SharedAudioBuffer`] types.

use driver_common::{AudioDriver, ConfigResult, DriverConfig, DriverStatus};
use std::time::{Duration, Instant};

use crate::shared_memory::{
    DEFAULT_HAL_CHANNEL_COUNT, HalInputReader, MAX_HAL_CHANNEL_COUNT, SharedAudioBuffer,
};

const DAEMON_CONFIG_ACK_TIMEOUT: Duration = Duration::from_millis(750);
const DAEMON_CONFIG_ACK_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// macOS CoreAudio HAL driver.
///
/// Reads audio from the shared memory region created by the Swift HAL virtual device.
/// The Swift driver captures system audio and writes it to `/tmp/sotf-{uid}/audio.shm`;
/// this driver reads from that ring buffer.
pub struct HalDriver {
    reader: Option<HalInputReader>,
    /// Separate buffer handle for config negotiation and status queries.
    /// We open this lazily and separately from the reader because the reader
    /// owns its own `SharedAudioBuffer` internally.
    config_buffer: Option<SharedAudioBuffer>,
    driver_installed: bool,
}

impl HalDriver {
    pub fn new() -> Self {
        Self {
            reader: None,
            config_buffer: None,
            driver_installed: false,
        }
    }

    /// Try to open (or reopen) the config buffer for status/config operations.
    fn ensure_config_buffer(&mut self) {
        if self.config_buffer.is_none()
            && let Ok(buf) = SharedAudioBuffer::open_default()
        {
            self.config_buffer = Some(buf);
        }
    }

    fn wait_for_daemon_config_ack(buf: &SharedAudioBuffer) -> ConfigResult {
        let deadline = Instant::now() + DAEMON_CONFIG_ACK_TIMEOUT;
        loop {
            match buf.config_status() {
                1 => return ConfigResult::Accepted,
                2 => {
                    return ConfigResult::Negotiated {
                        actual_rate: buf.actual_sample_rate(),
                        actual_frames: buf.actual_buffer_frames(),
                    };
                }
                3 => {
                    return ConfigResult::Error(format!(
                        "HAL rejected config request (error code {})",
                        buf.config_error_code()
                    ));
                }
                _ if Instant::now() >= deadline => {
                    return ConfigResult::Error(
                        "Timed out waiting for HAL to apply config request".to_string(),
                    );
                }
                _ => std::thread::sleep(DAEMON_CONFIG_ACK_POLL_INTERVAL),
            }
        }
    }
}

impl Default for HalDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDriver for HalDriver {
    fn initialize(&mut self) -> Result<(), String> {
        log::info!("[HalDriver] Initializing macOS HAL driver");

        // Check if driver bundle is installed
        self.driver_installed = check_hal_driver_installed();
        if !self.driver_installed {
            log::warn!("[HalDriver] HAL driver not installed at /Library/Audio/Plug-Ins/HAL/");
            log::warn!("[HalDriver] HAL capture will not be available until driver is installed");
            return Ok(());
        }

        log::info!("[HalDriver] HAL driver is installed");

        // The daemon owns shared-memory creation. The HAL plugin runs inside
        // coreaudiod, so it should only have to open an already-sized file from
        // its realtime paths.
        match SharedAudioBuffer::create_or_open_default(48_000, 512, DEFAULT_HAL_CHANNEL_COUNT) {
            Ok(buffer) => {
                log::info!(
                    "[HalDriver] Prepared shared memory: {}Hz, {}ch, {} frames",
                    buffer.sample_rate(),
                    buffer.channel_count(),
                    buffer.buffer_frames()
                );
                self.config_buffer = Some(buffer);
            }
            Err(_) => {
                log::warn!(
                    "[HalDriver] Shared memory not available yet (normal if no audio playing)"
                );
            }
        }

        // Initialize the reader
        self.reader = HalInputReader::new();
        if self.reader.is_some() {
            log::info!("[HalDriver] HAL input reader initialized");
        } else {
            log::warn!(
                "[HalDriver] HAL input reader not available (shared memory may not exist yet)"
            );
        }

        Ok(())
    }

    fn shutdown(&mut self) {
        // Clear engine_ready flag
        if let Some(ref buf) = self.config_buffer {
            buf.set_engine_ready(false);
            log::info!("[HalDriver] Cleared engine_ready flag");
        }

        self.reader = None;
        self.config_buffer = None;
        log::info!("[HalDriver] Shutdown complete");
    }

    fn status(&self) -> DriverStatus {
        let (sample_rate, channel_count, buffer_frames, capture_active, driver_ready) =
            if let Some(ref buf) = self.config_buffer {
                (
                    buf.sample_rate(),
                    buf.channel_count(),
                    buf.buffer_frames(),
                    buf.is_active(),
                    buf.driver_ready(),
                )
            } else {
                (0, 0, 0, false, false)
            };

        DriverStatus {
            platform_supported: true,
            driver_installed: self.driver_installed,
            capture_active,
            sample_rate,
            channel_count,
            buffer_frames,
            driver_name: "macOS CoreAudio HAL".to_string(),
            driver_ready,
        }
    }

    fn read_audio(&mut self, buffer: &mut [f32]) -> usize {
        if let Some(ref mut reader) = self.reader {
            reader.read(buffer)
        } else {
            0
        }
    }

    fn available_frames(&self) -> usize {
        if let Some(ref reader) = self.reader {
            reader.available_read_frames()
        } else {
            0
        }
    }

    fn sample_rate(&self) -> u32 {
        if let Some(ref reader) = self.reader {
            reader.sample_rate()
        } else if let Some(ref buf) = self.config_buffer {
            buf.sample_rate()
        } else {
            0
        }
    }

    fn channel_count(&self) -> u32 {
        if let Some(ref reader) = self.reader {
            reader.channel_count()
        } else if let Some(ref buf) = self.config_buffer {
            buf.channel_count()
        } else {
            0
        }
    }

    fn request_config(&mut self, config: DriverConfig) -> ConfigResult {
        self.ensure_config_buffer();
        if let Some(ref mut buf) = self.config_buffer {
            // Use 0 as sentinel for "keep current" — don't write zero to shared memory
            let current_rate = buf.sample_rate();
            let current_frames = buf.buffer_frames();
            let current_channels = buf.channel_count();
            let requested_rate = if config.sample_rate > 0 {
                config.sample_rate
            } else {
                current_rate
            };
            let requested_frames = if config.buffer_frames > 0 {
                config.buffer_frames
            } else {
                current_frames
            };
            let requested_channels = if config.channel_count > 0 {
                config.channel_count
            } else {
                current_channels
            };

            if requested_channels == 0 || requested_channels > MAX_HAL_CHANNEL_COUNT {
                return ConfigResult::Error(format!(
                    "HAL channel count must be between 1 and {}, got {}",
                    MAX_HAL_CHANNEL_COUNT, requested_channels
                ));
            }
            if requested_channels > buf.max_channel_count() {
                return ConfigResult::Error(format!(
                    "HAL shared memory was sized for at most {} channels, cannot request {}",
                    buf.max_channel_count(),
                    requested_channels
                ));
            }

            buf.request_config_change(requested_rate, requested_frames, requested_channels, 2); // Daemon initiated

            log::info!(
                "[HalDriver] Config request: {}Hz, {} frames, {} channels",
                requested_rate,
                requested_frames,
                requested_channels
            );
            Self::wait_for_daemon_config_ack(buf)
        } else {
            ConfigResult::Error("Shared memory not available".to_string())
        }
    }

    fn poll_config_change(&mut self) -> Option<DriverConfig> {
        self.ensure_config_buffer();
        let buf = self.config_buffer.as_ref()?;

        if buf.config_changed() && buf.config_source() == 1 {
            // Clear the flag immediately to prevent re-triggering on next poll.
            // acknowledge_config_change() also clears it, but if the daemon fails
            // to acknowledge (error/panic), we'd loop forever without this.
            buf.clear_config_changed();

            // HAL-initiated change
            let config = DriverConfig {
                sample_rate: buf.requested_sample_rate(),
                buffer_frames: buf.requested_buffer_frames(),
                channel_count: buf.channel_count(),
            };
            log::info!(
                "[HalDriver] HAL config change detected: {}Hz, {} frames, {} channels",
                config.sample_rate,
                config.buffer_frames,
                config.channel_count
            );
            Some(config)
        } else {
            None
        }
    }

    fn acknowledge_config_change(&mut self, actual: DriverConfig, result: ConfigResult) {
        if let Some(ref mut buf) = self.config_buffer {
            let (status, error_code) = match result {
                ConfigResult::Accepted => (1, 0),
                ConfigResult::Negotiated { .. } => (2, 0),
                ConfigResult::Error(_) => (3, 1),
            };
            buf.acknowledge_config_change(
                actual.sample_rate,
                actual.buffer_frames,
                status,
                error_code,
            );
        }
    }

    fn set_engine_ready(&mut self, ready: bool) {
        self.ensure_config_buffer();
        if let Some(ref buf) = self.config_buffer {
            buf.set_engine_ready(ready);
            log::info!("[HalDriver] engine_ready = {}", ready);
        } else {
            log::warn!("[HalDriver] Cannot set engine_ready: shared memory not available");
        }
    }
}

/// Check if the HAL driver bundle is installed
fn check_hal_driver_installed() -> bool {
    use std::path::Path;

    let driver_paths = [
        "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver",
        "/Library/Audio/Plug-Ins/HAL/AutoEQ.driver",
        "/Library/Audio/Plug-Ins/HAL/sotf_hal.driver",
    ];

    driver_paths.iter().any(|path| Path::new(path).exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::{Duration, Instant};

    use tempfile::NamedTempFile;

    fn spawn_config_ack(path: std::path::PathBuf, status: u32) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut buffer = SharedAudioBuffer::open(&path).expect("Failed to open shared memory");
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                if buffer.config_changed() && buffer.config_source() == 2 {
                    let requested_rate = buffer.requested_sample_rate();
                    let requested_frames = buffer.requested_buffer_frames();
                    let error_code = if status == 3 { 1 } else { 0 };
                    buffer.acknowledge_config_change(
                        requested_rate,
                        requested_frames,
                        status,
                        error_code,
                    );
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
            panic!("Timed out waiting for daemon config request");
        })
    }

    #[test]
    fn test_hal_driver_creation() {
        let driver = HalDriver::new();
        assert!(driver.reader.is_none());
        assert!(driver.config_buffer.is_none());
    }

    #[test]
    fn test_hal_driver_status_before_init() {
        let driver = HalDriver::new();
        let status = driver.status();
        assert!(status.platform_supported);
        assert!(!status.driver_installed);
        assert!(!status.capture_active);
    }

    #[test]
    fn test_hal_driver_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<HalDriver>();
    }

    #[test]
    fn test_hal_driver_as_audio_driver() {
        let driver: Box<dyn AudioDriver> = Box::new(HalDriver::new());
        let status = driver.status();
        assert!(status.platform_supported);
        assert_eq!(&status.driver_name, "macOS CoreAudio HAL");
    }

    #[test]
    fn test_request_config_writes_daemon_request_fields() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2)
            .expect("Failed to create shared memory");
        let ack = spawn_config_ack(temp_file.path().to_path_buf(), 1);

        let mut driver = HalDriver::new();
        driver.config_buffer = Some(buffer);

        let result = driver.request_config(DriverConfig {
            sample_rate: 96_000,
            buffer_frames: 256,
            channel_count: 0,
        });

        assert!(matches!(result, ConfigResult::Accepted));
        ack.join().expect("Config ack thread failed");

        let buffer = driver
            .config_buffer
            .as_ref()
            .expect("Expected config buffer");
        assert!(!buffer.config_changed());
        assert_eq!(buffer.config_source(), 2);
        assert_eq!(buffer.requested_sample_rate(), 96_000);
        assert_eq!(buffer.requested_buffer_frames(), 256);
        assert_eq!(buffer.actual_sample_rate(), 96_000);
        assert_eq!(buffer.actual_buffer_frames(), 256);
        assert_eq!(buffer.config_status(), 1);
    }

    #[test]
    fn test_request_config_zero_values_keep_current_geometry() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer = SharedAudioBuffer::create_or_open(temp_file.path(), 44_100, 1_024, 2)
            .expect("Failed to create shared memory");
        let ack = spawn_config_ack(temp_file.path().to_path_buf(), 1);

        let mut driver = HalDriver::new();
        driver.config_buffer = Some(buffer);

        let result = driver.request_config(DriverConfig {
            sample_rate: 0,
            buffer_frames: 0,
            channel_count: 0,
        });

        assert!(matches!(result, ConfigResult::Accepted));
        ack.join().expect("Config ack thread failed");

        let header = driver
            .config_buffer
            .as_ref()
            .expect("Expected config buffer")
            .header();
        assert_eq!(header.requested_sample_rate, 44_100);
        assert_eq!(header.requested_buffer_frames, 1_024);
        assert_eq!(header.config_source.load(Ordering::Acquire), 2);
        assert_eq!(header.config_changed.load(Ordering::Acquire), 0);
        assert_eq!(header.config_status.load(Ordering::Acquire), 1);
    }

    #[test]
    fn test_request_config_writes_channel_count_when_capacity_allows() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer =
            SharedAudioBuffer::create_or_open_with_capacity(temp_file.path(), 48_000, 512, 2, 32)
                .expect("Failed to create shared memory");
        let ack = spawn_config_ack(temp_file.path().to_path_buf(), 1);

        let mut driver = HalDriver::new();
        driver.config_buffer = Some(buffer);

        let result = driver.request_config(DriverConfig {
            sample_rate: 48_000,
            buffer_frames: 512,
            channel_count: 10,
        });

        assert!(matches!(result, ConfigResult::Accepted));
        ack.join().expect("Config ack thread failed");

        let buffer = driver
            .config_buffer
            .as_ref()
            .expect("Expected config buffer");
        assert_eq!(buffer.channel_count(), 10);
        assert_eq!(buffer.config_source(), 2);
        assert_eq!(buffer.config_status(), 1);
    }

    #[test]
    fn test_request_config_times_out_without_hal_ack() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2)
            .expect("Failed to create shared memory");

        let mut driver = HalDriver::new();
        driver.config_buffer = Some(buffer);

        let result = driver.request_config(DriverConfig {
            sample_rate: 96_000,
            buffer_frames: 256,
            channel_count: 0,
        });

        assert!(matches!(result, ConfigResult::Error(_)));
    }
}
