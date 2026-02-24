use sotf_plugins::plugin_gate::{GatePlugin, GatePluginParams};
use sotf_plugins::qa_util::{CountingAlloc, run_standard_tests};
use sotf_plugins::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = GatePluginParams {
        threshold_db: -20.0,
        ratio: 100.0, // Hard gate
        attack_ms: 1.0,
        hold_ms: 10.0,
        release_ms: 50.0,
        mix: 1.0,
        link_channels: true,
        sidechain_hpf_hz: 0.0,
    };

    let mut inner = GatePlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Gate Plugin ===");

    // Test 1: Open Gate (Input above threshold)
    println!("\n[Test 1] Open State (Input -10dB, Thresh -20dB)");
    let mut buffer = generate_dc(sample_rate, -10.0, 4800);
    let ctx = ProcessContext {
        sample_rate,
        num_frames: 4800,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer);
    println!("  Target: -10.00dB, Measured: {:.2}dB", peak);
    assert!((peak + 10.0).abs() < 0.1);

    // Test 2: Closed Gate (Input below threshold)
    println!("\n[Test 2] Closed State (Input -40dB, Thresh -20dB)");
    // Process 1 second to fully close
    let num_frames = 48000;
    let mut buffer = generate_dc(sample_rate, -40.0, num_frames);
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
    println!("  Expected: ~ -59.80dB, Measured: {:.2}dB", peak);
    assert!(peak < -59.0);

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "GatePlugin");

    println!("\n[ALL PASS] Gate QA Complete.");
}

fn generate_dc(_sr: u32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    vec![amp; frames]
}

fn measure_peak_db(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    20.0 * peak.max(1e-10).log10()
}
