use sotf_plugins::plugin_xtc::{XtcPlugin, XtcPluginParams};
use sotf_plugins::{Plugin, ProcessContext};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let mut params = XtcPluginParams::default();
    params.speaker_angle_deg = 30.0;
    params.head_radius_m = 0.0875;
    // Ensure auto-gain is on for energy preservation tests
    params.auto_gain_enabled = true;

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Xtc Plugin ===");

    // Test 1: Mono Signal Path (Center Image)
    println!("\n[Test 1] Mono Stability (L=R) with AutoGain");
    let num_frames = 48000; // 1 second to let AutoGain settle
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
        let ctx = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process(&input[pos*2..end*2], &mut output[pos*2..end*2], &ctx).unwrap();
        pos = end;
    }

    // In a mono signal, XTC should ideally preserve the signal with some gain scaling
    let measure_start = num_frames - 4800;
    let mut energy_in = 0.0f32;
    let mut energy_out = 0.0f32;
    for i in measure_start..num_frames {
        energy_in += output[i * 2].is_finite() as i32 as f32 * (input[i * 2].powi(2) + input[i * 2 + 1].powi(2));
        energy_out += output[i * 2].powi(2) + output[i * 2 + 1].powi(2);
    }
    
    let ratio = energy_out / energy_in;
    println!("  Energy Ratio (Out/In): {:.4}", ratio);
    // With AutoGain converged, ratio should be near 1.0 (Target: 0dB)
    assert!(ratio > 0.8 && ratio < 1.2, "AutoGain failed to normalize XTC energy");
    println!("  Mono Stability: PASS");

    // Test 2: Latency Reporting
    println!("\n[Test 2] Latency Reporting");
    let reported = plugin.latency_samples();
    println!("  Reported Latency: {} samples", reported);
    assert!(reported >= 1024);
    println!("  Latency: PASS");

    println!("\n[PASS] Xtc QA Complete.");
}
