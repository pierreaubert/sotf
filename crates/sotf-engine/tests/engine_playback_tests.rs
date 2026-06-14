//! Playback Thread Tests
//!
//! Unit tests for the playback thread that handles audio output to hardware.
//! All tests require BlackHole virtual audio device to avoid playing sound on real devices.

mod common;

#[cfg(test)]
#[path = "engine_playback_tests/tests.rs"]
mod tests;
