use sotf_plugins::plugin_expander::{ExpanderPlugin, ExpanderPluginParams};
use sotf_plugins::{InPlacePlugin, ProcessContext};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = ExpanderPluginParams {
        threshold_db: -20.0,
        ratio: 2.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        range_db: 40.0,
        knee_db: 0.0,
        hysteresis_db: 0.0,
        hold_ms: 0.0,
        mix: 1.0,
        link_channels: true,
        sidechain_hpf_hz: 0.0,
    };

    let mut plugin = ExpanderPlugin::from_params(channels, params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Expander Plugin ===");

    // Test 1: Open Gate (Input above threshold)
    println!("\n[Test 1] Open State (Input -10dB, Thresh -20dB)");
    let mut buffer = generate_dc(sample_rate, -10.0, 4800);
    let ctx = ProcessContext { sample_rate, num_frames: 4800 };
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Target: -10.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 10.0).abs() < 0.1);

    // Test 2: Expansion Logic (Input -40dB, Thresh -20dB, Ratio 2:1)
    // Difference = 20dB. Expansion = 20 * (1 - 1/2) = 10dB. Output should be -40 - 10 = -50dB
    println!("\n[Test 2] Expansion Accuracy (Input -40dB, Thresh -20dB, Ratio 2:1)");
    let num_frames = 48000;
    let mut buffer = generate_dc(sample_rate, -40.0, num_frames);
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
    println!("  Expected: -50.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 50.0).abs() < 0.1);

    // Test 3: Range Limit (Range 10dB)
    println!("\n[Test 3] Range Limitation (Range 10dB)");
    plugin.set_parameter("range".into(), sotf_plugins::ParameterValue::Float(10.0)).unwrap();
    let mut buffer = generate_dc(sample_rate, -60.0, num_frames);
    plugin.reset();
    
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx_block = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process_in_place(&mut buffer[pos..end], &ctx_block).unwrap();
        pos = end;
    }
    
    let peak = measure_peak_db(&buffer[num_frames - 4800..]);
    println!("  Expected: -70.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 70.0).abs() < 0.1);

    println!("\n[PASS] Expander QA Complete.");
}

fn generate_dc(sr: u32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    vec![amp; frames]
}

fn measure_peak_db(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    20.0 * peak.max(1e-10).log10()
}
