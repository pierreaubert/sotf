//! Stress Tests for Audio Engine
//!
//! Tests that stress the audio engine under heavy load and edge cases.
//! All tests require BlackHole virtual audio device to avoid playing sound on real devices.

mod common;

#[cfg(test)]
#[path = "engine_stress_tests/tests.rs"]
mod tests;
