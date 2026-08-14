use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, assert_no_allocs,
    measure_peak_db, run_standard_tests,
};
use sotf_plugin_linear_phase_eq::{BandConfig, LinearPhaseEqPlugin, LinearPhaseEqPluginParams};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn nearest_rank(samples: &[Duration], percentile: usize) -> Duration {
    assert!(!samples.is_empty());
    assert!((1..=100).contains(&percentile));
    let rank = (samples.len() * percentile).div_ceil(100);
    samples[rank - 1]
}

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
    const WARMUP_CALLBACKS: usize = 128;
    const SAMPLED_CALLBACKS: usize = 256;
    let mut aggregate = Vec::new();
    let mut actual_callback_misses = 0usize;
    let mut scheduler_quantum_misses = 0usize;
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
                    filters: vec![BandConfig {
                        filter_type: "Peak".to_string(),
                        frequency: 1_000.0,
                        q: 1.0,
                        gain_db: 6.0,
                        active: true,
                    }],
                },
            )
            .unwrap();
            for frames in [16, 32, 64, 127, 256, 512, 1_024] {
                let stimulus = (0..frames * channels)
                    .map(|sample| {
                        let frame = sample / channels;
                        (2.0 * PI * 1_000.0 * frame as f32 / sample_rate as f32).sin() * 0.1
                    })
                    .collect::<Vec<_>>();
                let mut audio = stimulus.clone();
                let context = ProcessContext::new(sample_rate, frames);
                for _ in 0..WARMUP_CALLBACKS {
                    audio.copy_from_slice(&stimulus);
                    candidate.process_in_place(&mut audio, &context).unwrap();
                }
                audio.copy_from_slice(&stimulus);
                assert_no_allocs("LinearPhaseEqPlugin partitioned process", || {
                    candidate.process_in_place(&mut audio, &context).unwrap();
                });
                let actual_deadline = Duration::from_secs_f64(frames as f64 / sample_rate as f64);
                let scheduler_deadline = Duration::from_secs_f64(
                    ParametricInPlacePlugin::realtime_quantum_frames(&candidate).max(frames) as f64
                        / sample_rate as f64,
                );
                let mut samples = Vec::with_capacity(SAMPLED_CALLBACKS);
                for _ in 0..SAMPLED_CALLBACKS {
                    audio.copy_from_slice(&stimulus);
                    let started = Instant::now();
                    candidate.process_in_place(&mut audio, &context).unwrap();
                    let elapsed = started.elapsed();
                    actual_callback_misses += usize::from(elapsed >= actual_deadline);
                    scheduler_quantum_misses += usize::from(elapsed >= scheduler_deadline);
                    samples.push(elapsed);
                }
                samples.sort_unstable();
                let p50 = nearest_rank(&samples, 50);
                let p95 = nearest_rank(&samples, 95);
                let p99 = nearest_rank(&samples, 99);
                let max = samples[SAMPLED_CALLBACKS - 1];
                aggregate.extend_from_slice(&samples);
                assert!(
                    p99 < scheduler_deadline && max < scheduler_deadline,
                    "{channels}ch length-index {fir_length_index} block {frames}: p50/p95/p99/max={p50:?}/{p95:?}/{p99:?}/{max:?}, negotiated scheduler deadline={scheduler_deadline:?}"
                );
            }
        }
    }
    aggregate.sort_unstable();
    println!(
        "  aggregate p50/p95/p99/max: {:?}/{:?}/{:?}/{:?}; actual-callback misses: {actual_callback_misses}; negotiated-quantum misses: {scheduler_quantum_misses}",
        nearest_rank(&aggregate, 50),
        nearest_rank(&aggregate, 95),
        nearest_rank(&aggregate, 99),
        aggregate[aggregate.len() - 1]
    );
    assert_eq!(scheduler_quantum_misses, 0);
    println!(
        "  engine queued-work contract: PASS (all cases zero-allocation and below the reported work horizon)"
    );
    if actual_callback_misses == 0 {
        println!(
            "  direct-callback observation: no misses on this run; AU/NIH callback compliance remains unproven without an approved fixed-rate adapter"
        );
    } else {
        println!(
            "  direct-callback status: UNRESOLVED ({actual_callback_misses} measured physical-deadline misses); AU/NIH require an approved fixed-rate adapter"
        );
    }

    println!(
        "\n[ENGINE QUEUED-CONTRACT PASS; DIRECT-CALLBACK COMPLIANCE UNRESOLVED] LinearPhaseEQ QA Complete."
    );
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
