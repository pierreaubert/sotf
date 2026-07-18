//! TUI Audio Player library.
//!
//! This crate provides the TUI-based music player application.
//! The library exposes the App state for testing and integration purposes.

pub mod app;
#[cfg(feature = "dev-api")]
pub mod dev_api;
pub mod events;
pub mod i18n;
pub mod media_controls;
pub mod theme;
pub mod ui;
