//! Shared test fixtures and helpers for the SOTF workspace.
//!
//! This crate is intended to be used only as a `dev-dependency`. It provides
//! deterministic audio generators, temporary databases, and (behind feature
//! flags) engine / plugin test harness helpers.

pub mod audio;
pub mod db;

#[cfg(feature = "engine")]
pub mod engine;

#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "plugin")]
pub mod mock_server;
