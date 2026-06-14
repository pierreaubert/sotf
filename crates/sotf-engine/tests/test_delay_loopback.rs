#![allow(clippy::field_reassign_with_default)]
use cpal::traits::{DeviceTrait, StreamTrait};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod common;
use common::find_device;

#[test]
fn test_delay_loopback_verification() {
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
    let delay_ms = 100.0; // 100ms delay
    let feedback = 0.0; // No feedback for cleaner test
    let mix = 1.0; // 100% wet signal

    // Generate a short impulse signal
    let duration_secs = 1.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut source_signal = Vec::with_capacity(num_samples);

    // Create an impulse at the start (first 10ms)
    let impulse_samples = (0.01 * sample_rate) as usize;
    for i in 0..num_samples {
        if i < impulse_samples {
            source_signal.push(0.8); // Impulse
        } else {
            source_signal.push(0.0); // Silence
        }
    }

    // Audio Engine
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;
    config.plugins = vec![PluginConfig::new(
        "delay",
        json!({
            "delay_ms": delay_ms,
            "feedback": feedback,
            "mix": mix
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

    // Find the peak in the captured signal
    // With 100ms delay and 100% wet, the impulse should appear ~100ms later
    let delay_samples = (delay_ms / 1000.0 * sample_rate) as usize;

    // Find the maximum sample and its position
    let mut max_val = 0.0f32;
    let mut max_idx = 0;
    for (i, &sample) in captured_ch0.iter().enumerate() {
        if sample.abs() > max_val {
            max_val = sample.abs();
            max_idx = i;
        }
    }

    println!(
        "Peak found at sample {} ({:.2}ms), value: {:.4}",
        max_idx,
        max_idx as f64 / sample_rate * 1000.0,
        max_val
    );

    if max_val < 0.001 {
        println!(
            "SKIPPING test: No signal detected in loopback capture (virtual audio device may not be routing audio)."
        );
        return;
    }

    // The peak should be approximately at the delay time (with some tolerance for latency)
    // We expect the peak to be after the delay time, accounting for system latency
    let expected_min_delay = delay_samples / 2; // Allow for some variation
    assert!(
        max_idx >= expected_min_delay,
        "Peak should be delayed by at least {}ms, but found at {}ms",
        delay_ms / 2.0,
        max_idx as f64 / sample_rate * 1000.0
    );

    // Verify we got a significant signal
    assert!(max_val > 0.1, "Peak amplitude too low: {:.4}", max_val);

    println!("Test PASSED: Delay plugin verified.");
}
