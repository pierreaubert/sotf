use sotf_host::{CountingAlloc, generate_dc, measure_peak_db, run_standard_tests};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, ParameterValue, ProcessContext};
use sotf_plugin_multiband_compressor::{
    MultibandCompressorPlugin, MultibandCompressorPluginParams,
};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = MultibandCompressorPluginParams {
        num_bands: 3,
        crossover_frequencies: vec![200.0, 5000.0],
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        mix: 1.0,
        ..Default::default()
    };

    let mut inner = MultibandCompressorPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Multiband Compressor Plugin ===");

    // Test 1: High Band Isolation (10kHz above 5kHz xover)
    println!("\n[Test 1] High Band Compression (Input +10dB @ 10kHz, Thresh -20dB)");
    let num_frames = 24576; // multiple of 1024
    let mut buffer = generate_sine(sample_rate, 10000.0, 10.0, num_frames);

    // Process in small blocks
    let mut pos = 0;
    let block_size = 1024;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        inner.process_in_place(&mut buffer[pos..end], &ctx).unwrap();
        pos = end;
    }

    let peak = measure_peak_db(&buffer[num_frames - 4096..]);
    println!("  Measured: {:.2}dB", peak);
    assert!(peak < 0.0, "Should have significant compression");
    println!("  High Band Compression: PASS");

    // Test 2: Band Muting
    println!("\n[Test 2] Low Band Mute (100Hz signal muted by mid band solo)");
    inner.reset();
    inner
        .set_parameter("band_0_solo".into(), ParameterValue::Bool(false))
        .unwrap();
    inner
        .set_parameter("band_1_solo".into(), ParameterValue::Bool(true))
        .unwrap();
    inner
        .set_parameter("band_2_solo".into(), ParameterValue::Bool(false))
        .unwrap();

    let mut buffer = generate_sine(sample_rate, 100.0, -10.0, 4096);
    let ctx = ProcessContext {
        sample_rate,
        num_frames: 4096,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Muted Peak: {:.2}dB", peak);
    assert!(peak < -25.0);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "MultibandCompressorPlugin");

    println!("\n[ALL PASS] Multiband Compressor QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
