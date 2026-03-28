use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_crossfeed::{CrossfeedPlugin, CrossfeedPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let params = CrossfeedPluginParams::default();

    let mut inner = CrossfeedPlugin::new(params).unwrap();
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Crossfeed Plugin ===");

    // Test 1: Hard-panned signal should bleed to opposite channel
    println!("\n[Test 1] Crossfeed bleed (left-only → right)");
    let num_frames = 48000; // 1 second
    let channels = 2;
    let mut buffer = vec![0.0f32; num_frames * channels];
    // Left channel = 1.0, Right channel = 0.0
    for i in 0..num_frames {
        buffer[i * channels] = 0.5;
        buffer[i * channels + 1] = 0.0;
    }
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    let right_last = buffer[(num_frames - 1) * channels + 1];
    println!("  Right channel (should have bleed): {:.4}", right_last);
    assert!(
        right_last.abs() > 0.01,
        "Crossfeed should bleed to right channel"
    );

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "CrossfeedPlugin");

    println!("\n[ALL PASS] Crossfeed QA Complete.");
}
