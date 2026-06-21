use super::consts::MAX_ENGINE_SAMPLE_CAPACITY;
use std::sync::mpsc::Receiver;

/// Take frame_send_buffer for sending, then restore it from a recycled Vec or
/// the local spare pool so the steady-state handoff does not allocate.
pub(super) fn take_frame_buffer(
    frame_send_buffer: &mut Vec<f32>,
    recycle_rx: &Receiver<Vec<f32>>,
    local_pool: &mut Vec<Vec<f32>>,
    len: usize,
) -> Vec<f32> {
    let mut frame_data = std::mem::take(frame_send_buffer);
    frame_data.truncate(len);

    *frame_send_buffer = match recycle_rx.try_recv() {
        Ok(mut v) => {
            v.clear();
            v
        }
        Err(_) => local_pool.pop().unwrap_or_default(),
    };

    // Guarantee that the replacement buffer has enough capacity for the
    // worst-case engine block so the next hot-path copy never reallocates.
    if frame_send_buffer.capacity() < MAX_ENGINE_SAMPLE_CAPACITY {
        frame_send_buffer.reserve(MAX_ENGINE_SAMPLE_CAPACITY - frame_send_buffer.len());
    }

    frame_data
}

#[cfg(any(test, all(target_os = "macos", feature = "hal")))]
pub(super) fn frames_to_sample_count(frames: usize, channels: usize, max_samples: usize) -> usize {
    frames.saturating_mul(channels).min(max_samples)
}
