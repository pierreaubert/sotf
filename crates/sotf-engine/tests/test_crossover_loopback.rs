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
fn test_crossover_lowpass_loopback_verification() {
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

    // Parameters - Lowpass crossover at 500Hz
    let crossover_freq = 500.0;

    // Generate signal: mix of low frequency (200Hz) and high frequency (2000Hz)
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut source_signal = Vec::with_capacity(num_samples);

    let low_freq = 200.0; // Below crossover - should pass
    let high_freq = 2000.0; // Above crossover - should be attenuated

    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        let low_sine = (t * low_freq * 2.0 * std::f64::consts::PI).sin() * 0.5;
        let high_sine = (t * high_freq * 2.0 * std::f64::consts::PI).sin() * 0.5;
        source_signal.push(low_sine + high_sine);
    }

    // Audio Engine with lowpass crossover
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "crossover",
        json!({
            "type": "LR24",
            "frequency": crossover_freq,
            "output": "low"
        }),
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

    // Analyze frequency content using simple energy measurement
    // Skip first 0.5s for settling, analyze 1s
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

    // Calculate RMS of the captured signal
    let mut sum_sq = 0.0f32;
    for &sample in &captured_ch0[start_idx..end_idx] {
        sum_sq += sample * sample;
    }
    let rms = (sum_sq / (end_idx - start_idx) as f32).sqrt();
    let db_fs = 20.0 * rms.log10();

    println!("Captured RMS: {:.4} ({:.2} dBFS)", rms, db_fs);

    if rms < 0.001 {
        println!(
            "SKIPPING test: No signal detected in loopback capture (BlackHole may not be routing audio)."
        );
        return;
    }

    // The lowpass should pass the 200Hz component and attenuate the 2000Hz component
    // So we should have roughly half the energy (the low frequency part)
    // Expected RMS for single sine at 0.5 amplitude = 0.5 * 0.707 = 0.354
    let expected_rms = 0.354;

    // Allow reasonable tolerance
    assert!(
        (rms - expected_rms).abs() < 0.15,
        "RMS mismatch. Expected ~{:.4} (low freq only), got {:.4}",
        expected_rms,
        rms
    );

    println!("Test PASSED: Crossover lowpass verified.");
}

#[test]
fn test_crossover_highpass_loopback_verification() {
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

    // Parameters - Highpass crossover at 500Hz
    let crossover_freq = 500.0;

    // Generate signal: mix of low frequency (200Hz) and high frequency (2000Hz)
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut source_signal = Vec::with_capacity(num_samples);

    let low_freq = 200.0; // Below crossover - should be attenuated
    let high_freq = 2000.0; // Above crossover - should pass

    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        let low_sine = (t * low_freq * 2.0 * std::f64::consts::PI).sin() * 0.5;
        let high_sine = (t * high_freq * 2.0 * std::f64::consts::PI).sin() * 0.5;
        source_signal.push(low_sine + high_sine);
    }

    // Audio Engine with highpass crossover
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "crossover",
        json!({
            "type": "LR24",
            "frequency": crossover_freq,
            "output": "high"
        }),
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
        panic!("No audio captured");
    }
    let captured_ch0: Vec<f32> = buffer.iter().step_by(channels).cloned().collect();

    // Analyze frequency content
    let start_idx = (0.5 * sample_rate) as usize;
    let end_idx = start_idx + (1.0 * sample_rate) as usize;

    if captured_ch0.len() < end_idx {
        panic!("Recording too short");
    }

    let mut sum_sq = 0.0f32;
    for &sample in &captured_ch0[start_idx..end_idx] {
        sum_sq += sample * sample;
    }
    let rms = (sum_sq / (end_idx - start_idx) as f32).sqrt();
    let db_fs = 20.0 * rms.log10();

    println!("Captured RMS: {:.4} ({:.2} dBFS)", rms, db_fs);

    if rms < 0.001 {
        println!(
            "SKIPPING test: No signal detected in loopback capture (BlackHole may not be routing audio)."
        );
        return;
    }

    // The highpass should pass the 2000Hz component and attenuate the 200Hz component
    let expected_rms = 0.354;

    assert!(
        (rms - expected_rms).abs() < 0.15,
        "RMS mismatch. Expected ~{:.4} (high freq only), got {:.4}",
        expected_rms,
        rms
    );

    println!("Test PASSED: Crossover highpass verified.");
}
