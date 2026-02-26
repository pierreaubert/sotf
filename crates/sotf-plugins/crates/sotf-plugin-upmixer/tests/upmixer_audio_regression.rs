//! Audio regression tests for upmixer plugin.
//!
//! These tests compare the current plugin output against pre-generated golden
//! reference files to detect audio regressions.
//!
//! Thresholds:
//! - RMSE < 0.01
//! - Correlation > 0.99

use hound::{SampleFormat, WavReader, WavSpec};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_upmixer::UpmixerPlugin;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

const GOLDEN_DIR: &str = "data_generated/test-plugin-upmixer";
const SAMPLE_RATE: u32 = 48000;
const FFT_SIZE: usize = 2048;

const RMSE_THRESHOLD: f32 = 0.01;
const CORRELATION_THRESHOLD: f32 = 0.99;

struct AudioMetrics {
    rmse: f32,
    correlation: f32,
    max_abs_error: f32,
}

fn load_wav(path: &Path) -> Vec<f32> {
    let file = File::open(path).expect(&format!("Failed to open: {}", path.display()));
    let reader = BufReader::new(file);
    let wav = WavReader::new(reader).expect(&format!("Failed to read WAV: {}", path.display()));

    let samples: Vec<f32> = if wav.spec().sample_format == SampleFormat::Float {
        wav.into_samples::<f32>().filter_map(|s| s.ok()).collect()
    } else {
        wav.into_samples::<i32>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 32767.0)
            .collect()
    };

    samples
}

fn compute_metrics(output: &[f32], reference: &[f32]) -> AudioMetrics {
    assert_eq!(
        output.len(),
        reference.len(),
        "Length mismatch: {} vs {}",
        output.len(),
        reference.len()
    );

    let n = output.len() as f32;

    // RMSE
    let mut sum_sq_error = 0.0_f32;
    let mut max_abs_error = 0.0_f32;

    for (o, r) in output.iter().zip(reference.iter()) {
        let error = o - r;
        sum_sq_error += error * error;
        max_abs_error = max_abs_error.max(error.abs());
    }

    let rmse = (sum_sq_error / n).sqrt();

    // Correlation
    let mut sum_x = 0.0_f32;
    let mut sum_y = 0.0_f32;
    let mut sum_xy = 0.0_f32;
    let mut sum_x2 = 0.0_f32;
    let mut sum_y2 = 0.0_f32;

    for (x, y) in output.iter().zip(reference.iter()) {
        sum_x += x;
        sum_y += y;
        sum_xy += x * y;
        sum_x2 += x * x;
        sum_y2 += y * y;
    }

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    let correlation = if denominator > 1e-9 {
        numerator / denominator
    } else {
        0.0
    };

    AudioMetrics {
        rmse,
        correlation: correlation.abs(),
        max_abs_error,
    }
}

fn create_upmixer(config: &str) -> UpmixerPlugin {
    UpmixerPlugin::new(
        FFT_SIZE, config, 1.0,   // gain_front_direct
        0.5,   // gain_front_ambient
        1.0,   // gain_rear_ambient
        120.0, // lfe_cutoff_hz
        0.5,   // stereo_width
        250.0, // bandpass_hz
        0.5,   // height_gain
        1.0,   // lfe_gain
        false, // enable_subharmonic_synth
        0.5,   // subharmonic_gain
    )
}

fn process_upmixer_signal(plugin: &mut UpmixerPlugin, signal_name: &str) -> Vec<f32> {
    let input = generate_test_signal(signal_name);
    let num_blocks = input.len() / (FFT_SIZE * 2);
    let num_output_channels = plugin.output_channels();
    let output_len = num_blocks * FFT_SIZE * num_output_channels;
    let mut output = vec![0.0_f32; output_len];

    for block in 0..num_blocks {
        let input_offset = block * FFT_SIZE * 2;
        let output_offset = block * FFT_SIZE * num_output_channels;

        let input_block = &input[input_offset..input_offset + FFT_SIZE * 2];
        let mut output_block = vec![0.0_f32; FFT_SIZE * num_output_channels];

        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: FFT_SIZE,
        };

        plugin
            .process(input_block, &mut output_block, &context)
            .unwrap();

        output[output_offset..output_offset + output_block.len()].copy_from_slice(&output_block);
    }

    output
}

fn generate_test_signal(name: &str) -> Vec<f32> {
    let num_blocks = 10;
    let num_frames = num_blocks * FFT_SIZE;
    let mut data = vec![0.0_f32; num_frames * 2];

    match name {
        "multisine" => {
            let freqs = [
                40.0, 80.0, 160.0, 320.0, 640.0, 1280.0, 2560.0, 5120.0, 10240.0, 16000.0,
            ];
            for i in 0..num_frames {
                let t = i as f32 / SAMPLE_RATE as f32;
                let ch = i % 2;
                let mut sum = 0.0_f32;
                for (fi, &freq) in freqs.iter().enumerate() {
                    let phase = 2.0 * std::f32::consts::PI * freq * t + (fi as f32 * 0.1);
                    sum += phase.sin() * 0.1;
                }
                data[i * 2] = if ch == 0 { sum } else { sum * 0.95 };
            }
        }
        "sweep_20_20k" => {
            let log_start = 20.0_f32.ln();
            let log_end = 20000.0_f32.ln();
            for i in 0..num_frames {
                let t = i as f32 / SAMPLE_RATE as f32;
                let freq = (log_start + t * (log_end - log_start)).exp();
                let phase = 2.0 * std::f32::consts::PI * freq * t;
                data[i * 2] = phase.sin() * 0.5;
                data[i * 2 + 1] = phase.sin() * 0.5;
            }
        }
        "dialogue" => {
            let fundamental = 180.0_f32;
            for i in 0..num_frames {
                let t = i as f32 / SAMPLE_RATE as f32;
                let envelope = ((t * 3.0).sin() * 0.5 + 0.5).max(0.0);
                let mut sum = 0.0_f32;
                for h in 1..=6 {
                    let freq = fundamental * h as f32;
                    let phase = 2.0 * std::f32::consts::PI * freq * t;
                    sum += phase.sin() * (1.0 / h as f32);
                }
                let sample = sum * envelope * 0.3;
                data[i * 2] = sample;
                data[i * 2 + 1] = sample * 0.98;
            }
        }
        "pink_noise" => {
            let mut b0 = 0.0_f32;
            let mut b1 = 0.0_f32;
            let mut b2 = 0.0_f32;
            let mut b3 = 0.0_f32;
            let mut b4 = 0.0_f32;
            let mut b5 = 0.0_f32;
            let mut b6 = 0.0_f32;

            let mut seed = 12345u32;
            let mut rand_f32 = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            };

            for i in 0..num_frames {
                let white = rand_f32();
                b0 = 0.99886 * b0 + white * 0.0555179;
                b1 = 0.99332 * b1 + white * 0.0750759;
                b2 = 0.96900 * b2 + white * 0.1538520;
                b3 = 0.86650 * b3 + white * 0.3104856;
                b4 = 0.55000 * b4 + white * 0.5329522;
                b5 = -0.7616 * b5 - white * 0.0168980;
                let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
                b6 = white * 0.115926;

                let sample = pink * 0.11;
                data[i * 2] = sample;
                data[i * 2 + 1] = sample * 0.98;
            }
        }
        _ => panic!("Unknown signal: {}", name),
    }

    data
}

fn get_golden_path(config: &str, signal: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(GOLDEN_DIR)
        .join(config)
        .join(format!("{}.wav", signal))
}

// ============================================================================
// Test Cases
// ============================================================================

#[test]
fn test_upmixer_5_1_multisine_regression() {
    let golden_path = get_golden_path("5.1", "multisine");
    if !golden_path.exists() {
        eprintln!(
            "Skipping: golden file not found at {}",
            golden_path.display()
        );
        return;
    }

    let golden = load_wav(&golden_path);

    let mut plugin = create_upmixer("5.1");
    plugin.initialize(SAMPLE_RATE).unwrap();

    let output = process_upmixer_signal(&mut plugin, "multisine");
    let metrics = compute_metrics(&output, &golden);

    assert!(
        metrics.rmse < RMSE_THRESHOLD,
        "RMSE {} exceeds threshold {}",
        metrics.rmse,
        RMSE_THRESHOLD
    );
    assert!(
        metrics.correlation > CORRELATION_THRESHOLD,
        "Correlation {} below threshold {}",
        metrics.correlation,
        CORRELATION_THRESHOLD
    );
}

#[test]
fn test_upmixer_5_1_sweep_regression() {
    let golden_path = get_golden_path("5.1", "sweep_20_20k");
    if !golden_path.exists() {
        eprintln!("Skipping: golden file not found");
        return;
    }

    let golden = load_wav(&golden_path);

    let mut plugin = create_upmixer("5.1");
    plugin.initialize(SAMPLE_RATE).unwrap();

    let output = process_upmixer_signal(&mut plugin, "sweep_20_20k");
    let metrics = compute_metrics(&output, &golden);

    assert!(
        metrics.rmse < RMSE_THRESHOLD,
        "RMSE {} exceeds threshold",
        metrics.rmse
    );
    assert!(
        metrics.correlation > CORRELATION_THRESHOLD,
        "Correlation {} below threshold",
        metrics.correlation
    );
}

#[test]
fn test_upmixer_5_1_dialogue_regression() {
    let golden_path = get_golden_path("5.1", "dialogue");
    if !golden_path.exists() {
        eprintln!("Skipping: golden file not found");
        return;
    }

    let golden = load_wav(&golden_path);

    let mut plugin = create_upmixer("5.1");
    plugin.initialize(SAMPLE_RATE).unwrap();

    let output = process_upmixer_signal(&mut plugin, "dialogue");
    let metrics = compute_metrics(&output, &golden);

    assert!(
        metrics.rmse < RMSE_THRESHOLD,
        "RMSE {} exceeds threshold",
        metrics.rmse
    );
    assert!(
        metrics.correlation > CORRELATION_THRESHOLD,
        "Correlation {} below threshold",
        metrics.correlation
    );
}

#[test]
fn test_upmixer_7_1_4_regression() {
    let golden_path = get_golden_path("7.1.4", "multisine");
    if !golden_path.exists() {
        eprintln!("Skipping: golden file not found");
        return;
    }

    let golden = load_wav(&golden_path);

    let mut plugin = create_upmixer("7.1.4");
    plugin.initialize(SAMPLE_RATE).unwrap();

    let output = process_upmixer_signal(&mut plugin, "multisine");
    let metrics = compute_metrics(&output, &golden);

    assert!(
        metrics.rmse < RMSE_THRESHOLD,
        "RMSE {} exceeds threshold",
        metrics.rmse
    );
    assert!(
        metrics.correlation > CORRELATION_THRESHOLD,
        "Correlation {} below threshold",
        metrics.correlation
    );
}

#[test]
fn test_upmixer_all_configs_produce_output() {
    for config in ["5.1", "7.1", "5.1.2", "7.1.4", "9.1.6"] {
        let mut plugin = create_upmixer(config);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let output = process_upmixer_signal(&mut plugin, "multisine");

        let energy: f32 = output.iter().map(|x| x * x).sum();
        assert!(energy > 0.0, "Config {} produced no output", config);
    }
}

#[test]
fn test_upmixer_no_clipping() {
    let mut plugin = create_upmixer("5.1");
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Process loud signal
    let mut loud_input = vec![0.9_f32; FFT_SIZE * 2];
    let mut output = vec![0.0_f32; FFT_SIZE * plugin.output_channels()];

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: FFT_SIZE,
    };

    plugin
        .process(&mut loud_input, &mut output, &context)
        .unwrap();

    let max_sample = output.iter().fold(0.0_f32, |m, &x| m.max(x.abs()));
    assert!(
        max_sample <= 1.0,
        "Output clipped: max sample = {}",
        max_sample
    );
}

#[test]
fn test_upmixer_silence_input() {
    let mut plugin = create_upmixer("5.1");
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = vec![0.0_f32; FFT_SIZE * 2];
    let mut output = vec![0.0_f32; FFT_SIZE * plugin.output_channels()];

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: FFT_SIZE,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // First block may have some output due to initialization, but subsequent should be silent
    let energy: f32 = output.iter().map(|x| x * x).sum();
    // Allow some tolerance for FFT edge effects
    assert!(
        energy < 1.0,
        "Silence input produced significant output: {}",
        energy
    );
}

#[test]
fn test_upmixer_stereo_imaging_preserved() {
    let mut plugin = create_upmixer("5.1");
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Mono input (same on L and R)
    let mut mono_input = vec![0.0_f32; FFT_SIZE * 2];
    for i in 0..FFT_SIZE {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        mono_input[i * 2] = sample; // L
        mono_input[i * 2 + 1] = sample; // R (same as L)
    }

    let mut output = vec![0.0_f32; FFT_SIZE * plugin.output_channels()];

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: FFT_SIZE,
    };

    plugin
        .process(&mut mono_input, &mut output, &context)
        .unwrap();

    // Front left and front right should have similar energy for mono input
    let num_ch = plugin.output_channels();
    let mut energy_left = 0.0_f32;
    let mut energy_right = 0.0_f32;

    for i in 0..FFT_SIZE {
        energy_left += output[i * num_ch].powi(2);
        energy_right += output[i * num_ch + 1].powi(2);
    }

    let ratio = energy_left / energy_right.max(1e-9);
    assert!(
        (ratio - 1.0).abs() < 0.5,
        "Mono input produced asymmetric output: ratio = {}",
        ratio
    );
}
