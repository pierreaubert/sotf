//! SOTF EQ Plugin with Slint UI
//!
//! A 4-band parametric equalizer using plinth-plugin framework and Slint UI.

mod editor;
mod parameters;
mod plugin;
mod processor;
mod view;

use plugin::SotfEqPlugin;
use plinth_plugin::{export_clap, export_vst3};

// Export the plugin for CLAP and VST3 formats
export_clap!(SotfEqPlugin);
export_vst3!(SotfEqPlugin);
