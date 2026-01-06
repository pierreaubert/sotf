//! Test scenarios for E2E testing.
//!
//! Each module contains related test scenarios:
//! - `startup`: Tests for application startup behavior
//! - `volume`: Tests for volume control interactions
//! - `playback`: Tests for audio playback functionality
//!
//! Component tests:
//! - `home`: Home screen components (footer, library, queue, header)
//! - `plugins`: Plugin UI tests and plugin chain management
//! - `integration`: End-to-end workflow tests

pub mod home;
pub mod integration;
pub mod playback;
pub mod plugins;
pub mod startup;
pub mod volume;
