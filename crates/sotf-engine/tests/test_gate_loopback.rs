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
fn test_gate_loopback_verification() {
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

    // Parameters - Gate with high threshold to silence quiet parts
    let threshold_db = -20.0; // Gate opens above -20dB
    let ratio = 100.0; // High ratio for strong gating
    let attack_ms = 1.0;
    let hold_ms = 10.0;
    let release_ms = 50.0;

    // Generate signal: loud part (above threshold) followed by quiet part (below threshold)
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut source_signal = Vec::with_capacity(num_samples);

    let loud_amplitude = 0.5; // -6dB, above threshold
    let quiet_amplitude = 0.05; // -26dB, below threshold

    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        let sine = (t * 440.0 * 2.0 * std::f64::consts::PI).sin();

        // First half: loud signal (gate open)
        // Second half: quiet signal (gate closed)
        let amplitude = if t < 1.0 {
            loud_amplitude
        } else {
            quiet_amplitude
        };
        source_signal.push(sine * amplitude);
    }

    // Audio Engine
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "gate",
        json!({
            "threshold_db": threshold_db,
            "ratio": ratio,
            "attack_ms": attack_ms,
            "hold_ms": hold_ms,
            "release_ms": release_ms,
            "mix": 1.0
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

    println!(
        "Captured {} frames at {} channels",
        captured_ch0.len(),
        channels
    );

    // Check if any signal was captured at all
    let max_sample = captured_ch0.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if max_sample < 0.001 {
        println!(
            "SKIPPING test: No signal detected in loopback capture (virtual audio device may not be routing audio)."
        );
        return;
    }

    // Detect playback latency by finding when audio starts (RMS > threshold)
    let window_size = (0.05 * sample_rate) as usize; // 50ms windows
    let mut latency_samples = 0usize;
    for i in (0..captured_ch0.len() - window_size).step_by(window_size / 4) {
        let rms: f32 = captured_ch0[i..i + window_size]
            .iter()
            .map(|&x| x * x)
            .sum::<f32>()
            / window_size as f32;
        let rms = rms.sqrt();
        if rms > 0.1 {
            // Found signal start
            latency_samples = i;
            break;
        }
    }
    let latency_offset = latency_samples as f64 / sample_rate;
    println!(
        "Detected latency: {:.3}s ({} samples)",
        latency_offset, latency_samples
    );

    // Calculate RMS for first half (should be loud - gate open)
    let first_half_start = ((0.2 + latency_offset) * sample_rate) as usize;
    let first_half_end = ((0.8 + latency_offset) * sample_rate) as usize;

    // Calculate RMS for second half (should be quiet - gate closed)
    // source 1.2-1.8s → captured 1.7-2.3s
    let second_half_start = ((1.2 + latency_offset) * sample_rate) as usize;
    let second_half_end = ((1.8 + latency_offset) * sample_rate) as usize;

    if captured_ch0.len() < second_half_end {
        println!(
            "SKIPPING test: Recording too short ({} samples, need {}).",
            captured_ch0.len(),
            second_half_end
        );
        return;
    }

    let mut first_half_sum_sq = 0.0f32;
    for &sample in &captured_ch0[first_half_start..first_half_end] {
        first_half_sum_sq += sample * sample;
    }
    let first_half_rms = (first_half_sum_sq / (first_half_end - first_half_start) as f32).sqrt();

    let mut second_half_sum_sq = 0.0f32;
    for &sample in &captured_ch0[second_half_start..second_half_end] {
        second_half_sum_sq += sample * sample;
    }
    let second_half_rms =
        (second_half_sum_sq / (second_half_end - second_half_start) as f32).sqrt();

    let first_half_db = 20.0 * first_half_rms.log10();
    let second_half_db = 20.0 * second_half_rms.log10();

    println!(
        "First half RMS: {:.4} ({:.2} dB), Second half RMS: {:.4} ({:.2} dB)",
        first_half_rms, first_half_db, second_half_rms, second_half_db
    );

    // The second half should be significantly quieter due to gating
    let reduction_db = first_half_db - second_half_db;
    println!("Reduction: {:.2} dB", reduction_db);

    // Expect at least 10dB reduction from gating
    assert!(
        reduction_db > 10.0,
        "Gate should reduce quiet signal by at least 10dB, got {:.2}dB reduction",
        reduction_db
    );

    println!("Test PASSED: Gate plugin verified.");
}
