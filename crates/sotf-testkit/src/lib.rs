//! Shared test fixtures and helpers for the SOTF workspace.
//!
//! This crate is intended to be used only as a `dev-dependency`. It provides
//! deterministic audio generators, temporary databases, and (behind feature
//! flags) engine / plugin test harness helpers.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    reason = "test helper crate: these pedantic lints add noise without improving test fixture correctness"
)]

pub mod assertions;
pub mod audio;
pub mod control;
pub mod db;

#[cfg(feature = "engine")]
pub mod audio_device;

#[cfg(feature = "engine")]
pub mod engine;

#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "plugin")]
pub mod mock_server;

#[cfg(feature = "engine")]
pub use audio_device::find_device;
