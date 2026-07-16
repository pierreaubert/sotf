//! Audio HAL Driver - Shared Memory Interface
//!
//! This library provides the Rust side of the shared memory interface
//! for communicating with the Swift HAL driver.
//!
//! The Swift HAL driver (in swift/) creates a virtual audio device on macOS
//! and uses shared memory at `/tmp/sotf-{uid}/audio.shm` to exchange audio data
//! with the Rust audio engine. Each user has their own shared memory region
//! for security isolation.
//!
//! # Data Flow
//!
//! ```text
//! macOS Audio Apps → Swift HAL Driver
//!                         ↓
//!             Shared Memory (interleaved, N channels)
//!                         ↓
//!             Rust Audio Engine (reads via HalInputReader)
//!                         ↓
//!             DSP Chain (GainPlugin for volume, EQ, etc.)
//!                         ↓
//!             Rust Audio Engine (writes via HalOutputWriter)
//!                         ↓
//!             Shared Memory (interleaved, N channels)
//!                         ↓
//!             Swift HAL Driver → macOS Audio System
//! ```
//!
//! # Channel Support
//!
//! The HAL driver supports dynamic channel counts (1-32 channels) as specified
//! in the shared memory header. The channel count is read from the header at
//! runtime, allowing for stereo, 5.1, 7.1, and other configurations.
//!
//! # Volume Control
//!
//! Volume control is handled via the DSP chain using `GainPlugin` from the
//! `plugins` crate. The `driver-hal` crate does not provide its own volume
//! types — the previous `volume` module was dead code and has been removed.
//!
//! # Real-time Safety
//!
//! All cross-process header fields are `AtomicU32`/`AtomicU64` to avoid data
//! races with the Swift HAL plugin. The audio I/O hot paths perform no
//! filesystem I/O and no allocations beyond the pre-allocated staging buffers
//! created in `HalInputReader::new`/`HalOutputWriter::new`.
//!
//! Per-call trace logging is gated behind the `audio-trace` feature flag.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "macos")]
pub mod driver;
#[cfg(target_os = "macos")]
pub mod encryption;
#[cfg(target_os = "macos")]
pub mod shared_memory;

#[cfg(target_os = "macos")]
pub use driver::HalDriver;
#[cfg(target_os = "macos")]
pub use encryption::{
    AUTH_TAG_SIZE, AudioCipher, compute_fingerprint, encrypted_to_samples, fingerprint_to_hex,
    generate_key, samples_to_encrypted,
};
#[cfg(target_os = "macos")]
pub use shared_memory::{
    HalInputReader, HalOutputWriter, SharedAudioBuffer, get_shared_memory_path,
};
