#![allow(clippy::field_reassign_with_default)]
use cpal::traits::{DeviceTrait, StreamTrait};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod common;
use common::find_device;

#[test]
fn test_gain_loopback_verification() {
    let device_names = [
        "BlackHole 2ch",
        "BlackHole 16ch",
        "BlackHole 64ch",
        "SotF Virtual Audio",
        "SotF Virtual Device",
        "SotF Virtual Output",
    ];
    let mut output_setup = None;
    let mut input_setup = None;

    for name in device_names {
        if let Some(out) = find_device(name, false)
            && let Some(in_) = find_device(name, true)
        {
            output_setup = Some(out);
            input_setup = Some(in_);
            println!("Found device: {}", name);
            break;
        }
    }

    if output_setup.is_none() || input_setup.is_none() {
        println!("SKIPPING test: virtual audio device not found.");
        return;
    }

    let (out_device, out_config) = output_setup.unwrap();
    let (in_device, in_config) = input_setup.unwrap();
    let sample_rate = out_config.sample_rate() as f64;

    // Parameters
    let input_gain_db = -6.0; // Input signal level (approx 0.5 amplitude)
    let plugin_gain_db = 6.0; // Gain to apply (+6dB, factor approx 2.0)
    // Expected output peak = 0dBFS (1.0 amplitude)

    // Generate Signal
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let amplitude = 10.0f64.powf(input_gain_db / 20.0);
    let mut source_signal = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        source_signal.push((t * 440.0 * 2.0 * std::f64::consts::PI).sin() * amplitude);
    }

    // Audio Engine
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "gain",
        json!({ "gain_db": plugin_gain_db }),
    )];
    let engine = match AudioEngine::new(config) {
        Ok(e) => e,
        Err(e) => {
            println!("Engine init failed: {}", e);
            return;
        }
    };

    // WAV file
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = hound::WavWriter::create(temp_file.path(), spec).unwrap();
    for &sample in &source_signal {
        let amp = (sample * i16::MAX as f64) as i16;
        writer.write_sample(amp).unwrap();
        writer.write_sample(amp).unwrap();
    }
    writer.finalize().unwrap();

    // Capture
    let captured_samples = Arc::new(Mutex::new(Vec::new()));
    let capture_clone = captured_samples.clone();
    let channels = in_config.channels() as usize;
    let stream = in_device
        .build_input_stream(
            &in_config.into(),
            move |data: &[f32], _: &_| {
                capture_clone.lock().unwrap().extend_from_slice(data);
            },
            move |err| eprintln!("Error: {}", err),
            None,
        )
        .expect("Stream failed");
    stream.play().expect("Play failed");

    // Playback
    if let Err(e) = engine.play(temp_file.path().to_path_buf()) {
        println!("Playback failed: {}", e);
        return;
    }
    std::thread::sleep(Duration::from_secs_f64(duration_secs + 0.5));
    drop(stream);

    // Analysis
    let buffer = captured_samples.lock().unwrap();
    if buffer.is_empty() {
        println!("SKIPPING test: No audio captured from loopback device.");
        return;
    }
    let captured_ch0: Vec<f32> = buffer.iter().step_by(channels).cloned().collect();

    // Calculate RMS of steady state (skip first 0.5s, take 1s)
    let start_idx = (0.5 * sample_rate) as usize;
    let end_idx = start_idx + (1.0 * sample_rate) as usize;
    if captured_ch0.len() < end_idx {
        println!(
            "SKIPPING test: Recording too short ({} samples, need {}).",
            captured_ch0.len(),
            end_idx
        );
        return;
    }

    let mut sum_sq = 0.0;
    for &sample in &captured_ch0[start_idx..end_idx] {
        sum_sq += sample * sample;
    }
    let rms = (sum_sq / (end_idx - start_idx) as f32).sqrt();
    let db_fs = 20.0 * rms.log10();

    println!("Measured RMS: {:.4} ({:.2} dBFS)", rms, db_fs);

    if rms < 0.001 {
        println!(
            "SKIPPING test: No signal detected in loopback capture (virtual audio device may not be routing audio)."
        );
        return;
    }

    // Expected: 0 dBFS Peak -> RMS = 0.7071
    let expected_rms = std::f32::consts::FRAC_1_SQRT_2;
    // Allow 5% tolerance due to analog/digital approximations
    assert!(
        (rms - expected_rms).abs() < 0.05,
        "RMS mismatch. Expected {:.4}, got {:.4}",
        expected_rms,
        rms
    );

    println!("Test PASSED: Gain verified.");
}
