//! QA-only playback performance capture.
//!
//! Frame metrics come from GPUI's frame profiler, update counts come from the
//! root view's playback-driven update loop, and allocation totals come from
//! mimalloc's safe process statistics API. Metrics are frozen when capture
//! stops so subsequent dev-API queries do not perturb the measured interval.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use gpui::WindowId;
use gpui::profiler::{
    FrameTiming, FrameTimingCollector, frame_trace_enabled, set_frame_trace_enabled,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::performance_metrics::{AllocationSnapshot, duration_ms, percentile_ms, rate, ratio};

struct ActiveCapture {
    window_id: WindowId,
    collector: FrameTimingCollector,
    started_at: Instant,
    update_frame_baseline: u64,
    allocation_baseline: AllocationSnapshot,
    frame_trace_was_enabled: bool,
    playback_clock_running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
struct Metrics {
    elapsed_ms: f64,
    frame_count_all_windows: u64,
    frame_count: u64,
    frames_per_second: f64,
    update_count: u64,
    updates_per_second: f64,
    draw_p50_ms: f64,
    draw_p95_ms: f64,
    draw_max_ms: f64,
    dirty_to_draw_p95_ms: f64,
    invalidations_total: u64,
    invalidations_per_frame: f64,
    allocations_total: u64,
    allocations_per_second: f64,
    allocated_bytes: u64,
    allocated_bytes_per_second: f64,
}

#[derive(Default)]
struct CaptureState {
    active: Option<ActiveCapture>,
    last: Option<Metrics>,
}

#[derive(Serialize)]
struct PerformanceArtifact<'a> {
    schema_version: u32,
    source: &'static str,
    metrics: &'a Metrics,
}

static CAPTURE: OnceLock<Mutex<CaptureState>> = OnceLock::new();

fn capture() -> &'static Mutex<CaptureState> {
    CAPTURE.get_or_init(|| Mutex::new(CaptureState::default()))
}

pub(super) fn start(window_id: WindowId, update_frame_count: u64) -> Result<Arc<AtomicBool>> {
    let mut state = capture()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.active.is_some() {
        return Err(anyhow!("performance capture is already active"));
    }

    verify_allocator_accounting()?;
    let allocation_baseline = allocator_snapshot()?;
    let frame_trace_was_enabled = frame_trace_enabled();
    set_frame_trace_enabled(true);
    let playback_clock_running = Arc::new(AtomicBool::new(true));
    state.last = None;
    state.active = Some(ActiveCapture {
        window_id,
        collector: FrameTimingCollector::new(),
        started_at: Instant::now(),
        update_frame_baseline: update_frame_count,
        allocation_baseline,
        frame_trace_was_enabled,
        playback_clock_running: playback_clock_running.clone(),
    });
    Ok(playback_clock_running)
}

pub(super) fn stop(update_frame_count: u64) -> Result<()> {
    let mut state = capture()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut active = state
        .active
        .take()
        .ok_or_else(|| anyhow!("performance capture is not active"))?;
    active
        .playback_clock_running
        .store(false, Ordering::Release);

    // Snapshot allocator counters before collecting frame timings so the
    // collector's result allocation is outside the measured interval.
    let allocation_after = allocator_snapshot();
    let elapsed = active.started_at.elapsed();
    let all_frames = active.collector.collect_unseen();
    let frame_count_all_windows = all_frames.len() as u64;
    let frames: Vec<_> = all_frames
        .into_iter()
        .filter(|frame| frame.window_id == active.window_id)
        .collect();
    if !active.frame_trace_was_enabled {
        set_frame_trace_enabled(false);
    }

    let allocation_after = allocation_after?;
    let metrics = build_metrics(
        elapsed,
        frame_count_all_windows,
        update_frame_count.saturating_sub(active.update_frame_baseline),
        active.allocation_baseline,
        allocation_after,
        &frames,
    );
    state.last = Some(metrics);
    write_artifact(&metrics)?;
    Ok(())
}

pub(super) fn resolve(path: &str) -> Result<Value> {
    let state = capture()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if path == "performance.active" {
        return Ok(json!(state.active.is_some()));
    }
    let metrics = state
        .last
        .as_ref()
        .ok_or_else(|| anyhow!("no completed performance capture is available"))?;
    Ok(match path {
        "performance.elapsed_ms" => json!(metrics.elapsed_ms),
        "performance.frame_count_all_windows" => json!(metrics.frame_count_all_windows),
        "performance.frame_count" => json!(metrics.frame_count),
        "performance.frames_per_second" => json!(metrics.frames_per_second),
        "performance.update_count" => json!(metrics.update_count),
        "performance.updates_per_second" => json!(metrics.updates_per_second),
        "performance.draw_p50_ms" => json!(metrics.draw_p50_ms),
        "performance.draw_p95_ms" => json!(metrics.draw_p95_ms),
        "performance.draw_max_ms" => json!(metrics.draw_max_ms),
        "performance.dirty_to_draw_p95_ms" => json!(metrics.dirty_to_draw_p95_ms),
        "performance.invalidations_total" => json!(metrics.invalidations_total),
        "performance.invalidations_per_frame" => json!(metrics.invalidations_per_frame),
        "performance.allocations_total" => json!(metrics.allocations_total),
        "performance.allocations_per_second" => json!(metrics.allocations_per_second),
        "performance.allocated_bytes" => json!(metrics.allocated_bytes),
        "performance.allocated_bytes_per_second" => {
            json!(metrics.allocated_bytes_per_second)
        }
        _ => return Err(anyhow!("unknown query path: `{path}`")),
    })
}

fn build_metrics(
    elapsed: Duration,
    frame_count_all_windows: u64,
    update_count: u64,
    allocation_baseline: AllocationSnapshot,
    allocation_after: AllocationSnapshot,
    frames: &[FrameTiming],
) -> Metrics {
    let elapsed_seconds = elapsed.as_secs_f64();
    let mut draw_durations: Vec<_> = frames.iter().map(FrameTiming::draw_duration).collect();
    let mut dirty_to_draw_durations: Vec<_> = frames
        .iter()
        .filter_map(FrameTiming::dirty_to_draw_duration)
        .collect();
    let invalidations_total = frames.iter().map(|frame| frame.invalidations).sum();
    let allocations_total = allocation_after
        .count
        .saturating_sub(allocation_baseline.count);
    let allocated_bytes = allocation_after
        .requested_bytes
        .saturating_sub(allocation_baseline.requested_bytes);

    Metrics {
        elapsed_ms: elapsed_seconds * 1_000.0,
        frame_count_all_windows,
        frame_count: frames.len() as u64,
        frames_per_second: rate(frames.len() as u64, elapsed_seconds),
        update_count,
        updates_per_second: rate(update_count, elapsed_seconds),
        draw_p50_ms: percentile_ms(&mut draw_durations, 0.50),
        draw_p95_ms: percentile_ms(&mut draw_durations, 0.95),
        draw_max_ms: duration_ms(draw_durations.iter().copied().max().unwrap_or_default()),
        dirty_to_draw_p95_ms: percentile_ms(&mut dirty_to_draw_durations, 0.95),
        invalidations_total,
        invalidations_per_frame: ratio(invalidations_total, frames.len() as u64),
        allocations_total,
        allocations_per_second: rate(allocations_total, elapsed_seconds),
        allocated_bytes,
        allocated_bytes_per_second: rate(allocated_bytes, elapsed_seconds),
    }
}

fn allocator_snapshot() -> Result<AllocationSnapshot> {
    let stats = mimalloc::MiMalloc::stats_json()
        .map_err(|error| anyhow!("mimalloc statistics unavailable: {error}"))?;
    let value: Value = serde_json::from_slice(stats.to_bytes())
        .map_err(|error| anyhow!("invalid mimalloc statistics JSON: {error}"))?;
    crate::performance_metrics::allocation_snapshot_from_value(&value)
}

fn verify_allocator_accounting() -> Result<()> {
    let before = allocator_snapshot()?;
    let probe = vec![0_u8; 8 * 1024];
    std::hint::black_box(&probe);
    let after = allocator_snapshot()?;
    if after.count <= before.count || after.requested_bytes <= before.requested_bytes {
        return Err(anyhow!(
            "mimalloc process counters did not observe the QA allocation probe"
        ));
    }
    Ok(())
}

fn write_artifact(metrics: &Metrics) -> Result<()> {
    let Some(qa_dir) = std::env::var_os("SOTF_QA_DIR") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(qa_dir).join("performance.json");
    let artifact = PerformanceArtifact {
        schema_version: 1,
        source: "gpui-frame-profiler+mimalloc-process-stats",
        metrics,
    };
    let bytes = serde_json::to_vec_pretty(&artifact)
        .map_err(|error| anyhow!("failed to serialize `{}`: {error}", path.display()))?;
    std::fs::write(&path, bytes)
        .map_err(|error| anyhow!("failed to write `{}`: {error}", path.display()))?;
    Ok(())
}
