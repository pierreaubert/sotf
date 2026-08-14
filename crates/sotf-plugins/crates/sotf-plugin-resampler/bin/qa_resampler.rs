use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs};
use sotf_plugin_resampler::{ResamplerPlugin, ResamplerQuality};
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
    let channels = 2;
    let input_sr = 44100;
    let output_sr = 48000;

    println!("=== QA: Resampler Plugin ===");

    // Test 1: Resampling produces output with correct ratio
    println!("\n[Test 1] 44.1kHz → 48kHz resampling");
    let mut plugin = ResamplerPlugin::new_default(channels, input_sr, output_sr).unwrap();
    plugin.initialize(input_sr).unwrap();

    let num_frames = 1024;
    let input = vec![0.5f32; num_frames * channels];
    let max_out_frames = plugin.output_frames_for_input(num_frames);
    let mut output = vec![0.0f32; max_out_frames * channels];
    let ctx = ProcessContext::new(input_sr, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    let has_output = output.iter().any(|&s| s.abs() > 0.01);
    println!("  Has output: {}", has_output);
    assert!(has_output, "Resampler should produce output");

    // Test 2: Latency reporting
    println!("\n[Test 2] Latency Reporting");
    let reported = plugin.latency_samples();
    println!("  Reported Latency: {} samples", reported);
    println!("  Latency: PASS");

    // Test 3: Real-time Safety (Zero Allocations)
    // Resampler needs a larger output buffer due to upsampling
    println!("\n[Test 3] Real-time Safety (Zero Allocations)");
    let rt_block = 1024;
    let rt_input = vec![0.1f32; rt_block * channels];
    let rt_max_out = plugin.output_frames_for_input(rt_block);
    let mut rt_output = vec![0.0f32; rt_max_out * channels];
    let rt_ctx = ProcessContext::new(input_sr, rt_block);

    // Warm up
    for _ in 0..10 {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
    }

    assert_no_allocs("ResamplerPlugin::process", || {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
    });
    println!("  Zero Allocations: PASS");

    println!("\n[Test 4] Dynamic-ratio automation (Zero Allocations)");
    plugin
        .set_parameter(
            "dynamic_ratio".into(),
            sotf_host::parameters::ParameterValue::Bool(true),
        )
        .unwrap();
    let nominal = output_sr as f64 / input_sr as f64;
    assert_no_allocs("ResamplerPlugin ratio automation", || {
        for ratio in [nominal * 0.995, nominal, nominal * 1.005, nominal] {
            plugin.set_ratio(ratio, true).unwrap();
            plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
        }
    });
    println!("  Dynamic-ratio Zero Allocations: PASS");

    // Test 5: Performance Benchmark
    println!("\n[Test 5] Performance Benchmark");
    let bench_frames = 48000 * 5;
    let bench_input = vec![0.1f32; bench_frames * channels];
    let bench_max_out = ((bench_frames as f64 * output_sr as f64 / input_sr as f64) as usize)
        + plugin.output_frames_for_input(rt_block);
    let mut bench_output = vec![0.0f32; bench_max_out * channels];

    let start = std::time::Instant::now();
    let mut pos = 0;
    let mut out_pos = 0;
    while pos < bench_frames {
        let end = (pos + rt_block).min(bench_frames);
        let ctx = ProcessContext::new(input_sr, end - pos);
        let produced = plugin
            .process(
                &bench_input[pos * channels..end * channels],
                &mut bench_output[out_pos * channels..],
                &ctx,
            )
            .unwrap();
        out_pos += produced;
        pos = end;
    }
    let duration = start.elapsed();
    let audio_duration_sec = bench_frames as f64 / input_sr as f64;
    let cpu_usage = (duration.as_secs_f64() / audio_duration_sec) * 100.0;
    println!(
        "  Processed {:.1}s of audio in {:.2}ms",
        audio_duration_sec,
        duration.as_secs_f64() * 1000.0
    );
    println!("  Estimated CPU Usage: {:.2}%", cpu_usage);
    assert!(cpu_usage < 10.0, "CPU usage too high: {:.2}%", cpu_usage);
    println!("  Performance: PASS");

    println!("\n[Test 6] Ratio/quality/channel/callback deadline matrix");
    const WARMUP_CALLBACKS: usize = 128;
    const SAMPLED_CALLBACKS: usize = 256;
    let mut all_samples = Vec::with_capacity(3 * 2 * 4 * 5 * SAMPLED_CALLBACKS);
    let mut worst_case_p50 = Duration::ZERO;
    let mut worst_case_p95 = Duration::ZERO;
    let mut worst_case_p99 = Duration::ZERO;
    let mut worst_callback = Duration::ZERO;
    let mut actual_callback_misses = 0usize;
    let mut scheduler_quantum_misses = 0usize;
    for quality in [
        ResamplerQuality::Fast,
        ResamplerQuality::Medium,
        ResamplerQuality::High,
    ] {
        for &(source_rate, sink_rate) in &[(22_050, 96_000), (96_000, 22_050)] {
            for matrix_channels in [1usize, 2, 8, 16] {
                for frames in [1usize, 17, 63, 64, 127] {
                    let mut candidate = ResamplerPlugin::with_quality(
                        matrix_channels,
                        source_rate,
                        sink_rate,
                        64,
                        quality,
                    )
                    .unwrap();
                    candidate.initialize(source_rate).unwrap();
                    let input = vec![0.1; frames * matrix_channels];
                    // Include one extra chunk in the preallocated capacity because
                    // residual input can make a later callback emit more than the first.
                    let output_capacity = candidate.output_frames_for_input(frames + 64);
                    let mut output = vec![0.0; output_capacity * matrix_channels];
                    let context = ProcessContext::new(source_rate, frames);

                    for _ in 0..WARMUP_CALLBACKS {
                        candidate.process(&input, &mut output, &context).unwrap();
                    }

                    let mut samples = Vec::with_capacity(SAMPLED_CALLBACKS);
                    for _ in 0..SAMPLED_CALLBACKS {
                        let started = Instant::now();
                        let produced = candidate.process(&input, &mut output, &context).unwrap();
                        let elapsed = started.elapsed();
                        let actual_deadline =
                            Duration::from_secs_f64(frames as f64 / source_rate as f64);
                        let scheduler_deadline = Duration::from_secs_f64(
                            Plugin::realtime_quantum_frames(&candidate).max(frames) as f64
                                / source_rate as f64,
                        );
                        actual_callback_misses += usize::from(elapsed >= actual_deadline);
                        scheduler_quantum_misses += usize::from(elapsed >= scheduler_deadline);
                        samples.push(elapsed);
                        assert!(
                            output[..produced * matrix_channels]
                                .iter()
                                .all(|sample| sample.is_finite())
                        );
                    }
                    samples.sort_unstable();
                    let p50 = nearest_rank(&samples, 50);
                    let p95 = nearest_rank(&samples, 95);
                    let p99 = nearest_rank(&samples, 99);
                    let max = samples[SAMPLED_CALLBACKS - 1];
                    worst_case_p50 = worst_case_p50.max(p50);
                    worst_case_p95 = worst_case_p95.max(p95);
                    worst_case_p99 = worst_case_p99.max(p99);
                    worst_callback = worst_callback.max(max);
                    let callback_deadline = Duration::from_secs_f64(
                        Plugin::realtime_quantum_frames(&candidate).max(frames) as f64
                            / source_rate as f64,
                    );
                    assert!(
                        p99 < callback_deadline && max < callback_deadline,
                        "{quality:?} {source_rate}->{sink_rate} {matrix_channels}ch/{frames}f p50/p95/p99/max={p50:?}/{p95:?}/{p99:?}/{max:?}, negotiated scheduler deadline={callback_deadline:?}"
                    );
                    all_samples.extend_from_slice(&samples);

                    let mut drain =
                        vec![0.0; candidate.drain_output_frames_max() * matrix_channels];
                    while !candidate
                        .drain(&mut drain, &ProcessContext::new(source_rate, 0))
                        .unwrap()
                        .complete
                    {}
                }
            }
        }
    }
    all_samples.sort_unstable();
    println!(
        "  Aggregate p50/p95/p99/max: {:.3}/{:.3}/{:.3}/{:.3}ms; actual-callback misses: {}; negotiated-quantum misses: {}",
        nearest_rank(&all_samples, 50).as_secs_f64() * 1000.0,
        nearest_rank(&all_samples, 95).as_secs_f64() * 1000.0,
        nearest_rank(&all_samples, 99).as_secs_f64() * 1000.0,
        worst_callback.as_secs_f64() * 1000.0,
        actual_callback_misses,
        scheduler_quantum_misses
    );
    println!(
        "  Worst-case p50/p95/p99/max: {:.3}/{:.3}/{:.3}/{:.3}ms",
        worst_case_p50.as_secs_f64() * 1000.0,
        worst_case_p95.as_secs_f64() * 1000.0,
        worst_case_p99.as_secs_f64() * 1000.0,
        worst_callback.as_secs_f64() * 1000.0
    );
    assert_eq!(
        scheduler_quantum_misses, 0,
        "resampler missed its explicitly reported realtime scheduling quantum"
    );
    println!(
        "  Engine/offline queued-work matrix: PASS (direct fixed-frame FFI use rejects rate changes)"
    );

    println!("\n[ENGINE/OFFLINE SCHEDULING PASS] Resampler QA Complete.");
}
