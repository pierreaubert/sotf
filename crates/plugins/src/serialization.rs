// ============================================================================
// Plugin Serialization
// ============================================================================

use super::error::PluginError;
use super::parameters::ParameterValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for plugins that support preset serialization
///
/// This trait enables plugins to save and load their state as presets.
/// Plugins implement this trait to support:
/// - Preset file save/load
/// - Parameter automation
/// - Plugin state snapshots
///
/// # Example
/// ```rust,ignore
/// use sotf_plugins::{SerializablePlugin, PluginPreset};
///
/// impl SerializablePlugin for EqPlugin {
///     fn serialize(&self) -> Result<PluginPreset, PluginError> {
///         Ok(PluginPreset {
///             name: "My EQ".to_string(),
///             plugin_id: "sotf-eq".to_string(),
///             version: env!("CARGO_PKG_VERSION").to_string(),
///             parameters: self.parameters_to_map(),
///             data: HashMap::new(),
///             metadata: PresetMetadata::default(),
///         })
///     }
///
///     fn deserialize(&mut self, preset: &PluginPreset) -> Result<(), PluginError> {
///         self.parameters_from_map(&preset.parameters)
///     }
/// }
/// ```
pub trait SerializablePlugin {
    /// Serialize plugin state to a preset
    fn serialize(&self) -> Result<PluginPreset, PluginError>;

    /// Deserialize plugin state from a preset
    fn deserialize(&mut self, preset: &PluginPreset) -> Result<(), PluginError>;

    /// Get all parameter values as a map
    fn parameters_to_map(&self) -> HashMap<String, ParameterValue>;

    /// Set parameters from a map
    fn parameters_from_map(
        &mut self,
        params: &HashMap<String, ParameterValue>,
    ) -> Result<(), PluginError>;
}

/// A serializable plugin preset
///
/// Presets contain all the state needed to recreate a plugin's configuration.
/// They can be saved to files and loaded later.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginPreset {
    /// Preset name
    pub name: String,

    /// Plugin identifier (unique ID, e.g., "sotf-eq", "sotf-compressor")
    pub plugin_id: String,

    /// Plugin version when preset was created
    pub version: String,

    /// Parameter values
    pub parameters: HashMap<String, ParameterValue>,

    /// Extended data (plugin-specific, serialized as JSON)
    pub data: HashMap<String, serde_json::Value>,

    /// User metadata
    #[serde(default)]
    pub metadata: PresetMetadata,
}

impl PluginPreset {
    /// Create a new preset with basic fields
    pub fn new(name: String, plugin_id: String, version: String) -> Self {
        Self {
            name,
            plugin_id,
            version,
            parameters: HashMap::new(),
            data: HashMap::new(),
            metadata: PresetMetadata::default(),
        }
    }

    /// Check if this preset is compatible with the given plugin ID
    pub fn is_compatible(&self, plugin_id: &str) -> bool {
        self.plugin_id == plugin_id
    }

    /// Add a tag to the preset
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.metadata.tags.push(tag.into());
    }

    /// Set the author
    pub fn set_author(&mut self, author: impl Into<String>) {
        self.metadata.author = Some(author.into());
    }

    /// Add a comment
    pub fn set_comment(&mut self, comment: impl Into<String>) {
        self.metadata.comment = Some(comment.into());
    }
}

impl Default for PluginPreset {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            plugin_id: "unknown".to_string(),
            version: "0.0.0".to_string(),
            parameters: HashMap::new(),
            data: HashMap::new(),
            metadata: PresetMetadata::default(),
        }
    }
}

/// User metadata for a preset
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PresetMetadata {
    /// Author name
    #[serde(default)]
    pub author: Option<String>,

    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// User comment/description
    #[serde(default)]
    pub comment: Option<String>,
}

impl PresetMetadata {
    /// Create empty metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the creation timestamp to now
    pub fn set_created_now(&mut self) {
        self.created_at = Some(Utc::now());
    }
}

/// Built-in preset bank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetBank {
    /// Bank name
    pub name: String,

    /// Presets in the bank
    pub presets: Vec<PluginPreset>,
}

impl PresetBank {
    /// Create a new empty bank
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            presets: Vec::new(),
        }
    }

    /// Add a preset to the bank
    pub fn add_preset(&mut self, preset: PluginPreset) {
        self.presets.push(preset);
    }

    /// Find a preset by name
    pub fn find_preset(&self, name: &str) -> Option<&PluginPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Get presets by tag
    pub fn presets_with_tag(&self, tag: &str) -> Vec<&PluginPreset> {
        self.presets
            .iter()
            .filter(|p| p.metadata.tags.contains(&tag.to_string()))
            .collect()
    }
}
