use serial_test::serial;
use sotf_audio::engine::playback_runtime_harness::{
    generated_frame, run_fuzzer, FrameWriterHarness, HarnessFrameWriteOutcome,
    PlaybackCallbackHarness, XorShift64,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static COUNTING_ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static COUNTING_ENABLED_FOR_THREAD: Cell<bool> = const { Cell::new(false) };
}

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let count_current_thread = COUNTING_ENABLED.load(Ordering::Relaxed)
            && COUNTING_ENABLED_FOR_THREAD
                .try_with(Cell::get)
                .unwrap_or(false);
        if count_current_thread {
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
        COUNTING_ENABLED_FOR_THREAD.with(|enabled| enabled.set(false));
        COUNTING_ENABLED.store(false, Ordering::SeqCst);
    }
}

fn enable_allocation_counting_for_current_thread() -> CountingGuard {
    COUNTING_ENABLED_FOR_THREAD.with(|enabled| enabled.set(true));
    COUNTING_ENABLED.store(true, Ordering::SeqCst);
    CountingGuard
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
fn playback_callback_harness_runs_consecutive_zero_allocation_windows() {
    const WINDOWS: usize = 3;
    let callback = std::thread::Builder::new()
        .name("cpal-callback-harness".to_string())
        .spawn(|| {
            let cases = [(512, 2), (512, 6), (512, 10), (512, 12), (2048, 12)];
            let mut harnesses = cases
                .into_iter()
                .map(|(frames, channels)| {
                    let samples = frames * channels;
                    (
                        PlaybackCallbackHarness::new(samples * 2, samples, channels, 48_000),
                        vec![0.25; samples],
                    )
                })
                .collect::<Vec<_>>();
            for (harness, input) in &mut harnesses {
                let _ = harness.process(input);
            }

            let mut callbacks = 0;
            for _ in 0..WINDOWS {
                ALLOC_COUNT.store(0, Ordering::SeqCst);
                let guard = enable_allocation_counting_for_current_thread();
                for _ in 0..32 {
                    for (harness, input) in &mut harnesses {
                        std::hint::black_box(harness.process(input));
                        callbacks += 1;
                    }
                }
                drop(guard);
                assert_eq!(
                    ALLOC_COUNT.load(Ordering::SeqCst),
                    0,
                    "playback callback allocated in a measured window"
                );
            }
            callbacks
        })
        .unwrap();

    assert_eq!(callback.join().unwrap(), WINDOWS * 32 * 5);
}

#[test]
#[serial]
fn playback_runtime_fuzzer_smoke() {
    const CASES: usize = 64;

    let stats = run_fuzzer(0x5eed, CASES).expect("playback runtime fuzzer should complete");

    assert_eq!(stats.cases, CASES);
    assert!(stats.sequences > 0);
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

#[test]
#[serial]
fn allocation_counter_ignores_background_thread_allocations() {
    let start = std::sync::Arc::new(AtomicBool::new(false));
    let done = std::sync::Arc::new(AtomicBool::new(false));
    let background_start = start.clone();
    let background_done = done.clone();
    let background = std::thread::spawn(move || {
        while !background_start.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        let mut background = Vec::with_capacity(1024);
        background.extend(0..1024);
        std::hint::black_box(background);
        background_done.store(true, Ordering::Release);
    });

    ALLOC_COUNT.store(0, Ordering::SeqCst);
    let guard = enable_allocation_counting_for_current_thread();

    start.store(true, Ordering::Release);
    while !done.load(Ordering::Acquire) {
        std::hint::spin_loop();
    }
    drop(guard);
    background.join().unwrap();

    assert_eq!(
        ALLOC_COUNT.load(Ordering::SeqCst),
        0,
        "allocation counter must ignore allocations from unrelated threads"
    );
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
    let _guard = enable_allocation_counting_for_current_thread();
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
