#![allow(clippy::field_reassign_with_default)]
//! Common test utilities for audio engine tests
//!
//! All tests should use a virtual audio device (BlackHole or SotF HAL driver)
//! to avoid playing sound on real audio devices during testing.

#![allow(dead_code)] // Test utilities may not be used in all test files

use hound::{WavSpec, WavWriter};
use sotf_audio::engine::EngineConfig;
use std::sync::OnceLock;
use tempfile::NamedTempFile;

/// Virtual audio device names to try (in order of preference)
/// BlackHole is preferred (most commonly installed), then SotF HAL driver
const VIRTUAL_DEVICES: &[&str] = &[
    "BlackHole 2ch",
    "BlackHole 16ch",
    "BlackHole 64ch",
    "SotF Virtual Audio",
];

/// Cached virtual device name (checked once per test run)
static VIRTUAL_DEVICE: OnceLock<Option<String>> = OnceLock::new();

/// Find an available virtual audio device.
///
/// Checks `AEQ_E2E_DEVICE` env var first (allows overriding the device),
/// then auto-detects BlackHole or SotF HAL driver.
pub fn find_virtual_device() -> Option<String> {
    // Allow explicit override via environment variable
    if let Ok(device) = std::env::var("AEQ_E2E_DEVICE") {
        if !device.is_empty() {
            return Some(device);
        }
    }

    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let devices: Vec<_> = host
        .output_devices()
        .map(|d| d.collect())
        .unwrap_or_default();

    for virtual_name in VIRTUAL_DEVICES {
        for device in &devices {
            if let Ok(desc) = device.description() {
                let name = desc.name().to_string();
                if name.contains(virtual_name) {
                    return Some(name);
                }
            }
        }
    }

    None
}

/// Get the virtual device name, panicking if not available.
///
/// This ensures all tests use a virtual audio device instead of real speakers.
/// Supports both SotF HAL driver and BlackHole.
pub fn require_virtual_device() -> String {
    VIRTUAL_DEVICE
        .get_or_init(find_virtual_device)
        .clone()
        .expect(
            "\n\n\
            ╔═══════════════════════════════════════════════════════════════════════╗\n\
            ║  AUDIO ENGINE TESTS REQUIRE A VIRTUAL AUDIO DEVICE                    ║\n\
            ╠═══════════════════════════════════════════════════════════════════════╣\n\
            ║  No virtual audio device found (BlackHole or SotF HAL).               ║\n\
            ║                                                                       ║\n\
            ║  Tests use virtual devices to avoid playing sound on real speakers.   ║\n\
            ║                                                                       ║\n\
            ║  Options:                                                             ║\n\
            ║  1. Install BlackHole: brew install blackhole-2ch                     ║\n\
            ║     or from: https://existential.audio/blackhole/                     ║\n\
            ║  2. Install the SotF HAL driver                                       ║\n\
            ║  3. Set AEQ_E2E_DEVICE='Your Device Name' to use a specific device   ║\n\
            ╚═══════════════════════════════════════════════════════════════════════╝\n\n",
        )
}

/// Backwards compatibility alias for require_virtual_device
pub fn require_blackhole_device() -> String {
    require_virtual_device()
}

/// Find an available BlackHole device (legacy alias)
pub fn find_blackhole_device() -> Option<String> {
    find_virtual_device()
}

/// Get the virtual device name as an Option for PlaybackThread tests.
pub fn virtual_device_option() -> Option<String> {
    Some(require_virtual_device())
}

/// Backwards compatibility alias
pub fn blackhole_device_option() -> Option<String> {
    virtual_device_option()
}

/// Create an EngineConfig configured for testing with a virtual audio device.
///
/// Panics if no virtual device (SotF HAL or BlackHole) is available.
pub fn test_engine_config() -> EngineConfig {
    let mut config = EngineConfig::default();
    config.output_device = Some(require_virtual_device());
    config.allow_virtual_output = true;
    config
}

/// Create an EngineConfig with specific settings, using a virtual audio device.
pub fn test_engine_config_with<F>(configure: F) -> EngineConfig
where
    F: FnOnce(&mut EngineConfig),
{
    let mut config = test_engine_config();
    configure(&mut config);
    config
}

/// Helper to create a test WAV file with a sine wave
pub fn create_test_wav(duration_secs: f32, sample_rate: u32, channels: u16) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_samples = (duration_secs * sample_rate as f32) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
        let amplitude = (sample * i16::MAX as f32 * 0.3) as i16;

        for _ in 0..channels {
            writer.write_sample(amplitude).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}

/// Helper to create a multi-channel test WAV file with distinct tones per channel
pub fn create_multichannel_test_wav(
    duration_secs: f32,
    sample_rate: u32,
    channels: u16,
) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_frames = (duration_secs * sample_rate as f32) as usize;

    // Generate different frequencies for each channel for easier identification
    let base_freq = 440.0; // A4
    for frame in 0..num_frames {
        let t = frame as f32 / sample_rate as f32;

        for ch in 0..channels {
            // Each channel gets a different frequency (440Hz, 550Hz, 660Hz, etc.)
            let freq = base_freq + (ch as f32 * 110.0);
            let sample = (t * freq * 2.0 * std::f32::consts::PI).sin();
            let amplitude = (sample * i16::MAX as f32 * 0.3) as i16;
            writer.write_sample(amplitude).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}
