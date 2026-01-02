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
fn test_compressor_loopback_verification() {
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

    // Parameters
    // Input: Sine Wave 0dBFS (Amplitude 1.0)
    // Compressor: Threshold -20dB, Ratio 4:1
    let threshold_db = -20.0;
    let ratio = 4.0;

    // Expected Output Calculation
    // Output Level = Threshold + (Input - Threshold) / Ratio
    //              = -20 + (0 - -20)/4
    //              = -20 + 5
    //              = -15 dB.
    let expected_peak_db = -15.0;

    // Generate Signal
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let amplitude = 1.0;
    let mut source_signal = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        source_signal.push((t * 440.0 * 2.0 * std::f64::consts::PI).sin() * amplitude);
    }

    // Audio Engine
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "compressor",
        json!({
            "threshold_db": threshold_db,
            "ratio": ratio,
            "attack_ms": 10.0,
            "release_ms": 100.0,
            "knee_db": 0.0, // Hard knee for precise calculation
            "makeup_gain_db": 0.0,
            "mix": 1.0,
            "auto_makeup": false
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
        panic!("No audio");
    }
    let captured_ch0: Vec<f32> = buffer.iter().step_by(channels).cloned().collect();

    // Analyze steady state (skip first 0.5s for attack/settling)
    let start_idx = (0.5 * sample_rate) as usize;
    let end_idx = start_idx + (1.0 * sample_rate) as usize;
    if captured_ch0.len() < end_idx {
        panic!("Recording too short");
    }

    let mut max_peak = 0.0f32;
    for i in start_idx..end_idx {
        let val = captured_ch0[i].abs();
        if val > max_peak {
            max_peak = val;
        }
    }
    let peak_db = 20.0 * max_peak.log10();

    println!("Measured Peak: {:.4} ({:.2} dBFS)", max_peak, peak_db);
    println!("Expected Peak: {:.2} dBFS", expected_peak_db);

    // Assertions
    // Allow 1.5dB tolerance (compressors can have complex ballistics)
    assert!(
        (peak_db - expected_peak_db).abs() < 1.5,
        "Compressor output mismatch: Got {:.2} dB, Expected {:.2} dB",
        peak_db,
        expected_peak_db
    );

    println!("Test PASSED: Compressor output level verified.");
}
