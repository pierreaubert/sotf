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
    let mut output = vec![0.0; num_frames * 2];

    let mut pos = 0;
    let block_size = 1024;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin_mono
            .process(&input[pos..end], &mut output[pos * 2..end * 2], &ctx)
            .unwrap();
        pos = end;
    }

    let mut energy_in = 0.0f32;
    let mut energy_out = 0.0f32;
    for i in num_frames - 2048..num_frames {
        energy_in += input[i].powi(2);
        energy_out += (output[i * 2].powi(2) + output[i * 2 + 1].powi(2)) * 0.5;
    }
    let ratio_mono = energy_out / energy_in;
    println!("  Energy Ratio (Mono): {:.4} (Target: ~1.0)", ratio_mono);

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
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin
            .process(&input[pos..end], &mut output_stereo[pos * 2..end * 2], &ctx)
            .unwrap();
        pos = end;
    }

    // Check signal frames (avoiding end-of-stream latency/silence)
    let check_idx = num_frames - 4096;
    let last_l = output_stereo[check_idx * 2];
    let last_r = output_stereo[check_idx * 2 + 1];
    let diff = (last_l - last_r).abs();
    println!("  L/R Difference at frame {}: {:.4}", check_idx, diff);
    assert!(
        diff > 0.01,
        "Pseudo-stereo should produce difference for 1kHz sine"
    );

    // Test 2: Energy Preservation
    let mut energy_in = 0.0f32;
    let mut energy_out = 0.0f32;
    // Measure energy in a stable region
    for i in num_frames - 8192..num_frames - 4096 {
        energy_in += input[i].powi(2);
        energy_out += (output_stereo[i * 2].powi(2) + output_stereo[i * 2 + 1].powi(2)) * 0.5;
    }
    let ratio = energy_out / energy_in;
    println!("  Energy Ratio (Stereo): {:.4} (Target: ~1.0)", ratio);
    // Decorrelation can cause some energy drop/boost depending on phase cancellation.
    // 0.5 to 1.5 is a reasonable range for this heuristic check.
    assert!(ratio > 0.5 && ratio < 1.5);

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
