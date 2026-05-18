use sotf_host::{CountingAlloc, measure_peak_db, run_standard_tests};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_multiband_expander::{MultibandExpanderPlugin, MultibandExpanderPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = MultibandExpanderPluginParams {
        num_bands: 3,
        crossover_frequencies: vec![200.0, 5000.0],
        threshold_db: -20.0,
        ratio: 2.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        range_db: 40.0,
        mix: 1.0,
        ..Default::default()
    };

    let mut inner = MultibandExpanderPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Multiband Expander Plugin ===");

    // Test 1: Expansion Accuracy
    println!("\n[Test 1] High Band Expansion (Input -40dB @ 10kHz, Thresh -20dB)");
    let num_frames = 48000;
    let mut buffer = generate_sine(sample_rate, 10000.0, -40.0, num_frames);

    let mut pos = 0;
    let block_size = 1024;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext::new(sample_rate, end - pos);
        inner.process_in_place(&mut buffer[pos..end], &ctx).unwrap();
        pos = end;
    }

    let peak = measure_peak_db(&buffer[num_frames - 4096..]);
    // Input -40dB, Thresh -20dB. Diff 20dB. Ratio 2:1. Expansion = 20 * 0.5 = 10dB. Output = -40 - 10 = -50dB
    println!("  Expected: ~-50.0dB, Measured: {:.2}dB", peak);
    assert!((peak + 50.0).abs() < 2.0);
    println!("  High Band Expansion: PASS");

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "MultibandExpanderPlugin");

    println!("\n[ALL PASS] Multiband Expander QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
