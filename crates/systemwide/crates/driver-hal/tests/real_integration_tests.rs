#![cfg(target_os = "macos")]
//! Real Integration Tests for HAL + Daemon Pipeline
//!
//! These tests verify the ACTUAL audio pipeline between:
//! - Swift HAL driver (CoreAudio plugin)
//! - Shared memory IPC (`/tmp/sotf-{uid}/audio.shm`)
//! - Rust daemon (sotf-daemon)
//! - Unix socket IPC
//!
//! # Prerequisites
//!
//! These tests require:
//! 1. HAL driver installed at `/Library/Audio/Plug-Ins/HAL/SotFHAL.driver`
//! 2. Daemon running (`cargo run --bin sotf-daemon --features hal`)
//! 3. macOS (HAL is macOS-only)
//!
//! # Running
//!
//! ```bash
//! # Run all real integration tests (requires setup)
//! cargo test -p driver-hal --test real_integration_tests -- --ignored
//!
//! # Run specific test
//! cargo test -p driver-hal --test real_integration_tests test_real_shared_memory_connection -- --ignored
//! ```
//!
//! # Safety
//!
//! These tests interact with real system resources. They are designed to be
//! read-only where possible and will not modify system state.

#[path = "real_integration_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "real_integration_tests/tests.rs"]
mod tests;
