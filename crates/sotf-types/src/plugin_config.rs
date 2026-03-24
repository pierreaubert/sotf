//! Plugin configuration types for serialization/deserialization.

use serde::{Deserialize, Serialize};

/// Plugin configuration for serialization/deserialization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin type identifier
    pub plugin_type: String,
    /// Plugin parameters
    pub parameters: serde_json::Value,
}

impl PluginConfig {
    /// Create a new plugin config
    pub fn new(plugin_type: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self {
            plugin_type: plugin_type.into(),
            parameters,
        }
    }
}

/// Graph-based plugin configuration for DAG processing.
///
/// Unlike `Vec<PluginConfig>` (linear chain), this supports parallel paths
/// needed for multi-driver crossover setups.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphConfig {
    pub nodes: Vec<PluginGraphNodeConfig>,
    pub edges: Vec<PluginGraphEdgeConfig>,
}

/// A node in the plugin graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphNodeConfig {
    /// Unique node ID (used to reference in edges)
    pub id: usize,
    pub plugin_type: String,
    pub parameters: serde_json::Value,
    /// Number of input channels this node expects
    pub input_channels: usize,
}

/// An edge connecting two nodes in the plugin graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphEdgeConfig {
    pub from_node: usize,
    pub to_node: usize,
}
