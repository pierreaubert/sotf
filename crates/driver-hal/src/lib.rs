//! Audio HAL Driver - Simplified Virtual Audio Device
//!
//! This library implements a Core Audio Hardware Abstraction Layer (HAL) driver
//! that creates a virtual audio device on macOS.
//!
//! The driver is intentionally minimal:
//! - Creates a virtual audio device that appears in macOS Sound preferences
//! - Provides bidirectional audio buffers (input from macOS, output for loopback)
//! - All audio processing and configuration handled by the audio player (src-audio)
//!
//! Data flow:
//! - Input: macOS apps → HAL device → input buffer → audio player reads
//! - Output (loopback): audio player writes → output buffer → HAL device → macOS apps

// Allow Apple's naming convention for Core Audio constants
#![allow(non_upper_case_globals)]

use std::sync::Once;

// Module declarations
pub mod api;
pub mod audio_buffer;
pub mod bridge;
pub mod hal_driver;
pub mod utils;

// Re-exports for easier use
pub use api::{BufferStats, HalAudioHandle, HalInputReader, HalOutputWriter};
pub use audio_buffer::{AudioBuffer, AudioBufferConfig, AudioBufferConsumer, AudioBufferProducer};
pub use hal_driver::HALDriver;

// Error types
pub use anyhow::{Error, Result};
pub use thiserror::Error as ThisError;

/// Custom error types for the audio driver
#[derive(ThisError, Debug)]
pub enum AudioDriverError {
    #[error("Core Audio error: {0}")]
    CoreAudio(i32),

    #[error("Audio Unit error: {0}")]
    AudioUnit(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Device error: {0}")]
    Device(String),

    #[error("Buffer error: {0}")]
    Buffer(String),
}

/// Initialize logging for the driver
static INIT: Once = Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
        log::info!(
            "================================================================================"
        );
        log::info!("🎵 Audio HAL Driver v{} Starting...", DRIVER_VERSION);
        log::info!("   Name: {}", DRIVER_NAME);
        log::info!("   Manufacturer: {}", DRIVER_MANUFACTURER);
        log::info!(
            "================================================================================"
        );
    });
}

/// Driver version information
pub const DRIVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DRIVER_NAME: &str = "AudioHALDriver";
pub const DRIVER_MANUFACTURER: &str = "Pierre";

// Core Audio HAL driver entry points (C ABI)
use coreaudio_sys::*;
use libc::c_void;

/// Main entry point called by Core Audio when loading the driver
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn AudioDriverPlugInOpen(
    driver_ref: *mut c_void,
    driver: *mut *mut AudioServerPlugInDriverInterface,
) -> OSStatus {
    // Initialize logging first
    init_logging();
    log::info!("🚀 AudioDriverPlugInOpen entry point called from Core Audio");
    let result = unsafe { bridge::audio_driver_plugin_open(driver_ref, driver) };
    log::info!("🏁 AudioDriverPlugInOpen returning: {}", result);
    result
}

/// Called when Core Audio unloads the driver
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn AudioDriverPlugInClose(
    driver: *mut AudioServerPlugInDriverInterface,
) -> OSStatus {
    log::info!("🚪 AudioDriverPlugInClose entry point called from Core Audio");
    let result = unsafe { bridge::audio_driver_plugin_close(driver) };
    log::info!("🏁 AudioDriverPlugInClose returning: {}", result);
    result
}

/// Entry point for factory function
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn AudioDriverPlugInFactory(uuid: CFUUIDRef) -> *mut c_void {
    // Initialize logging first
    init_logging();
    log::info!("🏭 AudioDriverPlugInFactory entry point called from Core Audio");
    let result = unsafe {
        bridge::audio_driver_plugin_factory(uuid as *const _ as core_foundation::uuid::CFUUIDRef)
    };
    log::info!("🏁 AudioDriverPlugInFactory returning: {:p}", result);
    result
}

/// Alias for factory function (for backward compatibility)
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn AutoEQHalFactory(uuid: CFUUIDRef) -> *mut c_void {
    log::info!("🏭 AutoEQHalFactory alias called, forwarding to AudioDriverPlugInFactory");
    unsafe { AudioDriverPlugInFactory(uuid) }
}
