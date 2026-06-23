#![allow(clippy::field_reassign_with_default)]
//! Common test utilities for audio engine tests
//!
//! All tests should use a virtual audio device (BlackHole or SotF HAL driver)
//! to avoid playing sound on real audio devices during testing.
//! Tests that need a device use `skip_without_device!()` to gracefully skip
//! when no virtual device is available (e.g. in sandboxed CI environments).

#![allow(dead_code)] // Test utilities may not be used in all test files
#![allow(unused_imports)]
#![allow(unused_macros)]

// Device discovery and engine test harness helpers are maintained in
// `sotf-testkit` so they can be reused by other crates. Re-export them here
// to keep existing `common::*` call sites working.
pub use sotf_testkit::engine::{
    find_virtual_device, get_virtual_device, require_virtual_device, test_engine_config,
    test_engine_config_with,
};
pub use sotf_testkit::find_device;
pub use sotf_testkit::skip_without_device;

use hound::{WavSpec, WavWriter};
use sotf_audio::engine::EngineConfig;
use tempfile::NamedTempFile;

/// Backwards compatibility alias for `require_virtual_device`.
pub fn require_blackhole_device() -> String {
    require_virtual_device()
}

/// Find an available BlackHole device (legacy alias).
pub fn find_blackhole_device() -> Option<String> {
    find_virtual_device()
}

/// Get the virtual device name as an Option for PlaybackThread tests.
/// Returns `None` if no virtual device is available (skips gracefully).
pub fn virtual_device_option() -> Option<String> {
    get_virtual_device()
}

/// Backwards compatibility alias.
pub fn blackhole_device_option() -> Option<String> {
    virtual_device_option()
}

/// Create an EngineConfig configured for testing with a virtual audio device.
///
/// Returns `None` if no virtual device is available.
pub fn try_test_engine_config() -> Option<EngineConfig> {
    let device = get_virtual_device()?;
    let mut config = EngineConfig::default();
    config.output_device = Some(device);
    config.allow_virtual_output = true;
    Some(config)
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
