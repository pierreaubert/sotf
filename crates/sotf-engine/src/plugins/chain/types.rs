use super::super::Plugin;
use super::misc::default_plugin_preset_version;
use serde::{Deserialize, Serialize};

/// Versioned wrapper for plugin presets (used for saving)
#[derive(Debug, Clone, Serialize)]
pub(super) struct PluginPreset {
    pub(super) version: u32,
    pub(super) plugins: Vec<Plugin>,
}

/// Lenient versioned wrapper for plugin presets (used for loading).
/// Plugins are raw JSON values so that individual plugin deserialization
/// failures don't reject the entire file.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct PluginPresetRaw {
    #[serde(default = "default_plugin_preset_version")]
    pub(super) version: u32,
    pub(super) plugins: Vec<serde_json::Value>,
}
