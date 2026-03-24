use sotf_host::{CountingAlloc, generate_dc, measure_peak_db, run_standard_tests};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_plugin_compressor::{CompressorPlugin, CompressorPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = CompressorPluginParams {
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        knee_db: 0.0,
        makeup_gain_db: 0.0,
        mix: 1.0,
        auto_makeup: false,
        link_channels: true,
        sidechain_hpf_hz: 0.0,
        sidechain_hpf_order: "2nd".to_string(),
        detection_mode: "peak".to_string(),
        lookahead_ms: 0.0,
        program_dependent_release: false,
        measured_auto_makeup: false,
        sidechain_external: false,
    };

    let mut inner = CompressorPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Compressor Plugin ===");

    // Test 1: Unity Gain below threshold
    println!("\n[Test 1] Below Threshold (-30dB)");
    let mut buffer = generate_dc(-30.0, 4800);
    let ctx = ProcessContext {
        sample_rate,
        num_frames: 4800,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Target: -30.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 30.0).abs() < 0.1);

    // Test 2: Ratio Verification (-10dB input, -20dB threshold, 4:1 ratio)
    // Overshoot = 10dB. GR = 10 * (1 - 1/4) = 7.5dB. Output should be -10 - 7.5 = -17.5dB
    println!("\n[Test 2] Ratio Accuracy (Input -10dB, Thresh -20dB, Ratio 4:1)");
    let num_frames = 48000; // 1 second
    let mut buffer = generate_dc(-10.0, num_frames);
    inner.reset();

    let block_size = 4096;
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx_block = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        inner
            .process_in_place(&mut buffer[pos..end], &ctx_block)
            .unwrap();
        pos = end;
    }

    let peak = measure_peak_db(&buffer[num_frames - 4800..]);
    println!("  Expected: -17.50dB, Measured: {:.2}dB", peak);
    assert!((peak + 17.5).abs() < 0.1);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "CompressorPlugin");

    println!("\n[ALL PASS] Compressor QA Complete.");
}
