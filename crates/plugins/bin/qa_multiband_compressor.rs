use sotf_plugins::plugin_multiband_compressor::{MultibandCompressorPlugin, MultibandCompressorPluginParams};
use sotf_plugins::{InPlacePlugin, ProcessContext, ParameterValue};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let mut params = MultibandCompressorPluginParams::default();
    params.num_bands = 3;
    params.crossover_frequencies = vec![200.0, 5000.0];
    params.threshold_db = -20.0;
    params.ratio = 4.0;
    params.attack_ms = 5.0;
    params.release_ms = 50.0;
    params.mix = 1.0;

    let mut plugin = MultibandCompressorPlugin::from_params(channels, params);
    plugin.initialize(sample_rate).unwrap();

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
        let ctx = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process_in_place(&mut buffer[pos..end], &ctx).unwrap();
        pos = end;
    }
    
    let peak = measure_peak_db(&buffer[num_frames-4096..]);
    println!("  Measured: {:.2}dB", peak);
    assert!(peak < 0.0, "Should have significant compression");
    println!("  High Band Compression: PASS");

    // Test 2: Band Muting
    println!("\n[Test 2] Low Band Mute (100Hz signal muted by mid band solo)");
    plugin.reset();
    plugin.set_parameter("band_0_solo".into(), ParameterValue::Bool(false)).unwrap();
    plugin.set_parameter("band_1_solo".into(), ParameterValue::Bool(true)).unwrap();
    plugin.set_parameter("band_2_solo".into(), ParameterValue::Bool(false)).unwrap();
    
    let mut buffer = generate_sine(sample_rate, 100.0, -10.0, 4096);
    let ctx = ProcessContext { sample_rate, num_frames: 4096 };
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Muted Peak: {:.2}dB", peak);
    // 100Hz is in band 0. Band 1 starts at 200Hz.
    // 24dB/oct crossover should give significant attenuation.
    // -28dB was measured, so -25dB is a safe threshold.
    assert!(peak < -25.0); 

    println!("\n[PASS] Multiband Compressor QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames).map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp).collect()
}

fn measure_peak_db(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    20.0 * peak.max(1e-10).log10()
}
