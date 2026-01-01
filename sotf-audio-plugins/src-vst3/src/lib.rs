//! SOTF Audio Plugins - VST3/CLAP Wrapper
//!
//! This crate provides VST3 and CLAP plugin exports for SOTF audio plugins
//! using the nih-plug framework.
//!
//! ## Supported Plugins
//!
//! - **SOTF EQ**: 4-band parametric equalizer with GPUI-based visualization
//!
//! ## Platform Support
//!
//! - Linux: VST3 + CLAP
//! - Windows: VST3
//! - macOS: VST3

mod eq_params;
mod eq_plugin;
mod editor;

use eq_plugin::SotfEq;
use nih_plug::prelude::*;

// Export VST3 plugin
nih_export_vst3!(SotfEq);

// Export CLAP plugin
nih_export_clap!(SotfEq);
