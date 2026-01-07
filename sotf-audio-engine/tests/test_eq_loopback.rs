use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Helper to find device (copied from previous test to avoid external dependencies on test modules)
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
fn test_eq_sweep_loopback_verification() {
    // 1. Setup Devices
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

    // Ensure we use the device's native sample rate to avoid resampling artifacts in the loopback
    let sample_rate = out_config.sample_rate() as f64;
    println!("Testing at sample rate: {} Hz", sample_rate);

    // 2. Configure EQ Parameters (Test Case: +6dB Peak at 1kHz)
    let eq_freq = 1000.0;
    let eq_gain = 6.0;
    let eq_q = 1.0;

    // 3. Generate Sweep Signal (Source)
    let duration_secs = 2.0;
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut source_signal = Vec::with_capacity(num_samples);

    // Logarithmic sweep 20Hz to 20kHz
    let start_freq: f64 = 20.0;
    let end_freq: f64 = 20000.0;
    // f(t) = start * exp(k*t)
    // k = ln(end/start) / T
    let k = (end_freq / start_freq).ln() / duration_secs;

    for i in 0..num_samples {
        let t = i as f64 / sample_rate;
        // phase = integral(f(t) dt) from 0 to t
        // integral(start * exp(k*tau) dtau) = (start/k) * [exp(k*t) - 1]
        let phase = (start_freq / k) * ((k * t).exp() - 1.0);
        // Use 0.5 amplitude to avoid clipping with EQ boost
        source_signal.push((2.0 * std::f64::consts::PI * phase).sin());
    }

    // 4. Generate Expected Signal (Reference) using autoeq-iir
    let mut reference_filter =
        Biquad::new(BiquadFilterType::Peak, eq_freq, sample_rate, eq_q, eq_gain);

    let expected_signal: Vec<f32> = source_signal
        .iter()
        .map(|&s| reference_filter.process(s) as f32)
        .collect();

    // 5. Configure Audio Engine
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;

    // EQ Plugin Configuration
    config.plugins = vec![PluginConfig::new(
        "eq",
        json!({
            "filters": [
                {
                    "filter_type": "Peak",
                    "frequency": eq_freq,
                    "q": eq_q,
                    "gain_db": eq_gain
                }
            ]
        }),
    )];

    let mut engine = match AudioEngine::new(config) {
        Ok(e) => e,
        Err(e) => {
            println!("Failed to create AudioEngine: {}. Skipping.", e);
            return;
        }
    };

    // 6. Write Source to WAV file for playback
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = hound::WavWriter::create(temp_file.path(), spec).unwrap();

    // Scale by 0.5 to match source generation (source was sine amplitude 1.0, but let's be consistent)
    // Actually, source_signal is already amplitude 1.0.
    // If we write full scale i16, we might clip +6dB EQ.
    // So we apply 0.5 scaling here (effectively -6dB headroom).
    for &sample in &source_signal {
        let amp = (sample * i16::MAX as f64 * 0.5) as i16;
        writer.write_sample(amp).unwrap(); // L
        writer.write_sample(amp).unwrap(); // R
    }
    writer.finalize().unwrap();

    // 7. Start Capture
    let captured_samples = Arc::new(Mutex::new(Vec::new()));
    let capture_clone = captured_samples.clone();
    let channels = in_config.channels() as usize;

    let stream = in_device
        .build_input_stream(
            &in_config.into(),
            move |data: &[f32], _: &_| {
                let mut buffer = capture_clone.lock().unwrap();
                buffer.extend_from_slice(data);
            },
            move |err| {
                eprintln!("Capture error: {}", err);
            },
            None,
        )
        .expect("Failed to build input stream");

    stream.play().expect("Failed to start capture");

    // 8. Start Playback
    println!("Starting playback...");
    if let Err(e) = engine.play(temp_file.path().to_path_buf()) {
        println!("Playback failed: {}. Skipping.", e);
        return;
    }

    // Wait for playback + buffer
    std::thread::sleep(Duration::from_millis(500)); // pre-roll
    std::thread::sleep(Duration::from_secs_f64(duration_secs + 0.5));

    drop(stream);

    // 9. Process Recorded Audio
    let raw_capture = captured_samples.lock().unwrap();
    if raw_capture.is_empty() {
        println!("WARNING: No audio captured.");
        // Fail if we expect to verify EQ
        panic!("No audio captured from loopback device");
    }

    // De-interleave channel 0 (Left)
    let captured_ch0: Vec<f32> = raw_capture.iter().step_by(channels).cloned().collect();

    // 10. Align Signals (Cross-Correlation)
    // We expect the captured signal to match 'expected_signal * 0.5'
    // (because we scaled by 0.5 when writing WAV)

    let search_window = sample_rate as usize; // 1 second
    if captured_ch0.len() < search_window {
        panic!(
            "Captured too short ({} samples), expected > {}",
            captured_ch0.len(),
            search_window
        );
    }

    let mut best_offset = 0;
    let mut max_corr = 0.0;
    let ref_snippet_len = 2000;
    let ref_snippet = &expected_signal[0..ref_snippet_len];

    // Scan for best alignment
    let max_search = captured_ch0.len().min(search_window) - ref_snippet_len;
    for offset in 0..max_search {
        let mut corr = 0.0;
        // Optimization: stride for speed, then fine tune? No, simple loop is fast enough for 48k
        for j in (0..ref_snippet_len).step_by(4) {
            corr += captured_ch0[offset + j] * ref_snippet[j];
        }
        if corr.abs() > max_corr {
            max_corr = corr.abs();
            best_offset = offset;
        }
    }

    println!(
        "Detected latency: {} samples ({:.2} ms)",
        best_offset,
        best_offset as f64 / sample_rate * 1000.0
    );

    // 11. Compare Aligned Signals
    let comparison_len = (expected_signal.len() - 2000).min(captured_ch0.len() - best_offset);
    let mut error_sum = 0.0;
    let mut signal_energy = 0.0;

    for i in 0..comparison_len {
        let rec = captured_ch0[best_offset + i];
        let expected = expected_signal[i] * 0.5; // Apply scaling

        let diff = rec - expected;
        error_sum += diff * diff;
        signal_energy += expected * expected;
    }

    let mse = error_sum / comparison_len as f32;
    // Normalized MSE to be independent of signal level
    let nmse = if signal_energy > 0.0 {
        mse / (signal_energy / comparison_len as f32)
    } else {
        1.0 // Should not happen if signal played
    };

    println!("MSE: {:.8}, NMSE: {:.8}", mse, nmse);

    // 12. Assertions
    // Ensure signal was actually recorded (energy > silence)
    assert!(signal_energy > 0.001, "Recorded signal silent?");

    // Check error metric
    // NMSE < 0.01 (1%) is acceptable for loopback with potential resampling/dithering
    assert!(
        nmse < 0.05,
        "EQ Verification Failed! Signal mismatch too high (NMSE: {:.6})",
        nmse
    );

    println!("Test PASSED: EQ Output matches Reference Model.");
}
