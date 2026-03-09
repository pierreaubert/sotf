//! Phase 3: Lifecycle Sequence Tests
//!
//! These tests verify that state is preserved correctly across sequences of operations,
//! simulating realistic user workflows. Unlike unit tests that test individual operations,
//! lifecycle tests verify multi-step scenarios.
//!
//! Key scenarios tested:
//! - Multi-song playback with state preservation
//! - Search/filter workflows
//! - Queue management sequences
//! - Input mode transitions
//! - Plugin configuration sequences

#[path = "../common/mod.rs"]
#[allow(dead_code)]
mod common;

mod input_sequences;
mod library_sequences;
mod playback_sequences;
mod queue_sequences;
