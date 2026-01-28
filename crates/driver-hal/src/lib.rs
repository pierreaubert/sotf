//! Audio HAL Driver - Shared Memory Interface
//!
//! This library provides the Rust side of the shared memory interface
//! for communicating with the Swift HAL driver.
//!
//! The Swift HAL driver (in swift/) creates a virtual audio device on macOS
//! and uses shared memory at /tmp/sotf-audio-shm to exchange audio data
//! with the Rust audio engine.
//!
//! Data flow:
//! - Swift HAL captures audio from macOS apps
//! - Swift HAL writes to shared memory
//! - Rust engine reads from shared memory, processes, and writes back
//! - Swift HAL reads processed audio and outputs to macOS apps

pub mod shared_memory;

pub use shared_memory::{
    HalInputReader, HalOutputWriter, SharedAudioBuffer, SHARED_MEMORY_PATH,
};
