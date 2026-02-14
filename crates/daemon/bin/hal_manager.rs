//! HAL Driver Manager for sotf_daemon
//!
//! This module manages the HAL driver lifecycle within the daemon process.
//! With the Swift HAL driver architecture:
//!
//! - The Swift HAL driver creates and manages shared memory at `/tmp/sotf-audio-shm`
//! - The Rust daemon connects to this shared memory for audio data exchange
//! - No explicit buffer initialization is needed from Rust side
//!
//! # Architecture
//!
//! - **Swift HAL Driver**: Installs as a macOS audio plugin, creates shared memory,
//!   captures audio from apps, and outputs processed audio
//! - **Shared Memory**: Lock-free ring buffers for bidirectional audio exchange
//! - **Rust Daemon**: Connects to shared memory via `driver_hal::SharedAudioBuffer`
//!
//! # Plugin Access
//!
//! - `HalInputPlugin`: Reads audio from shared memory (app audio captured by HAL)
//! - `HalOutputPlugin`: Writes processed audio back to shared memory (for HAL output)

#[cfg(target_os = "macos")]
use driver_hal::SharedAudioBuffer;

/// HAL Manager - handles HAL driver status and connectivity
pub struct HalManager {
    #[cfg(target_os = "macos")]
    connected: bool,
}

impl HalManager {
    /// Create a new HAL manager
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            connected: false,
        }
    }

    /// Initialize the HAL manager and verify connectivity
    ///
    /// This checks that the Swift HAL driver is installed and that we can
    /// connect to the shared memory. The Swift driver creates the shared
    /// memory, so this is a connectivity check rather than initialization.
    pub fn initialize(&mut self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            if self.connected {
                log::warn!("HAL manager already connected");
                return Ok(());
            }

            log::info!("Checking HAL driver connectivity...");

            // Check if driver is installed
            if !check_hal_driver_installed() {
                log::warn!("HAL driver not installed at /Library/Audio/Plug-Ins/HAL/");
                log::warn!("HAL plugins will not be available until driver is installed");
                // Don't fail - daemon can still run without HAL
                return Ok(());
            }

            log::info!("HAL driver is installed");

            // Try to connect to shared memory
            match SharedAudioBuffer::open_default() {
                Ok(_buffer) => {
                    self.connected = true;
                    log::info!("Connected to HAL shared memory");
                    log::info!("HAL input/output plugins are now available");
                }
                Err(_) => {
                    log::warn!("HAL shared memory not available yet");
                    log::warn!("This is normal if no app is using the HAL audio device");
                    log::warn!("HAL plugins will connect when audio starts playing");
                    // Don't fail - shared memory is created on-demand by HAL driver
                }
            }

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::warn!("HAL driver not available on this platform (macOS only)");
            Ok(())
        }
    }

    /// Shutdown the HAL manager
    ///
    /// Clears the engine_ready flag to signal the Swift HAL driver that
    /// the Rust engine is no longer processing audio.
    pub fn shutdown(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // Clear engine_ready flag so Swift HAL driver stops sending audio
            if let Ok(buffer) = SharedAudioBuffer::open_default() {
                buffer.set_engine_ready(false);
                log::info!("Cleared engine_ready flag in shared memory");
            }

            if self.connected {
                log::info!("HAL manager shutting down");
                self.connected = false;
            }
        }
    }

    /// Check if HAL manager has verified connectivity
    pub fn is_initialized(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.connected
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

// NOTE: Drop implementation intentionally omitted
// Shutdown must be called explicitly to avoid race conditions

/// Get HAL driver status information
pub fn get_hal_status() -> HalStatus {
    #[cfg(target_os = "macos")]
    {
        let shm_available = SharedAudioBuffer::open_default().is_ok();

        HalStatus {
            platform_supported: true,
            buffer_initialized: shm_available,
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
    /// Whether the shared memory is available
    pub buffer_initialized: bool,
    /// Whether the HAL driver is installed as a system plugin
    pub driver_installed: bool,
}

impl HalStatus {
    /// Check if HAL is ready to use
    pub fn is_ready(&self) -> bool {
        self.platform_supported && self.driver_installed
    }
}

/// Check if the HAL driver is installed as a system plugin
///
/// This checks for the .driver bundle in /Library/Audio/Plug-Ins/HAL/
#[cfg(target_os = "macos")]
fn check_hal_driver_installed() -> bool {
    use std::path::Path;

    let driver_paths = [
        "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver",
        // Legacy names for backward compatibility
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
    fn test_hal_status() {
        let status = get_hal_status();

        #[cfg(target_os = "macos")]
        assert!(status.platform_supported);

        #[cfg(not(target_os = "macos"))]
        assert!(!status.platform_supported);
    }
}
