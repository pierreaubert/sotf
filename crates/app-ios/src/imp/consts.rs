use super::types::RemoteCommand;
use crossbeam::queue::SegQueue;
use sotf_audio_player::Player;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

/// Global handle to the player so C FFI callbacks can control playback.
/// Set once during app initialization, never changes.
pub(super) static GLOBAL_PLAYER: OnceLock<Arc<parking_lot::Mutex<Player>>> = OnceLock::new();

/// Lock-free queue for remote commands enqueued by native iOS callbacks
/// (Control Center, lock screen, document picker, QR scanner) and drained by
/// the GPUI tick on the main thread.
pub(super) static PENDING_REMOTE_COMMANDS: OnceLock<SegQueue<RemoteCommand>> = OnceLock::new();

/// Lock-free queue for file paths imported via the iOS document picker.
pub(super) static PENDING_IMPORTED_FILES: OnceLock<SegQueue<PathBuf>> = OnceLock::new();

/// Lock-free queue for payloads scanned by the native QR code reader.
pub(super) static PENDING_QR_PAYLOADS: OnceLock<SegQueue<String>> = OnceLock::new();

/// Lock-free queue for Dynamic Type font-scale updates from UIKit.
pub(super) static PENDING_DYNAMIC_TYPE_SCALES: OnceLock<SegQueue<f32>> = OnceLock::new();

/// Test-only mutex used to serialize tests that touch the global pending queues.
/// `parking_lot` is used so a panicking test does not poison the lock.
#[cfg(test)]
pub(super) static QUEUE_TEST_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
