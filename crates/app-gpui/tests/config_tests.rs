//! Config and component tests for GPUI App.
//!
//! These tests verify config serialization, playback state defaults,
//! image cache behavior, tick scale calculations, and icon properties.
//! Extracted from inline tests to work around GPUI macro recursion issues.

#[path = "config_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "config_tests/tests.rs"]
mod tests;
