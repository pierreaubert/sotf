//! Event-Level Integration Tests (Behavioral Specification)
//!
//! These tests define a behavioral model of the expected key dispatch logic,
//! verifying that keybindings fire in normal mode and are blocked in text input
//! modes. They exercise a test-only `EventHandlerState`, NOT production code.
//!
//! NOTE: Full GPUI event integration tests cannot be compiled due to the
//! gpui::test macro causing stack overflow in syn (SIGBUS). This is a known
//! limitation documented in Cargo.toml.
//!
//! When the GPUI macro issue is resolved, these behavioral specs can be
//! replaced with tests that dispatch real events through the GPUI event system.

mod event_types;
mod state_tests;

// Re-export for external use
pub use event_types::*;
