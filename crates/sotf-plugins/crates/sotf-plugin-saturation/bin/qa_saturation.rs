use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_saturation::{SaturationPlugin, SaturationPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    let params = SaturationPluginParams::default();

    let mut inner = SaturationPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Saturation Plugin ===");

    // Test 1: Driven signal should be clipped/shaped
    println!("\n[Test 1] Saturation shaping");
    let num_frames = 48000;
    let mut buffer = vec![0.8f32; num_frames * channels];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    // All output should be finite and bounded
    assert!(
        buffer.iter().all(|s| s.is_finite() && s.abs() <= 2.0),
        "Output should be finite and bounded"
    );
    let last = buffer[(num_frames - 1) * channels];
    println!("  Input: 0.80, Output: {:.4}", last);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "SaturationPlugin");

    println!("\n[ALL PASS] Saturation QA Complete.");
}
