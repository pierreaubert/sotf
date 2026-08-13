use super::super::DecoderCommand;
use std::sync::mpsc::{Receiver, SyncSender};
use std::time::Duration;

pub(super) const SPIN_MS_SLEEP_DECODER: u64 = 1;

pub(super) const SEND_OR_INTERRUPT_MAX_RETRIES: usize = 200;

#[cfg(all(target_os = "macos", feature = "hal"))]
pub(super) const HAL_RECONNECT_INTERVAL: Duration = Duration::from_secs(1);

/// Float PCM in CoreAudio can exceed 1.0 briefly, but values this large are
/// unsafe and indicate a feedback loop, stale/corrupt shared memory, or a
/// format/key mismatch. Drop the whole block instead of feeding a runaway path.
#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
pub(super) const HAL_INPUT_RUNAWAY_PEAK_LIMIT: f32 = 8.0;

/// Maximum engine block size (frames) used to pre-size decoder/processing
/// scratch buffers. Blocks larger than this still work, but may allocate once
/// on the first oversized call.
pub(super) const MAX_ENGINE_BLOCK_FRAMES: usize = 8192;

/// Maximum channel count used to pre-size engine scratch buffers.
pub(super) const MAX_ENGINE_CHANNELS: usize = crate::EngineConfig::MAX_CHANNELS;

/// Worst-case interleaved sample count for one engine block.
pub(super) const MAX_ENGINE_SAMPLE_CAPACITY: usize = MAX_ENGINE_BLOCK_FRAMES * MAX_ENGINE_CHANNELS;

/// Resampler output headroom for the worst common ratio (up to ~2x).
pub(super) const MAX_RESAMPLE_OUTPUT_SAMPLES: usize = MAX_ENGINE_SAMPLE_CAPACITY * 2;

/// Maximum size of resample staging buffer to prevent unbounded growth.
/// Sized for several complete max-size blocks to absorb resampler jitter
/// (e.g., 48kHz→44.1kHz produces slightly smaller blocks per input chunk).
pub(super) const MAX_RESAMPLE_STAGING_SAMPLES: usize = MAX_ENGINE_SAMPLE_CAPACITY * 4;

pub(super) const DECODER_LOCAL_FRAME_POOL_SIZE: usize = 8;

pub(super) const DECODER_LOCAL_FRAME_CAPACITY: usize = MAX_ENGINE_SAMPLE_CAPACITY;

/// Helper to send a message with backpressure handling and interruption support
pub(super) fn send_or_interrupt<T>(
    tx: &SyncSender<T>,
    rx: &Receiver<DecoderCommand>,
    mut msg: T,
) -> Result<Option<(DecoderCommand, T)>, String> {
    let mut retries = 0;
    loop {
        match tx.try_send(msg) {
            Ok(_) => return Ok(None),
            Err(std::sync::mpsc::TrySendError::Full(returned_msg)) => {
                // Buffer full - check for interruption
                if let Ok(cmd) = rx.try_recv() {
                    return Ok(Some((cmd, returned_msg)));
                }
                retries += 1;
                if retries > SEND_OR_INTERRUPT_MAX_RETRIES {
                    return Err("Decoder output queue stuck for >200ms".to_string());
                }
                msg = returned_msg;
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(format!("Channel disconnected: {}", e)),
        }
    }
}
