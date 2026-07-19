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
    pub fn new(id: usize, plugin_type: &PluginType) -> Result<Self, String> {
        Ok(Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type)?,
            permanent: false,
            suspended: false,
            name: None,
        })
    }

    /// Create a permanent plugin that cannot be removed
    pub fn new_permanent(id: usize, plugin_type: &PluginType) -> Result<Self, String> {
        Ok(Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type)?,
            permanent: true,
            suspended: false,
            name: None,
        })
    }

    /// Construct a plugin from concrete settings without requiring a generic
    /// [`PluginType`] default. This is the only supported construction path
    /// for external plugins.
    pub fn from_settings(id: usize, settings: PluginSettings) -> Result<Self, String> {
        let name = Self::validated_default_name(&settings)?;

        Ok(Self {
            id,
            enabled: true,
            settings,
            permanent: false,
            suspended: false,
            name,
        })
    }

    /// Revalidate deserialized settings before they are committed to a graph.
    pub fn validate(&self) -> Result<(), String> {
        Self::validated_default_name(&self.settings).map(|_| ())
    }

    fn validated_default_name(settings: &PluginSettings) -> Result<Option<String>, String> {
        let name = match settings {
            PluginSettings::External { state } => {
                state.validate()?;
                if state.descriptor.is_instrument || state.descriptor.audio_inputs == 0 {
                    return Err(format!(
                        "External plugin '{}' is an instrument; the audio-effect rack requires at least one input channel",
                        state.descriptor.name
                    ));
                }
                if state.sandbox_mode != sotf_plugins::ExternalPluginSandboxMode::Isolated {
                    return Err(format!(
                        "External plugin '{}' must use isolated hosting",
                        state.descriptor.name
                    ));
                }
                if state.descriptor.scan_status != sotf_plugins::PluginScanStatus::Loadable {
                    return Err(format!(
                        "External plugin '{}' is not loadable in this build ({:?})",
                        state.descriptor.name, state.descriptor.scan_status
                    ));
                }
                Some(state.descriptor.name.clone())
            }
            _ => None,
        };
        Ok(name)
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
            let mut config = self.settings.to_plugin_config(sample_rate);
            if matches!(self.settings, PluginSettings::External { .. })
                && let Some(parameters) = config.parameters.as_object_mut()
            {
                parameters.insert(
                    sotf_plugins::EXTERNAL_PLUGIN_INSTANCE_ID_PARAMETER.to_string(),
                    serde_json::json!(self.id),
                );
            }
            Some(config)
        } else {
            None
        }
    }
}
