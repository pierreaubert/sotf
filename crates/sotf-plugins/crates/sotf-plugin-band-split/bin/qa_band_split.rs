use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_split::{BandSplitPlugin, BandSplitPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let in_ch = 1;
    let params = BandSplitPluginParams {
        frequencies: vec![],
        frequency: 1000.0,
        num_bands: 2,
        crossover_type: "LR24".to_string(),
    };

    let mut plugin = BandSplitPlugin::from_params(in_ch, &params).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: BandSplit Plugin ===");

    // Test 1: Low frequency goes to band 0
    // Output per frame: [band0_ch0, band1_ch0] (out_ch = 2 for 1ch * 2bands)
    println!("\n[Test 1] Band separation (crossover at 1kHz, mono)");
    let num_frames = 4096;
    let input = generate_sine(sample_rate, 100.0, -10.0, num_frames);
    let out_ch = in_ch * 2; // 2 bands
    let mut output = vec![0.0f32; num_frames * out_ch];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    plugin.process(&input, &mut output, &ctx).unwrap();

    // Band 0 (low): every even sample in output
    let low_energy: f32 = (0..num_frames)
        .map(|f| {
            let s = output[f * out_ch];
            s * s
        })
        .sum::<f32>()
        / num_frames as f32;
    // Band 1 (high): every odd sample in output
    let high_energy: f32 = (0..num_frames)
        .map(|f| {
            let s = output[f * out_ch + 1];
            s * s
        })
        .sum::<f32>()
        / num_frames as f32;
    println!(
        "  100Hz: Low band energy={:.6}, High band energy={:.6}",
        low_energy, high_energy
    );
    assert!(
        low_energy > high_energy * 10.0,
        "100Hz should be mostly in low band"
    );

    // Run standard QA tests
    run_standard_tests(&mut plugin, "BandSplitPlugin");

    println!("\n[ALL PASS] BandSplit QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
