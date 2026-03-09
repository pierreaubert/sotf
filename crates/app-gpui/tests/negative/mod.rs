//! Negative Tests for player-gpui
//!
//! These tests verify that things that SHOULD NOT happen, don't happen.
//! They test REAL production types (LibraryController, QueueController,
//! PlaybackController, InputMode) — not test doubles.
//!
//! # Philosophy
//!
//! Traditional tests verify "X works" but not "X doesn't break Y".
//! Negative tests fill this gap by asserting conditions that must NEVER be violated.
//!
//! # Usage
//!
//! ```bash
//! cargo test -p sotf-gpui --test negative
//! ```

mod invalid_state_tests;
mod keybinding_conflicts;
mod playback_state_persistence;
