use sotf_host::plugin::{InPlacePluginAdapter, ProcessContext};
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, run_standard_tests,
};
use sotf_plugin_transient_shaper::{TransientShaperPlugin, TransientShaperPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    let params = TransientShaperPluginParams {
        attack: 0.0,
        sustain: 0.0,
        sensitivity_db: 0.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };

    let mut inner = TransientShaperPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: TransientShaper Plugin ===");

    // Test 1: Neutral settings should pass through unchanged
    println!("\n[Test 1] Neutral passthrough (attack=0, sustain=0)");
    let num_frames = 24000;
    let mut buffer = vec![0.5f32; num_frames * channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    let last = buffer[(num_frames - 1) * channels];
    println!("  Input: 0.50, Output: {:.4}", last);
    assert!(
        (last - 0.5).abs() < 0.1,
        "Neutral settings should be close to passthrough"
    );

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(ParametricInPlacePluginAdapter::new(inner));
    run_standard_tests(&mut plugin, "TransientShaperPlugin");

    println!("\n[ALL PASS] TransientShaper QA Complete.");
}
