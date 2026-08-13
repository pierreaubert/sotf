use sotf_host::{CountingAlloc, assert_no_allocs, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_beamformer::{BeamformerPlugin, BeamformerPluginParams};
use std::time::Instant;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let num_mics = 2;

    let mut plugin = BeamformerPlugin::from_params(
        sample_rate,
        BeamformerPluginParams {
            beamformer_type: 2,
            ..Default::default()
        },
    )
    .unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Beamformer Plugin ===");

    // Test 1: GSC mode — sample-by-sample, zero latency
    println!("\n[Test 1] GSC mode processing (512 frames)");
    let num_frames = 512;
    let input = vec![0.1f32; num_frames * num_mics];
    let mut output = vec![0.0f32; num_frames];
    let ctx = ProcessContext::new(sample_rate, num_frames);

    plugin.process(&input, &mut output, &ctx).unwrap();
    println!("  GSC process completed: PASS");

    // Test 2: MVDR mode
    println!("\n[Test 2] MVDR mode processing");
    let mut mvdr = BeamformerPlugin::from_params(
        sample_rate,
        BeamformerPluginParams {
            beamformer_type: 0,
            ..Default::default()
        },
    )
    .unwrap();
    mvdr.initialize(sample_rate).unwrap();
    let mut output2 = vec![0.0f32; num_frames];
    mvdr.process(&input, &mut output2, &ctx).unwrap();
    println!("  MVDR process completed: PASS");

    // Test 3: MVDR zero-allocation check
    println!("\n[Test 3] MVDR Real-time Safety (Zero Allocations)");
    // Warm up MVDR path
    for _ in 0..10 {
        mvdr.process(&input, &mut output2, &ctx).unwrap();
    }
    assert_no_allocs("BeamformerPlugin::MVDR::process", || {
        mvdr.process(&input, &mut output2, &ctx).unwrap();
    });
    println!("  MVDR Zero Allocations: PASS");

    println!("\n[Test 4] Worst-layout callback deadlines (8 microphones)");
    for algorithm in 0..=2 {
        let mut candidate = BeamformerPlugin::from_params(
            sample_rate,
            BeamformerPluginParams {
                num_mics: 8,
                beamformer_type: algorithm,
                ..Default::default()
            },
        )
        .unwrap();
        candidate.initialize(sample_rate).unwrap();
        let callback_frames = 512;
        let input = vec![0.05; callback_frames * 8];
        let mut output = vec![0.0; callback_frames];
        let context = ProcessContext::new(sample_rate, callback_frames);
        for _ in 0..8 {
            candidate.process(&input, &mut output, &context).unwrap();
        }
        assert_no_allocs("Beamformer max-layout process", || {
            candidate.process(&input, &mut output, &context).unwrap();
        });
        let mut timings = Vec::with_capacity(100);
        for _ in 0..100 {
            let start = Instant::now();
            candidate.process(&input, &mut output, &context).unwrap();
            timings.push(start.elapsed());
        }
        timings.sort_unstable();
        let p50 = timings[50].as_secs_f64() * 1000.0;
        let p95 = timings[95].as_secs_f64() * 1000.0;
        let max = timings[99].as_secs_f64() * 1000.0;
        let deadline = callback_frames as f64 * 1000.0 / sample_rate as f64;
        println!(
            "  algorithm {algorithm}: p50/p95/max {p50:.3}/{p95:.3}/{max:.3} ms, deadline {deadline:.3} ms"
        );
        assert!(max < deadline);
    }

    // Run standard QA tests (tests GSC allocation safety + perf benchmark)
    run_standard_tests(&mut plugin, "BeamformerPlugin");

    println!("\n[ALL PASS] Beamformer QA Complete.");
}
