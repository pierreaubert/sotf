//! Test scenarios for E2E testing.
//!
//! Each module contains related test scenarios:
//! - `startup`: Tests for application startup behavior
//! - `volume`: Tests for volume control interactions
//! - `playback`: Tests for audio playback functionality
//!
//! Component tests:
//! - `dialogs`: Application dialogs (help, about)
//! - `home`: Home screen components (footer, library, queue, header)
//! - `plugins`: Plugin UI tests and plugin chain management
//! - `settings`: Application settings (audio device, library, theme, language, keybindings)
//! - `wizards`: Multi-step wizard flows (headphone EQ, room EQ, recording)
//! - `integration`: End-to-end workflow tests

pub mod dialogs;
pub mod home;
pub mod integration;
pub mod playback;
pub mod plugins;
pub mod settings;
pub mod startup;
pub mod volume;
pub mod wizards;
