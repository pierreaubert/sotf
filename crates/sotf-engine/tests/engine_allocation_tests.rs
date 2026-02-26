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
    // Start engine with hal_mode to force continuous processing of silence
    let mut config = EngineConfig::default();
    config.hal_mode = true;
    config.plugins = vec![PluginConfig::new(
        "gain",
        serde_json::json!({"gain_db": -3.0}),
    )];

    let _engine = AudioEngine::new(config).unwrap();

    // Give some time for startup and ramp-up (recycles filling up)
    std::thread::sleep(Duration::from_millis(500));

    reset_alloc_count();
    set_counting(true);

    // Wait for some frames to be processed in steady state
    std::thread::sleep(Duration::from_millis(200));

    set_counting(false);
    let count = get_alloc_count();

    // Steady state should perform zero heap allocations in the hot path
    assert!(
        count == 0,
        "Engine hot path performed {} allocations in steady state",
        count
    );
}
