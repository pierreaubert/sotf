use sotf_plugins::plugin_compressor::{CompressorPlugin, CompressorPluginParams};
use sotf_plugins::{InPlacePlugin, ProcessContext};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = CompressorPluginParams {
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        knee_db: 0.0,
        makeup_gain_db: 0.0,
        mix: 1.0,
        auto_makeup: false,
        link_channels: true,
        sidechain_hpf_hz: 0.0,
    };

    let mut plugin = CompressorPlugin::from_params(channels, params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Compressor Plugin ===");

    // Test 1: Unity Gain below threshold
    println!("\n[Test 1] Below Threshold (-30dB)");
    let mut buffer = generate_dc(sample_rate, -30.0, 4800);
    let ctx = ProcessContext { sample_rate, num_frames: 4800 };
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Target: -30.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 30.0).abs() < 0.1);

    // Test 2: Ratio Verification (-10dB input, -20dB threshold, 4:1 ratio)
    // Overshoot = 10dB. GR = 10 * (1 - 1/4) = 7.5dB. Output should be -10 - 7.5 = -17.5dB
    println!("\n[Test 2] Ratio Accuracy (Input -10dB, Thresh -20dB, Ratio 4:1)");
    let num_frames = 48000; // 1 second
    let mut buffer = generate_dc(sample_rate, -10.0, num_frames);
    plugin.reset();
    
    let block_size = 4096;
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx_block = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process_in_place(&mut buffer[pos..end], &ctx_block).unwrap();
        pos = end;
    }
    
    let peak = measure_peak_db(&buffer[num_frames - 4800..]);
    println!("  Expected: -17.50dB, Measured: {:.2}dB", peak);
    assert!((peak + 17.5).abs() < 0.1);

    // Test 3: Soft Knee Transition
    println!("\n[Test 3] Soft Knee (Knee 10dB)");
    plugin.set_parameter("knee".into(), sotf_plugins::ParameterValue::Float(10.0)).unwrap();
    // Input at threshold (-20dB) should already have some GR with soft knee
    let mut buffer = generate_dc(sample_rate, -20.0, 4800);
    let ctx_block = ProcessContext { sample_rate, num_frames: 4800 };
    plugin.process_in_place(&mut buffer, &ctx_block).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Input -20dB (at thresh), Output: {:.2}dB", peak);
    assert!(peak < -21.0); // Soft knee should reduce by ~1.8dB at threshold for 10dB knee

    println!("\n[PASS] Compressor QA Complete.");
}

fn generate_dc(sr: u32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    vec![amp; frames]
}

fn measure_peak_db(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    20.0 * peak.max(1e-10).log10()
}
