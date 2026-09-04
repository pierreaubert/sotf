//! Configuration types and parameters for the A/B Compare plugin.

use serde::{Deserialize, Deserializer, Serialize};
use sotf_host::auto_gain::AutoGainLoudnessType;

/// Deserialize `MixMode` from the toolbar's integer index (`0` = Pot, `1` =
/// Binary) or from either UI spelling (`"Pot"`/`"Potentiometer"`, `"Binary"`).
fn deserialize_mix_mode<'de, D>(deserializer: D) -> Result<MixMode, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = MixMode;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("mix mode index (0/1) or label (\"Pot\"/\"Binary\")")
        }
        fn visit_u64<E>(self, v: u64) -> Result<MixMode, E>
        where
            E: serde::de::Error,
        {
            match v {
                0 => Ok(MixMode::Potentiometer),
                1 => Ok(MixMode::Binary),
                _ => Err(E::custom(format!("invalid mix mode index {v}"))),
            }
        }
        fn visit_i64<E>(self, v: i64) -> Result<MixMode, E>
        where
            E: serde::de::Error,
        {
            u64::try_from(v)
                .map_err(|_| E::custom(format!("invalid mix mode index {v}")))
                .and_then(|index| self.visit_u64(index))
        }
        fn visit_str<E>(self, v: &str) -> Result<MixMode, E>
        where
            E: serde::de::Error,
        {
            if v.eq_ignore_ascii_case("pot") || v.eq_ignore_ascii_case("potentiometer") {
                Ok(MixMode::Potentiometer)
            } else if v.eq_ignore_ascii_case("binary") {
                Ok(MixMode::Binary)
            } else {
                Err(E::custom(format!("unknown mix mode {v:?}")))
            }
        }
    }
    deserializer.deserialize_any(Visitor)
}

/// Deserialize the selected path from `0`/`1` or `"A"`/`"B"`.
fn deserialize_selected_path<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = i32;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("selected path 0/1 or \"A\"/\"B\"")
        }
        fn visit_u64<E>(self, v: u64) -> Result<i32, E>
        where
            E: serde::de::Error,
        {
            match v {
                0 => Ok(0),
                1 => Ok(1),
                _ => Err(E::custom(format!("invalid selected path {v}"))),
            }
        }
        fn visit_i64<E>(self, v: i64) -> Result<i32, E>
        where
            E: serde::de::Error,
        {
            i32::try_from(v)
                .ok()
                .filter(|path| *path == 0 || *path == 1)
                .ok_or_else(|| E::custom(format!("invalid selected path {v}")))
        }
        fn visit_str<E>(self, v: &str) -> Result<i32, E>
        where
            E: serde::de::Error,
        {
            if v.eq_ignore_ascii_case("a") {
                Ok(0)
            } else if v.eq_ignore_ascii_case("b") {
                Ok(1)
            } else {
                Err(E::custom(format!("unknown selected path {v:?}")))
            }
        }
    }
    deserializer.deserialize_any(Visitor)
}

/// Deserialize `LoudnessType` from the toolbar's integer index (`0` =
/// Momentary, `1` = ShortTerm) or the enum spelling.
fn deserialize_loudness_type<'de, D>(deserializer: D) -> Result<LoudnessType, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = LoudnessType;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("loudness type index (0/1) or label")
        }
        fn visit_u64<E>(self, v: u64) -> Result<LoudnessType, E>
        where
            E: serde::de::Error,
        {
            match v {
                0 => Ok(LoudnessType::Momentary),
                1 => Ok(LoudnessType::ShortTerm),
                _ => Err(E::custom(format!("invalid loudness type index {v}"))),
            }
        }
        fn visit_i64<E>(self, v: i64) -> Result<LoudnessType, E>
        where
            E: serde::de::Error,
        {
            u64::try_from(v)
                .map_err(|_| E::custom(format!("invalid loudness type index {v}")))
                .and_then(|index| self.visit_u64(index))
        }
        fn visit_str<E>(self, v: &str) -> Result<LoudnessType, E>
        where
            E: serde::de::Error,
        {
            if v.eq_ignore_ascii_case("momentary") {
                Ok(LoudnessType::Momentary)
            } else if v.eq_ignore_ascii_case("shortterm")
                || v.eq_ignore_ascii_case("short_term")
            {
                Ok(LoudnessType::ShortTerm)
            } else {
                Err(E::custom(format!("unknown loudness type {v:?}")))
            }
        }
    }
    deserializer.deserialize_any(Visitor)
}

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
    /// First destination channel written by the selected source channels.
    #[serde(default)]
    pub destination_offset: usize,
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
    #[serde(default, deserialize_with = "deserialize_mix_mode")]
    pub mix_mode: MixMode,

    /// Mix value: -1.0 = pure A, 0.0 = 50/50, +1.0 = pure B
    #[serde(default)]
    pub mix: f32,

    /// Selected path for binary mode (0 = A, 1 = B)
    #[serde(default, deserialize_with = "deserialize_selected_path")]
    pub selected_path: i32,

    /// Bypass both A and B, output original input
    #[serde(default)]
    pub bypass: bool,

    /// Enable automatic loudness matching
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Loudness measurement type for auto-gain
    #[serde(default, deserialize_with = "deserialize_loudness_type")]
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

    /// Invert phase of path A output (multiply by -1.0)
    #[serde(default)]
    pub phase_invert_a: bool,

    /// Invert phase of path B output (multiply by -1.0)
    #[serde(default)]
    pub phase_invert_b: bool,

    /// Difference mode: output A - B instead of crossfade
    #[serde(default)]
    pub difference_mode: bool,

    /// Band mask low frequency in Hz (highpass cutoff for comparison output)
    #[serde(default = "default_band_mask_low_hz")]
    pub band_mask_low_hz: f32,

    /// Band mask high frequency in Hz (lowpass cutoff for comparison output)
    #[serde(default = "default_band_mask_high_hz")]
    pub band_mask_high_hz: f32,
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

fn default_band_mask_low_hz() -> f32 {
    20.0
}

fn default_band_mask_high_hz() -> f32 {
    20000.0
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
            phase_invert_a: false,
            phase_invert_b: false,
            difference_mode: false,
            band_mask_low_hz: 20.0,
            band_mask_high_hz: 20000.0,
        }
    }
}
