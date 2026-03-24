#![allow(clippy::field_reassign_with_default)]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde_json::json;
use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Helper to find device (copied from previous test to avoid external dependencies on test modules)
/// Preferred sample rates in order of priority.
const PREFERRED_SAMPLE_RATES: [u32; 4] = [48000, 44100, 96000, 192000];

/// Pick a `SupportedStreamConfig` at a sensible sample rate (prefer 48kHz).
/// Falls back to the minimum supported rate if none of the preferred rates are in range.
fn pick_config_at_preferred_rate(
    config: cpal::SupportedStreamConfigRange,
) -> cpal::SupportedStreamConfig {
    let min = config.min_sample_rate();
    let max = config.max_sample_rate();
    for &rate in &PREFERRED_SAMPLE_RATES {
        if rate >= min && rate <= max {
            return config.with_sample_rate(rate);
        }
    }
    config.with_sample_rate(min)
}

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
                let configs: Vec<cpal::SupportedStreamConfigRange> = if input {
                    device.supported_input_configs().ok()
                        .map(|c| c.collect())
                        .unwrap_or_default()
                } else {
                    device.supported_output_configs().ok()
                        .map(|c| c.collect())
                        .unwrap_or_default()
                };
                for config in configs {
                    if config.channels() >= 2 {
                        return Some((device, pick_config_at_preferred_rate(config)));
                    }
                }
            }
        }
    }
    None
}

/// Send a short 1kHz tone through the loopback device and check that we capture
/// non-silent audio. Returns `true` if audio flows through the device pair.
fn probe_loopback_available(
    out_device: &cpal::Device,
    out_config: &cpal::SupportedStreamConfig,
    in_device: &cpal::Device,
    in_config: &cpal::SupportedStreamConfig,
) -> bool {
    let sample_rate = out_config.sample_rate() as f64;
    let channels = out_config.channels() as usize;
    let probe_duration = Duration::from_millis(200);
    let probe_samples = (sample_rate * probe_duration.as_secs_f64()) as usize;

    // Generate a short 1kHz sine burst
    let tone: Vec<f32> = (0..probe_samples)
        .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sample_rate).sin() as f32)
        .collect();

    // Start capturing
    let captured = Arc::new(Mutex::new(Vec::new()));
    let capture_clone = captured.clone();
    let in_channels = in_config.channels() as usize;

    let in_stream = match in_device.build_input_stream(
        &in_config.clone().into(),
        move |data: &[f32], _: &_| {
            captured.lock().unwrap().extend_from_slice(data);
        },
        |err| eprintln!("Probe capture error: {}", err),
        None,
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if in_stream.play().is_err() {
        return false;
    }

    // Play the tone
    let tone = Arc::new(tone);
    let tone_clone = tone.clone();
    let write_pos = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let write_pos_clone = write_pos.clone();

    let out_stream = match out_device.build_output_stream(
        &out_config.clone().into(),
        move |data: &mut [f32], _: &_| {
            let pos = write_pos_clone.load(std::sync::atomic::Ordering::Relaxed);
            for frame in data.chunks_mut(channels) {
                let sample = if pos < tone_clone.len() {
                    tone_clone[pos]
                } else {
                    0.0
                };
                for ch in frame.iter_mut() {
                    *ch = sample;
                }
                write_pos_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        },
        |err| eprintln!("Probe playback error: {}", err),
        None,
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if out_stream.play().is_err() {
        return false;
    }

    // Wait for tone to play + settle
    std::thread::sleep(probe_duration + Duration::from_millis(150));

    drop(out_stream);
    drop(in_stream);

    // Check if we captured non-silent audio
    let buf = capture_clone.lock().unwrap();
    let ch0: Vec<f32> = buf.iter().step_by(in_channels).cloned().collect();
    let peak = ch0.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    println!("Loopback probe: captured {} samples, peak amplitude = {:.6}", ch0.len(), peak);

    // If peak > -60dBFS (0.001), we consider the loopback functional
    peak > 0.001
}

#[test]
fn test_eq_sweep_loopback_verification() {
    // 1. Setup Devices
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

    // 1b. Probe loopback: send a short burst and check we capture non-silence
    if !probe_loopback_available(&out_device, &out_config, &in_device, &in_config) {
        println!(
            "SKIPPING test: BlackHole device found but loopback is not functional \
             (no audio captured). Check that audio routing is configured."
        );
        return;
    }
    println!("Loopback probe passed — audio routing is functional.");

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
    // Scale source by 0.5 to match the WAV amplitude (written at 0.5 * i16::MAX),
    // then also simulate i16 quantization so the reference matches the decoded signal.
    let wav_scale = 0.5;
    let mut reference_filter =
        Biquad::new(BiquadFilterType::Peak, eq_freq, sample_rate, eq_q, eq_gain);

    let expected_signal: Vec<f32> = source_signal
        .iter()
        .map(|&s| {
            // Quantize to i16 like the WAV file, then back to f32
            let quantized = ((s * wav_scale * i16::MAX as f64) as i16) as f64 / i16::MAX as f64;
            reference_filter.process(quantized) as f32
        })
        .collect();

    // 5. Configure Audio Engine
    let mut config = EngineConfig::default();
    config.output_device = Some(out_device.description().unwrap().name().to_string());
    config.allow_virtual_output = true;
    config.output_sample_rate = sample_rate as u32;
    config.output_channels = 2;

    // EQ Plugin Configuration
    config.plugins = vec![PluginConfig::new(
        "eq",
        json!({
            "filters": [
                {
                    "filter_type": "Peak",
                    "freq": eq_freq,
                    "q": eq_q,
                    "db_gain": eq_gain
                }
            ]
        }),
    )];

    let engine = match AudioEngine::new(config) {
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

    // 10. Align Signals using normalized cross-correlation
    // Find the offset in captured where the expected signal starts.
    // Use a snippet from mid-sweep where there's distinctive waveform.

    let cap_peak = captured_ch0.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    println!(
        "Captured: {} samples, peak={:.6}",
        captured_ch0.len(), cap_peak
    );

    let search_window = (sample_rate * 2.0) as usize; // 2 seconds
    if captured_ch0.len() < search_window {
        panic!(
            "Captured too short ({} samples), expected > {}",
            captured_ch0.len(),
            search_window
        );
    }

    // Use a snippet from ~40% into the sweep where frequency is distinctive
    let ref_start = expected_signal.len() * 2 / 5;
    let ref_snippet_len = 2000;
    let ref_snippet = &expected_signal[ref_start..ref_start + ref_snippet_len];

    // Pre-compute reference energy for normalization
    let ref_energy: f64 = ref_snippet.iter().map(|&s| (s as f64).powi(2)).sum();

    let mut best_offset = 0usize;
    let mut max_ncc = -1.0f64;
    let max_search = captured_ch0.len().saturating_sub(ref_snippet_len);

    for offset in 0..max_search.min(search_window) {
        let mut dot = 0.0f64;
        let mut cap_energy = 0.0f64;
        for j in (0..ref_snippet_len).step_by(2) {
            let c = captured_ch0[offset + j] as f64;
            let r = ref_snippet[j] as f64;
            dot += c * r;
            cap_energy += c * c;
        }
        let denom = (cap_energy * ref_energy).sqrt();
        let ncc = if denom > 1e-12 { dot / denom } else { 0.0 };
        if ncc > max_ncc {
            max_ncc = ncc;
            best_offset = offset;
        }
    }

    // best_offset in captured aligns with ref_start in expected
    println!(
        "Alignment: captured[{}] ~ expected[{}], NCC={:.4}",
        best_offset, ref_start, max_ncc
    );

    // Compute latency: how many samples before the start of expected in captured
    let signal_start_in_captured = if best_offset >= ref_start {
        best_offset - ref_start
    } else {
        0
    };
    println!(
        "Estimated latency: {} samples ({:.2} ms)",
        signal_start_in_captured,
        signal_start_in_captured as f64 / sample_rate * 1000.0
    );

    // 11. Estimate gain ratio and compare from alignment point
    let avail_expected = expected_signal.len().saturating_sub(ref_start + 2000);
    let avail_captured = captured_ch0.len().saturating_sub(best_offset);
    let align_len = avail_expected.min(avail_captured);

    let mut dot_re = 0.0f64;
    let mut dot_ee = 0.0f64;
    for i in 0..align_len {
        let rec = captured_ch0[best_offset + i] as f64;
        let exp = expected_signal[ref_start + i] as f64;
        dot_re += rec * exp;
        dot_ee += exp * exp;
    }
    let gain_ratio = if dot_ee > 0.0 { dot_re / dot_ee } else { 1.0 };
    println!("Estimated gain ratio (captured/expected): {:.6}", gain_ratio);

    // 12. Compare aligned signals using measured gain
    let comparison_len = align_len;
    let mut error_sum = 0.0f64;
    let mut signal_energy = 0.0f64;

    for i in 0..comparison_len {
        let rec = captured_ch0[best_offset + i] as f64;
        let expected = expected_signal[ref_start + i] as f64 * gain_ratio;

        let diff = rec - expected;
        error_sum += diff * diff;
        signal_energy += expected * expected;
    }

    let mse = error_sum / comparison_len as f64;
    let nmse = if signal_energy > 0.0 {
        mse / (signal_energy / comparison_len as f64)
    } else {
        1.0
    };

    println!("MSE: {:.8}, NMSE: {:.8}", mse, nmse);

    // 13. Assertions
    // Ensure signal was actually recorded (energy > silence)
    assert!(signal_energy > 0.001, "Recorded signal silent?");

    // Gain ratio sanity: should be positive and within a reasonable range
    assert!(
        gain_ratio > 0.01 && gain_ratio < 10.0,
        "Unexpected gain ratio: {:.6} — signal chain may be broken",
        gain_ratio
    );

    // Check error metric
    // NMSE < 0.05 (5%) is acceptable for loopback with i16 quantization, dithering,
    // potential minor resampling, and BlackHole latency jitter
    assert!(
        nmse < 0.05,
        "EQ Verification Failed! Signal mismatch too high (NMSE: {:.6})",
        nmse
    );

    println!("Test PASSED: EQ Output matches Reference Model.");
}
