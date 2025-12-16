//! Room EQ - Multi-channel room equalization optimizer
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

pub use autoeq::MeasurementSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Configuration Data Structures
// ============================================================================

/// Complete room configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoomConfig {
    /// Configuration version (semantic versioning, e.g., "1.0.0")
    #[serde(default = "default_config_version")]
    pub version: String,

    /// Map of channel name to speaker configuration
    pub speakers: HashMap<String, SpeakerConfig>,

    /// Optional crossover configuration for multi-driver groups
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossovers: Option<HashMap<String, CrossoverConfig>>,

    /// Optional target curve (freq, spl)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_curve: Option<TargetCurveConfig>,

    /// Optimizer configuration
    #[serde(default)]
    pub optimizer: OptimizerConfig,
}

pub fn default_config_version() -> String {
    "1.0.0".to_string()
}

/// Speaker configuration (can be single measurement or group)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SpeakerConfig {
    /// Single channel (simple case)
    Single(MeasurementSource),

    /// Group of measurements (multi-driver case)
    Group(SpeakerGroup),

    /// Multiple subwoofers optimization
    MultiSub(MultiSubGroup),

    /// Double Bass Array (DBA) optimization
    Dba(DBAConfig),
}

/// Group of measurements for a single speaker (multi-driver)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerGroup {
    /// Name of the group
    pub name: String,

    /// Measurements in this group
    pub measurements: Vec<MeasurementSource>,

    /// Crossover configuration for this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossover: Option<String>, // References crossovers map
}

/// Configuration for multiple subwoofers
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSubGroup {
    /// Name of the subwoofer group (e.g. "subs")
    pub name: String,

    /// Measurements for each subwoofer
    pub subwoofers: Vec<MeasurementSource>,
}

/// Configuration for Double Bass Array (DBA)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DBAConfig {
    /// Name of the DBA system
    pub name: String,

    /// Measurements for the front array
    pub front: Vec<MeasurementSource>,

    /// Measurements for the rear array
    pub rear: Vec<MeasurementSource>,
}

/// Crossover configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossoverConfig {
    /// Crossover type (e.g., "LR24", "LR48", "Butterworth24")
    #[serde(rename = "type")]
    pub crossover_type: String,

    /// Crossover frequency in Hz (if fixed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<f64>,

    /// Frequency range for automatic optimization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_range: Option<(f64, f64)>,
}

/// Target curve configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TargetCurveConfig {
    /// Path to CSV file (freq, spl columns)
    Path(PathBuf),

    /// Predefined target (e.g., "flat", "harman")
    Predefined(String),
}

/// FIR filter configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FirConfig {
    /// Number of taps (coefficients)
    #[serde(default = "default_fir_taps")]
    pub taps: usize,
    /// Phase response type: "linear" or "minimum"
    #[serde(default = "default_fir_phase")]
    pub phase: String,
}

/// Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OptimizerConfig {
    /// Optimization mode: "iir" (default), "fir", "mixed"
    #[serde(default = "default_opt_mode")]
    pub mode: String,

    /// FIR configuration (if mode is "fir" or "mixed")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fir: Option<FirConfig>,

    /// Loss function type ("flat" or "score")
    #[serde(default = "default_loss_type")]
    pub loss_type: String,

    /// Optimization algorithm
    #[serde(default = "default_algorithm")]
    pub algorithm: String,

    /// Number of PEQ filters per channel
    #[serde(default = "default_num_filters")]
    pub num_filters: usize,

    /// Minimum Q factor
    #[serde(default = "default_min_q")]
    pub min_q: f64,

    /// Maximum Q factor
    #[serde(default = "default_max_q")]
    pub max_q: f64,

    /// Minimum gain in dB
    #[serde(default = "default_min_db")]
    pub min_db: f64,

    /// Maximum gain in dB
    #[serde(default = "default_max_db")]
    pub max_db: f64,

    /// Minimum frequency in Hz
    #[serde(default = "default_min_freq")]
    pub min_freq: f64,

    /// Maximum frequency in Hz
    #[serde(default = "default_max_freq")]
    pub max_freq: f64,

    /// Maximum number of iterations
    #[serde(default = "default_max_iter")]
    pub max_iter: usize,

    /// PEQ model (e.g., "pk", "ls-pk-hs", "free")
    #[serde(default = "default_peq_model")]
    pub peq_model: String,
}

// Default values for OptimizerConfig
fn default_loss_type() -> String {
    "flat".to_string()
}
fn default_algorithm() -> String {
    "cobyla".to_string()
}
fn default_peq_model() -> String {
    "pk".to_string()
}
fn default_opt_mode() -> String {
    "iir".to_string()
}
fn default_fir_taps() -> usize {
    4096
}
fn default_fir_phase() -> String {
    "kirkeby".to_string()
}
fn default_num_filters() -> usize {
    10
}
fn default_min_q() -> f64 {
    0.5
}
fn default_max_q() -> f64 {
    10.0
}
fn default_min_db() -> f64 {
    -12.0
}
fn default_max_db() -> f64 {
    12.0
}
fn default_min_freq() -> f64 {
    20.0
}
fn default_max_freq() -> f64 {
    20000.0
}
fn default_max_iter() -> usize {
    10000
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            loss_type: default_loss_type(),
            algorithm: default_algorithm(),
            num_filters: default_num_filters(),
            min_q: default_min_q(),
            max_q: default_max_q(),
            min_db: default_min_db(),
            max_db: default_max_db(),
            min_freq: default_min_freq(),
            max_freq: default_max_freq(),
            max_iter: default_max_iter(),
            peq_model: default_peq_model(),
            mode: default_opt_mode(),
            fir: None,
        }
    }
}

// ============================================================================
// Output Data Structures
// ============================================================================

/// DSP chain output (AudioEngine PluginConfig format)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DspChainOutput {
    /// Output version
    #[serde(default = "default_config_version")]
    pub version: String,

    /// Per-channel DSP chains
    pub channels: HashMap<String, ChannelDspChain>,

    /// Metadata about the optimization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<OptimizationMetadata>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelDspChain {
    /// Channel name
    pub channel: String,

    /// Ordered list of plugins (AudioEngine PluginConfig format)
    pub plugins: Vec<PluginConfigWrapper>,

    /// Per-driver DSP chains for active crossover (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drivers: Option<Vec<DriverDspChain>>,
}

/// DSP chain for an individual driver in a multi-driver speaker
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DriverDspChain {
    /// Driver name (e.g., "woofer", "tweeter")
    pub name: String,

    /// Driver index in the array (0 = lowest frequency)
    pub index: usize,

    /// Ordered list of plugins for this driver (gain, crossover filters)
    pub plugins: Vec<PluginConfigWrapper>,
}

/// Wrapper for AudioEngine PluginConfig (re-exported from src-audio)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginConfigWrapper {
    pub plugin_type: String,
    pub parameters: serde_json::Value,
}

/// Optimization metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OptimizationMetadata {
    /// Pre-optimization score
    pub pre_score: f64,

    /// Post-optimization score
    pub post_score: f64,

    /// Optimization algorithm used
    pub algorithm: String,

    /// Number of iterations
    pub iterations: usize,

    /// Timestamp
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use autoeq::MeasurementRef;

    #[test]
    fn test_measurement_ref_path() {
        let path_ref = MeasurementRef::Path(PathBuf::from("test.csv"));
        assert_eq!(path_ref.path(), &PathBuf::from("test.csv"));
        assert_eq!(path_ref.name(), None);

        let named_ref = MeasurementRef::Named {
            path: PathBuf::from("named.csv"),
            name: Some("Test Measurement".to_string()),
        };
        assert_eq!(named_ref.path(), &PathBuf::from("named.csv"));
        assert_eq!(named_ref.name(), Some("Test Measurement"));
    }

    #[test]
    fn test_room_config_serialization() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementRef::Path(
                PathBuf::from("left.csv"),
            ))),
        );

        let config = RoomConfig {
            version: default_config_version(),
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
        };

        // Should serialize and deserialize
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let _deserialized: RoomConfig = serde_json::from_str(&json).expect("Failed to deserialize");
    }

    #[test]
    fn test_speaker_group_serialization() {
        let group = SpeakerGroup {
            name: "2-Way Speaker".to_string(),
            measurements: vec![
                MeasurementSource::Single(MeasurementRef::Path(PathBuf::from("woofer.csv"))),
                MeasurementSource::Single(MeasurementRef::Path(PathBuf::from("tweeter.csv"))),
            ],
            crossover: Some("default_lr24".to_string()),
        };

        let json = serde_json::to_string(&group).expect("Failed to serialize");
        let _deserialized: SpeakerGroup =
            serde_json::from_str(&json).expect("Failed to deserialize");
    }

    #[test]
    fn test_crossover_config_serialization() {
        let crossover = CrossoverConfig {
            crossover_type: "LR24".to_string(),
            frequency: Some(2500.0),
            frequency_range: None,
        };

        let json = serde_json::to_string(&crossover).expect("Failed to serialize");
        let deserialized: CrossoverConfig =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.crossover_type, "LR24");
        assert_eq!(deserialized.frequency, Some(2500.0));
    }
}
