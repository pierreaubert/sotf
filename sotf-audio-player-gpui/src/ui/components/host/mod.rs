//! Plugin Host UI Components
//!
//! This module contains the plugin host (rack) UI and related functionality:
//! - `rack`: Plugin rack/chain visualization and signal flow display
//! - `plugin_editing`: Plugin parameter editing logic (App impl methods)

mod plugin_editing;
mod rack;

pub use plugin_editing::get_param_count;
pub use rack::PluginDragInfo;
