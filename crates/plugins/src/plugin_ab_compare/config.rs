//! Configuration types and parameters for the A/B Compare plugin.

use crate::auto_gain::AutoGainLoudnessType;
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration Types
// ============================================================================

/// Configuration for a processing path (A or B)
/// Can represent a single plugin, a rack (chain), or a graph
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type")]
pub enum PathConfig {
    /// Empty path - pass-through
    #[default]
    None,
    /// Single plugin
    Plugin {
        plugin_type: String,
        #[serde(default)]
        parameters: serde_json::Value,
    },
    /// Linear chain of plugins (rack)
    Rack { plugins: Vec<PluginInRack> },
    /// Full graph with nodes and edges
    Graph {
        nodes: Vec<GraphNodeConfig>,
        edges: Vec<GraphEdgeConfig>,
    },
}

/// A plugin in a rack (chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInRack {
    pub plugin_type: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// A node in a graph configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeConfig {
    pub id: String,
    pub plugin_type: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// An edge in a graph configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdgeConfig {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub channel_map: Option<Vec<usize>>,
}

/// Mix mode for A/B comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MixMode {
    /// Continuous mix with potentiometer (-1.0 to +1.0)
    #[default]
    Potentiometer,
    /// Binary A/B switch
    Binary,
}

/// Loudness measurement type for auto-gain
/// Re-exported from auto_gain module for API compatibility
pub type LoudnessType = AutoGainLoudnessType;

/// Configuration parameters for ABComparePlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABComparePluginParams {
    /// Configuration for path A
    #[serde(default)]
    pub path_a: PathConfig,

    /// Configuration for path B
    #[serde(default)]
    pub path_b: PathConfig,

    /// Mix mode (potentiometer or binary switch)
    #[serde(default)]
    pub mix_mode: MixMode,

    /// Mix value: -1.0 = pure A, 0.0 = 50/50, +1.0 = pure B
    #[serde(default)]
    pub mix: f32,

    /// Selected path for binary mode (0 = A, 1 = B)
    #[serde(default)]
    pub selected_path: i32,

    /// Bypass both A and B, output original input
    #[serde(default)]
    pub bypass: bool,

    /// Enable automatic loudness matching
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Loudness measurement type for auto-gain
    #[serde(default)]
    pub loudness_type: LoudnessType,

    /// Gain smoothing time in ms
    #[serde(default = "default_gain_smoothing_ms")]
    pub gain_smoothing_ms: f32,

    /// Maximum auto-gain correction in dB
    #[serde(default = "default_max_auto_gain_db")]
    pub max_auto_gain_db: f32,

    /// Mix transition time in ms
    #[serde(default = "default_mix_transition_ms")]
    pub mix_transition_ms: f32,
}

fn default_auto_gain_enabled() -> bool {
    true
}

fn default_gain_smoothing_ms() -> f32 {
    100.0
}

fn default_max_auto_gain_db() -> f32 {
    12.0
}

fn default_mix_transition_ms() -> f32 {
    50.0
}

impl Default for ABComparePluginParams {
    fn default() -> Self {
        Self {
            path_a: PathConfig::None,
            path_b: PathConfig::None,
            mix_mode: MixMode::Potentiometer,
            mix: 0.0,
            selected_path: 0,
            bypass: false,
            auto_gain_enabled: true,
            loudness_type: LoudnessType::Momentary,
            gain_smoothing_ms: 100.0,
            max_auto_gain_db: 12.0,
            mix_transition_ms: 50.0,
        }
    }
}
