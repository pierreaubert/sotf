#![allow(clippy::field_reassign_with_default)]
//! Engine Types Integration Tests
//!
//! Tests for the audio engine types including:
//! - AudioFrame creation and manipulation
//! - Message types
//! - State management
//! - Plugin configuration

#[cfg(test)]
#[path = "engine_types_tests/tests.rs"]
mod tests;
