use crate::engine::volume_ramp::VolumeRampState;
use rtrb::Consumer;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Shared state between the playback thread and cpal callback.
/// All fields are lock-free atomics for real-time safety.
pub(crate) struct CpalPlaybackState {
    pub capacity: usize,
    pub volume: Arc<AtomicU32>,
    pub muted: Arc<AtomicBool>,
    pub(super) volume_ramp: VolumeRampState,
    pub flush_requested: Arc<AtomicBool>,
    pub underrun_count: Arc<AtomicU64>,
    pub last_buffer_level: Arc<AtomicU64>,
    pub total_callback_samples: Arc<AtomicU64>,
    pub callback_count: Arc<AtomicU64>,
}

impl CpalPlaybackState {
    #[cfg(test)]
    pub(super) fn new(capacity: usize) -> Self {
        Self::new_with_controls(capacity, 1.0, false)
    }

    pub(super) fn new_with_controls(capacity: usize, volume: f32, muted: bool) -> Self {
        let effective_gain = if muted { 0.0 } else { volume };
        Self {
            capacity,
            volume: Arc::new(AtomicU32::new(volume.to_bits())),
            muted: Arc::new(AtomicBool::new(muted)),
            volume_ramp: VolumeRampState::new(effective_gain),
            flush_requested: Arc::new(AtomicBool::new(false)),
            underrun_count: Arc::new(AtomicU64::new(0)),
            last_buffer_level: Arc::new(AtomicU64::new(100)),
            total_callback_samples: Arc::new(AtomicU64::new(0)),
            callback_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Read f32 samples from the ring buffer into a scratch buffer.
#[inline(always)]
pub(super) fn read_ring_buffer(
    consumer: &mut Consumer<f32>,
    scratch: &mut [f32],
    requested: usize,
    state: &CpalPlaybackState,
    capacity: usize,
) -> bool {
    if state.flush_requested.load(Ordering::Relaxed) {
        let available = consumer.slots().min(requested);
        if available > 0
            && let Ok(chunk) = consumer.read_chunk(available)
        {
            chunk.commit_all();
        }
        scratch[..requested].fill(0.0);
        if consumer.slots() == 0 {
            state.flush_requested.store(false, Ordering::Relaxed);
        }
        let fill_percent = (consumer.slots() * 100).checked_div(capacity).unwrap_or(0);
        state
            .last_buffer_level
            .store(fill_percent as u64, Ordering::Relaxed);
        return false;
    }

    let mut underrun = false;

    if let Ok(chunk) = consumer.read_chunk(requested) {
        let (first, second) = chunk.as_slices();
        let first_len = first.len();
        let second_len = second.len();
        if first_len > 0 {
            scratch[..first_len].copy_from_slice(first);
        }
        if second_len > 0 {
            scratch[first_len..first_len + second_len].copy_from_slice(second);
        }
        chunk.commit_all();
        state
            .total_callback_samples
            .fetch_add(requested as u64, Ordering::Relaxed);
    } else {
        let available = consumer.slots().min(requested);
        if let Ok(chunk) = consumer.read_chunk(available) {
            let (first, second) = chunk.as_slices();
            let first_len = first.len();
            let second_len = second.len();
            if first_len > 0 {
                scratch[..first_len].copy_from_slice(first);
            }
            if second_len > 0 {
                scratch[first_len..first_len + second_len].copy_from_slice(second);
            }
            chunk.commit_all();
        }
        state
            .total_callback_samples
            .fetch_add(available as u64, Ordering::Relaxed);
        if available < requested {
            scratch[available..requested].fill(0.0);
        }
        underrun = true;
        state.underrun_count.fetch_add(1, Ordering::Relaxed);
    }

    let slots = consumer.slots();
    let fill_percent = (slots * 100).checked_div(capacity).unwrap_or(0);
    state
        .last_buffer_level
        .store(fill_percent as u64, Ordering::Relaxed);

    underrun
}
