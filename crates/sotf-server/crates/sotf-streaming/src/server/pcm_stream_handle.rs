use super::pcm_stream_chunk::PcmStreamChunk;
use super::shared_stats::SharedStats;
use super::types::PcmStreamStats;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{SyncSender, TrySendError};

#[derive(Clone)]
pub struct PcmStreamHandle {
    pub(super) chunk_tx: SyncSender<PcmStreamChunk>,
    pub(super) stats: Arc<SharedStats>,
}

impl PcmStreamHandle {
    pub fn publish(
        &self,
        samples: &[f32],
        num_frames: usize,
        channels: usize,
        sample_rate: u32,
    ) -> bool {
        let Ok(channels) = u16::try_from(channels) else {
            self.stats.dropped_chunks.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        let chunk = match PcmStreamChunk::new(samples.to_vec(), num_frames, channels, sample_rate) {
            Ok(chunk) => chunk,
            Err(e) => {
                log::warn!("[PCM Stream] Dropping invalid chunk: {}", e);
                self.stats.dropped_chunks.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        };

        match self.chunk_tx.try_send(chunk) {
            Ok(()) => {
                self.stats.published_chunks.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .published_frames
                    .fetch_add(num_frames as u64, Ordering::Relaxed);
                self.stats
                    .published_bytes
                    .fetch_add(std::mem::size_of_val(samples) as u64, Ordering::Relaxed);
                true
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.stats.dropped_chunks.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.stats.local_addr
    }

    pub fn stats(&self) -> PcmStreamStats {
        self.stats.snapshot()
    }
}
