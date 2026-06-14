//! State-level event integration tests.
//!
//! These tests verify event handling logic without requiring the full GPUI
//! test infrastructure. They test the same scenarios as the full integration
//! tests but at the state/handler level.

use super::event_types::*;

mod event_handler_state;
#[cfg(test)]
mod tests;
mod types;
