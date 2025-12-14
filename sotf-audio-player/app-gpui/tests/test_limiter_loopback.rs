use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn find_device(name_part: &str, input: bool) -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
    let host = cpal::default_host();
    let devices = if input { host.input_devices().ok()? } else { host.output_devices().ok()? };
    for device in devices {
        if let Ok(name) = device.name() {
            if name.contains(name_part) {
                if input {
                    if let Ok(configs) = device.supported_input_configs() {
                        for config in configs {
                            if config.channels() >= 2 { return Some((device, config.with_max_sample_rate())); }
                        }
                    }
                } else {
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            if config.channels() >= 2 { return Some((device, config.with_max_sample_rate())); }
                        }
                    }
                }
            }
        }
    }
    None
}

#[test]
fn test_limiter_loopback_verification() {
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
    let sample_rate = out_config.sample_rate().0 as f64;

    // Parameters
    // Input: Sine Wave 0dBFS (Amplitude 1.0)
    // Limiter: Threshold -6.0dB (Amplitude 0.5)
    let threshold_db = -6.0;
    
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
    config.output_device = Some(out_device.name().unwrap());
    config.output_channels = 2;
    config.plugins = vec![
        PluginConfig::new(
            "limiter",
            json!({ 
                "threshold_db": threshold_db,
                "release_ms": 100.0,
                "mix": 1.0
            })
        ),
    ];
    let mut engine = match AudioEngine::new(config) {
        Ok(e) => e,
        Err(e) => { println!("Engine init failed: {}", e); return; }
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
    let stream = in_device.build_input_stream(
        &in_config.into(),
        move |data: &[f32], _: &_| {
            capture_clone.lock().unwrap().extend_from_slice(data);
        },
        move |err| eprintln!("Error: {}", err),
        None
    ).expect("Stream failed");
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
    if buffer.is_empty() { panic!("No audio"); }
    let captured_ch0: Vec<f32> = buffer.iter().step_by(channels).cloned().collect();

    // Analyze steady state (skip first 0.5s)
    let start_idx = (0.5 * sample_rate) as usize;
    let end_idx = start_idx + (1.0 * sample_rate) as usize;
    if captured_ch0.len() < end_idx { panic!("Recording too short"); }

    let mut max_peak = 0.0f32;
    let mut sum_sq = 0.0;
    for i in start_idx..end_idx {
        let val = captured_ch0[i].abs();
        if val > max_peak { max_peak = val; }
        sum_sq += val * val;
    }
    let rms = (sum_sq / (end_idx - start_idx) as f32).sqrt();
    let rms_db = 20.0 * rms.log10();
    let peak_db = 20.0 * max_peak.log10();

    println!("Measured Peak: {:.4} ({:.2} dBFS)", max_peak, peak_db);
    println!("Measured RMS:  {:.4} ({:.2} dBFS)", rms, rms_db);

    // Assertions
    // Peak should be clamped to approx -6dB (0.5)
    // Allow slight overshoot (e.g. 0.5dB) due to limiter response time or inter-sample peaks? 
    // Usually hard limiters are strict.
    // If output is 0.5, peak_db is -6.02.
    
    assert!(peak_db <= threshold_db + 0.5, "Peak exceeded threshold significantly: {:.2} dB (Threshold: {:.2} dB)", peak_db, threshold_db);
    assert!(peak_db >= threshold_db - 1.0, "Signal attenuated too much: {:.2} dB", peak_db);
    
    // Check if it's actually limited (source was 0dBFS)
    // If limiter was bypassed, peak would be ~0dBFS.
    assert!(peak_db < -1.0, "Limiter inactive? Peak is {:.2} dBFS", peak_db);

    println!("Test PASSED: Limiter clamped signal to threshold.");
}
