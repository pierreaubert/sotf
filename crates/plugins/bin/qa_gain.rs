use sotf_plugins::plugin_gain::{GainPlugin, GainPluginParams};
use sotf_plugins::{CountingAlloc, run_standard_tests, generate_dc, measure_peak_db};
use sotf_plugins::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    let params = GainPluginParams {
        gain_db: -6.0,
        channel_gains: vec![],
    };

    let mut inner = GainPlugin::from_params(channels, params).unwrap();
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Gain Plugin ===");

    // Test 1: Global Gain
    println!("\n[Test 1] Global Gain (-6.00dB)");
    let num_frames = 24000; // 500ms
    let mut buffer = vec![1.0; num_frames * channels];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = 20.0 * buffer[num_frames * channels - 1].abs().log10();
    println!("  Target: -6.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 6.00).abs() < 0.01);

    // Test 2: Per-Channel Gain
    println!("\n[Test 2] Per-Channel Gain (Ch0: 0dB, Ch1: -100dB)");
    inner.set_channel_gain_db(0, 0.0).unwrap();
    inner.set_channel_gain_db(1, -100.0).unwrap();

    // Process enough frames for convergence (5 * 20ms = 100ms minimum, but -100dB needs more)
    let mut buffer = vec![1.0; num_frames * channels];
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    let peak0 = 20.0 * buffer[(num_frames - 1) * channels].abs().log10();
    let peak1 = 20.0 * buffer[(num_frames - 1) * channels + 1].abs().log10();
    println!(
        "  Ch0 Measured: {:.2}dB, Ch1 Measured: {:.2}dB",
        peak0, peak1
    );
    assert!(peak0.abs() < 0.01);
    assert!(peak1 < -99.0);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "GainPlugin");

    println!("\n[ALL PASS] Gain QA Complete.");
}
