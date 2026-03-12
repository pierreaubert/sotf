#![allow(clippy::field_reassign_with_default)]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn find_device(
    name_part: &str,
    input: bool,
) -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
    let host = cpal::default_host();
    let devices = if input {
        host.input_devices().ok()?
    } else {
        host.output_devices().ok()?
    };
    for device in devices {
        if let Ok(desc) = device.description() {
            let name = desc.name().to_string();
            if name.contains(name_part) {
                if input {
                    if let Ok(configs) = device.supported_input_configs() {
                        for config in configs {
                            if config.channels() >= 2 {
                                return Some((device, config.with_max_sample_rate()));
                            }
                        }
                    }
                } else {
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            if config.channels() >= 2 {
                                return Some((device, config.with_max_sample_rate()));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[test]
#[ignore] // Requires BlackHole loopback audio routing to be configured
fn test_matrix_swap_channels_loopback_verification() {
    let device_names = ["BlackHole 2ch", "BlackHole 16ch", "BlackHole 64ch"];
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
        println!("SKIPPING test: BlackHole device not found.");
        return;
    }

    let (out_device, out_config) = output_setup.unwrap();
    let (in_device, in_config) = input_setup.unwrap();
    let sample_rate = out_config.sample_rate() as f64;

    // Generate stereo signal with different frequencies per channel
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let left_freq = 440.0; // A4
    let right_freq = 880.0; // A5

    // Audio Engine with matrix that swaps channels
    // Matrix: [0, 1, 1, 0] means Out0 = In1, Out1 = In0
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "matrix",
        json!({
            "input_channels": 2,
            "output_channels": 2,
            "matrix": [0.0, 1.0, 1.0, 0.0]  // Swap L/R
        }),
    )];
    let engine = match AudioEngine::new(config) {
        Ok(e) => e,
        Err(e) => {
            println!("Engine init failed: {}", e);
            return;
        }
    };

    // WAV file with different frequencies per channel
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = hound::WavWriter::create(temp_file.path(), spec).unwrap();
    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        let left = (t * left_freq * 2.0 * std::f64::consts::PI).sin() * 0.5;
        let right = (t * right_freq * 2.0 * std::f64::consts::PI).sin() * 0.5;
        writer
            .write_sample((left * i16::MAX as f64) as i16)
            .unwrap();
        writer
            .write_sample((right * i16::MAX as f64) as i16)
            .unwrap();
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
        panic!("No audio captured");
    }

    // Extract left and right channels
    let captured_left: Vec<f32> = buffer.iter().step_by(channels).cloned().collect();
    let captured_right: Vec<f32> = buffer.iter().skip(1).step_by(channels).cloned().collect();

    // Calculate RMS for each channel
    let start_idx = (0.5 * sample_rate) as usize;
    let end_idx = start_idx + (1.0 * sample_rate) as usize;

    if captured_left.len() < end_idx || captured_right.len() < end_idx {
        panic!("Recording too short");
    }

    let mut left_sum_sq = 0.0f32;
    let mut right_sum_sq = 0.0f32;
    for i in start_idx..end_idx {
        left_sum_sq += captured_left[i] * captured_left[i];
        right_sum_sq += captured_right[i] * captured_right[i];
    }
    let left_rms = (left_sum_sq / (end_idx - start_idx) as f32).sqrt();
    let right_rms = (right_sum_sq / (end_idx - start_idx) as f32).sqrt();

    println!("Left RMS: {:.4}, Right RMS: {:.4}", left_rms, right_rms);

    // Both channels should have signal (swapped)
    assert!(
        left_rms > 0.2,
        "Left channel should have signal (from right input), got RMS: {:.4}",
        left_rms
    );
    assert!(
        right_rms > 0.2,
        "Right channel should have signal (from left input), got RMS: {:.4}",
        right_rms
    );

    // The RMS values should be similar since both input signals have same amplitude
    let rms_diff = (left_rms - right_rms).abs();
    assert!(
        rms_diff < 0.1,
        "Both channels should have similar RMS after swap, diff: {:.4}",
        rms_diff
    );

    println!("Test PASSED: Matrix channel swap verified.");
}

#[test]
#[ignore] // Requires BlackHole loopback audio routing to be configured
fn test_matrix_mono_sum_loopback_verification() {
    let device_names = ["BlackHole 2ch", "BlackHole 16ch", "BlackHole 64ch"];
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
        println!("SKIPPING test: BlackHole device not found.");
        return;
    }

    let (out_device, out_config) = output_setup.unwrap();
    let (in_device, in_config) = input_setup.unwrap();
    let sample_rate = out_config.sample_rate() as f64;

    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;

    // Audio Engine with matrix that sums to mono
    // Matrix: [0.5, 0.5, 0.5, 0.5] means Out0 = 0.5*In0 + 0.5*In1, Out1 = 0.5*In0 + 0.5*In1
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "matrix",
        json!({
            "input_channels": 2,
            "output_channels": 2,
            "matrix": [0.5, 0.5, 0.5, 0.5]  // Sum to mono on both outputs
        }),
    )];
    let engine = match AudioEngine::new(config) {
        Ok(e) => e,
        Err(e) => {
            println!("Engine init failed: {}", e);
            return;
        }
    };

    // WAV file with signal only on left channel
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = hound::WavWriter::create(temp_file.path(), spec).unwrap();
    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        let left = (t * 440.0 * 2.0 * std::f64::consts::PI).sin() * 0.8;
        let right = 0.0; // Silent right channel
        writer
            .write_sample((left * i16::MAX as f64) as i16)
            .unwrap();
        writer
            .write_sample((right * i16::MAX as f64) as i16)
            .unwrap();
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
        panic!("No audio captured");
    }

    let captured_left: Vec<f32> = buffer.iter().step_by(channels).cloned().collect();
    let captured_right: Vec<f32> = buffer.iter().skip(1).step_by(channels).cloned().collect();

    let start_idx = (0.5 * sample_rate) as usize;
    let end_idx = start_idx + (1.0 * sample_rate) as usize;

    if captured_left.len() < end_idx || captured_right.len() < end_idx {
        panic!("Recording too short");
    }

    let mut left_sum_sq = 0.0f32;
    let mut right_sum_sq = 0.0f32;
    for i in start_idx..end_idx {
        left_sum_sq += captured_left[i] * captured_left[i];
        right_sum_sq += captured_right[i] * captured_right[i];
    }
    let left_rms = (left_sum_sq / (end_idx - start_idx) as f32).sqrt();
    let right_rms = (right_sum_sq / (end_idx - start_idx) as f32).sqrt();

    println!("Left RMS: {:.4}, Right RMS: {:.4}", left_rms, right_rms);

    // Both channels should have signal (mono sum)
    assert!(
        left_rms > 0.1,
        "Left channel should have signal from mono sum, got RMS: {:.4}",
        left_rms
    );
    assert!(
        right_rms > 0.1,
        "Right channel should have signal from mono sum, got RMS: {:.4}",
        right_rms
    );

    // Both channels should be identical (mono)
    let rms_diff = (left_rms - right_rms).abs();
    assert!(
        rms_diff < 0.05,
        "Both channels should have identical RMS (mono), diff: {:.4}",
        rms_diff
    );

    println!("Test PASSED: Matrix mono sum verified.");
}
