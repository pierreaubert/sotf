//! Navigation integration tests.
//!
//! Verify that key sequences drive the TUI through the expected screen,
//! input-mode, and wizard-step transitions.

#[path = "test_events_navigation/app.rs"]
mod app;
#[path = "test_events_navigation/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "test_events_navigation/tests.rs"]
mod tests;
