use sotf_audio::engine::playback_runtime_harness::{
    FrameWriterHarness, HarnessFrameWriteOutcome, XorShift64, generated_frame,
};
use serial_test::serial;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

struct CountingGuard;

impl Drop for CountingGuard {
    fn drop(&mut self) {
        COUNTING_ENABLED.store(false, Ordering::SeqCst);
    }
}

#[test]
#[serial]
fn playback_frame_writer_hot_path_does_not_allocate() {
    assert_zero_alloc("direct_2ch", 512, 2, 2, 4096, 0);
    assert_zero_alloc("upmix_2_to_8", 512, 2, 8, 8192, 0);
    assert_zero_alloc("downmix_6_to_2", 512, 6, 2, 8192, 0);
    assert_zero_alloc("downmix_10_to_2", 512, 10, 2, 8192, 0);
    assert_zero_alloc("fallback_4_to_6", 512, 4, 6, 8192, 0);
    assert_zero_alloc("full_buffer_drop", 512, 2, 2, 1024, 1024);
}

#[test]
#[serial]
fn converted_frame_capacity_miss_is_not_reported_as_normal_drop() {
    let mut rng = XorShift64::new(0x5eed);
    let frame = generated_frame(512, 2, 1024, &mut rng);
    let mut harness = FrameWriterHarness::new(8192, 8, 16, 0);

    let report = harness.write(frame);

    assert_eq!(report.recycled_buffers, 1);
    assert_eq!(
        report.outcome,
        HarnessFrameWriteOutcome::ConversionBufferTooSmall
    );
    assert_eq!(report.slots_before, report.slots_after);
}

fn assert_zero_alloc(
    label: &str,
    frames: usize,
    input_channels: usize,
    output_channels: usize,
    ring_capacity: usize,
    prefill_samples: usize,
) {
    let mut rng = XorShift64::new(0x5eed);
    let samples = frames * input_channels;
    let warmup_frame = generated_frame(frames, input_channels, samples, &mut rng);
    let frame = generated_frame(frames, input_channels, samples, &mut rng);
    let mut harness = FrameWriterHarness::for_frame(
        ring_capacity,
        output_channels,
        &warmup_frame,
        prefill_samples,
    );
    let _ = harness.write(warmup_frame);
    harness.rebuild(ring_capacity, output_channels, prefill_samples);
    // One post-rebuild warmup write to force any lazy per-ring-buffer init
    // before we start counting allocations.
    let post_rebuild_warmup = generated_frame(frames, input_channels, samples, &mut rng);
    let _ = harness.write(post_rebuild_warmup);

    ALLOC_COUNT.store(0, Ordering::SeqCst);
    COUNTING_ENABLED.store(true, Ordering::SeqCst);
    let _guard = CountingGuard;
    let report = harness.write(frame);

    let allocations = ALLOC_COUNT.load(Ordering::SeqCst);
    assert_eq!(
        allocations, 0,
        "{label} allocated {allocations} times in playback frame hot path"
    );
    assert_eq!(
        report.recycled_buffers, 1,
        "{label} must recycle exactly once"
    );
    if prefill_samples >= ring_capacity {
        assert_eq!(report.outcome, HarnessFrameWriteOutcome::Dropped);
    } else {
        assert!(
            matches!(
                report.outcome,
                HarnessFrameWriteOutcome::Written { .. } | HarnessFrameWriteOutcome::Dropped
            ),
            "{label} must return a valid write outcome"
        );
    }
}
