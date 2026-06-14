use super::plugin_settings::PluginSettings;
use super::plugin_type::PluginType;
use crate::engine::PluginConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: usize,
    pub enabled: bool,
    pub settings: PluginSettings,
    /// If true, this plugin cannot be removed from the chain (part of default rack)
    #[serde(default)]
    pub permanent: bool,
    /// Temporarily disabled due to channel incompatibility; auto-restores on compatible tracks
    #[serde(skip)]
    pub suspended: bool,
    /// Optional user-facing name (e.g., "Room EQ", "Broadband EQ"). When None,
    /// the UI falls back to the plugin type display name. Persisted to JSON
    /// so named instances survive save/reload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Plugin {
    pub fn new(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
            permanent: false,
            suspended: false,
            name: None,
        }
    }

    /// Create a permanent plugin that cannot be removed
    pub fn new_permanent(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
            permanent: true,
            suspended: false,
            name: None,
        }
    }

    pub fn plugin_type(&self) -> PluginType {
        self.settings.plugin_type()
    }

    /// Returns true if this plugin is permanent and cannot be removed
    pub fn is_permanent(&self) -> bool {
        self.permanent
    }

    /// User-facing display name. Falls back to the plugin type's static name
    /// when no custom name has been set.
    pub fn display_name(&self) -> String {
        match &self.name {
            Some(n) if !n.is_empty() => n.clone(),
            _ => self.plugin_type().name().to_string(),
        }
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> Option<PluginConfig> {
        if self.enabled && !self.suspended {
            Some(self.settings.to_plugin_config(sample_rate))
        } else {
            None
        }
    }
}
