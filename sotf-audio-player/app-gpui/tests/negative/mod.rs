//! Negative Tests for sotf-audio-player-gpui
//!
//! These tests verify that things that SHOULD NOT happen, don't happen.
//! They catch bugs like keybinding conflicts, state corruption, and invalid configurations.
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
