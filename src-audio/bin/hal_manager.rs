//! HAL Driver Manager for sotf_daemon
//!
//! This module manages the HAL driver lifecycle within the daemon process.
//! When the daemon starts, it automatically initializes the HAL audio buffer,
//! making the HAL plugins functional without any user intervention.

#[cfg(target_os = "macos")]
use sotf_hal::audio_buffer::init_global_buffer;

/// HAL Manager - handles HAL driver initialization and lifecycle
pub struct HalManager {
    #[cfg(target_os = "macos")]
    initialized: bool,
}

impl HalManager {
    /// Create a new HAL manager
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            initialized: false,
        }
    }

    /// Initialize the HAL driver and audio buffers
    ///
    /// This should be called once at daemon startup.
    /// It initializes the global audio buffer that the HAL plugins use.
    ///
    /// # Arguments
    /// * `capacity_ms` - Buffer capacity in milliseconds (default: 500ms)
    /// * `sample_rate` - Sample rate in Hz (default: 48000)
    /// * `channels` - Number of channels (default: 2 for stereo)
    pub fn initialize(&mut self, capacity_ms: usize, sample_rate: u32, channels: usize) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            if self.initialized {
                log::warn!("HAL manager already initialized");
                return Ok(());
            }

            log::info!("🎵 Initializing HAL driver...");
            log::info!("   Buffer capacity: {}ms", capacity_ms);
            log::info!("   Sample rate: {} Hz", sample_rate);
            log::info!("   Channels: {}", channels);

            // Initialize the global audio buffer
            init_global_buffer(capacity_ms, sample_rate, channels);

            self.initialized = true;
            log::info!("✅ HAL driver initialized successfully");
            log::info!("   HAL input/output plugins are now available");

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (capacity_ms, sample_rate, channels);
            log::warn!("HAL driver not available on this platform (macOS only)");
            Ok(())
        }
    }

    /// Initialize with default settings
    ///
    /// Uses:
    /// - 500ms buffer capacity
    /// - 48kHz sample rate
    /// - 2 channels (stereo)
    pub fn initialize_default(&mut self) -> Result<(), String> {
        self.initialize(500, 48000, 2)
    }

    /// Check if HAL is initialized
    pub fn is_initialized(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.initialized
        }

        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// Shutdown the HAL driver
    pub fn shutdown(&mut self) {
        #[cfg(target_os = "macos")]
        {
            if !self.initialized {
                return;
            }

            log::info!("🛑 Shutting down HAL driver...");
            sotf_hal::audio_buffer::shutdown_global_buffer();
            self.initialized = false;
            log::info!("✅ HAL driver shut down");
        }
    }
}

impl Drop for HalManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Helper function to create default HAL plugin configuration
///
/// Returns a plugin chain with HAL input and output:
/// - HalInputPlugin: Reads audio from macOS apps
/// - HalOutputPlugin: Writes processed audio back (loopback)
#[cfg(target_os = "macos")]
pub fn create_hal_plugin_chain(channels: usize) -> Vec<sotf_audio::PluginConfig> {
    use sotf_audio::PluginConfig;

    vec![
        // Input from HAL
        PluginConfig::new(
            "hal_input",
            serde_json::json!({
                "channels": channels
            }),
        ),
        // Output to HAL (loopback)
        PluginConfig::new(
            "hal_output",
            serde_json::json!({
                "channels": channels
            }),
        ),
    ]
}

/// Get HAL driver status information
pub fn get_hal_status() -> HalStatus {
    #[cfg(target_os = "macos")]
    {
        use sotf_hal::audio_buffer::get_global_buffer;

        let buffer_initialized = get_global_buffer().is_some();

        HalStatus {
            platform_supported: true,
            buffer_initialized,
            driver_installed: check_hal_driver_installed(),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        HalStatus {
            platform_supported: false,
            buffer_initialized: false,
            driver_installed: false,
        }
    }
}

/// HAL driver status
#[derive(Debug, Clone)]
pub struct HalStatus {
    /// Whether this platform supports HAL (macOS only)
    pub platform_supported: bool,
    /// Whether the audio buffer is initialized
    pub buffer_initialized: bool,
    /// Whether the HAL driver is installed as a system plugin
    pub driver_installed: bool,
}

impl HalStatus {
    /// Check if HAL is ready to use
    pub fn is_ready(&self) -> bool {
        self.platform_supported && self.buffer_initialized
    }
}

/// Check if the HAL driver is installed as a system plugin
///
/// This checks for the .driver bundle in /Library/Audio/Plug-Ins/HAL/
#[cfg(target_os = "macos")]
fn check_hal_driver_installed() -> bool {
    use std::path::Path;

    let driver_paths = [
        "/Library/Audio/Plug-Ins/HAL/AutoEQ.driver",
        "/Library/Audio/Plug-Ins/HAL/sotf_hal.driver",
    ];

    driver_paths.iter().any(|path| Path::new(path).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hal_manager_creation() {
        let manager = HalManager::new();
        assert!(!manager.is_initialized());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hal_manager_initialization() {
        let mut manager = HalManager::new();
        assert!(manager.initialize_default().is_ok());
        assert!(manager.is_initialized());
    }

    #[test]
    fn test_hal_status() {
        let status = get_hal_status();

        #[cfg(target_os = "macos")]
        assert!(status.platform_supported);

        #[cfg(not(target_os = "macos"))]
        assert!(!status.platform_supported);
    }
}
