use sotf_host::{CountingAlloc, generate_dc, measure_peak_db, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_xtc::{XtcPlugin, XtcPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let params = XtcPluginParams {
        speaker_angle_deg: 30.0,
        head_radius_m: 0.0875,
        auto_gain_enabled: true,
        ..Default::default()
    };

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Xtc Plugin ===");

    // Test 1: Mono Signal Path (Center Image)
    println!("\n[Test 1] Mono Stability (L=R) with AutoGain");
    let num_frames = 48000;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let s = (2.0 * PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5;
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let block_size = 1024;
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin
            .process(
                &input[pos * 2..end * 2],
                &mut output[pos * 2..end * 2],
                &ctx,
            )
            .unwrap();
        pos = end;
    }

    let measure_start = num_frames - 4800;
    let mut energy_in = 0.0f32;
    let mut energy_out = 0.0f32;
    for i in measure_start..num_frames {
        energy_in += input[i * 2].powi(2) + input[i * 2 + 1].powi(2);
        energy_out += output[i * 2].powi(2) + output[i * 2 + 1].powi(2);
    }

    let ratio = energy_out / energy_in;
    println!("  Energy Ratio (Out/In): {:.4}", ratio);
    assert!(
        ratio > 0.8 && ratio < 1.2,
        "AutoGain failed to normalize XTC energy"
    );
    println!("  Mono Stability: PASS");

    // Run standard QA tests (Latency, Real-time safety, Performance)
    run_standard_tests(&mut plugin, "XtcPlugin");

    println!("\n[ALL PASS] Xtc QA Complete.");
}
