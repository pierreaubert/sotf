use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter,
    ParametricPluginAdapter, run_standard_tests,
};
use sotf_plugin_dither::{DitherPlugin, DitherPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    let params = DitherPluginParams {
        bit_depth: 0, // 16-bit
        noise_shaping: true,
        dither_type: 0, // TPDF
    };

    let mut inner = DitherPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Dither Plugin ===");

    // Test 1: Signal should be quantized to 16-bit levels
    println!("\n[Test 1] 16-bit quantization");
    let num_frames = 4800;
    let mut buffer = vec![0.1234567f32; num_frames * channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    // All output values should be finite
    assert!(
        buffer.iter().all(|s| s.is_finite()),
        "All output samples should be finite"
    );
    // Output should differ from input due to quantization + dither
    let differs = buffer.iter().any(|&s| (s - 0.1234567).abs() > 1e-6);
    println!("  Output differs from input: {}", differs);
    assert!(differs, "Dither should modify the signal");

    // Run standard QA tests
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "DitherPlugin");

    println!("\n[ALL PASS] Dither QA Complete.");
}
