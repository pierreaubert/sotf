use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, assert_no_allocs,
    measure_peak_db, run_standard_tests,
};
use sotf_plugin_linear_phase_eq::{BandConfig, LinearPhaseEqPlugin, LinearPhaseEqPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 1, // Medium FIR length
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 6.0,
            active: true,
        }],
    };

    let mut inner = LinearPhaseEqPlugin::from_params(channels, sample_rate, params).unwrap();
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: LinearPhaseEQ Plugin ===");

    // Test 1: Peak boost at 1kHz — process in blocks to handle STFT latency
    println!("\n[Test 1] Peak Boost (+6dB at 1kHz)");
    let block_size = 1024;
    let ctx = ProcessContext::new(sample_rate, block_size);

    // Warm up: process enough blocks for the FIR pipeline to fill
    for _ in 0..20 {
        let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, block_size);
        inner.process_in_place(&mut buffer, &ctx).unwrap();
    }

    // Measure on a settled block
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, block_size);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let peak = measure_peak_db(&buffer[block_size / 2..]);
    println!("  Expected: ~-4.0dB, Measured: {:.2}dB", peak);
    assert!(
        (peak + 4.0).abs() < 2.0,
        "1kHz should be boosted ~6dB, got {:.2}dB",
        peak
    );

    // Run standard QA tests
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "LinearPhaseEqPlugin");

    println!("\n[Test 5] Partitioned-convolution layout/block matrix");
    for channels in [1, 2, 8, 12] {
        for fir_length_index in 0..4 {
            let mut candidate = LinearPhaseEqPlugin::from_params(
                channels,
                sample_rate,
                LinearPhaseEqPluginParams {
                    num_filters: 1,
                    fir_length_index,
                    phase_mode_index: 0,
                    auto_gain: false,
                    mix: 1.0,
                    filters: vec![],
                },
            )
            .unwrap();
            for frames in [16, 32, 64, 127, 256, 512, 1_024] {
                let mut audio = vec![0.0; frames * channels];
                let context = ProcessContext::new(sample_rate, frames);
                candidate.process_in_place(&mut audio, &context).unwrap();
                assert_no_allocs("LinearPhaseEqPlugin partitioned process", || {
                    candidate.process_in_place(&mut audio, &context).unwrap();
                });
                let started = std::time::Instant::now();
                candidate.process_in_place(&mut audio, &context).unwrap();
                let elapsed = started.elapsed().as_secs_f64();
                let deadline = frames as f64 / sample_rate as f64;
                assert!(
                    elapsed < deadline,
                    "{channels}ch length-index {fir_length_index} block {frames}: {elapsed:.6}s >= {deadline:.6}s"
                );
            }
        }
    }
    println!("  all channels/taps/blocks zero-allocation and below deadline: PASS");

    println!("\n[ALL PASS] LinearPhaseEQ QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
