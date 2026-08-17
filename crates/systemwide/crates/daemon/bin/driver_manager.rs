//! Platform Audio Driver Manager
//!
//! Manages the lifecycle of the platform-specific audio capture driver.
//! On macOS, uses the CoreAudio HAL driver. On Linux (future), PipeWire.
//! On Windows (future), APO. Falls back to NullDriver when no driver is available.

use driver_common::{AudioDriver, ConfigResult, DriverConfig, DriverError, DriverStatus};

const DRIVER_OVERRIDE_ENV: &str = "SOTF_SYSTEMWIDE_DRIVER";

/// Driver Manager - handles platform driver lifecycle
pub struct DriverManager {
    driver: Box<dyn AudioDriver>,
    engine_ready: bool,
}

impl DriverManager {
    /// Create a new driver manager with the appropriate platform driver.
    pub fn new() -> Self {
        Self {
            driver: create_platform_driver(),
            engine_ready: false,
        }
    }

    #[cfg(test)]
    pub fn from_driver(driver: Box<dyn AudioDriver>) -> Self {
        Self {
            driver,
            engine_ready: false,
        }
    }

    /// Initialize the driver and verify connectivity.
    pub fn initialize(&mut self) -> Result<(), DriverError> {
        self.driver.initialize()
    }

    /// Shut down the driver.
    pub fn shutdown(&mut self) {
        if self.engine_ready {
            self.driver.set_engine_ready(false);
            self.engine_ready = false;
        }
        self.driver.shutdown();
    }

    /// Get current driver status.
    pub fn status(&self) -> DriverStatus {
        self.driver.status()
    }

    /// Get a mutable reference to the underlying driver.
    #[allow(dead_code)]
    pub fn driver_mut(&mut self) -> &mut dyn AudioDriver {
        &mut *self.driver
    }

    /// Get an immutable reference to the underlying driver.
    #[allow(dead_code)]
    pub fn driver(&self) -> &dyn AudioDriver {
        &*self.driver
    }

    /// Set engine_ready flag on the driver.
    pub fn set_engine_ready(&mut self, ready: bool) {
        if self.engine_ready == ready {
            return;
        }
        self.driver.set_engine_ready(ready);
        self.engine_ready = ready;
    }

    /// Poll for driver-initiated config changes.
    pub fn poll_config_change(&mut self) -> Option<DriverConfig> {
        self.driver.poll_config_change()
    }

    /// Acknowledge a config change.
    pub fn acknowledge_config_change(&mut self, actual: DriverConfig, result: ConfigResult) {
        self.driver.acknowledge_config_change(actual, result);
    }

    /// Request a configuration change.
    pub fn request_config(&mut self, config: DriverConfig) -> ConfigResult {
        self.driver.request_config(config)
    }
}

/// Create the appropriate platform audio driver.
fn create_platform_driver() -> Box<dyn AudioDriver> {
    create_platform_driver_for_choice(std::env::var(DRIVER_OVERRIDE_ENV).ok().as_deref())
}

fn create_platform_driver_for_choice(choice: Option<&str>) -> Box<dyn AudioDriver> {
    match choice.map(|value| value.trim().to_ascii_lowercase()) {
        Some(choice) if choice == "fake" || choice == "lab" => {
            log::info!("[DriverManager] Using systemwide lab fake driver");
            return Box::new(LabDriver::new());
        }
        Some(choice) if choice == "null" => {
            log::info!(
                "[DriverManager] Forcing NullDriver via {}",
                DRIVER_OVERRIDE_ENV
            );
            return Box::new(driver_common::NullDriver::new());
        }
        Some(choice) if !choice.is_empty() => {
            log::warn!(
                "[DriverManager] Ignoring unknown {}={}",
                DRIVER_OVERRIDE_ENV,
                choice
            );
        }
        _ => {}
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    {
        log::info!("[DriverManager] Creating macOS HAL driver");
        return Box::new(driver_hal::HalDriver::new());
    }

    // Future: Linux PipeWire driver
    // #[cfg(all(target_os = "linux", feature = "pipewire"))]
    // { return Box::new(driver_linux::PipeWireDriver::new()); }

    // Future: Windows APO driver
    // #[cfg(all(target_os = "windows", feature = "apo"))]
    // { return Box::new(driver_windows::WindowsDriver::new()); }

    #[allow(unreachable_code)]
    {
        log::info!("[DriverManager] No platform driver available, using NullDriver");
        Box::new(driver_common::NullDriver::new())
    }
}

#[derive(Debug)]
struct LabDriver {
    status: DriverStatus,
    phase: f32,
    engine_ready: bool,
}

impl LabDriver {
    fn new() -> Self {
        Self {
            status: DriverStatus::new(
                true,
                true,
                false,
                48_000,
                2,
                512,
                "Systemwide Lab Driver",
                true,
            ),
            phase: 0.0,
            engine_ready: false,
        }
    }
}

impl AudioDriver for LabDriver {
    fn initialize(&mut self) -> Result<(), DriverError> {
        self.status.capture_active = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.status.capture_active = false;
    }

    fn status(&self) -> DriverStatus {
        self.status.clone()
    }

    fn read_audio(&mut self, buffer: &mut [f32]) -> usize {
        if !self.status.capture_active || !self.engine_ready || self.status.sample_rate == 0 {
            buffer.fill(0.0);
            return 0;
        }

        let channels = self.status.channel_count.max(1) as usize;
        let sample_rate = self.status.sample_rate as f32;
        let step = 440.0 / sample_rate;

        let complete_samples = buffer.len() - (buffer.len() % channels);
        for frame in buffer[..complete_samples].chunks_exact_mut(channels) {
            let sample = (self.phase * std::f32::consts::TAU).sin() * 0.1;
            self.phase = (self.phase + step) % 1.0;
            for out in frame {
                *out = sample;
            }
        }

        complete_samples / channels
    }

    fn available_frames(&self) -> usize {
        if self.status.capture_active {
            self.status.buffer_frames as usize
        } else {
            0
        }
    }

    fn sample_rate(&self) -> u32 {
        self.status.sample_rate
    }

    fn channel_count(&self) -> u32 {
        self.status.channel_count
    }

    fn request_config(&mut self, config: DriverConfig) -> ConfigResult {
        if config.sample_rate != 0 {
            self.status.sample_rate = config.sample_rate;
        }
        if config.buffer_frames != 0 {
            self.status.buffer_frames = config.buffer_frames;
        }
        if config.channel_count != 0 {
            self.status.channel_count = config.channel_count;
        }
        ConfigResult::Accepted
    }

    fn poll_config_change(&mut self) -> Option<DriverConfig> {
        None
    }

    fn acknowledge_config_change(&mut self, _actual: DriverConfig, _result: ConfigResult) {}

    fn set_engine_ready(&mut self, ready: bool) {
        self.engine_ready = ready;
    }
}

/// Get driver status (convenience function matching old API)
pub fn get_driver_status(manager: &DriverManager) -> DriverStatus {
    manager.status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct CountingDriver {
        ready_calls: Arc<AtomicUsize>,
        ready: bool,
    }

    impl AudioDriver for CountingDriver {
        fn initialize(&mut self) -> Result<(), DriverError> {
            Ok(())
        }
        fn shutdown(&mut self) {}
        fn status(&self) -> DriverStatus {
            DriverStatus::new(true, true, false, 48_000, 2, 512, "counting", self.ready)
        }
        fn read_audio(&mut self, buffer: &mut [f32]) -> usize {
            buffer.fill(0.0);
            0
        }
        fn available_frames(&self) -> usize {
            0
        }
        fn sample_rate(&self) -> u32 {
            48_000
        }
        fn channel_count(&self) -> u32 {
            2
        }
        fn request_config(&mut self, _config: DriverConfig) -> ConfigResult {
            ConfigResult::Accepted
        }
        fn poll_config_change(&mut self) -> Option<DriverConfig> {
            None
        }
        fn acknowledge_config_change(&mut self, _actual: DriverConfig, _result: ConfigResult) {}
        fn set_engine_ready(&mut self, ready: bool) {
            self.ready_calls.fetch_add(1, Ordering::Relaxed);
            self.ready = ready;
        }
    }

    #[test]
    fn set_engine_ready_is_idempotent_and_shutdown_clears_state() {
        let calls = Arc::new(AtomicUsize::new(0));
        let driver = CountingDriver {
            ready_calls: Arc::clone(&calls),
            ready: false,
        };
        let mut manager = DriverManager::from_driver(Box::new(driver));

        manager.set_engine_ready(true);
        manager.set_engine_ready(true);
        manager.set_engine_ready(false);
        manager.set_engine_ready(false);
        assert_eq!(calls.load(Ordering::Relaxed), 2);

        manager.set_engine_ready(true);
        manager.shutdown();
        assert_eq!(calls.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn test_driver_manager_creation() {
        let manager = DriverManager::new();
        let status = manager.status();
        // On macOS with hal feature, platform_supported should be true
        // On other platforms, NullDriver returns false
        #[cfg(all(target_os = "macos", feature = "hal"))]
        assert!(status.platform_supported);
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        assert!(!status.platform_supported);
        let _ = status;
    }

    #[test]
    fn driver_override_can_force_lab_driver_without_installed_hal() {
        let mut driver = create_platform_driver_for_choice(Some("lab"));
        driver.initialize().expect("lab driver initializes");
        driver.set_engine_ready(true);

        let status = driver.status();
        assert_eq!(status.driver_name, "Systemwide Lab Driver");
        assert!(status.platform_supported);
        assert!(status.driver_installed);
        assert!(status.capture_active);

        let mut buffer = vec![0.0; (status.buffer_frames * status.channel_count) as usize];
        let expected_frames = buffer.len() / status.channel_count as usize;
        assert_eq!(driver.available_frames(), status.buffer_frames as usize);
        assert_eq!(driver.read_audio(&mut buffer), expected_frames);
        assert!(buffer.iter().any(|sample| *sample != 0.0));
    }

    #[test]
    fn lab_driver_never_writes_or_reports_a_partial_frame() {
        let mut driver = create_platform_driver_for_choice(Some("lab"));
        driver.initialize().expect("lab driver initializes");
        driver.set_engine_ready(true);

        let mut buffer = vec![f32::NAN; 7];
        assert_eq!(driver.read_audio(&mut buffer), 3);
        assert!(buffer[..6].iter().all(|sample| sample.is_finite()));
        assert!(
            buffer[6].is_nan(),
            "partial-frame tail must remain untouched"
        );
    }

    #[test]
    fn lab_driver_conforms_to_audio_driver_contract() {
        driver_common::test_support::assert_audio_driver_contract(LabDriver::new())
            .expect("LabDriver contract");
    }

    #[test]
    fn driver_override_can_force_null_driver() {
        let driver = create_platform_driver_for_choice(Some("null"));
        let status = driver.status();

        assert_eq!(status.driver_name, "None");
        assert!(!status.platform_supported);
    }
}
