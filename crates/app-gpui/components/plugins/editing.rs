//! Plugin management and editing methods.
//!
//! Thin wrapper over `PluginController` from sotf-player. Each method delegates
//! to the controller and handles the returned `PluginUpdateEffect` by setting
//! `pending_plugin_update` on the GPUI-specific `PluginState`.

pub use sotf_audio_player::get_param_count;

mod misc;
mod plugin_editing_manager;

pub use plugin_editing_manager::*;
