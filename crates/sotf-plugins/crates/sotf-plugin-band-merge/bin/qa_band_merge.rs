use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_merge::{BandMergePlugin, BandMergePluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let output_channels = 2;
    let num_bands = 2;
    let params = BandMergePluginParams {
        bands: num_bands,
        band_gains_db: vec![0.0, 0.0],
        band_mutes: vec![false, false],
    };

    let mut plugin = BandMergePlugin::from_params(output_channels, &params).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: BandMerge Plugin ===");

    // Test 1: Sum two identical bands should double amplitude
    println!("\n[Test 1] Sum of two bands");
    let num_frames = 4096;
    // Input: output_channels * num_bands = 2 * 2 = 4 interleaved channels
    let input_channels = output_channels * num_bands;
    let mut input = vec![0.0f32; num_frames * input_channels];
    for i in 0..num_frames {
        for ch in 0..output_channels {
            // Band 0: value 0.5
            input[i * input_channels + ch] = 0.5;
            // Band 1: value 0.3
            input[i * input_channels + output_channels + ch] = 0.3;
        }
    }
    let mut output = vec![0.0f32; num_frames * output_channels];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    plugin.process(&input, &mut output, &ctx).unwrap();

    // Output should be sum of bands: 0.5 + 0.3 = 0.8
    let last_sample = output[(num_frames - 1) * output_channels];
    println!("  Expected: 0.80, Measured: {:.2}", last_sample);
    assert!(
        (last_sample - 0.8).abs() < 0.01,
        "Sum should be ~0.8, got {}",
        last_sample
    );

    // Run standard QA tests
    run_standard_tests(&mut plugin, "BandMergePlugin");

    println!("\n[ALL PASS] BandMerge QA Complete.");
}
