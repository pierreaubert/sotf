use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_matrix::MatrixPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let input_channels = 2;
    let output_channels = 2;

    let mut plugin = MatrixPlugin::new(input_channels, output_channels);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Matrix Plugin ===");

    // Test 1: Identity matrix — passthrough
    println!("\n[Test 1] Identity matrix passthrough");
    let num_frames = 4096;
    let mut input = vec![0.0f32; num_frames * input_channels];
    for i in 0..num_frames {
        input[i * input_channels] = 0.5; // L
        input[i * input_channels + 1] = 0.3; // R
    }
    let mut output = vec![0.0f32; num_frames * output_channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    let out_l = output[(num_frames - 1) * output_channels];
    let out_r = output[(num_frames - 1) * output_channels + 1];
    println!("  L: in=0.50, out={:.4}", out_l);
    println!("  R: in=0.30, out={:.4}", out_r);
    assert!((out_l - 0.5).abs() < 0.05, "L should pass through identity");
    assert!((out_r - 0.3).abs() < 0.05, "R should pass through identity");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "MatrixPlugin");

    println!("\n[ALL PASS] Matrix QA Complete.");
}
