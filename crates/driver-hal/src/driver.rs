//! [`AudioDriver`] implementation wrapping the macOS CoreAudio HAL shared memory interface.
//!
//! This adapter connects the platform-agnostic [`AudioDriver`] trait to the existing
//! [`HalInputReader`] and [`SharedAudioBuffer`] types.

use driver_common::{AudioDriver, ConfigResult, DriverConfig, DriverStatus};

use crate::shared_memory::{HalInputReader, SharedAudioBuffer};

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

        // Try to connect to shared memory
        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                log::info!(
                    "[HalDriver] Connected to shared memory: {}Hz, {}ch, {} frames",
                    buffer.sample_rate(),
                    buffer.channel_count(),
                    buffer.buffer_frames()
                );
                self.config_buffer = Some(buffer);
            }
            Err(_) => {
                log::warn!("[HalDriver] Shared memory not available yet (normal if no audio playing)");
            }
        }

        // Initialize the reader
        self.reader = HalInputReader::new();
        if self.reader.is_some() {
            log::info!("[HalDriver] HAL input reader initialized");
        } else {
            log::warn!("[HalDriver] HAL input reader not available (shared memory may not exist yet)");
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
        let (sample_rate, channel_count, buffer_frames, capture_active) =
            if let Some(ref buf) = self.config_buffer {
                (
                    buf.sample_rate(),
                    buf.channel_count(),
                    buf.buffer_frames(),
                    buf.is_active(),
                )
            } else {
                (0, 0, 0, false)
            };

        DriverStatus {
            platform_supported: true,
            driver_installed: self.driver_installed,
            capture_active,
            sample_rate,
            channel_count,
            buffer_frames,
            driver_name: "macOS CoreAudio HAL".to_string(),
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
            buf.set_actual_sample_rate(if config.sample_rate > 0 {
                config.sample_rate
            } else {
                current_rate
            });
            buf.set_actual_buffer_frames(if config.buffer_frames > 0 {
                config.buffer_frames
            } else {
                current_frames
            });
            buf.set_config_source(2); // Daemon initiated
            buf.set_config_changed();

            log::info!(
                "[HalDriver] Config request: {}Hz, {} frames",
                config.sample_rate,
                config.buffer_frames
            );
            ConfigResult::Accepted
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
            };
            log::info!(
                "[HalDriver] HAL config change detected: {}Hz, {} frames",
                config.sample_rate,
                config.buffer_frames
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
}
