//! Engine Latency Tests
//!
//! Comprehensive tests for engine latency, timing, and phase transitions.
//! Tests cover:
//! - Phase transition latencies (play, pause, resume, stop, seek)
//! - Position accuracy during playback
//! - Buffer underrun monitoring
//! - Rapid state change stress tests
//! - Plugin chain hot-swap latency

mod common;

#[cfg(test)]
#[path = "engine_latency_tests/tests.rs"]
mod tests;
