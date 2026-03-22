//! Format-agnostic adapter for SOTF audio plugins.
//!
//! This crate provides a shared bridge layer used by both AU (via plugins-ffi)
//! and VST3/CLAP (via plugins-nih) to create, configure, and process SOTF plugins.

pub mod buffers;
pub mod factory;
pub mod param_bridge;
pub mod state;

pub use factory::create_plugin;
pub use param_bridge::ParamBridge;
