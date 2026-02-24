use sotf_plugins::plugin_limiter::{LimiterPlugin, LimiterPluginParams};
use sotf_plugins::{CountingAlloc, run_standard_tests, generate_dc, measure_peak_db};
use sotf_plugins::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = LimiterPluginParams {
        threshold_db: -1.0,
        release_ms: 10.0,
        lookahead_ms: 5.0,
        soft: false,
        mix: 1.0,
    };

    let mut inner = LimiterPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Limiter Plugin ===");

    // Test 1: Ceiling Enforcement
    println!("\n[Test 1] Ceiling Enforcement (Input +6dB, Thresh -1dB)");
    let num_frames = 4800;
    let mut buffer = generate_dc(6.0, num_frames);
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[1000..]); // Skip lookahead fill
    println!("  Ceiling: -1.00dB, Measured Peak: {:.2}dB", peak);
    assert!(peak <= -0.99 && peak > -1.1);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "LimiterPlugin");

    println!("\n[ALL PASS] Limiter QA Complete.");
}


