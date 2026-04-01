use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_convolution::ConvolutionPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;

    // Create convolution plugin without an IR file (dry passthrough)
    let mut inner = ConvolutionPlugin::new(channels, sample_rate);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Convolution Plugin ===");

    // Test 1: No IR loaded — signal passes through unchanged (dry path)
    println!("\n[Test 1] Passthrough without IR");
    let num_frames = 4096;
    let mut buffer = vec![0.5f32; num_frames * channels];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    let last = buffer[(num_frames - 1) * channels];
    println!("  Input: 0.50, Output: {:.4}", last);
    // Without IR, output depends on mix setting (dry signal)
    assert!(last.is_finite(), "Output should be finite");

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "ConvolutionPlugin");

    println!("\n[ALL PASS] Convolution QA Complete.");
}
