use super::stream_position::StreamPosition;
use crate::decoder::core::DecodedAudio;
use crate::decoder::error::AudioDecoderError;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};

/// Commands that can be sent to the audio stream
#[derive(Debug, Clone)]
pub enum StreamCommand {
    /// Start playback
    Play,
    /// Pause playback (buffering continues)
    Pause,
    /// Stop playback and reset to beginning
    Stop,
    /// Seek to specific frame position
    Seek(u64),
    /// Get current position
    GetPosition,
}

/// Events emitted by the audio stream
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream has started
    Started,
    /// Stream is playing
    Playing,
    /// Stream is paused
    Paused,
    /// Stream has stopped
    Stopped,
    /// Stream position update
    Position(StreamPosition),
    /// Decoded audio chunk ready for consumers
    Audio(DecodedAudio),
    /// End of stream reached
    EndOfStream,
    /// An error occurred
    Error(AudioDecoderError),
    /// Buffering progress (0.0 to 1.0)
    Buffering(f32),
}

/// Audio streaming state
#[derive(Debug, Clone, PartialEq)]
pub enum StreamState {
    Idle,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Error,
}

pub(super) fn lock_stream_state(state: &Mutex<StreamState>) -> MutexGuard<'_, StreamState> {
    state.lock().unwrap_or_else(|poisoned| {
        log::warn!("[AudioStream] Recovering from poisoned stream state lock");
        poisoned.into_inner()
    })
}

pub(super) fn send_stream_event(
    event_tx: &Sender<StreamEvent>,
    event: StreamEvent,
    context: &str,
) -> bool {
    if let Err(e) = event_tx.send(event) {
        crate::rate_limited_log!(
            trace,
            5,
            "[AudioStream] Dropped stream event in {}: {}",
            context,
            e
        );
        false
    } else {
        true
    }
}
