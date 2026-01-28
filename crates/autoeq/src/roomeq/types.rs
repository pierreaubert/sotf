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

pub use crate::MeasurementSource;
use crate::Curve;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Output Data Structures
// ============================================================================

/// Frequency response curve data for serialization
///
/// Represents a curve with frequency points and SPL values.
/// SPL values are normalized (mean-subtracted in the 1000-2000 Hz range)
/// for consistent comparison across measurements.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CurveData {
    /// Frequency points in Hz
    pub freq: Vec<f64>,
    /// Sound Pressure Level in dB (normalized)
    pub spl: Vec<f64>,
}

impl From<Curve> for CurveData {
    fn from(curve: Curve) -> Self {
        CurveData {
            freq: curve.freq.to_vec(),
            spl: curve.spl.to_vec(),
        }
    }
}

impl From<&Curve> for CurveData {
    fn from(curve: &Curve) -> Self {
        CurveData {
            freq: curve.freq.to_vec(),
            spl: curve.spl.to_vec(),
        }
    }
}

impl From<CurveData> for Curve {
    fn from(data: CurveData) -> Self {
        Curve {
            freq: ndarray::Array1::from(data.freq),
            spl: ndarray::Array1::from(data.spl),
            phase: None,
        }
    }
}

// ============================================================================
// Configuration Data Structures
// ============================================================================

/// Recording configuration stored with measurements
/// Contains device settings and signal parameters used during measurement capture
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecordingConfiguration {
    /// Playback device name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_device_name: Option<String>,
    /// Playback device ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_device_id: Option<String>,
    /// Playback sample rate in Hz
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_sample_rate: Option<u32>,
    /// Playback channel count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_channels: Option<usize>,
    /// Speaker configuration (e.g., "5.1", "7.1.4", "Stereo")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_configuration: Option<String>,
    /// Channel names in order (e.g., ["L", "R", "C", "LFE", "SL", "SR"])
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_names: Option<Vec<String>>,

    /// Recording device name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_device_name: Option<String>,
    /// Recording device ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_device_id: Option<String>,
    /// Recording sample rate in Hz
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_sample_rate: Option<u32>,
    /// Recording channel count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_channels: Option<usize>,

    /// Microphone calibration file path (if used)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_calibration_path: Option<String>,
    /// Recording output directory
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_directory: Option<String>,

    /// Signal type used for measurements (e.g., "Sweep", "Pink Noise")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_type: Option<String>,
    /// Signal duration in seconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_duration_secs: Option<f32>,
    /// Signal level in dB
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_level_db: Option<f32>,

    /// Sweep start frequency in Hz (only applicable when signal_type is "Sweep")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_start_freq: Option<f32>,
    /// Sweep end frequency in Hz (only applicable when signal_type is "Sweep")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_end_freq: Option<f32>,
}

impl Default for RecordingConfiguration {
    fn default() -> Self {
        Self {
            playback_device_name: None,
            playback_device_id: None,
            playback_sample_rate: None,
            playback_channels: None,
            speaker_configuration: None,
            channel_names: None,
            recording_device_name: None,
            recording_device_id: None,
            recording_sample_rate: None,
            recording_channels: None,
            mic_calibration_path: None,
            recording_directory: None,
            signal_type: None,
            signal_duration_secs: None,
            signal_level_db: None,
            sweep_start_freq: None,
            sweep_end_freq: None,
        }
    }
}

/// Complete room configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoomConfig {
    /// Configuration version (semantic versioning, e.g., "1.0.0")
    #[serde(default = "default_config_version")]
    pub version: String,

    /// Map of channel name to speaker configuration
    pub speakers: HashMap<String, SpeakerConfig>,

    /// Optional crossover configuration for multi-driver groups
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossovers: Option<HashMap<String, CrossoverConfig>>,

    /// Optional target curve (freq, spl)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_curve: Option<TargetCurveConfig>,

    /// Optional group delay optimization configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_delay: Option<Vec<GroupDelayConfig>>,

    /// Optimizer configuration
    #[serde(default)]
    pub optimizer: OptimizerConfig,

    /// Recording configuration (device settings, signal parameters used during capture)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_config: Option<RecordingConfiguration>,
}

impl RoomConfig {
    /// Resolve relative paths in this room configuration against a base directory.
    /// This is useful when loading a config file and need to resolve relative paths
    /// (like csv_path in InlineMeasurement) relative to the config file's directory.
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for speaker in self.speakers.values_mut() {
            speaker.resolve_paths(base_dir);
        }
        // Also resolve target_curve path if it's a file path
        if let Some(TargetCurveConfig::Path(ref mut path)) = self.target_curve {
            if path.is_relative() {
                *path = base_dir.join(&*path);
            }
        }
    }
}

/// Default configuration version
pub fn default_config_version() -> String {
    "1.1.0".to_string()
}

/// Group delay optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupDelayConfig {
    /// Subwoofer channel name
    pub subwoofer: String,

    /// List of speaker channel names to align with this subwoofer
    pub speakers: Vec<String>,

    /// Minimum frequency for optimization (Hz)
    #[serde(default = "default_group_delay_min_freq")]
    pub min_freq: f64,

    /// Maximum frequency for optimization (Hz)
    #[serde(default = "default_group_delay_max_freq")]
    pub max_freq: f64,
}

fn default_group_delay_min_freq() -> f64 {
    30.0
}
fn default_group_delay_max_freq() -> f64 {
    120.0
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

impl SpeakerConfig {
    /// Resolve relative paths in this speaker configuration against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        match self {
            SpeakerConfig::Single(source) => source.resolve_paths(base_dir),
            SpeakerConfig::Group(group) => group.resolve_paths(base_dir),
            SpeakerConfig::MultiSub(group) => group.resolve_paths(base_dir),
            SpeakerConfig::Dba(config) => config.resolve_paths(base_dir),
        }
    }
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

impl SpeakerGroup {
    /// Resolve relative paths in this speaker group against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for m in &mut self.measurements {
            m.resolve_paths(base_dir);
        }
    }
}

/// Configuration for multiple subwoofers
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSubGroup {
    /// Name of the subwoofer group (e.g. "subs")
    pub name: String,

    /// Measurements for each subwoofer
    pub subwoofers: Vec<MeasurementSource>,
}

impl MultiSubGroup {
    /// Resolve relative paths in this multi-sub group against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for m in &mut self.subwoofers {
            m.resolve_paths(base_dir);
        }
    }
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

impl DBAConfig {
    /// Resolve relative paths in this DBA config against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for m in &mut self.front {
            m.resolve_paths(base_dir);
        }
        for m in &mut self.rear {
            m.resolve_paths(base_dir);
        }
    }
}

/// Crossover configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossoverConfig {
    /// Crossover type (e.g., "LR24", "LR48", "Butterworth24")
    #[serde(rename = "type")]
    pub crossover_type: String,

    /// Crossover frequency in Hz (for 2-way speakers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<f64>,

    /// Crossover frequencies in Hz (for 3-way and above)
    /// e.g., [500, 3000] for woofer/mid at 500Hz, mid/tweeter at 3000Hz
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequencies: Option<Vec<f64>>,

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

/// Configuration for frequency-based mixed mode crossover
///
/// When specified with mode="mixed", the optimizer will use different filter types
/// for different frequency bands separated by a crossover frequency.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixedModeConfig {
    /// Crossover frequency dividing IIR and FIR bands (Hz)
    #[serde(default = "default_crossover_freq")]
    pub crossover_freq: f64,

    /// Crossover filter type: "LR24", "LR48"
    #[serde(default = "default_crossover_type")]
    pub crossover_type: String,

    /// Which band uses FIR: "low" or "high" (default: "low")
    /// FIR is typically better for low frequencies (bass room modes)
    #[serde(default = "default_fir_band")]
    pub fir_band: String,
}

fn default_crossover_freq() -> f64 {
    300.0
}
fn default_crossover_type() -> String {
    "LR24".to_string()
}
fn default_fir_band() -> String {
    "low".to_string()
}

impl Default for MixedModeConfig {
    fn default() -> Self {
        Self {
            crossover_freq: default_crossover_freq(),
            crossover_type: default_crossover_type(),
            fir_band: default_fir_band(),
        }
    }
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

    /// Mixed mode configuration (frequency-based crossover)
    /// When mode == "mixed" and this is Some, uses frequency-based crossover
    /// (FIR on one band, IIR on the other). When None, uses legacy sequential mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixed_config: Option<MixedModeConfig>,

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

    /// Population size for DE optimizer
    #[serde(default = "default_population")]
    pub population: usize,

    /// PEQ model (e.g., "pk", "ls-pk-hs", "free")
    #[serde(default = "default_peq_model")]
    pub peq_model: String,

    /// Random seed for reproducible results (None for random)
    /// When set, the optimizer will produce deterministic results
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

// Default values for OptimizerConfig
fn default_loss_type() -> String {
    "flat".to_string()
}
fn default_algorithm() -> String {
    "autoeq:de".to_string()
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
    7
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
    1200.0
}
fn default_max_iter() -> usize {
    10000
}
fn default_population() -> usize {
    300
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
            population: default_population(),
            peq_model: default_peq_model(),
            mode: default_opt_mode(),
            fir: None,
            mixed_config: None,
            seed: None,
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

    /// Initial frequency response curve before optimization (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_curve: Option<CurveData>,

    /// Final frequency response curve after applying correction (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_curve: Option<CurveData>,
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
    use crate::MeasurementRef;

    #[test]
    fn test_measurement_ref_path() {
        let path_ref = MeasurementRef::Path(PathBuf::from("test.csv"));
        assert_eq!(path_ref.path(), Some(&PathBuf::from("test.csv")));
        assert_eq!(path_ref.name(), None);

        let named_ref = MeasurementRef::Named {
            path: PathBuf::from("named.csv"),
            name: Some("Test Measurement".to_string()),
        };
        assert_eq!(named_ref.path(), Some(&PathBuf::from("named.csv")));
        assert_eq!(named_ref.name(), Some("Test Measurement"));

        // Test inline measurement
        let inline_ref = MeasurementRef::Inline(crate::InlineMeasurement {
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![-5.0, 0.0, -3.0],
            phase_deg: Some(vec![0.0, 45.0, 90.0]),
            name: Some("Inline Test".to_string()),
            wav_path: None,
            csv_path: None,
        });
        assert_eq!(inline_ref.path(), None);
        assert_eq!(inline_ref.name(), Some("Inline Test"));
        assert!(inline_ref.is_inline());
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
            group_delay: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
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
            frequencies: None,
            frequency_range: None,
        };

        let json = serde_json::to_string(&crossover).expect("Failed to serialize");
        let deserialized: CrossoverConfig =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.crossover_type, "LR24");
        assert_eq!(deserialized.frequency, Some(2500.0));
    }
}
