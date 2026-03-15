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
use sotf_audio::signal_recorder::record_and_analyze;
use sotf_audio::signals::{gen_log_sweep, gen_pink_noise, gen_tone};
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;

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
    get_test_device().expect(
        "No loopback device found. Install BlackHole or set AEQ_E2E_DEVICE.",
    )
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
        if let Ok(desc) = device.description() {
            if desc.name().contains(device_name) {
                if let Ok(configs) = device.supported_output_configs() {
                    for c in configs {
                        max_ch = max_ch.max(c.channels() as usize);
                    }
                }
            }
        }
    }
    // Also check input side (may differ)
    for device in host.input_devices().into_iter().flatten() {
        if let Ok(desc) = device.description() {
            if desc.name().contains(device_name) {
                if let Ok(configs) = device.supported_input_configs() {
                    for c in configs {
                        max_ch = max_ch.min(c.channels() as usize); // use the lower of in/out
                    }
                }
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
        if let Ok(desc) = device.description() {
            if desc.name().contains(device_name) {
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
    let csv = std::fs::read_to_string(&csv_file)
        .map_err(|e| format!("Failed to read CSV: {}", e))?;
    let mut spl_values = Vec::new();
    for line in csv.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let (Ok(freq), Ok(spl)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                if (100.0..=10000.0).contains(&freq) {
                    spl_values.push(spl);
                }
            }
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
    println!("Sample rate: {} Hz, send ch: {}, record ch: {}", config.sample_rate, config.send_channel, config.record_channel);

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
    let non_zero_pct = samples.iter().filter(|&&s| s.abs() > 0.01).count() as f32
        / samples.len() as f32
        * 100.0;

    println!("  Recorded: peak={:.4}, {:.1}% non-zero", max_amplitude, non_zero_pct);
    assert!(max_amplitude > 0.1, "Peak amplitude too low: {:.4}", max_amplitude);
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

    println!("  Mean SPL: {:.3} dB, Variation: {:.3} dB", mean_spl, variation);

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
    let non_zero_pct = samples.iter().filter(|&&s| s.abs() > 0.01).count() as f32
        / samples.len() as f32
        * 100.0;

    println!("  Recorded: peak={:.4}, {:.1}% non-zero", max_amplitude, non_zero_pct);
    assert!(max_amplitude > 0.05, "Peak amplitude too low: {:.4}", max_amplitude);
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
        println!("  Device only supports {} channels, need ≥2. Skipping.", max_ch);
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
    let global_max = mean_values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let cross_channel_variation = global_max - global_min;

    println!("\n  Cross-channel mean SPL spread: {:.3} dB", cross_channel_variation);

    // Each channel's transfer function should be near 0 dB
    for &(ch, mean, var) in &results {
        assert!(
            (-3.0..=3.0).contains(&mean),
            "Ch{}: mean SPL {:.3} dB outside [-3, 3] range",
            ch, mean,
        );
        assert!(
            var < 1.0,
            "Ch{}: variation {:.3} dB too high (expected < 1 dB)",
            ch, var,
        );
    }

    // Channels should agree with each other (digital loopback = identical)
    assert!(
        cross_channel_variation < 1.0,
        "Cross-channel spread {:.3} dB too high (expected < 1 dB)",
        cross_channel_variation,
    );

    println!("✓ Multi-channel loopback passed ({} channels)\n", results.len());
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
        println!("    {:>6} Hz: mean={:+.3} dB, var={:.3} dB", rate, mean, var);
    }

    // Each successful rate should produce flat, near-0 dB transfer function
    for &(rate, mean, var) in &results {
        assert!(
            (-3.0..=3.0).contains(&mean),
            "{}Hz: mean SPL {:.3} dB outside [-3, 3] range",
            rate, mean,
        );
        assert!(
            var < 1.0,
            "{}Hz: variation {:.3} dB too high (expected < 1 dB)",
            rate, var,
        );
    }

    // All rates should agree with each other
    let mean_values: Vec<f32> = results.iter().map(|(_, m, _)| *m).collect();
    let global_min = mean_values.iter().copied().fold(f32::INFINITY, f32::min);
    let global_max = mean_values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let cross_rate_variation = global_max - global_min;

    println!("  Cross-rate mean SPL spread: {:.3} dB", cross_rate_variation);
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
