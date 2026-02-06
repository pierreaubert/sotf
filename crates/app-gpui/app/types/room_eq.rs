// ============================================================================
// Room EQ Screen Types
// ============================================================================

use serde::{Deserialize, Serialize};

use super::recording::{RecordingResult, RecordingState};
use autoeq::roomeq::{
    CrossoverConfig as BackendCrossoverConfig, ExcursionProtectionConfig as BackendExcursionProtectionConfig,
    FirConfig as BackendFirConfig, HighFreqFilterConfig, HighpassType, LowFreqFilterConfig,
    MeasurementSource, MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
    OptimizerConfig as BackendOptimizerConfig, PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
    RoomConfig, SchroederSplitConfig as BackendSchroederSplitConfig, SpeakerConfig,
    SpeakerGroup, TargetTiltConfig as BackendTargetTiltConfig, TiltType,
};
use std::collections::HashMap;

/// Wrapper for InteractiveChartState that implements Debug
#[derive(Clone)]
pub struct InteractiveChartStateWrapper(pub gpui_px::interaction::InteractiveChartState);

impl std::fmt::Debug for InteractiveChartStateWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractiveChartStateWrapper")
            .field("is_zoomed", &self.0.is_zoomed())
            .finish()
    }
}

impl InteractiveChartStateWrapper {
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self(gpui_px::interaction::InteractiveChartState::new(
            x_min, x_max, y_min, y_max,
        ))
    }

    pub fn with_log_x(mut self, is_log: bool) -> Self {
        self.0 = self.0.with_log_x(is_log);
        self
    }

    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.0 = self.0.with_size(width, height);
        self
    }

    pub fn inner(&self) -> &gpui_px::interaction::InteractiveChartState {
        &self.0
    }
}

/// Room EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomEqStep {
    /// Step 1: Load/import measurement data
    #[default]
    LoadData,
    /// Step 2: Select optimization mode (IIR/FIR)
    SelectMode,
    /// Step 3: Configure channels and optimizer settings
    Configure,
    /// Step 4: Run optimization (per-channel, then combined)
    Optimize,
    /// Step 5: Review results and visualizations
    Review,
    /// Step 6: Export DSP chain and apply
    Export,
}

impl RoomEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [RoomEqStep] {
        &[
            RoomEqStep::LoadData,
            RoomEqStep::SelectMode,
            RoomEqStep::Configure,
            RoomEqStep::Optimize,
            RoomEqStep::Review,
            RoomEqStep::Export,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            RoomEqStep::LoadData => 0,
            RoomEqStep::SelectMode => 1,
            RoomEqStep::Configure => 2,
            RoomEqStep::Optimize => 3,
            RoomEqStep::Review => 4,
            RoomEqStep::Export => 5,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            RoomEqStep::LoadData => "Load Data",
            RoomEqStep::SelectMode => "Mode",
            RoomEqStep::Configure => "Configure",
            RoomEqStep::Optimize => "Optimize",
            RoomEqStep::Review => "Review",
            RoomEqStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => Some(RoomEqStep::SelectMode),
            RoomEqStep::SelectMode => Some(RoomEqStep::Configure),
            RoomEqStep::Configure => Some(RoomEqStep::Optimize),
            RoomEqStep::Optimize => Some(RoomEqStep::Review),
            RoomEqStep::Review => Some(RoomEqStep::Export),
            RoomEqStep::Export => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => None,
            RoomEqStep::SelectMode => Some(RoomEqStep::LoadData),
            RoomEqStep::Configure => Some(RoomEqStep::SelectMode),
            RoomEqStep::Optimize => Some(RoomEqStep::Configure),
            RoomEqStep::Review => Some(RoomEqStep::Optimize),
            RoomEqStep::Export => Some(RoomEqStep::Review),
        }
    }
}

/// Source of measurement data for Room EQ
#[derive(Debug, Clone, PartialEq)]
pub enum RoomEqDataSource {
    /// Use recordings from current session (RecordingState)
    FromRecording,
    /// Loaded from a JSON file
    FromFile(std::path::PathBuf),
}

impl Default for RoomEqDataSource {
    fn default() -> Self {
        RoomEqDataSource::FromRecording
    }
}

/// Recording configuration stored with measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfiguration {
    /// Playback device name
    pub playback_device_name: String,
    /// Playback device ID
    pub playback_device_id: String,
    /// Playback sample rate
    pub playback_sample_rate: u32,
    /// Playback channel count
    pub playback_channels: usize,
    /// Speaker configuration (e.g., "5.1", "7.1.4")
    pub speaker_configuration: String,
    /// Channel names in order
    pub channel_names: Vec<String>,

    /// Recording device name
    pub recording_device_name: String,
    /// Recording device ID
    pub recording_device_id: String,
    /// Recording sample rate
    pub recording_sample_rate: u32,
    /// Recording channel count
    pub recording_channels: usize,

    /// Microphone calibration file path (if used)
    pub mic_calibration_path: Option<String>,
    /// Recording output directory
    pub recording_directory: Option<String>,

    /// Signal type used for measurements
    pub signal_type: String,
    /// Signal duration in seconds
    pub signal_duration_secs: f32,
    /// Signal level in dB
    pub signal_level_db: f32,

    /// Sweep start frequency in Hz (only applicable when signal_type is "Sweep")
    #[serde(default)]
    pub sweep_start_freq: Option<f32>,
    /// Sweep end frequency in Hz (only applicable when signal_type is "Sweep")
    #[serde(default)]
    pub sweep_end_freq: Option<f32>,
}

/// File format for saving/loading room EQ measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqMeasurementsFile {
    /// File format version
    pub version: u32,
    /// Channel measurements
    pub channels: Vec<ChannelMeasurement>,
    /// Recording configuration (devices, settings used)
    #[serde(default)]
    pub configuration: Option<RecordingConfiguration>,
}

impl RoomEqMeasurementsFile {
    pub const CURRENT_VERSION: u32 = 2;

    pub fn new(channels: Vec<ChannelMeasurement>) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            channels,
            configuration: None,
        }
    }

    pub fn with_configuration(
        channels: Vec<ChannelMeasurement>,
        configuration: RecordingConfiguration,
    ) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            channels,
            configuration: Some(configuration),
        }
    }

    /// Deserialize from JSON string with automatic version migration
    pub fn from_json_str(json: &str) -> Result<Self, serde_json::Error> {
        let mut value: serde_json::Value = serde_json::from_str(json)?;

        // Check version (default to 1 if missing)
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

        if version < Self::CURRENT_VERSION {
            log::info!(
                "Migrating recordings.json from version {} to {}",
                version,
                Self::CURRENT_VERSION
            );
            value = Self::convert_v1_to_v2(value);
        }

        serde_json::from_value(value)
    }

    /// Convert V1 (no version field) to V2
    fn convert_v1_to_v2(mut value: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = value.as_object_mut() {
            // Add version field
            obj.insert("version".to_string(), serde_json::json!(2));

            // Ensure channels field exists (if it was flat array, this would be different,
            // but assuming V1 was RoomEqMeasurementsFile struct without version)
            if !obj.contains_key("channels") {
                // If it looks like the root was just the fields of ChannelMeasurement? No.
                // Assuming standard struct serialization.
                // If legacy format was different, handle it here.
                // For now, we assume V1 was just missing 'version'.
            }
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::recording::RecordingResult;

    #[test]
    fn test_migration_v1_to_v2() {
        // V1 JSON (missing version)
        let v1_json = r#"{
            "channels": [
                {
                    "channel_name": "L",
                    "measurement": {
                        "channel": 0,
                        "wav_path": "test.wav",
                        "frequencies": [],
                        "magnitude_db": [],
                        "phase_deg": []
                    },
                    "is_group": false,
                    "group_drivers": []
                }
            ],
            "configuration": null
        }"#;

        let result = RoomEqMeasurementsFile::from_json_str(v1_json).expect("Migration failed");

        assert_eq!(result.version, 2);
        assert_eq!(result.channels.len(), 1);
        assert_eq!(result.channels[0].channel_name, "L");
    }

    #[test]
    fn test_load_v2() {
        // V2 JSON (with version)
        let v2_json = r#"{
            "version": 2,
            "channels": [],
            "configuration": null
        }"#;

        let result = RoomEqMeasurementsFile::from_json_str(v2_json).expect("Loading V2 failed");
        assert_eq!(result.version, 2);
    }
}

/// Measurement data for a single channel (may have multiple drivers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeasurement {
    /// Channel name (e.g., "L", "R", "C")
    pub channel_name: String,
    /// Primary measurement (single driver or combined)
    pub measurement: RecordingResult,
    /// Whether this is a multi-driver setup
    pub is_group: bool,
    /// Individual driver measurements (for multi-driver)
    pub group_drivers: Vec<RecordingResult>,
}

/// Speaker configuration type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpeakerConfigType {
    /// Single full-range driver or measurement
    #[default]
    Single,
    /// Multi-driver with active crossover
    MultiDriver,
}

/// Crossover type for multi-driver speakers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CrossoverType {
    /// Linkwitz-Riley 2nd order (12dB/octave)
    LR12,
    /// Linkwitz-Riley 4th order (24dB/octave)
    #[default]
    LR24,
    /// Linkwitz-Riley 8th order (48dB/octave)
    LR48,
    /// Butterworth 2nd order (12dB/octave)
    Butterworth12,
    /// Butterworth 4th order (24dB/octave)
    Butterworth24,
}

impl CrossoverType {
    pub fn all() -> &'static [CrossoverType] {
        &[
            CrossoverType::LR12,
            CrossoverType::LR24,
            CrossoverType::LR48,
            CrossoverType::Butterworth12,
            CrossoverType::Butterworth24,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CrossoverType::LR12 => "Linkwitz-Riley 12dB",
            CrossoverType::LR24 => "Linkwitz-Riley 24dB",
            CrossoverType::LR48 => "Linkwitz-Riley 48dB",
            CrossoverType::Butterworth12 => "Butterworth 12dB",
            CrossoverType::Butterworth24 => "Butterworth 24dB",
        }
    }
}

/// Configuration for a speaker channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqSpeakerConfig {
    /// Channel name
    pub channel_name: String,
    /// Single or multi-driver
    pub config_type: SpeakerConfigType,
    /// Crossover type (for multi-driver)
    pub crossover_type: CrossoverType,
    /// Driver names (for multi-driver), e.g., ["woofer", "tweeter"]
    pub driver_names: Vec<String>,
    /// Initial crossover frequency hints (for multi-driver)
    pub crossover_freq_hints: Vec<f64>,
}

impl Default for RoomEqSpeakerConfig {
    fn default() -> Self {
        Self {
            channel_name: String::new(),
            config_type: SpeakerConfigType::Single,
            crossover_type: CrossoverType::LR24,
            driver_names: Vec::new(),
            crossover_freq_hints: Vec::new(),
        }
    }
}

/// Multi-speaker optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MultiSpeakerMode {
    /// Optimize each speaker sequentially (legacy mode)
    #[default]
    Sequential,
    /// Optimize all speakers together in a single optimizer call
    Combined,
}

impl MultiSpeakerMode {
    pub fn all() -> &'static [MultiSpeakerMode] {
        &[MultiSpeakerMode::Sequential, MultiSpeakerMode::Combined]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MultiSpeakerMode::Sequential => "Sequential (per-channel)",
            MultiSpeakerMode::Combined => "Combined (all channels)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            MultiSpeakerMode::Sequential => "Optimize each speaker independently, one at a time",
            MultiSpeakerMode::Combined => {
                "Optimize all speakers together for globally optimal solution"
            }
        }
    }
}

/// Optimization algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqAlgorithm {
    /// COBYLA (Constrained Optimization BY Linear Approximations)
    #[default]
    Cobyla,
    /// Differential Evolution
    DifferentialEvolution,
    /// Nelder-Mead simplex
    NelderMead,
}

impl RoomEqAlgorithm {
    pub fn all() -> &'static [RoomEqAlgorithm] {
        &[
            RoomEqAlgorithm::Cobyla,
            RoomEqAlgorithm::DifferentialEvolution,
            RoomEqAlgorithm::NelderMead,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "COBYLA",
            RoomEqAlgorithm::DifferentialEvolution => "Differential Evolution",
            RoomEqAlgorithm::NelderMead => "Nelder-Mead",
        }
    }

    pub fn to_autoeq_string(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "cobyla",
            RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
            RoomEqAlgorithm::NelderMead => "nelder-mead",
        }
    }
}

// === Type conversions for room_eq library ===

impl From<SpeakerConfigType> for sotf_audio_player::room_eq::SpeakerConfigType {
    fn from(val: SpeakerConfigType) -> Self {
        match val {
            SpeakerConfigType::Single => sotf_audio_player::room_eq::SpeakerConfigType::Single,
            SpeakerConfigType::MultiDriver => {
                sotf_audio_player::room_eq::SpeakerConfigType::MultiDriver
            }
        }
    }
}

impl From<CrossoverType> for sotf_audio_player::room_eq::CrossoverType {
    fn from(val: CrossoverType) -> Self {
        match val {
            CrossoverType::LR12 => sotf_audio_player::room_eq::CrossoverType::LR12,
            CrossoverType::LR24 => sotf_audio_player::room_eq::CrossoverType::LR24,
            CrossoverType::LR48 => sotf_audio_player::room_eq::CrossoverType::LR48,
            CrossoverType::Butterworth12 => {
                sotf_audio_player::room_eq::CrossoverType::Butterworth12
            }
            CrossoverType::Butterworth24 => {
                // Map to closest available - Butterworth12 (LR24 is closer behavior)
                sotf_audio_player::room_eq::CrossoverType::LR24
            }
        }
    }
}

// Algorithm conversion removed - Algorithm type no longer exported from library

/// Optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqOptimizationMode {
    /// IIR (Parametric EQ)
    #[default]
    Iir,
    /// FIR (Convolution)
    Fir,
    /// Mixed (IIR + FIR)
    Mixed,
}

impl RoomEqOptimizationMode {
    pub fn all() -> &'static [RoomEqOptimizationMode] {
        &[
            RoomEqOptimizationMode::Iir,
            RoomEqOptimizationMode::Fir,
            RoomEqOptimizationMode::Mixed,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "IIR (Parametric EQ)",
            RoomEqOptimizationMode::Fir => "FIR (Convolution)",
            RoomEqOptimizationMode::Mixed => "Mixed (IIR + FIR)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "Uses standard biquad filters. Low latency, efficient.",
            RoomEqOptimizationMode::Fir => {
                "Uses impulse response convolution. Can correct phase, but higher latency."
            }
            RoomEqOptimizationMode::Mixed => {
                "Combines IIR for high frequencies and FIR for low frequencies."
            }
        }
    }

    pub fn to_code(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "iir",
            RoomEqOptimizationMode::Fir => "fir",
            RoomEqOptimizationMode::Mixed => "mixed",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "fir" => RoomEqOptimizationMode::Fir,
            "mixed" => RoomEqOptimizationMode::Mixed,
            _ => RoomEqOptimizationMode::Iir,
        }
    }
}

/// FIR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqFirConfig {
    /// Number of taps
    pub taps: usize,
    /// Phase type ("linear" or "kirkeby")
    pub phase: String,
}

impl Default for RoomEqFirConfig {
    fn default() -> Self {
        Self {
            taps: 4096,
            phase: "kirkeby".to_string(),
        }
    }
}

/// Target curve tilt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetTiltConfig {
    pub enabled: bool,
    pub tilt_type: String, // "flat", "harman", "custom"
    pub slope: f64,
    pub reference_freq: f64,
    pub bass_shelf_db: f64,
    pub bass_shelf_freq: f64,
}

impl Default for TargetTiltConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tilt_type: "flat".to_string(),
            slope: -0.8,
            reference_freq: 1000.0,
            bass_shelf_db: 0.0,
            bass_shelf_freq: 200.0,
        }
    }
}

/// Excursion protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcursionProtectionConfig {
    pub enabled: bool,
    pub auto_detect_f3: bool,
    pub manual_f3_hz: f64,
    pub filter_order: usize,
    pub filter_type: String, // "lr", "bw"
    pub margin_octaves: f64,
}

impl Default for ExcursionProtectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_detect_f3: true,
            manual_f3_hz: 40.0,
            filter_order: 4,
            filter_type: "lr".to_string(),
            margin_octaves: 0.25,
        }
    }
}

/// Schroeder split configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchroederSplitConfig {
    pub enabled: bool,
    pub schroeder_freq: f64,
    pub low_freq_max_q: f64,
    pub low_freq_allow_boost: bool,
    pub high_freq_max_q: f64,
    pub high_freq_shelving_only: bool,
}

impl Default for SchroederSplitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schroeder_freq: 300.0,
            low_freq_max_q: 10.0,
            low_freq_allow_boost: false,
            high_freq_max_q: 1.0,
            high_freq_shelving_only: false,
        }
    }
}

/// Phase alignment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseAlignmentConfig {
    pub enabled: bool,
    pub min_freq: f64,
    pub max_freq: f64,
    pub optimize_polarity: bool,
    pub max_delay_ms: f64,
}

impl Default for PhaseAlignmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_freq: 60.0,
            max_freq: 100.0,
            optimize_polarity: true,
            max_delay_ms: 30.0,
        }
    }
}

/// Multi-seat configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSeatConfig {
    pub enabled: bool,
    pub strategy: String, // "variance", "primary", "average"
    pub primary_seat: usize,
    pub max_deviation_db: f64,
}

impl Default for MultiSeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "variance".to_string(),
            primary_seat: 0,
            max_deviation_db: 6.0,
        }
    }
}

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    /// Optimization mode
    #[serde(default)]
    pub mode: RoomEqOptimizationMode,
    /// FIR configuration
    #[serde(default)]
    pub fir: RoomEqFirConfig,
    /// Multi-speaker optimization mode
    pub multi_speaker_mode: MultiSpeakerMode,
    /// Optimization algorithm
    pub algorithm: RoomEqAlgorithm,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of PEQ filters per channel
    pub num_filters: usize,
    /// Minimum Q factor
    pub min_q: f64,
    /// Maximum Q factor
    pub max_q: f64,
    /// Minimum gain in dB
    pub min_db: f64,
    /// Maximum gain in dB
    pub max_db: f64,
    /// Minimum frequency in Hz
    pub min_freq: f64,
    /// Maximum frequency in Hz
    pub max_freq: f64,
    /// Maximum number of iterations
    pub max_iter: usize,
    /// PEQ model (e.g., "pk", "ls-pk-hs")
    pub peq_model: String,
    /// Population size for evolutionary algorithms
    pub population: usize,
    /// Mutation factor (F) for DE
    pub de_f: f64,
    /// Crossover rate (CR) for DE
    pub de_cr: f64,
    /// DE strategy (e.g., "currenttobest1bin")
    pub strategy: String,
    /// Enable local refinement after global optimization
    pub refine: bool,
    /// Local algorithm for refinement
    pub local_algo: String,
    /// Enable smoothing
    pub smooth: bool,
    /// Smoothing window size (1-24)
    pub smooth_n: usize,
    /// Spacing constraint weight (0-1000)
    pub spacing_weight: f64,
    /// Minimum spacing between filters in octaves (0.01-1.0)
    pub min_spacing_oct: f64,
    /// Relative tolerance for convergence
    pub tolerance: f64,
    /// Absolute tolerance for convergence
    pub atolerance: f64,
    /// Loss function type (e.g., "flat", "score")
    pub loss_type: String,
    /// Enable psychoacoustic smoothing
    pub psychoacoustic: bool,
    /// Enable asymmetric loss (penalize peaks more than dips)
    pub asymmetric_loss: bool,
    /// Target curve (e.g., "flat", "harman")
    pub target_curve: String,
    /// System type (e.g., "stereo", "multichannel")
    pub system_type: String,

    // --- Advanced Room Correction (Scenario B) ---
    #[serde(default)]
    pub target_tilt: TargetTiltConfig,
    #[serde(default)]
    pub excursion_protection: ExcursionProtectionConfig,
    #[serde(default)]
    pub schroeder_split: SchroederSplitConfig,

    // --- Advanced System Optimization (Scenario A) ---
    #[serde(default)]
    pub phase_alignment: PhaseAlignmentConfig,
    #[serde(default)]
    pub multi_seat: MultiSeatConfig,
}

impl Default for RoomEqOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: RoomEqOptimizationMode::default(),
            fir: RoomEqFirConfig::default(),
            multi_speaker_mode: MultiSpeakerMode::Combined,
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            sample_rate: 48000,
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 3.0,
            min_freq: 20.0,
            max_freq: 16000.0,
            max_iter: 10000,
            peq_model: "pk".to_string(),
            population: 40,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 6,
            spacing_weight: 1.0,
            min_spacing_oct: 0.08,
            tolerance: 0.00001,
            atolerance: 0.00001,
            loss_type: "flat".to_string(),
            psychoacoustic: true,
            asymmetric_loss: true,
            target_curve: "flat".to_string(),
            system_type: "stereo".to_string(),
            target_tilt: TargetTiltConfig::default(),
            excursion_protection: ExcursionProtectionConfig::default(),
            schroeder_split: SchroederSplitConfig::default(),
            phase_alignment: PhaseAlignmentConfig::default(),
            multi_seat: MultiSeatConfig::default(),
        }
    }
}

/// Optimization status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationStatus {
    /// Not started
    #[default]
    Idle,
    /// Currently running
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user
    Cancelled,
}

/// EQ filter configuration (for display and export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqFilterConfig {
    /// Filter type (peak, lowshelf, highshelf)
    pub filter_type: String,
    /// Center frequency in Hz
    pub frequency: f64,
    /// Q factor
    pub q: f64,
    /// Gain in dB
    pub gain_db: f64,
}

/// Optimization result for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOptResult {
    /// Channel name
    pub channel_name: String,
    /// Pre-optimization score (RMS error)
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Optimized EQ filters
    pub eq_filters: Vec<EqFilterConfig>,
    /// Optimized crossover frequencies (for multi-driver)
    pub crossover_freqs: Option<Vec<f64>>,
    /// Optimized driver gains in dB (for multi-driver)
    pub driver_gains: Option<Vec<f64>>,
    /// Original frequency response
    pub original_response: Option<Vec<(f64, f64)>>,
    /// Corrected frequency response
    pub corrected_response: Option<Vec<(f64, f64)>>,
    /// Normalized frequency response
    pub normalized_response: Option<Vec<(f64, f64)>>,
}

/// DSP chain output format (matches roomeq output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainOutput {
    /// Per-channel DSP chains
    pub channels: std::collections::HashMap<String, ChannelDspChain>,
    /// Optimization metadata
    pub metadata: Option<DspChainMetadata>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDspChain {
    /// Channel name
    pub channel: String,
    /// Ordered list of plugins
    pub plugins: Vec<DspPluginConfig>,
    /// Per-driver chains (for multi-driver)
    pub drivers: Option<Vec<DriverDspChain>>,
}

/// DSP chain for a driver in multi-driver setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDspChain {
    /// Driver name
    pub name: String,
    /// Driver index (0 = lowest frequency)
    pub index: usize,
    /// Plugins for this driver
    pub plugins: Vec<DspPluginConfig>,
}

/// DSP plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspPluginConfig {
    /// Plugin type (eq, gain, crossover)
    pub plugin_type: String,
    /// Plugin parameters as JSON
    pub parameters: serde_json::Value,
}

/// DSP chain metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainMetadata {
    /// Average pre-optimization score
    pub pre_score: f64,
    /// Average post-optimization score
    pub post_score: f64,
    /// Algorithm used
    pub algorithm: String,
    /// Number of iterations
    pub iterations: usize,
    /// Timestamp
    pub timestamp: String,
}

/// A control point for custom target curve editing
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TargetCurveControlPoint {
    /// Frequency in Hz (20-20000)
    pub frequency: f64,
    /// Level in dB
    pub level_db: f64,
}

impl TargetCurveControlPoint {
    pub fn new(frequency: f64, level_db: f64) -> Self {
        Self {
            frequency,
            level_db,
        }
    }
}

/// Custom target curve defined by control points
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomTargetCurve {
    /// Control points sorted by frequency
    pub control_points: Vec<TargetCurveControlPoint>,
}

impl CustomTargetCurve {
    /// Create a new flat custom curve with default points at 20Hz and 20kHz
    pub fn new_flat() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(20000.0, 0.0),
            ],
        }
    }

    /// Create Near-field target: Flat 20-1000Hz, then down to -1dB at 20kHz
    pub fn new_near_field() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(1000.0, 0.0),
                TargetCurveControlPoint::new(20000.0, -1.0),
            ],
        }
    }

    /// Create Mid-field target: +4dB at 40Hz, down to -3dB at 20kHz
    pub fn new_mid_field() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 4.0),
                TargetCurveControlPoint::new(40.0, 4.0),
                TargetCurveControlPoint::new(160.0, 0.5), // Transition to near flat
                TargetCurveControlPoint::new(20000.0, -3.0),
            ],
        }
    }

    /// Create Far-field target: Flat up to 2kHz, then rolloff 3dB/oct
    pub fn new_far_field() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(2000.0, 0.0),
                TargetCurveControlPoint::new(4000.0, -3.0),
                TargetCurveControlPoint::new(8000.0, -6.0),
                TargetCurveControlPoint::new(16000.0, -9.0),
                TargetCurveControlPoint::new(20000.0, -9.96),
            ],
        }
    }

    /// Add a control point and keep sorted by frequency
    pub fn add_point(&mut self, point: TargetCurveControlPoint) {
        self.control_points.push(point);
        self.control_points
            .sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    }

    /// Remove a control point by index (keeps at least 2 points)
    pub fn remove_point(&mut self, index: usize) {
        if self.control_points.len() > 2 && index < self.control_points.len() {
            self.control_points.remove(index);
        }
    }

    /// Update a control point position
    pub fn update_point(&mut self, index: usize, frequency: f64, level_db: f64) {
        if let Some(point) = self.control_points.get_mut(index) {
            point.frequency = frequency.clamp(20.0, 20000.0);
            point.level_db = level_db.clamp(-24.0, 24.0);
        }
        // Re-sort after update
        self.control_points
            .sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    }

    /// Generate the target curve as 200 log-spaced points
    /// Returns Vec<(frequency_hz, level_db)>
    pub fn generate_curve(&self) -> Vec<(f64, f64)> {
        const NUM_POINTS: usize = 200;
        const MIN_FREQ: f64 = 20.0;
        const MAX_FREQ: f64 = 20000.0;

        if self.control_points.len() < 2 {
            // Return flat curve if not enough points
            return (0..NUM_POINTS)
                .map(|i| {
                    let t = i as f64 / (NUM_POINTS - 1) as f64;
                    let freq = (MIN_FREQ.ln() + t * (MAX_FREQ.ln() - MIN_FREQ.ln())).exp();
                    (freq, 0.0)
                })
                .collect();
        }

        // Generate log-spaced frequencies
        let frequencies: Vec<f64> = (0..NUM_POINTS)
            .map(|i| {
                let t = i as f64 / (NUM_POINTS - 1) as f64;
                (MIN_FREQ.ln() + t * (MAX_FREQ.ln() - MIN_FREQ.ln())).exp()
            })
            .collect();

        // Interpolate values at each frequency
        frequencies
            .iter()
            .map(|&freq| {
                let level = self.interpolate_at(freq);
                (freq, level)
            })
            .collect()
    }

    /// Linear interpolation between control points (in log-frequency space)
    fn interpolate_at(&self, freq: f64) -> f64 {
        if self.control_points.is_empty() {
            return 0.0;
        }

        // Find surrounding control points
        let mut lower_idx = 0;
        for (i, point) in self.control_points.iter().enumerate() {
            if point.frequency <= freq {
                lower_idx = i;
            } else {
                break;
            }
        }

        let upper_idx = (lower_idx + 1).min(self.control_points.len() - 1);

        if lower_idx == upper_idx {
            return self.control_points[lower_idx].level_db;
        }

        let lower = &self.control_points[lower_idx];
        let upper = &self.control_points[upper_idx];

        // Linear interpolation in log-frequency space
        let log_freq = freq.ln();
        let log_lower = lower.frequency.ln();
        let log_upper = upper.frequency.ln();

        if (log_upper - log_lower).abs() < 1e-10 {
            return lower.level_db;
        }

        let t = (log_freq - log_lower) / (log_upper - log_lower);
        lower.level_db + t * (upper.level_db - lower.level_db)
    }
}

/// UI state for Room EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct RoomEqDropdowns {
    pub data_source_open: bool,
    pub opt_mode_open: bool,
    pub fir_phase_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub crossover_type_open: bool,
    pub export_format_open: bool,
    /// DE strategy dropdown
    pub strategy_open: bool,
    /// Local algorithm dropdown
    pub local_algo_open: bool,
    /// Loss type dropdown
    pub loss_type_open: bool,
    /// Target curve dropdown
    pub target_curve_open: bool,
    /// System type dropdown
    pub system_type_open: bool,

    // Advanced dropdowns
    pub tilt_type_open: bool,
    pub excursion_filter_type_open: bool,
    pub multi_seat_strategy_open: bool,

    /// Review step smoothing dropdown
    pub review_smoothing_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
    /// Custom target curve editor modal open
    pub custom_target_modal_open: bool,
    /// Custom target presets dropdown open
    pub custom_target_presets_open: bool,
    /// Currently dragging control point index (None if not dragging)
    pub dragging_control_point: Option<usize>,
}

/// Field identifiers for AutoEQ form editing (legacy compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoEqField {
    NumFilters,
    MinQ,
    MaxQ,
    MinDb,
    MaxDb,
    MinFreq,
    MaxFreq,
    MaxIter,
}

/// Complete Room EQ screen state
#[derive(Debug, Clone)]
pub struct RoomEqState {
    /// Current step in the workflow
    pub step: RoomEqStep,

    // === Step 1: Load Data ===
    /// Source of measurement data
    pub data_source: RoomEqDataSource,
    /// Loaded channel measurements
    pub channel_measurements: Vec<ChannelMeasurement>,

    // === Step 2: Configuration ===
    /// Per-channel speaker configurations
    pub speaker_configs: Vec<RoomEqSpeakerConfig>,
    /// Global optimizer configuration
    pub optimizer_config: RoomEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Currently optimizing channel name
    pub current_channel: Option<String>,
    /// Per-channel optimization results
    pub channel_results: Vec<ChannelOptResult>,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,
    /// Progress history for visualization: (iteration, loss, optional_score)
    pub progress_history: Vec<(usize, f64, Option<f64>)>,
    /// Current iteration number
    pub current_iteration: usize,
    /// Current loss value
    pub current_loss: f64,

    // === Step 5: Export ===
    /// Generated DSP chain output
    pub dsp_output: Option<DspChainOutput>,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
    /// Review graph smoothing level in octaves (0 = none, 1 = 1 octave, etc.)
    pub review_smoothing_octaves: f64,
    /// Selected channel index for review (0-based)
    pub review_selected_channel: usize,
    /// Interactive chart state for review graph (zoom/pan) - initialized lazily
    pub review_chart_state: Option<InteractiveChartStateWrapper>,
    /// Whether to auto-scale Y axis for review graph (if false, uses fixed range)
    pub review_y_axis_auto: bool,
    /// Interactive chart state for progress chart (zoom/pan) - initialized lazily
    pub progress_chart_state: Option<InteractiveChartStateWrapper>,
    /// Custom target curve for manual entry mode
    pub custom_target_curve: CustomTargetCurve,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            data_source: RoomEqDataSource::FromRecording,
            channel_measurements: Vec::new(),
            speaker_configs: Vec::new(),
            optimizer_config: RoomEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            progress_history: Vec::new(),
            current_iteration: 0,
            current_loss: 0.0,
            dsp_output: None,
            dropdowns: RoomEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
            review_smoothing_octaves: 1.0, // Default to 1 octave smoothing
            review_selected_channel: 0,
            review_chart_state: None,
            review_y_axis_auto: true,
            progress_chart_state: None,
            custom_target_curve: CustomTargetCurve::new_flat(),
        }
    }
}

impl RoomEqState {
    /// Check if we have measurement data loaded
    pub fn has_measurements(&self) -> bool {
        !self.channel_measurements.is_empty()
    }

    /// Get the number of channels
    pub fn channel_count(&self) -> usize {
        self.channel_measurements.len()
    }

    /// Check if optimization is complete
    pub fn is_optimization_complete(&self) -> bool {
        self.optimization_status == OptimizationStatus::Completed
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Initialize speaker configs from measurements
    pub fn init_speaker_configs(&mut self) {
        self.speaker_configs = self
            .channel_measurements
            .iter()
            .map(|m| RoomEqSpeakerConfig {
                channel_name: m.channel_name.clone(),
                config_type: if m.is_group {
                    SpeakerConfigType::MultiDriver
                } else {
                    SpeakerConfigType::Single
                },
                crossover_type: CrossoverType::LR24,
                driver_names: if m.is_group {
                    m.group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("driver_{}", i + 1))
                        .collect()
                } else {
                    Vec::new()
                },
                crossover_freq_hints: Vec::new(),
            })
            .collect();
    }

    /// Load measurements from recording state
    pub fn load_from_recording(&mut self, recording_state: &RecordingState) {
        self.channel_measurements = recording_state
            .channel_recordings
            .iter()
            .filter_map(|r| {
                r.result.as_ref().map(|result| ChannelMeasurement {
                    channel_name: r.channel_name.clone(),
                    measurement: result.clone(),
                    is_group: false,
                    group_drivers: Vec::new(),
                })
            })
            .collect();

        self.data_source = RoomEqDataSource::FromRecording;
        self.init_speaker_configs();
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.current_channel = None;
        self.channel_results.clear();
        self.overall_progress = 0.0;
        self.progress_history.clear();
        self.current_iteration = 0;
        self.current_loss = 0.0;
        self.error_message = None;
    }

    /// Get average pre-score
    pub fn average_pre_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.pre_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Get average post-score
    pub fn average_post_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.post_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Convert UI state to backend RoomConfig
    pub fn to_room_config(&self) -> RoomConfig {
        let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
        let mut crossovers: HashMap<String, BackendCrossoverConfig> = HashMap::new();

        // Helper to convert measurement to curve
        let to_curve = |meas: &ChannelMeasurement| -> autoeq::Curve {
            let frequencies: Vec<f64> = meas.measurement.frequencies.iter().map(|&f| f as f64).collect();
            let magnitude_db: Vec<f64> = meas.measurement.magnitude_db.iter().map(|&db| db as f64).collect();

            autoeq::Curve {
                freq: ndarray::Array1::from_vec(frequencies),
                spl: ndarray::Array1::from_vec(magnitude_db),
                phase: None,
            }
        };

        // Helper to convert recording result to curve
        let result_to_curve = |res: &RecordingResult| -> autoeq::Curve {
            let frequencies: Vec<f64> = res.frequencies.iter().map(|&f| f as f64).collect();
            let magnitude_db: Vec<f64> = res.magnitude_db.iter().map(|&db| db as f64).collect();

            autoeq::Curve {
                freq: ndarray::Array1::from_vec(frequencies),
                spl: ndarray::Array1::from_vec(magnitude_db),
                phase: None,
            }
        };

        // Iterate over configured speakers
        for speaker_config in &self.speaker_configs {
            let channel_name = &speaker_config.channel_name;
            
            // Find corresponding measurement
            if let Some(meas) = self.channel_measurements.iter().find(|m| &m.channel_name == channel_name) {
                match speaker_config.config_type {
                    SpeakerConfigType::Single => {
                        let curve = to_curve(meas);
                        speakers.insert(
                            channel_name.clone(),
                            SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
                        );
                    }
                    SpeakerConfigType::MultiDriver => {
                        let mut driver_measurements = Vec::new();
                        if meas.is_group && !meas.group_drivers.is_empty() {
                            for driver_res in &meas.group_drivers {
                                driver_measurements.push(MeasurementSource::InMemory(result_to_curve(driver_res)));
                            }
                        } else {
                            driver_measurements.push(MeasurementSource::InMemory(to_curve(meas)));
                        }

                        let xover_id = format!("xover_{}", channel_name);
                        let xover_type = match speaker_config.crossover_type {
                            CrossoverType::LR12 => "LR12",
                            CrossoverType::LR24 => "LR24",
                            CrossoverType::LR48 => "LR48",
                            CrossoverType::Butterworth12 => "Butterworth12",
                            CrossoverType::Butterworth24 => "Butterworth24",
                        };

                        crossovers.insert(xover_id.clone(), BackendCrossoverConfig {
                            crossover_type: xover_type.to_string(),
                            frequency: None,
                            frequencies: None,
                            frequency_range: None,
                        });

                        speakers.insert(
                            channel_name.clone(),
                            SpeakerConfig::Group(SpeakerGroup {
                                name: channel_name.clone(),
                                measurements: driver_measurements,
                                crossover: Some(xover_id),
                            }),
                        );
                    }
                }
            }
        }

        let algorithm = self.optimizer_config.algorithm.to_autoeq_string().to_string();

        let optimizer = BackendOptimizerConfig {
            loss_type: self.optimizer_config.loss_type.clone(),
            algorithm,
            num_filters: self.optimizer_config.num_filters,
            min_q: self.optimizer_config.min_q,
            max_q: self.optimizer_config.max_q,
            min_db: self.optimizer_config.min_db,
            max_db: self.optimizer_config.max_db,
            min_freq: self.optimizer_config.min_freq,
            max_freq: self.optimizer_config.max_freq,
            max_iter: self.optimizer_config.max_iter,
            population: self.optimizer_config.population,
            peq_model: self.optimizer_config.peq_model.clone(),
            mode: self.optimizer_config.mode.to_code().to_string(),
            fir: Some(BackendFirConfig {
                taps: self.optimizer_config.fir.taps,
                phase: self.optimizer_config.fir.phase.clone(),
                correct_excess_phase: false,
                phase_smoothing: 0.167,
            }),
            seed: None,
            mixed_config: None,
            refine: self.optimizer_config.refine,
            local_algo: self.optimizer_config.local_algo.clone(),
            psychoacoustic: self.optimizer_config.psychoacoustic,
            asymmetric_loss: self.optimizer_config.asymmetric_loss,
            target_tilt: if self.optimizer_config.target_tilt.enabled {
                let tilt_type = match self.optimizer_config.target_tilt.tilt_type.as_str() {
                    "harman" => TiltType::Harman,
                    "custom" => TiltType::Custom,
                    _ => TiltType::Flat,
                };
                Some(BackendTargetTiltConfig {
                    tilt_type,
                    slope_db_per_octave: self.optimizer_config.target_tilt.slope,
                    reference_freq: self.optimizer_config.target_tilt.reference_freq,
                    bass_shelf_db: self.optimizer_config.target_tilt.bass_shelf_db,
                    bass_shelf_freq: self.optimizer_config.target_tilt.bass_shelf_freq,
                })
            } else {
                None
            },
            excursion_protection: if self.optimizer_config.excursion_protection.enabled {
                let filter_type = if self.optimizer_config.excursion_protection.filter_type == "bw" {
                    HighpassType::Butterworth
                } else {
                    HighpassType::LinkwitzRiley
                };
                Some(BackendExcursionProtectionConfig {
                    enabled: true,
                    auto_detect_f3: self.optimizer_config.excursion_protection.auto_detect_f3,
                    manual_f3_hz: Some(self.optimizer_config.excursion_protection.manual_f3_hz),
                    filter_order: self.optimizer_config.excursion_protection.filter_order,
                    filter_type,
                    margin_octaves: self.optimizer_config.excursion_protection.margin_octaves,
                })
            } else {
                None
            },
            schroeder_split: if self.optimizer_config.schroeder_split.enabled {
                Some(BackendSchroederSplitConfig {
                    enabled: true,
                    schroeder_freq: self.optimizer_config.schroeder_split.schroeder_freq,
                    room_dimensions: None,
                    low_freq_config: LowFreqFilterConfig {
                        max_q: self.optimizer_config.schroeder_split.low_freq_max_q,
                        min_q: 0.5,
                        allow_boost: self.optimizer_config.schroeder_split.low_freq_allow_boost,
                    },
                    high_freq_config: HighFreqFilterConfig {
                        max_q: self.optimizer_config.schroeder_split.high_freq_max_q,
                        shelving_only: self.optimizer_config.schroeder_split.high_freq_shelving_only,
                    },
                })
            } else {
                None
            },
            phase_alignment: if self.optimizer_config.phase_alignment.enabled {
                Some(BackendPhaseAlignmentConfig {
                    enabled: true,
                    min_freq: self.optimizer_config.phase_alignment.min_freq,
                    max_freq: self.optimizer_config.phase_alignment.max_freq,
                    optimize_polarity: self.optimizer_config.phase_alignment.optimize_polarity,
                    max_delay_ms: self.optimizer_config.phase_alignment.max_delay_ms,
                })
            } else {
                None
            },
            multi_seat: if self.optimizer_config.multi_seat.enabled {
                let strategy = match self.optimizer_config.multi_seat.strategy.as_str() {
                    "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                    "average" => MultiSeatStrategy::Average,
                    _ => MultiSeatStrategy::MinimizeVariance,
                };
                Some(BackendMultiSeatConfig {
                    enabled: true,
                    strategy,
                    primary_seat: self.optimizer_config.multi_seat.primary_seat,
                    max_deviation_db: self.optimizer_config.multi_seat.max_deviation_db,
                })
            } else {
                None
            },
        };

        RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            group_delay: None,
            optimizer,
            recording_config: None,
        }
    }

    /// Validate the current configuration
    pub fn validate(&self) -> autoeq::roomeq::ValidationResult {
        let config = self.to_room_config();
        autoeq::roomeq::validate_room_config(&config)
    }

    /// Calculate the level offset needed to normalize a curve to 0dB.
    /// Uses mean SPL in the 1kHz to 2kHz range by default.
    pub fn calculate_normalization_offset(frequencies: &[f64], spl: &[f64]) -> f64 {
        let min_freq = 1000.0;
        let max_freq = 2000.0;

        let mut sum = 0.0;
        let mut count = 0;

        for (i, &f) in frequencies.iter().enumerate() {
            if f >= min_freq && f <= max_freq {
                if let Some(&db) = spl.get(i) {
                    sum += db;
                    count += 1;
                }
            }
        }

        if count > 0 {
            sum / count as f64
        } else {
            // Fallback: overall mean
            if spl.is_empty() {
                0.0
            } else {
                spl.iter().sum::<f64>() / spl.len() as f64
            }
        }
    }

    /// Normalize a set of points by subtracting an offset.
    pub fn normalize_points(points: &[(f64, f64)], offset: f64) -> Vec<(f64, f64)> {
        points.iter().map(|&(f, db)| (f, db - offset)).collect()
    }
}
