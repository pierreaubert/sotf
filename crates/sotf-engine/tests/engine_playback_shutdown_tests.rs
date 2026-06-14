//! Real-device playback shutdown race tests.
//!
//! These tests exercise the full `AudioEngine` play → shutdown lifecycle on the
//! host's default audio output device. They are tagged with
//! `#[sotf_test::requires_hardware]` and skipped by default because they open
//! real (or virtual) audio hardware and can produce audible output.

#![allow(clippy::field_reassign_with_default)]

use cpal::traits::{DeviceTrait, HostTrait};
use sotf_audio::engine::{AudioEngine, EngineConfig};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Detect whether the host has any audio output device available.
///
/// Uses `cpal::default_host().default_output_device()` as requested by the
/// Phase 2.3 spec. Returns `None` if there is no default output device.
fn default_output_device_name() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.description().ok().map(|desc| desc.name().to_string()))
}

/// Build a short-engine config that targets the default output device.
fn playback_shutdown_config() -> EngineConfig {
    let mut config = EngineConfig::default();
    config.output_sample_rate = 48_000;
    config.output_channels = 2;
    config.input_channels = 2;
    config.frame_size = 256;
    config.buffer_ms = 100;
    config.allow_virtual_output = true;
    config.output_device = None;
    config
}

/// Generate a short stereo sine WAV file and return its path plus the
/// `NamedTempFile` handle. The caller must keep the handle alive so the file
/// isn't deleted before playback finishes.
fn make_short_sine_wav() -> (PathBuf, tempfile::NamedTempFile) {
    let temp = tempfile::Builder::new()
        .suffix(".wav")
        .tempfile()
        .expect("failed to create temp wav");
    let path = temp.path().to_path_buf();

    let sample_rate = 48_000;
    let channels = 2u16;
    let duration_secs = 0.1f32;
    let freq_hz = 440.0f32;
    let samples = (duration_secs * sample_rate as f32) as usize;

    let mono: Vec<f32> = (0..samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (t * freq_hz * 2.0 * std::f32::consts::PI).sin() * 0.05 // quiet
        })
        .collect();
    let interleaved: Vec<f32> = mono
        .iter()
        .flat_map(|&s| std::iter::repeat_n(s, channels as usize))
        .collect();

    sotf_testkit::audio::write_wav(&path, sample_rate, channels, &interleaved)
        .expect("failed to write sine wav");
    (path, temp)
}

#[sotf_test::requires_hardware]
#[test]
fn audio_engine_play_shutdown_race_repeated() {
    let Some(device_name) = default_output_device_name() else {
        eprintln!(
            "SKIPPED: {} — no default output device available",
            module_path!()
        );
        return;
    };
    eprintln!(
        "Running playback shutdown race test on default output device: {}",
        device_name
    );

    let config = playback_shutdown_config();
    let (wav_path, _wav_temp) = make_short_sine_wav();
    const ITERATIONS: usize = 8;
    const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

    for i in 0..ITERATIONS {
        let engine = AudioEngine::new(config.clone()).unwrap_or_else(|e| {
            panic!("iteration {}: failed to create AudioEngine: {}", i, e)
        });

        engine
            .play(wav_path.clone())
            .unwrap_or_else(|e| panic!("iteration {}: play failed: {}", i, e));

        // Give playback a moment to start before tearing it down.
        std::thread::sleep(Duration::from_millis(50));

        let start = Instant::now();
        engine
            .shutdown()
            .unwrap_or_else(|e| panic!("iteration {}: shutdown command failed: {}", i, e));
        // Dropping the engine joins the manager thread handle.
        drop(engine);
        let elapsed = start.elapsed();

        assert!(
            elapsed < SHUTDOWN_TIMEOUT,
            "iteration {}: shutdown took {:?}, expected < {:?}",
            i,
            elapsed,
            SHUTDOWN_TIMEOUT
        );
    }

    eprintln!(
        "Successfully completed {} play→shutdown cycles",
        ITERATIONS
    );
}
