//! Server-sent event types for the SOTF remote API.
//!
//! These events are broadcast from the headless server to connected
//! clients over an SSE stream at `/api/v1/events`.

use serde::{Deserialize, Serialize};

/// Events emitted by the SOTF server that remote clients can subscribe to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SotfServerEvent {
    /// Playback state changed (play, pause, stop, next, previous, seek).
    PlaybackChanged,
    /// Queue was mutated (add, delete, clear, jump).
    QueueChanged {
        /// Monotonically-increasing playlist version so clients can
        /// detect when their cached queue is stale.
        playlist_version: u32,
    },
    /// Volume was changed.
    VolumeChanged {
        /// Volume level 0–100.
        volume: u8,
    },
    /// Live stream metadata updated (e.g. ICY title from radio).
    StreamMetadataChanged {
        /// Optional title from the stream.
        title: Option<String>,
        /// Optional artist from the stream.
        artist: Option<String>,
    },
    /// Library scanner progress update.
    ScannerProgress {
        /// Number of items processed so far.
        done: usize,
        /// Total number of items to process.
        total: usize,
    },
    /// Library metadata changed and clients should invalidate cached pages.
    LibraryChanged {
        /// Monotonically-increasing library version for cache keys.
        library_version: u64,
    },
    /// A playback or server error occurred.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

impl SotfServerEvent {
    /// Serialize the event to a JSON payload suitable for an SSE `data:` field.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"error\":\"serialization\"}".to_string())
    }
}

/// A broadcast sender for server events.
///
/// Wrapped in an `Arc` so it can be cloned cheaply into adapters and
/// shared across the server state.
pub type EventBroadcaster = tokio::sync::broadcast::Sender<SotfServerEvent>;

/// Create a new event broadcaster with the default capacity.
pub fn new_event_broadcaster(capacity: usize) -> EventBroadcaster {
    tokio::sync::broadcast::channel(capacity).0
}
