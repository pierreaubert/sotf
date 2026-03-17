//! E2E Loopback Tests for Audio Engine Recording
//!
//! Tests are gated by AEQ_E2E=1 environment variable and require a loopback
//! audio device (BlackHole, SotF Virtual Audio, or specified via AEQ_E2E_DEVICE).
//!
//! Run with:
//!   AEQ_E2E=1 cargo test -p sotf-engine --no-default-features --test e2e_audio -- --test-threads=1
//!
//! Environment variables:
//!   AEQ_E2E=1              Enable e2e tests (required)
//!   AEQ_E2E_DEVICE=name    Override loopback device (auto-detects BlackHole/SotF otherwise)
//!   AEQ_E2E_SR=48000       Override sample rate for single-rate tests (default: 48000)
//!   AEQ_E2E_SEND_CH=0      Override send channel for single-channel tests (default: 0)
//!   AEQ_E2E_RECORD_CH=0    Override record channel for single-channel tests (default: 0)

use hound::{WavSpec, WavWriter};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, PlaybackState, PluginConfig};
use sotf_audio::signal_recorder::record_and_analyze;
use sotf_audio::signals::{gen_log_sweep, gen_pink_noise, gen_tone};
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

mod common;

// Serialize all e2e tests — they share a single loopback device
static DEVICE_LOCK: Mutex<()> = Mutex::new(());

// ============================================================================
// Test Configuration
// ============================================================================

fn should_run_e2e_tests() -> bool {
    env::var("AEQ_E2E").ok().as_deref() == Some("1")
}

fn get_test_device() -> Option<String> {
    env::var("AEQ_E2E_DEVICE")
        .ok()
        .or_else(common::find_blackhole_device)
}

fn require_test_device() -> String {
    get_test_device().expect("No loopback device found. Install BlackHole or set AEQ_E2E_DEVICE.")
}

struct TestConfig {
    sample_rate: u32,
    send_channel: u16,
    record_channel: u16,
}

fn get_test_config() -> TestConfig {
    TestConfig {
        sample_rate: env::var("AEQ_E2E_SR")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(48000),
        send_channel: env::var("AEQ_E2E_SEND_CH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        record_channel: env::var("AEQ_E2E_RECORD_CH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    }
}

fn test_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("e2e-tests")
}

fn write_wav_file(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV file: {}", e))?;
    for &sample in samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;
    Ok(())
}

/// Query the max channel count supported by a device (as both input and output)
fn device_max_channels(device_name: &str) -> usize {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();

    let mut max_ch = 0usize;
    for device in host.output_devices().into_iter().flatten() {
        if let Ok(desc) = device.description()
            && desc.name().contains(device_name)
            && let Ok(configs) = device.supported_output_configs()
        {
            for c in configs {
                max_ch = max_ch.max(c.channels() as usize);
            }
        }
    }
    // Also check input side (may differ)
    for device in host.input_devices().into_iter().flatten() {
        if let Ok(desc) = device.description()
            && desc.name().contains(device_name)
            && let Ok(configs) = device.supported_input_configs()
        {
            for c in configs {
                max_ch = max_ch.min(c.channels() as usize); // use the lower of in/out
            }
        }
    }
    max_ch
}

/// Query supported sample rates for a device
fn device_supported_sample_rates(device_name: &str) -> Vec<u32> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();

    let candidates = [44100, 48000, 88200, 96000, 176400, 192000];
    let mut supported = Vec::new();

    for device in host.output_devices().into_iter().flatten() {
        if let Ok(desc) = device.description()
            && desc.name().contains(device_name)
        {
            if let Ok(configs) = device.supported_output_configs() {
                let configs: Vec<_> = configs.collect();
                for &rate in &candidates {
                    let ok = configs
                        .iter()
                        .any(|c| c.min_sample_rate() <= rate && c.max_sample_rate() >= rate);
                    if ok && !supported.contains(&rate) {
                        supported.push(rate);
                    }
                }
            }
            break;
        }
    }
    supported
}

/// Run a sweep loopback on a specific channel+rate and return (mean_spl, variation)
fn sweep_loopback(
    device: &str,
    sample_rate: u32,
    send_ch: u16,
    record_ch: u16,
    tag: &str,
) -> Result<(f32, f32), String> {
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    let sweep = gen_log_sweep(20.0, 20000.0, 0.5, sample_rate, 3.0);
    let temp_wav = output_dir.join(format!("e2e_{tag}_playback.wav"));
    let recorded_wav = output_dir.join(format!("e2e_{tag}_recorded.wav"));
    let csv_file = output_dir.join(format!("e2e_{tag}_analysis.csv"));

    write_wav_file(&temp_wav, &sweep, sample_rate)?;

    record_and_analyze(
        &temp_wav,
        &recorded_wav,
        &sweep,
        sample_rate,
        &csv_file,
        send_ch,
        record_ch,
        Some(device),
        Some(device),
        None,
        Some((20.0, 20000.0)),
    )?;

    // Parse CSV and compute statistics in 100 Hz – 10 kHz
    let csv =
        std::fs::read_to_string(&csv_file).map_err(|e| format!("Failed to read CSV: {}", e))?;
    let mut spl_values = Vec::new();
    for line in csv.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2
            && let (Ok(freq), Ok(spl)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>())
            && (100.0..=10000.0).contains(&freq)
        {
            spl_values.push(spl);
        }
    }

    if spl_values.is_empty() {
        return Err("No SPL data in 100-10kHz range".to_string());
    }

    let mean = spl_values.iter().sum::<f32>() / spl_values.len() as f32;
    let min = spl_values.iter().copied().fold(f32::INFINITY, f32::min);
    let max = spl_values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let variation = max - min;

    Ok((mean, variation))
}

// ============================================================================
// Test 1: Basic Tone Loopback (recording captures signal)
// ============================================================================

#[test]
fn test_loopback_tone() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1). Set AEQ_E2E=1 to run.");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let config = get_test_config();
    let device = require_test_device();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== E2E Test: Tone Signal Loopback ===");
    println!("Device: {}", device);
    println!(
        "Sample rate: {} Hz, send ch: {}, record ch: {}",
        config.sample_rate, config.send_channel, config.record_channel
    );

    let tone = gen_tone(1000.0, 0.5, config.sample_rate, 2.0);
    let temp_wav = output_dir.join("e2e_tone_playback.wav");
    let recorded_wav = output_dir.join("e2e_tone_recorded.wav");
    let csv_file = output_dir.join("e2e_tone_analysis.csv");

    write_wav_file(&temp_wav, &tone, config.sample_rate).unwrap();

    // Tone cross-correlation may fail (periodic signal), but recording must succeed
    let result = record_and_analyze(
        &temp_wav,
        &recorded_wav,
        &tone,
        config.sample_rate,
        &csv_file,
        config.send_channel,
        config.record_channel,
        Some(device.as_str()),
        Some(device.as_str()),
        None,
        None,
    );
    if let Err(e) = &result {
        println!("  Note: Analysis failed (expected for tones): {}", e);
    }

    assert!(recorded_wav.exists(), "Recording file not created");
    let mut reader = hound::WavReader::open(&recorded_wav).unwrap();
    let samples: Vec<f32> = reader.samples().collect::<Result<Vec<_>, _>>().unwrap();

    let max_amplitude = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let non_zero_pct =
        samples.iter().filter(|&&s| s.abs() > 0.01).count() as f32 / samples.len() as f32 * 100.0;

    println!(
        "  Recorded: peak={:.4}, {:.1}% non-zero",
        max_amplitude, non_zero_pct
    );
    assert!(
        max_amplitude > 0.1,
        "Peak amplitude too low: {:.4}",
        max_amplitude
    );
    assert!(non_zero_pct > 10.0, "Too many zeros: {:.1}%", non_zero_pct);

    println!("✓ Tone loopback passed\n");
}

// ============================================================================
// Test 2: Sweep Accuracy (single channel)
// ============================================================================

#[test]
fn test_loopback_sweep_accuracy() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let config = get_test_config();
    let device = require_test_device();

    println!("\n=== E2E Test: Sweep Loopback Accuracy ===");

    let (mean_spl, variation) = sweep_loopback(
        &device,
        config.sample_rate,
        config.send_channel,
        config.record_channel,
        "sweep_accuracy",
    )
    .expect("Sweep loopback failed");

    println!(
        "  Mean SPL: {:.3} dB, Variation: {:.3} dB",
        mean_spl, variation
    );

    assert!(
        (-3.0..=3.0).contains(&mean_spl),
        "Mean SPL ({:.3} dB) should be near 0 dB for digital loopback",
        mean_spl,
    );
    assert!(
        variation < 1.0,
        "SPL variation ({:.3} dB) too high for digital loopback",
        variation,
    );

    println!("✓ Sweep accuracy passed\n");
}

// ============================================================================
// Test 3: Pink Noise (recording captures signal)
// ============================================================================

#[test]
fn test_loopback_pink_noise() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let config = get_test_config();
    let device = require_test_device();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== E2E Test: Pink Noise Loopback ===");

    let noise = gen_pink_noise(0.3, config.sample_rate, 2.0);
    let temp_wav = output_dir.join("e2e_noise_playback.wav");
    let recorded_wav = output_dir.join("e2e_noise_recorded.wav");
    let csv_file = output_dir.join("e2e_noise_analysis.csv");

    write_wav_file(&temp_wav, &noise, config.sample_rate).unwrap();

    let result = record_and_analyze(
        &temp_wav,
        &recorded_wav,
        &noise,
        config.sample_rate,
        &csv_file,
        config.send_channel,
        config.record_channel,
        Some(device.as_str()),
        Some(device.as_str()),
        None,
        None,
    );
    if let Err(e) = &result {
        println!("  Note: Analysis failed (expected for noise): {}", e);
    }

    assert!(recorded_wav.exists(), "Recording file not created");
    let mut reader = hound::WavReader::open(&recorded_wav).unwrap();
    let samples: Vec<f32> = reader.samples().collect::<Result<Vec<_>, _>>().unwrap();

    let max_amplitude = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let non_zero_pct =
        samples.iter().filter(|&&s| s.abs() > 0.01).count() as f32 / samples.len() as f32 * 100.0;

    println!(
        "  Recorded: peak={:.4}, {:.1}% non-zero",
        max_amplitude, non_zero_pct
    );
    assert!(
        max_amplitude > 0.05,
        "Peak amplitude too low: {:.4}",
        max_amplitude
    );
    assert!(non_zero_pct > 10.0, "Too many zeros: {:.1}%", non_zero_pct);

    println!("✓ Pink noise loopback passed\n");
}

// ============================================================================
// Test 4: Multi-Channel — send/record on channels 0..3, verify all identical
// ============================================================================

#[test]
fn test_loopback_multi_channel() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();
    let max_ch = device_max_channels(&device);

    println!("\n=== E2E Test: Multi-Channel Loopback ===");
    println!("Device: {} ({} channels)", device, max_ch);

    let test_channels: Vec<u16> = (0..4).filter(|&ch| (ch as usize) < max_ch).collect();
    if test_channels.len() < 2 {
        println!(
            "  Device only supports {} channels, need ≥2. Skipping.",
            max_ch
        );
        return;
    }

    println!("  Testing channels: {:?}", test_channels);
    let sample_rate = 48000u32;

    let mut results: Vec<(u16, f32, f32)> = Vec::new();

    for &ch in &test_channels {
        let tag = format!("multi_ch{}", ch);
        println!("  Channel {} ...", ch);

        match sweep_loopback(&device, sample_rate, ch, ch, &tag) {
            Ok((mean, var)) => {
                println!("    mean={:.3} dB, variation={:.3} dB", mean, var);
                results.push((ch, mean, var));
            }
            Err(e) => {
                panic!("Channel {} failed: {}", ch, e);
            }
        }
    }

    // All channels should produce similar results
    let mean_values: Vec<f32> = results.iter().map(|(_, m, _)| *m).collect();
    let global_min = mean_values.iter().copied().fold(f32::INFINITY, f32::min);
    let global_max = mean_values
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let cross_channel_variation = global_max - global_min;

    println!(
        "\n  Cross-channel mean SPL spread: {:.3} dB",
        cross_channel_variation
    );

    // Each channel's transfer function should be near 0 dB
    for &(ch, mean, var) in &results {
        assert!(
            (-3.0..=3.0).contains(&mean),
            "Ch{}: mean SPL {:.3} dB outside [-3, 3] range",
            ch,
            mean,
        );
        assert!(
            var < 1.0,
            "Ch{}: variation {:.3} dB too high (expected < 1 dB)",
            ch,
            var,
        );
    }

    // Channels should agree with each other (digital loopback = identical)
    assert!(
        cross_channel_variation < 1.0,
        "Cross-channel spread {:.3} dB too high (expected < 1 dB)",
        cross_channel_variation,
    );

    println!(
        "✓ Multi-channel loopback passed ({} channels)\n",
        results.len()
    );
}

// ============================================================================
// Test 5: Multi-Sample-Rate — verify sweep works at all device-supported rates
// ============================================================================

#[test]
fn test_loopback_multi_sample_rate() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();
    let supported_rates = device_supported_sample_rates(&device);

    println!("\n=== E2E Test: Multi-Sample-Rate Loopback ===");
    println!("Device: {}", device);
    println!("Supported rates: {:?}", supported_rates);

    if supported_rates.is_empty() {
        println!("  No supported rates detected. Skipping.");
        return;
    }

    let mut results: Vec<(u32, f32, f32)> = Vec::new();

    for &rate in &supported_rates {
        let tag = format!("sr_{}", rate);
        println!("  {} Hz ...", rate);

        match sweep_loopback(&device, rate, 0, 0, &tag) {
            Ok((mean, var)) => {
                println!("    mean={:.3} dB, variation={:.3} dB", mean, var);
                results.push((rate, mean, var));
            }
            Err(e) => {
                // Some high sample rates may not be fully functional on all devices
                println!("    FAILED: {} (continuing)", e);
            }
        }
    }

    assert!(
        !results.is_empty(),
        "No sample rates worked at all! Tested: {:?}",
        supported_rates,
    );

    println!("\n  Results:");
    for &(rate, mean, var) in &results {
        println!(
            "    {:>6} Hz: mean={:+.3} dB, var={:.3} dB",
            rate, mean, var
        );
    }

    // Each successful rate should produce flat, near-0 dB transfer function
    for &(rate, mean, var) in &results {
        assert!(
            (-3.0..=3.0).contains(&mean),
            "{}Hz: mean SPL {:.3} dB outside [-3, 3] range",
            rate,
            mean,
        );
        assert!(
            var < 1.0,
            "{}Hz: variation {:.3} dB too high (expected < 1 dB)",
            rate,
            var,
        );
    }

    // All rates should agree with each other
    let mean_values: Vec<f32> = results.iter().map(|(_, m, _)| *m).collect();
    let global_min = mean_values.iter().copied().fold(f32::INFINITY, f32::min);
    let global_max = mean_values
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let cross_rate_variation = global_max - global_min;

    println!(
        "  Cross-rate mean SPL spread: {:.3} dB",
        cross_rate_variation
    );
    assert!(
        cross_rate_variation < 2.0,
        "Cross-rate spread {:.3} dB too high (expected < 2 dB)",
        cross_rate_variation,
    );

    println!(
        "✓ Multi-sample-rate passed ({}/{} rates)\n",
        results.len(),
        supported_rates.len()
    );
}

// ============================================================================
// Test 6: Upmixer plugin insertion and parameter change during playback
// ============================================================================

/// Create a stereo pink noise WAV file for playback tests
fn create_stereo_pink_noise_wav(
    duration_secs: f32,
    sample_rate: u32,
) -> (PathBuf, tempfile::NamedTempFile) {
    let left = gen_pink_noise(0.3, sample_rate, duration_secs);
    let right = gen_pink_noise(0.3, sample_rate, duration_secs);

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();
    for i in 0..left.len() {
        writer.write_sample(left[i]).unwrap();
        writer.write_sample(right[i]).unwrap();
    }
    writer.finalize().unwrap();

    (temp_file.path().to_path_buf(), temp_file)
}

/// Upmixer speaker configurations to test, with their expected output channel counts
const UPMIXER_CONFIGS: &[(&str, usize)] = &[
    ("5.0", 5),
    ("5.1", 6),
    ("7.1", 8),
    ("5.1.4", 10),
    ("7.1.4", 12),
];

fn upmixer_plugin(speaker_config: &str) -> PluginConfig {
    PluginConfig::new(
        "upmixer",
        json!({
            "speaker_config": speaker_config,
        }),
    )
}

#[test]
fn test_upmixer_insert_and_config_change_during_playback() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();
    let max_ch = device_max_channels(&device);

    println!("\n=== E2E Test: Upmixer Insert & Config Change During Playback ===");
    println!("Device: {} ({} channels)", device, max_ch);

    // Create a long stereo pink noise file (enough for all config changes)
    let sample_rate = 48000u32;
    let (wav_path, _temp_file) = create_stereo_pink_noise_wav(20.0, sample_rate);

    // Start engine with stereo output, no plugins
    let config = common::test_engine_config_with(|c| {
        c.output_channels = 2;
    });
    let engine = AudioEngine::new(config).unwrap();
    engine.play(&wav_path).unwrap();

    std::thread::sleep(Duration::from_millis(300));
    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Should be playing after start"
    );
    let pos_start = state.position;
    println!(
        "  Stereo playback started: pos={:.2}s, channels={}",
        pos_start, state.num_channels
    );

    // Step 1: Cycle through upmixer configs, verifying playback continues after each change
    // Note: state.num_channels reflects the *hardware* output (may differ from plugin chain
    // output if the device adjusts, e.g., BlackHole 64ch always uses 64ch). The key checks
    // are: update succeeds, playback continues (position advances), no errors.
    let mut prev_pos = pos_start;

    for &(speaker_config, expected_channels) in UPMIXER_CONFIGS {
        println!(
            "\n  --- Switching to {} ({} ch) ---",
            speaker_config, expected_channels
        );

        engine
            .update_plugin_chain(vec![upmixer_plugin(speaker_config)])
            .unwrap_or_else(|e| panic!("Failed to switch to {}: {}", speaker_config, e));

        // Wait for config change to take effect
        std::thread::sleep(Duration::from_millis(500));

        let state = engine.get_state();
        assert_eq!(
            state.playback_state,
            PlaybackState::Playing,
            "{}: playback should continue",
            speaker_config
        );

        let pos_now = state.position;
        assert!(
            pos_now > prev_pos,
            "{}: position should advance: {:.2} > {:.2}",
            speaker_config,
            pos_now,
            prev_pos
        );

        assert!(
            state.last_error.is_none(),
            "{}: unexpected error: {:?}",
            speaker_config,
            state.last_error
        );

        println!(
            "  {}: pos={:.2}s, hw_ch={}, no errors, OK",
            speaker_config, pos_now, state.num_channels
        );
        prev_pos = pos_now;
    }

    // Step 2: Remove upmixer (back to stereo passthrough)
    println!("\n  --- Removing upmixer (stereo passthrough) ---");
    engine
        .update_plugin_chain(vec![])
        .expect("Failed to remove plugins");

    std::thread::sleep(Duration::from_millis(500));
    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Should still play after removing upmixer"
    );
    let pos_final = state.position;
    assert!(
        pos_final > prev_pos,
        "Position should advance after removing upmixer: {:.2} > {:.2}",
        pos_final,
        prev_pos
    );
    println!(
        "  Stereo restored: pos={:.2}s, hw_ch={}, OK",
        pos_final, state.num_channels
    );

    println!(
        "\n✓ Upmixer test passed: {} configs + stereo restore, playback continuous from {:.2}s to {:.2}s\n",
        UPMIXER_CONFIGS.len(),
        pos_start,
        pos_final
    );
}

// ============================================================================
// Test 7: Sequential songs with upmixer (simulates TUI auto-advance)
// ============================================================================

#[test]
fn test_upmixer_sequential_songs() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();

    println!("\n=== E2E Test: Sequential Songs with Upmixer ===");
    println!("Device: {}", device);

    let sample_rate = 48000u32;
    let upmixer = upmixer_plugin("5.1");

    // Simulate TUI flow: AudioEngineManager + load_file + start_playback
    use sotf_audio::AudioEngineManager;

    // --- Song 1 ---
    println!("\n  --- Song 1 ---");
    let (wav1, _tmp1) = create_stereo_pink_noise_wav(3.0, sample_rate);

    let mut manager = AudioEngineManager::new();
    manager.set_allow_virtual_output(true);
    manager.load_file(&wav1).unwrap();

    // The TUI calculates output_channels from the plugin chain.
    // With upmixer 5.1, output = 6. But device may only support 2.
    // The TUI clamps to device max. For our test, just use 6 directly
    // (the engine/playback thread handles downmix).
    manager
        .start_playback(Some(device.clone()), vec![upmixer.clone()], 6)
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));
    let state1 = manager.get_engine_state();
    assert_eq!(
        state1.playback_state,
        PlaybackState::Playing,
        "Song 1 should be playing"
    );
    let pos1 = state1.position;
    println!(
        "  Song 1 playing: pos={:.2}s, ch={}",
        pos1, state1.num_channels
    );
    assert!(pos1 > 0.0, "Song 1 position should advance");

    // --- Stop and start Song 2 (simulates TUI auto-advance) ---
    println!("\n  --- Stopping Song 1, starting Song 2 ---");
    manager.stop().unwrap();

    let (wav2, _tmp2) = create_stereo_pink_noise_wav(3.0, sample_rate);
    manager.load_file(&wav2).unwrap();
    manager
        .start_playback(Some(device.clone()), vec![upmixer.clone()], 6)
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));
    let state2 = manager.get_engine_state();
    assert_eq!(
        state2.playback_state,
        PlaybackState::Playing,
        "Song 2 should be playing after transition"
    );
    let pos2 = state2.position;
    println!(
        "  Song 2 playing: pos={:.2}s, ch={}",
        pos2, state2.num_channels
    );
    assert!(pos2 > 0.0, "Song 2 position should advance");

    // --- Stop and start Song 3 (one more transition) ---
    println!("\n  --- Stopping Song 2, starting Song 3 ---");
    manager.stop().unwrap();

    let (wav3, _tmp3) = create_stereo_pink_noise_wav(3.0, sample_rate);
    manager.load_file(&wav3).unwrap();
    manager
        .start_playback(Some(device.clone()), vec![upmixer.clone()], 6)
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));
    let state3 = manager.get_engine_state();
    assert_eq!(
        state3.playback_state,
        PlaybackState::Playing,
        "Song 3 should be playing after second transition"
    );
    let pos3 = state3.position;
    println!(
        "  Song 3 playing: pos={:.2}s, ch={}",
        pos3, state3.num_channels
    );
    assert!(pos3 > 0.0, "Song 3 position should advance");

    manager.stop().unwrap();

    println!("\n✓ Sequential songs with upmixer passed (3 songs)\n");
}

// ============================================================================
// Test 8: Dynamic upmixer add/remove (simulates TUI plugin update)
// ============================================================================

#[test]
fn test_upmixer_dynamic_add_remove() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();

    println!("\n=== E2E Test: Dynamic Upmixer Add/Remove ===");
    println!("Device: {}", device);

    let sample_rate = 48000u32;
    let (wav_path, _tmp) = create_stereo_pink_noise_wav(10.0, sample_rate);

    use sotf_audio::AudioEngineManager;

    let mut manager = AudioEngineManager::new();
    manager.set_allow_virtual_output(true);
    manager.load_file(&wav_path).unwrap();

    // Start with NO plugins (stereo passthrough)
    manager
        .start_playback(Some(device.clone()), vec![], 2)
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));
    let state = manager.get_engine_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    let pos_before = state.position;
    println!("  Stereo playing: pos={:.2}s", pos_before);

    // Dynamically add upmixer (simulates TUI plugin update)
    println!("  Adding upmixer 5.1...");
    manager
        .update_plugin_chain(vec![upmixer_plugin("5.1")])
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));
    let state = manager.get_engine_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Should still be playing after adding upmixer"
    );
    let pos_after_add = state.position;
    assert!(
        pos_after_add > pos_before,
        "Position should advance after adding upmixer: {:.2} > {:.2}",
        pos_after_add,
        pos_before
    );
    println!(
        "  After upmixer add: pos={:.2}s, ch={}",
        pos_after_add, state.num_channels
    );

    // Remove upmixer
    println!("  Removing upmixer...");
    manager.update_plugin_chain(vec![]).unwrap();

    std::thread::sleep(Duration::from_millis(500));
    let state = manager.get_engine_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Should still be playing after removing upmixer"
    );
    let pos_after_remove = state.position;
    assert!(
        pos_after_remove > pos_after_add,
        "Position should advance after removing upmixer: {:.2} > {:.2}",
        pos_after_remove,
        pos_after_add
    );
    println!(
        "  After upmixer remove: pos={:.2}s, ch={}",
        pos_after_remove, state.num_channels
    );

    manager.stop().unwrap();

    println!("\n✓ Dynamic upmixer add/remove passed\n");
}

// ============================================================================
// Test 9: Stress test — rapid track changes with natural end-of-stream
//
// Simulates TUI auto-advance: short tracks play to completion (natural EOS),
// then the next track starts immediately. This exercises the critical path
// where the playback thread has already exited but the manager is still alive.
// The bug this catches: sending commands (SetVolume, Stop) to a dead playback
// thread after EOS drain would return "sending on a closed channel" errors.
// ============================================================================

#[test]
fn test_stress_rapid_track_changes_with_natural_eos() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();
    let sample_rate = 48000u32;

    println!("\n=== E2E Test: Stress Rapid Track Changes (Natural EOS) ===");
    println!("Device: {}", device);

    use sotf_audio::AudioEngineManager;
    use sotf_audio::manager::StreamingState;

    // Configurations to cycle through: (plugins, output_channels, label)
    let configs: Vec<(Vec<PluginConfig>, usize, &str)> = vec![
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("5.1")], 6, "upmixer 5.1"),
        (vec![upmixer_plugin("5.0")], 5, "upmixer 5.0"),
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("7.1")], 8, "upmixer 7.1"),
        (vec![upmixer_plugin("5.1.4")], 10, "upmixer 5.1.4"),
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("5.1")], 6, "upmixer 5.1"),
    ];

    let num_tracks = configs.len();

    // Create short audio files (1.5s each — short enough to end quickly, long enough
    // to verify playback started).
    let track_duration = 1.5;
    let tracks: Vec<(PathBuf, tempfile::NamedTempFile)> = (0..num_tracks)
        .map(|_| create_stereo_pink_noise_wav(track_duration, sample_rate))
        .collect();

    let mut manager = AudioEngineManager::new();
    manager.set_allow_virtual_output(true);

    for (i, ((plugins, output_channels, label), (wav_path, _tmp))) in
        configs.iter().zip(tracks.iter()).enumerate()
    {
        println!(
            "\n  --- Track {}/{}: {} ({} ch) ---",
            i + 1,
            num_tracks,
            label,
            output_channels
        );

        // Load and start playback
        manager.load_file(wav_path).unwrap();
        manager
            .start_playback(Some(device.clone()), plugins.clone(), *output_channels)
            .unwrap_or_else(|e| panic!("Track {}: start_playback failed: {}", i + 1, e));

        // Verify playback started
        std::thread::sleep(Duration::from_millis(200));
        let state = manager.get_engine_state();
        assert_eq!(
            state.playback_state,
            PlaybackState::Playing,
            "Track {}: should be playing",
            i + 1
        );
        assert!(
            state.position > 0.0,
            "Track {}: position should advance",
            i + 1
        );
        println!(
            "    Playing: pos={:.2}s, ch={}",
            state.position, state.num_channels
        );

        // Wait for natural end-of-stream (track plays to completion)
        let eos_timeout = Duration::from_secs(10);
        let eos_start = std::time::Instant::now();
        let mut got_eos = false;
        while eos_start.elapsed() < eos_timeout {
            // Drain events like the TUI does
            let events = manager.drain_events();
            for event in &events {
                if matches!(
                    event,
                    sotf_audio::manager::StreamingEvent::EndOfStream
                        | sotf_audio::manager::StreamingEvent::Error(_)
                ) {
                    got_eos = true;
                }
            }
            if got_eos {
                break;
            }
            // Also check state transition (Playing -> Stopped -> Idle via drain_events)
            if manager.get_state() == StreamingState::Idle {
                got_eos = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            got_eos,
            "Track {}: timed out waiting for end-of-stream",
            i + 1
        );
        println!("    End of stream detected");

        // === Simulate TUI auto-advance ===
        // The TUI calls set_volume() before load_and_play(). This is the critical
        // path: the old engine's playback thread has exited after EOS drain, so
        // SetVolume must not fail with "sending on a closed channel".
        let volume = 0.5 + (i as f32 * 0.05);
        manager
            .set_volume(volume)
            .unwrap_or_else(|e| panic!("Track {}: set_volume after EOS failed: {}", i + 1, e));

        // Stop the old engine (also hits the dead playback thread)
        manager
            .stop()
            .unwrap_or_else(|e| panic!("Track {}: stop after EOS failed: {}", i + 1, e));

        println!("    Stopped OK, advancing to next track");
    }

    println!(
        "\n✓ Stress rapid track changes passed ({} tracks with natural EOS)\n",
        num_tracks
    );
}

// ============================================================================
// Test 10: Stress test — rapid track changes with explicit stop (no EOS wait)
//
// Simulates the user rapidly skipping tracks: stop mid-playback, immediately
// start the next track. Cycles through different plugin configs to exercise
// channel count changes.
// ============================================================================

#[test]
fn test_stress_rapid_skip_with_plugin_changes() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }
    let _ = env_logger::try_init();
    let _lock = DEVICE_LOCK.lock().unwrap();

    let device = require_test_device();
    let sample_rate = 48000u32;

    println!("\n=== E2E Test: Stress Rapid Skip with Plugin Changes ===");
    println!("Device: {}", device);

    use sotf_audio::AudioEngineManager;

    let configs: Vec<(Vec<PluginConfig>, usize, &str)> = vec![
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("5.1")], 6, "5.1"),
        (vec![upmixer_plugin("7.1")], 8, "7.1"),
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("5.1.4")], 10, "5.1.4"),
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("5.1")], 6, "5.1"),
        (vec![upmixer_plugin("5.0")], 5, "5.0"),
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("7.1.4")], 12, "7.1.4"),
        (vec![], 2, "stereo"),
        (vec![upmixer_plugin("5.1")], 6, "5.1"),
    ];

    let num_tracks = configs.len();

    // Longer files since we're skipping mid-playback
    let tracks: Vec<(PathBuf, tempfile::NamedTempFile)> = (0..num_tracks)
        .map(|_| create_stereo_pink_noise_wav(5.0, sample_rate))
        .collect();

    let mut manager = AudioEngineManager::new();
    manager.set_allow_virtual_output(true);

    for (i, ((plugins, output_channels, label), (wav_path, _tmp))) in
        configs.iter().zip(tracks.iter()).enumerate()
    {
        println!(
            "  Track {}/{}: {} ({} ch)",
            i + 1,
            num_tracks,
            label,
            output_channels
        );

        manager.load_file(wav_path).unwrap();
        manager
            .start_playback(Some(device.clone()), plugins.clone(), *output_channels)
            .unwrap_or_else(|e| panic!("Track {}: start failed: {}", i + 1, e));

        // Play for just 200ms then skip
        std::thread::sleep(Duration::from_millis(200));

        let state = manager.get_engine_state();
        assert_eq!(
            state.playback_state,
            PlaybackState::Playing,
            "Track {}: should be playing",
            i + 1
        );

        // Stop mid-playback (simulates user pressing "next")
        manager
            .stop()
            .unwrap_or_else(|e| panic!("Track {}: stop failed: {}", i + 1, e));
    }

    println!(
        "\n✓ Stress rapid skip with plugin changes passed ({} tracks)\n",
        num_tracks
    );
}
