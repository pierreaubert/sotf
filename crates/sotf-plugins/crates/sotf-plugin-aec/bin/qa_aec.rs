use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_aec::{AecPlugin, AecPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let params = AecPluginParams {
        echo_tail_ms: 100.0,
        step_size: 0.5,
        post_filter_enabled: true,
    };

    let mut plugin = AecPlugin::from_params(sample_rate, params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: AEC Plugin ===");

    // Test 1: Basic processing — no crash
    println!("\n[Test 1] Basic processing (512 frames)");
    let num_frames = 512;
    let input = vec![0.1f32; num_frames * 2]; // 2-channel interleaved
    let mut output = vec![0.0f32; num_frames]; // 1-channel output
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    plugin.process(&input, &mut output, &ctx).unwrap();
    println!("  Process completed: PASS");

    // Test 2: Latency reporting
    println!("\n[Test 2] Latency Reporting");
    let latency = plugin.latency_samples();
    println!("  Reported latency: {} samples", latency);
    assert!(latency > 0, "AEC should report non-zero latency");
    println!("  Latency > 0: PASS");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "AecPlugin");

    println!("\n[ALL PASS] AEC QA Complete.");
}
