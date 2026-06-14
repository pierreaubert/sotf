use super::consts::DAEMON_CONFIG_ACK_POLL_INTERVAL;
use super::consts::DAEMON_CONFIG_ACK_TIMEOUT;
use super::consts::DAEMON_HEARTBEAT_INTERVAL;
use super::misc::check_hal_driver_installed;
use super::misc::spawn_engine_heartbeat;
use crate::shared_memory::{
    DEFAULT_HAL_CHANNEL_COUNT, HalInputReader, MAX_HAL_CHANNEL_COUNT, SharedAudioBuffer,
};
use driver_common::{AudioDriver, ConfigResult, DriverConfig, DriverError, DriverStatus};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Instant;

/// macOS CoreAudio HAL driver.
///
/// Reads audio from the shared memory region created by the Swift HAL virtual device.
/// The Swift driver captures system audio and writes it to `/tmp/sotf-{uid}/audio.shm`;
/// this driver reads from that ring buffer.
pub struct HalDriver {
    pub(super) reader: Option<HalInputReader>,
    /// Separate buffer handle for config negotiation and status queries.
    /// We open this lazily and separately from the reader because the reader
    /// owns its own `SharedAudioBuffer` internally.
    pub(super) config_buffer: Option<SharedAudioBuffer>,
    pub(super) driver_installed: bool,
    pub(super) heartbeat_stop: Option<Arc<AtomicBool>>,
    pub(super) heartbeat_thread: Option<JoinHandle<()>>,
}

impl HalDriver {
    pub fn new() -> Self {
        Self {
            reader: None,
            config_buffer: None,
            driver_installed: false,
            heartbeat_stop: None,
            heartbeat_thread: None,
        }
    }

    /// Try to open (or reopen) the config buffer for status/config operations.
    pub(super) fn ensure_config_buffer(&mut self) {
        if self.config_buffer.is_none()
            && let Ok(buf) = SharedAudioBuffer::open_default()
        {
            self.config_buffer = Some(buf);
        }
    }

    pub(super) fn wait_for_daemon_config_ack(buf: &SharedAudioBuffer) -> ConfigResult {
        let deadline = Instant::now() + DAEMON_CONFIG_ACK_TIMEOUT;
        loop {
            match buf.config_status() {
                1 => return ConfigResult::Accepted,
                2 => {
                    return ConfigResult::negotiated(
                        buf.actual_sample_rate(),
                        buf.actual_buffer_frames(),
                        buf.channel_count(),
                    );
                }
                3 => {
                    return ConfigResult::error(DriverError::invalid_config(
                        "hal_config",
                        format!(
                            "HAL rejected config request (error code {})",
                            buf.config_error_code()
                        ),
                    ));
                }
                _ if Instant::now() >= deadline => {
                    return ConfigResult::error(DriverError::timeout(
                        "Timed out waiting for HAL to apply config request",
                    ));
                }
                _ => std::thread::sleep(DAEMON_CONFIG_ACK_POLL_INTERVAL),
            }
        }
    }

    pub(super) fn start_engine_heartbeat(&mut self) {
        if self.heartbeat_thread.is_some() {
            return;
        }

        self.ensure_config_buffer();
        let Some(path) = self
            .config_buffer
            .as_ref()
            .map(|buf| buf.path().to_path_buf())
        else {
            return;
        };

        match spawn_engine_heartbeat(path, DAEMON_HEARTBEAT_INTERVAL) {
            Ok((stop, handle)) => {
                self.heartbeat_stop = Some(stop);
                self.heartbeat_thread = Some(handle);
            }
            Err(e) => {
                log::warn!("[HalDriver] Failed to start daemon heartbeat thread: {}", e);
            }
        }
    }

    pub(super) fn stop_engine_heartbeat(&mut self) {
        if let Some(stop) = self.heartbeat_stop.take() {
            stop.store(true, Ordering::Release);
        }

        if let Some(handle) = self.heartbeat_thread.take() {
            handle.thread().unpark();
            if handle.join().is_err() {
                log::warn!("[HalDriver] Daemon heartbeat thread panicked");
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
    fn initialize(&mut self) -> Result<(), DriverError> {
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
        self.stop_engine_heartbeat();

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
            let requested_rate = config.sample_rate_or(current_rate);
            let requested_frames = config.buffer_frames_or(current_frames);
            let requested_channels = config.channel_count_or(current_channels);

            if requested_channels == 0 || requested_channels > MAX_HAL_CHANNEL_COUNT {
                return ConfigResult::error(DriverError::invalid_config(
                    "channel_count",
                    format!(
                        "HAL channel count must be between 1 and {}, got {}",
                        MAX_HAL_CHANNEL_COUNT, requested_channels
                    ),
                ));
            }
            if requested_channels > buf.max_channel_count() {
                return ConfigResult::error(DriverError::invalid_config(
                    "channel_count",
                    format!(
                        "HAL shared memory was sized for at most {} channels, cannot request {}",
                        buf.max_channel_count(),
                        requested_channels
                    ),
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
            ConfigResult::error(DriverError::not_available("Shared memory not available"))
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
            if ready {
                self.start_engine_heartbeat();
            } else {
                self.stop_engine_heartbeat();
            }
            log::info!("[HalDriver] engine_ready = {}", ready);
        } else {
            log::warn!("[HalDriver] Cannot set engine_ready: shared memory not available");
        }
    }
}

impl Drop for HalDriver {
    fn drop(&mut self) {
        self.stop_engine_heartbeat();
    }
}
