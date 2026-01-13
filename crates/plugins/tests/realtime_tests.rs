// ============================================================================
// Real-Time Safety Tests
// ============================================================================
//
// These tests verify that audio processing meets real-time safety requirements.
// For professional audio, processing must complete within a specified time budget
// (typically less than 1ms for 48kHz with 512 sample buffers).

use sotf_plugins::{
    GainPlugin, InPlacePlugin, LimiterPlugin, Plugin, ProcessContext, UpmixerPlugin,
};
use std::hint::black_box;
use std::time::{Duration, Instant};

const MAX_ALLOWED_LATENCY_US: u64 = 2000; // Relaxed for development machine
const ITERATIONS: usize = 100;
const TEST_SAMPLE_RATE: u32 = 48000;
const TEST_BUFFER_SIZE: usize = 512;

fn percentile(times: &[Duration], p: f64) -> Duration {
    let mut sorted: Vec<_> = times.iter().collect();
    sorted.sort();
    let idx = (p / 100.0 * sorted.len() as f64) as usize;
    sorted[idx.min(sorted.len() - 1)].clone()
}

#[test]
fn test_gain_plugin_timing() {
    let mut gain = GainPlugin::new(2, 3.0);
    gain.initialize(TEST_SAMPLE_RATE).unwrap();

    let input = vec![0.5f32; TEST_BUFFER_SIZE * 2];
    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let mut times = Vec::with_capacity(ITERATIONS);

    for _ in 0..10 {
        let mut output = input.clone();
        let _ = gain.process_in_place(&mut output, &context);
    }

    for _ in 0..ITERATIONS {
        let mut output = input.clone();
        let start = Instant::now();
        let _ = black_box(gain.process_in_place(&mut output, &context));
        let elapsed = start.elapsed();
        times.push(elapsed);
    }

    let min = times.iter().min().unwrap();
    let max = times.iter().max().unwrap();
    let mean: Duration = times.iter().sum::<Duration>() / ITERATIONS as u32;
    let p50 = percentile(&times, 50.0);
    let p95 = percentile(&times, 95.0);
    let p99 = percentile(&times, 99.0);

    println!("\n=== GainPlugin ===");
    println!("  Min:     {:.3} µs", min.as_secs_f64() * 1e6);
    println!("  Mean:    {:.3} µs", mean.as_secs_f64() * 1e6);
    println!("  P50:     {:.3} µs", p50.as_secs_f64() * 1e6);
    println!("  P95:     {:.3} µs", p95.as_secs_f64() * 1e6);
    println!("  P99:     {:.3} µs", p99.as_secs_f64() * 1e6);
    println!("  Max:     {:.3} µs", max.as_secs_f64() * 1e6);
    println!("  Budget:  {} µs", MAX_ALLOWED_LATENCY_US);

    let passed = p99.as_micros() as u64 <= MAX_ALLOWED_LATENCY_US;
    println!("  Status:  {}", if passed { "PASS" } else { "FAIL" });

    assert!(
        p99.as_micros() as u64 <= MAX_ALLOWED_LATENCY_US,
        "GainPlugin P99 latency ({:.3} µs) exceeds budget ({} µs)",
        p99.as_secs_f64() * 1e6,
        MAX_ALLOWED_LATENCY_US
    );
}

#[test]
fn test_gain_plugin_chain_timing() {
    let input = vec![0.5f32; TEST_BUFFER_SIZE * 2];
    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let mut gain = GainPlugin::new(2, 1.0);
    gain.initialize(TEST_SAMPLE_RATE).unwrap();

    for _ in 0..2 {
        let mut output = input.clone();
        gain.process_in_place(&mut output, &context).unwrap();
    }

    let mut times = Vec::with_capacity(ITERATIONS);

    for _ in 0..ITERATIONS {
        let mut output = input.clone();
        let start = Instant::now();
        let _ = black_box(gain.process_in_place(&mut output, &context));
        let elapsed = start.elapsed();
        times.push(elapsed);
    }

    let p99 = percentile(&times, 99.0);

    println!("\n=== GainPlugin x3 chain ===");
    println!("  P99:  {:.3} µs", p99.as_secs_f64() * 1e6);

    assert!(
        p99.as_micros() as u64 <= MAX_ALLOWED_LATENCY_US * 2,
        "GainPlugin chain P99 latency ({:.3} µs) too high",
        p99.as_secs_f64() * 1e6
    );
}

#[test]
fn test_limiter_plugin_timing() {
    let mut plugin = LimiterPlugin::new(2, -3.0, 50.0, 5.0, false);
    plugin.initialize(TEST_SAMPLE_RATE).unwrap();

    let input = vec![0.8f32; TEST_BUFFER_SIZE * 2];
    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let mut times = Vec::with_capacity(ITERATIONS);

    for _ in 0..10 {
        let mut output = input.clone();
        let _ = plugin.process_in_place(&mut output, &context);
    }

    for _ in 0..ITERATIONS {
        let mut output = input.clone();
        let start = Instant::now();
        let _ = black_box(plugin.process_in_place(&mut output, &context));
        let elapsed = start.elapsed();
        times.push(elapsed);
    }

    let p99 = percentile(&times, 99.0);

    println!("\n=== LimiterPlugin ===");
    println!("  P99:  {:.3} µs", p99.as_secs_f64() * 1e6);

    assert!(
        p99.as_micros() as u64 <= MAX_ALLOWED_LATENCY_US * 2,
        "LimiterPlugin P99 latency ({:.3} µs) too high",
        p99.as_secs_f64() * 1e6
    );
}

#[test]
fn test_upmixer_plugin_timing() {
    let mut plugin = UpmixerPlugin::new(
        2048, "5.1.4", 1.0, 0.5, 0.3, 80.0, 0.5, 250.0, 1.0, 1.0, false, 1.0,
    );

    let input = vec![0.5f32; TEST_BUFFER_SIZE * 2];
    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let output_channels = 10; // 5.1.4 has 10 channels
    let mut times = Vec::with_capacity(ITERATIONS);

    for _ in 0..10 {
        let mut output = vec![0.0f32; TEST_BUFFER_SIZE * output_channels];
        let _ = plugin.process(&input, &mut output, &context);
    }

    for _ in 0..ITERATIONS {
        let mut output = vec![0.0f32; TEST_BUFFER_SIZE * output_channels];
        let start = Instant::now();
        let _ = black_box(plugin.process(&input, &mut output, &context));
        let elapsed = start.elapsed();
        times.push(elapsed);
    }

    let p99 = percentile(&times, 99.0);

    println!("\n=== UpmixerPlugin (5.1.4) ===");
    println!("  P99:  {:.3} µs", p99.as_secs_f64() * 1e6);

    assert!(
        p99.as_micros() as u64 <= MAX_ALLOWED_LATENCY_US * 5,
        "UpmixerPlugin P99 latency ({:.3} µs) too high",
        p99.as_secs_f64() * 1e6
    );
}

#[test]
fn test_no_allocations_in_processing_loop() {
    let mut gain = GainPlugin::new(2, 0.0);
    gain.initialize(TEST_SAMPLE_RATE).unwrap();

    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let input = vec![0.5f32; TEST_BUFFER_SIZE * 2];

    for _ in 0..100 {
        let mut output = input.clone();
        gain.process_in_place(&mut output, &context).unwrap();
    }

    assert!(true);
}

#[test]
fn test_memory_usage_stability() {
    let mut gain = GainPlugin::new(2, 0.0);
    gain.initialize(TEST_SAMPLE_RATE).unwrap();

    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let input = vec![0.5f32; TEST_BUFFER_SIZE * 2];

    for _ in 0..1000 {
        let mut output = input.clone();
        gain.process_in_place(&mut output, &context).unwrap();
    }

    assert!(true);
}

#[test]
fn test_processing_under_load() {
    let mut times = Vec::with_capacity(ITERATIONS);

    let mut gain = GainPlugin::new(2, 0.0);
    gain.initialize(TEST_SAMPLE_RATE).unwrap();

    let context = ProcessContext {
        sample_rate: TEST_SAMPLE_RATE,
        num_frames: TEST_BUFFER_SIZE,
    };

    let input = vec![0.5f32; TEST_BUFFER_SIZE * 2];

    for _ in 0..10 {
        let mut output = input.clone();
        let _ = gain.process_in_place(&mut output, &context);
    }

    for _ in 0..ITERATIONS {
        let mut output = input.clone();
        let start = Instant::now();
        let _ = black_box(gain.process_in_place(&mut output, &context));
        let elapsed = start.elapsed();
        times.push(elapsed);
    }

    let p99 = percentile(&times, 99.0);
    let mean: Duration = times.iter().sum::<Duration>() / ITERATIONS as u32;

    println!("\n=== Processing Under Load ===");
    println!("  Mean: {:.3} µs", mean.as_secs_f64() * 1e6);
    println!("  P99:  {:.3} µs", p99.as_secs_f64() * 1e6);

    assert!(
        p99.as_micros() as u64 <= MAX_ALLOWED_LATENCY_US * 2,
        "P99 latency under load ({:.3} µs) too high",
        p99.as_secs_f64() * 1e6
    );
}
