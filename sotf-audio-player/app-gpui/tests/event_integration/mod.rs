//! Phase 4: Event-Level Integration Tests
//!
//! NOTE: Full GPUI event integration tests cannot be compiled due to the
//! gpui::test macro causing stack overflow in syn (SIGBUS). This is a known
//! limitation documented in Cargo.toml.
//!
//! Instead, this module provides:
//! 1. Event simulation types and infrastructure (for future use)
//! 2. State-level tests that verify the event handling logic
//! 3. Documentation of the intended full integration test patterns
//!
//! When the GPUI macro issue is resolved, the tests in keyboard_tests.rs
//! and action_tests.rs can be uncommented to enable full event simulation.

#[path = "../common/mod.rs"]
mod common;

mod event_types;
mod state_tests;

// Re-export for external use
pub use event_types::*;
