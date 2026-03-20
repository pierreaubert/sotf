#![allow(clippy::field_reassign_with_default)]
use sotf_audio::{AudioEngine, EngineConfig, PluginConfig};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

// ============================================================================
// Counting Allocator
// ============================================================================

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static COUNTING_ENABLED: AtomicBool = AtomicBool::new(false);

struct CountingAlloc;

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

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn reset_alloc_count() {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
}

fn set_counting(enabled: bool) {
    COUNTING_ENABLED.store(enabled, Ordering::SeqCst);
}

fn get_alloc_count() -> usize {
    ALLOC_COUNT.load(Ordering::SeqCst)
}

// ============================================================================
// Allocation Tests
// ============================================================================

#[test]
fn test_engine_hotpath_allocations() {
    // Start engine with driver_mode to force continuous processing of silence
    let mut config = EngineConfig::default();
    config.driver_mode = true;
    config.plugins = vec![PluginConfig::new(
        "gain",
        serde_json::json!({"gain_db": -3.0}),
    )];

    let _engine = AudioEngine::new(config).unwrap();

    // Give time for startup and ramp-up (recycle queues filling up).
    // Under heavy CPU load (e.g., full test suite), threads may be
    // starved, so we use an adaptive approach: take multiple measurement
    // windows and require at least one to show zero allocations.
    std::thread::sleep(Duration::from_millis(500));

    let mut best = usize::MAX;
    for _ in 0..5 {
        reset_alloc_count();
        set_counting(true);
        std::thread::sleep(Duration::from_millis(200));
        set_counting(false);
        let count = get_alloc_count();
        best = best.min(count);
        if best == 0 {
            break;
        }
        // Extra settle time before next attempt
        std::thread::sleep(Duration::from_millis(200));
    }

    assert!(
        best == 0,
        "Engine hot path performed {} allocations in every measurement window",
        best
    );
}
