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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Configuration Data Structures
// ============================================================================

/// Complete room configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConfig {
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

/// Speaker configuration (can be single measurement or group)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SpeakerConfig {
    /// Single measurement (simple case)
    Single(MeasurementRef),

    /// Group of measurements (multi-driver case)
    Group(SpeakerGroup),
}

/// Group of measurements for a single speaker (multi-driver)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerGroup {
    /// Name of the group
    pub name: String,

    /// Measurements in this group
    pub measurements: Vec<MeasurementRef>,

    /// Crossover configuration for this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossover: Option<String>, // References crossovers map
}

/// Reference to a measurement file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MeasurementRef {
    /// Path to CSV file (freq, spl, phase columns)
    Path(PathBuf),

    /// Named measurement with optional metadata
    Named {
        path: PathBuf,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

impl MeasurementRef {
    pub fn path(&self) -> &PathBuf {
        match self {
            MeasurementRef::Path(p) => p,
            MeasurementRef::Named { path, .. } => path,
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> Option<&str> {
        match self {
            MeasurementRef::Path(_) => None,
            MeasurementRef::Named { name, .. } => name.as_deref(),
        }
    }
}

/// Crossover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TargetCurveConfig {
    /// Path to CSV file (freq, spl columns)
    Path(PathBuf),

    /// Predefined target (e.g., "flat", "harman")
    Predefined(String),
}

/// Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
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
}

// Default values for OptimizerConfig
fn default_loss_type() -> String { "flat".to_string() }
fn default_algorithm() -> String { "cobyla".to_string() }
fn default_num_filters() -> usize { 10 }
fn default_min_q() -> f64 { 0.5 }
fn default_max_q() -> f64 { 10.0 }
fn default_min_db() -> f64 { -12.0 }
fn default_max_db() -> f64 { 12.0 }
fn default_min_freq() -> f64 { 20.0 }
fn default_max_freq() -> f64 { 20000.0 }
fn default_max_iter() -> usize { 10000 }

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
        }
    }
}

// ============================================================================
// Output Data Structures
// ============================================================================

/// DSP chain output (AudioEngine PluginConfig format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainOutput {
    /// Per-channel DSP chains
    pub channels: HashMap<String, ChannelDspChain>,

    /// Metadata about the optimization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<OptimizationMetadata>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDspChain {
    /// Channel name
    pub channel: String,

    /// Ordered list of plugins (AudioEngine PluginConfig format)
    pub plugins: Vec<PluginConfigWrapper>,
}

/// Wrapper for AudioEngine PluginConfig (re-exported from src-audio)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigWrapper {
    pub plugin_type: String,
    pub parameters: serde_json::Value,
}

/// Optimization metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            SpeakerConfig::Single(MeasurementRef::Path(PathBuf::from("left.csv"))),
        );

        let config = RoomConfig {
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
        };

        // Should serialize and deserialize
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let _deserialized: RoomConfig =
            serde_json::from_str(&json).expect("Failed to deserialize");
    }

    #[test]
    fn test_speaker_group_serialization() {
        let group = SpeakerGroup {
            name: "2-Way Speaker".to_string(),
            measurements: vec![
                MeasurementRef::Path(PathBuf::from("woofer.csv")),
                MeasurementRef::Path(PathBuf::from("tweeter.csv")),
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
