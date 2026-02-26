use sotf_plugin_expander::{ExpanderPlugin, ExpanderPluginParams};
use sotf_host::{CountingAlloc, run_standard_tests, generate_dc, measure_peak_db};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = ExpanderPluginParams {
        threshold_db: -20.0,
        ratio: 2.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        range_db: 40.0,
        knee_db: 0.0,
        hysteresis_db: 0.0,
        hold_ms: 0.0,
        mix: 1.0,
        link_channels: true,
        sidechain_hpf_hz: 0.0,
    };

    let mut inner = ExpanderPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Expander Plugin ===");

    // Test 1: Open Gate (Input above threshold)
    println!("\n[Test 1] Open State (Input -10dB, Thresh -20dB)");
    let mut buffer = generate_dc(-10.0, 4800);
    let ctx = ProcessContext {
        sample_rate,
        num_frames: 4800,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Target: -10.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 10.0).abs() < 0.1);

    // Test 2: Expansion Logic (Input -40dB, Thresh -20dB, Ratio 2:1)
    // Difference = 20dB. Expansion = 20 * (1 - 1/2) = 10dB. Output should be -40 - 10 = -50dB
    println!("\n[Test 2] Expansion Accuracy (Input -40dB, Thresh -20dB, Ratio 2:1)");
    let num_frames = 48000;
    let mut buffer = generate_dc(-40.0, num_frames);
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
    println!("  Expected: -50.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 50.0).abs() < 0.1);

    // Test 3: Range Limit (Range 10dB)
    println!("\n[Test 3] Range Limitation (Range 10dB)");
    inner
        .set_parameter("range".into(), sotf_host::ParameterValue::Float(10.0))
        .unwrap();
    let mut buffer = generate_dc(-60.0, num_frames);
    inner.reset();

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
    println!("  Expected: -70.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 70.0).abs() < 0.1);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "ExpanderPlugin");

    println!("\n[ALL PASS] Expander QA Complete.");
}


