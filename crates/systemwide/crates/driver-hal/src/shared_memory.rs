//! Shared memory interface for communication with Swift HAL driver
//!
//! This module provides a Rust interface to the shared memory region
//! created by the Swift HAL driver for audio data exchange.
//!
//! # Cross-process memory model
//!
//! Every field in [`SharedAudioHeader`] that may be touched by both the daemon
//! (this crate) and the Swift HAL plugin running inside `coreaudiod` is an
//! atomic type (`AtomicU32`/`AtomicU64`). Plain non-atomic stores from one
//! process while the other process is reading the same word would be a data
//! race in the Rust/C++ abstract machines (undefined behaviour), so we publish
//! every cross-process value through `store(_, Ordering::Release)` and consume
//! it via `load(Ordering::Acquire)`. The Swift side performs equivalent
//! atomic accesses via `std::atomic`.
//!
//! The `key_fingerprint` is exposed externally as an `[u8; 8]` but stored as
//! an `AtomicU64` in big-endian byte order so that the 8 bytes are published
//! in a single atomic store.
//!
//! # Reconfiguration protocol
//!
//! Geometry changes go through [`SharedAudioBuffer::reconfigure_quiesced`].
//! The `configuring` word is a cross-process bitset:
//!
//! - bit 0 requests reconfiguration and blocks new IO commits;
//! - bit 1 reserves the reader cursor publication;
//! - bit 2 reserves the writer cursor publication.
//!
//! A reader or writer may copy audio before the request arrives. It must claim
//! its publication bit immediately before storing its cursor. Reconfiguration
//! claims bit 0, waits for the publication bits to clear, then resets the
//! geometry and ring positions. If that bounded wait times out, it releases
//! bit 0 and leaves all geometry and ring state unchanged. `configuring_ack`
//! remains a compatibility indication for the Swift IO path, but the commit
//! bits are the correctness mechanism that prevents a stale cursor store.
//!
//! The legacy `set_sample_rate` / `set_buffer_frames` / `set_channel_count`
//! setters route through this path so they no longer race the HAL writer.

use std::sync::atomic::{AtomicU32, AtomicU64};

mod consts;
mod current;
mod encrypted;
mod ensure;
mod grant;
mod hal_input_reader;
mod hal_output_writer;
mod misc;
mod shared_audio_buffer;
#[cfg(test)]
mod tests;
mod types;
mod validate;

pub use consts::*;
pub use hal_input_reader::*;
pub use hal_output_writer::*;
pub use misc::*;
pub use shared_audio_buffer::*;

/// The checked-in cross-language layout contract. Keeping this as an
/// `include_str!` makes the Rust build consume the same manifest that the
/// packaged Swift HAL bundle carries as a resource; tests below validate the
/// actual Rust offsets against every manifest field.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const SHARED_MEMORY_LAYOUT_MANIFEST: &str = include_str!("../shared_memory_layout.json");

/// Header structure for shared memory region.
///
/// Must match the Swift side exactly. All cross-process fields are atomic
/// (`AtomicU32`/`AtomicU64`). `AtomicU32` and `AtomicU64` have the same
/// memory layout and alignment as plain `u32`/`u64` (guaranteed by the
/// standard library), so the byte layout remains compatible with the
/// Swift `struct SharedAudioHeader` mirror.
#[repr(C, align(8))]
pub struct SharedAudioHeader {
    /// Magic number for validation (0x534F5446 = 'SOTF')
    pub magic: AtomicU32,
    /// Protocol version
    pub version: AtomicU32,
    /// Current sample rate in Hz
    pub sample_rate: AtomicU32,
    /// Frames per buffer
    pub buffer_frames: AtomicU32,
    /// Number of audio channels
    pub channel_count: AtomicU32,

    // Ring buffer state (atomic)
    /// Write position in samples
    pub write_position: AtomicU64,
    /// Read position in samples
    pub read_position: AtomicU64,

    // Control flags (atomic)
    /// IO is running
    pub active: AtomicU32,
    /// Configuration changed (engine should reload)
    pub config_changed: AtomicU32,
    /// Driver is initialized and ready
    pub driver_ready: AtomicU32,
    /// Rust engine is connected and ready
    pub engine_ready: AtomicU32,

    // Encryption fields (version 2+)
    /// Encryption enabled flag: 0 = disabled, 1 = enabled
    pub encrypted: AtomicU32,
    /// First 8 bytes of SHA256 hash of the encryption key, stored in
    /// big-endian byte order (so `to_be_bytes` gives the canonical 8-byte
    /// fingerprint).
    pub key_fingerprint: AtomicU64,
    /// Frame counter for nonce generation (monotonically increasing, never reuse!)
    pub frame_counter: AtomicU64,

    // Config negotiation fields (version 3+)
    /// Requested sample rate (set by requester, either HAL or Daemon)
    pub requested_sample_rate: AtomicU32,
    /// Requested buffer frames (set by requester)
    pub requested_buffer_frames: AtomicU32,
    /// Actual sample rate in use (set by responder after negotiation)
    pub actual_sample_rate: AtomicU32,
    /// Actual buffer frames in use (set by responder after negotiation)
    pub actual_buffer_frames: AtomicU32,
    /// Config status: 0=pending, 1=accepted, 2=negotiated, 3=error
    pub config_status: AtomicU32,
    /// Config source: 1=HAL initiated, 2=Daemon initiated
    pub config_source: AtomicU32,
    /// Error code if config_status=3
    pub config_error_code: AtomicU32,

    // Statistics
    /// Number of times encrypted write failed due to insufficient buffer space
    pub encryption_overflow_count: AtomicU64,
    /// Daemon liveness heartbeat in Unix epoch milliseconds.
    pub daemon_heartbeat_ms: AtomicU64,

    // Reconfiguration handshake (version 5+)
    /// 1 while the daemon is performing a quiesced reconfiguration. The
    /// Swift HAL plugin must drop any pending write and refrain from
    /// publishing new `write_position` values while this is set.
    pub configuring: AtomicU32,
    /// Set by an IO participant after it observes `configuring = 1`.
    pub configuring_ack: AtomicU32,
    /// Channel count requested by the initiator. The live `channel_count`
    /// field changes only inside `reconfigure_quiesced`.
    pub requested_channel_count: AtomicU32,
}
