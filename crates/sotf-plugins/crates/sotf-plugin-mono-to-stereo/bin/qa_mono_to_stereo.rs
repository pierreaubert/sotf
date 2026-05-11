use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let params = MonoToStereoPluginParams {
        stereo_width: 1.0,
        freq_dependent: false,
        haas_delay_ms: 0.0,
    };

    let mut plugin = MonoToStereoPlugin::from_params(1, params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: MonoToStereo Plugin ===");

    // Test 0: Mono Passthrough (width = 0)
    println!("\n[Test 0] Mono Passthrough (width = 0)");
    let params_mono = MonoToStereoPluginParams {
        stereo_width: 0.0,
        freq_dependent: false,
        haas_delay_ms: 0.0,
    };
    let mut plugin_mono = MonoToStereoPlugin::from_params(1, params_mono);
    plugin_mono.initialize(sample_rate).unwrap();

    let num_frames = 48000;
    let input = generate_sine(sample_rate, 1000.0, -6.0, num_frames);
    let block_size = 1024;
    let mut output = vec![0.0; num_frames * 2];
    process_streaming(
        &mut plugin_mono,
        &input,
        &mut output,
        sample_rate,
        block_size,
    );

    let ratio_mono = energy_ratio(&input, &output, 8192, num_frames - 8192);
    println!("  Energy Ratio (Mono): {:.4} (Target: ~1.0)", ratio_mono);
    assert!(ratio_mono > 0.95 && ratio_mono < 1.05);

    // Test 1: Pseudo-Stereo Width (using 1kHz sine where decorrelation is active)
    println!("\n[Test 1] Pseudo-Stereo Width (1kHz Sine)");
    let mut plugin = MonoToStereoPlugin::from_params(
        1,
        MonoToStereoPluginParams {
            stereo_width: 1.0,
            freq_dependent: false,
            haas_delay_ms: 0.0,
        },
    );
    plugin.initialize(sample_rate).unwrap();

    let mut output_stereo = vec![0.0; num_frames * 2];
    process_streaming(
        &mut plugin,
        &input,
        &mut output_stereo,
        sample_rate,
        block_size,
    );

    // Check a settled region instead of one arbitrary frame: a phase-shifted
    // sine can cross L/R at individual samples while still being decorrelated.
    let diff = rms_lr_difference(&output_stereo, 8192, num_frames - 8192);
    println!("  RMS L/R Difference: {:.4}", diff);
    assert!(
        diff > 0.01,
        "Pseudo-stereo should produce difference for 1kHz sine"
    );

    // Test 1b: Energy Preservation
    println!("\n[Test 1b] Energy Preservation (Broadband)");
    let broadband = generate_broadband(sample_rate, -18.0, num_frames);
    let mut output_broadband = vec![0.0; num_frames * 2];
    plugin.reset();
    process_streaming(
        &mut plugin,
        &broadband,
        &mut output_broadband,
        sample_rate,
        block_size,
    );

    let ratio = energy_ratio(&broadband, &output_broadband, 8192, num_frames - 8192);
    println!("  Energy Ratio (Stereo): {:.4} (Target: ~1.0)", ratio);
    assert!(ratio > 0.8 && ratio < 1.2);

    // Run standard QA tests
    run_standard_tests(&mut plugin, "MonoToStereoPlugin");

    println!("\n[ALL PASS] MonoToStereo QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}

fn generate_broadband(sr: u32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let mut s = 0.0_f32;
            let mut freq = 200.0_f32;
            while freq < 16000.0 {
                s += (2.0 * PI * freq * t).sin();
                freq *= 1.07;
            }
            s * amp * 0.04
        })
        .collect()
}

fn process_streaming(
    plugin: &mut MonoToStereoPlugin,
    input: &[f32],
    output: &mut [f32],
    sample_rate: u32,
    block_size: usize,
) {
    let mut pos = 0;
    while pos < input.len() {
        let end = (pos + block_size).min(input.len());
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin
            .process(&input[pos..end], &mut output[pos * 2..end * 2], &ctx)
            .unwrap();
        pos = end;
    }
}

fn energy_ratio(input: &[f32], output: &[f32], start: usize, end: usize) -> f32 {
    let mut energy_in = 0.0_f32;
    let mut energy_out = 0.0_f32;
    for i in start..end {
        energy_in += input[i].powi(2);
        energy_out += (output[i * 2].powi(2) + output[i * 2 + 1].powi(2)) * 0.5;
    }
    energy_out / energy_in
}

fn rms_lr_difference(output: &[f32], start: usize, end: usize) -> f32 {
    let mut diff_sq = 0.0_f32;
    for i in start..end {
        diff_sq += (output[i * 2] - output[i * 2 + 1]).powi(2);
    }
    (diff_sq / (end - start) as f32).sqrt()
}
