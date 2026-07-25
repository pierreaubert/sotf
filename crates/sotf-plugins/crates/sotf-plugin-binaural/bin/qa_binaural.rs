use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_binaural::BinauralDecoderPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let input_channels = 2; // stereo input

    let mut plugin = BinauralDecoderPlugin::new(
        input_channels,
        1024,               // fft_size
        None,               // no HRTF file (uses built-in)
        0.5,                // externalization
        0.0,                // near_field_strength
        false,              // diffuse_field_eq
        80.0,               // lfe_crossover
        3.0,                // lfe_distance
        0.0,                // lfe_level
        Default::default(), // room_model
    );
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Binaural Plugin ===");

    // Test 1: Process stereo to binaural
    println!("\n[Test 1] Stereo → binaural processing");
    let num_frames = 1024;
    let input = vec![0.5f32; num_frames * input_channels];
    let output_channels = plugin.output_channels();
    let mut output = vec![0.0f32; num_frames * output_channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    // Output should be finite
    assert!(
        output.iter().all(|s| s.is_finite()),
        "All binaural output should be finite"
    );
    println!("  Output channels: {}, all finite: PASS", output_channels);

    // Run standard QA tests
    run_standard_tests(&mut plugin, "BinauralDecoderPlugin");

    println!("\n[ALL PASS] Binaural QA Complete.");
}
