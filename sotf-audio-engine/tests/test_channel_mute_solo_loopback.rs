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
        if let Ok(name) = device.name() {
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
fn test_channel_mute_loopback_verification() {
    let device_names = ["BlackHole 2ch", "BlackHole 16ch", "BlackHole 64ch"];
    let mut output_setup = None;
    let mut input_setup = None;

    for name in device_names {
        if let Some(out) = find_device(name, false) {
            if let Some(in_) = find_device(name, true) {
                output_setup = Some(out);
                input_setup = Some(in_);
                println!("Found device: {}", name);
                break;
            }
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
    let left_freq = 440.0;
    let right_freq = 880.0;

    // Audio Engine with left channel muted
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.name().unwrap());
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "channel_mute_solo",
        json!({
            "enabled": true,
            "channel_states": [
                {"muted": true, "soloed": false, "dimmed": false},  // Left muted
                {"muted": false, "soloed": false, "dimmed": false}  // Right not muted
            ]
        }),
    )];
    let mut engine = match AudioEngine::new(config) {
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

    // Calculate RMS for each channel (skip first 0.5s)
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

    println!(
        "Left RMS: {:.4} ({:.2} dB), Right RMS: {:.4} ({:.2} dB)",
        left_rms,
        20.0 * left_rms.log10(),
        right_rms,
        20.0 * right_rms.log10()
    );

    // Left channel should be muted (very low)
    assert!(
        left_rms < 0.01,
        "Left channel should be muted, got RMS: {:.4}",
        left_rms
    );

    // Right channel should have signal
    assert!(
        right_rms > 0.2,
        "Right channel should have signal, got RMS: {:.4}",
        right_rms
    );

    println!("Test PASSED: Channel mute verified.");
}

#[test]
fn test_channel_solo_loopback_verification() {
    let device_names = ["BlackHole 2ch", "BlackHole 16ch", "BlackHole 64ch"];
    let mut output_setup = None;
    let mut input_setup = None;

    for name in device_names {
        if let Some(out) = find_device(name, false) {
            if let Some(in_) = find_device(name, true) {
                output_setup = Some(out);
                input_setup = Some(in_);
                println!("Found device: {}", name);
                break;
            }
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
    let left_freq = 440.0;
    let right_freq = 880.0;

    // Audio Engine with left channel soloed (right should be muted)
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.name().unwrap());
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "channel_mute_solo",
        json!({
            "enabled": true,
            "channel_states": [
                {"muted": false, "soloed": true, "dimmed": false},  // Left soloed
                {"muted": false, "soloed": false, "dimmed": false}  // Right not soloed
            ]
        }),
    )];
    let mut engine = match AudioEngine::new(config) {
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

    println!(
        "Left RMS: {:.4} ({:.2} dB), Right RMS: {:.4} ({:.2} dB)",
        left_rms,
        20.0 * left_rms.log10(),
        right_rms,
        20.0 * right_rms.log10()
    );

    // Left channel (soloed) should have signal
    assert!(
        left_rms > 0.2,
        "Left channel (soloed) should have signal, got RMS: {:.4}",
        left_rms
    );

    // Right channel should be muted (not soloed)
    assert!(
        right_rms < 0.01,
        "Right channel should be muted (not soloed), got RMS: {:.4}",
        right_rms
    );

    println!("Test PASSED: Channel solo verified.");
}
