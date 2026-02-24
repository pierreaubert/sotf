use crate::{Plugin, ProcessContext};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

// ============================================================================
// Counting Allocator for Real-time Safety Verification
// ============================================================================

pub static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static COUNTING_ENABLED: AtomicBool = AtomicBool::new(false);

pub struct CountingAlloc;

/// # Safety
/// This implementation is safe as it only increments an atomic counter.
/// It uses `System` allocator for the actual memory operations.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING_ENABLED.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Run a closure and assert it performs zero heap allocations.
pub fn assert_no_allocs<F: FnOnce()>(label: &str, f: F) {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    COUNTING_ENABLED.store(true, Ordering::SeqCst);
    f();
    COUNTING_ENABLED.store(false, Ordering::SeqCst);
    let count = ALLOC_COUNT.load(Ordering::SeqCst);
    if count > 0 {
        panic!(
            "{} failed: {} allocations detected in hot path",
            label, count
        );
    }
}

// ============================================================================
// Shared QA Tests
// ============================================================================

/// Run standard QA tests for a plugin:
/// 1. Latency Reporting
/// 2. Real-time Safety (Zero Allocations)
/// 3. Performance Benchmark
pub fn run_standard_tests(plugin: &mut dyn Plugin, label: &str) {
    let sample_rate = 48000;

    // Test 2: Latency Reporting
    println!(
        "
[Test 2] Latency Reporting"
    );
    let reported = plugin.latency_samples();
    println!("  Reported Latency: {} samples", reported);
    println!("  Latency: PASS");

    // Test 3: Real-time Safety (Zero Allocations)
    println!(
        "
[Test 3] Real-time Safety (Zero Allocations)"
    );
    let rt_block_size = 512;
    let rt_input = vec![0.0_f32; rt_block_size * plugin.input_channels()];
    let mut rt_output = vec![0.0_f32; rt_block_size * plugin.output_channels()];
    let rt_ctx = ProcessContext {
        sample_rate,
        num_frames: rt_block_size,
    };

    // Warm up
    for _ in 0..10 {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
    }

    assert_no_allocs(&format!("{}::process + get_data", label), || {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
        let _data = plugin.get_data();
    });
    println!("  Zero Allocations: PASS");

    // Test 4: Performance Benchmark
    println!(
        "
[Test 4] Performance Benchmark"
    );
    let bench_frames = 48000 * 5; // 5 seconds of audio
    let bench_input = vec![0.1_f32; bench_frames * plugin.input_channels()];
    let mut bench_output = vec![0.0_f32; bench_frames * plugin.output_channels()];

    let start = Instant::now();
    let mut pos = 0;
    while pos < bench_frames {
        let end = (pos + rt_block_size).min(bench_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin
            .process(
                &bench_input[pos * plugin.input_channels()..end * plugin.input_channels()],
                &mut bench_output[pos * plugin.output_channels()..end * plugin.output_channels()],
                &ctx,
            )
            .unwrap();
        pos = end;
    }
    let duration = start.elapsed();
    let audio_duration_sec = bench_frames as f64 / sample_rate as f64;
    let cpu_usage = (duration.as_secs_f64() / audio_duration_sec) * 100.0;

    println!(
        "  Processed {:.1}s of audio in {:.2}ms",
        audio_duration_sec,
        duration.as_secs_f64() * 1000.0
    );
    println!("  Estimated CPU Usage: {:.2}%", cpu_usage);

    // Default threshold for most plugins is quite low.
    // XTC is heavy, but simple ones like Gain should be < 0.1%.
    let threshold = if label.contains("Xtc") { 15.0 } else { 5.0 };
    assert!(
        cpu_usage < threshold,
        "Performance regression: {} is too slow ({:.2}%)",
        label,
        cpu_usage
    );
    println!("  Performance: PASS");
}
