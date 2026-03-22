//! Platform Audio Driver Manager
//!
//! Manages the lifecycle of the platform-specific audio capture driver.
//! On macOS, uses the CoreAudio HAL driver. On Linux (future), PipeWire.
//! On Windows (future), APO. Falls back to NullDriver when no driver is available.

use driver_common::{AudioDriver, ConfigResult, DriverConfig, DriverStatus};

/// Driver Manager - handles platform driver lifecycle
pub struct DriverManager {
    driver: Box<dyn AudioDriver>,
}

impl DriverManager {
    /// Create a new driver manager with the appropriate platform driver.
    pub fn new() -> Self {
        Self {
            driver: create_platform_driver(),
        }
    }

    /// Initialize the driver and verify connectivity.
    pub fn initialize(&mut self) -> Result<(), String> {
        self.driver.initialize()
    }

    /// Shut down the driver.
    pub fn shutdown(&mut self) {
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
        self.driver.set_engine_ready(ready);
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

/// Get driver status (convenience function matching old API)
pub fn get_driver_status(manager: &DriverManager) -> DriverStatus {
    manager.status()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
