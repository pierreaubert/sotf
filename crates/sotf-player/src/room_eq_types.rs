//! Shared room EQ domain types used by both GPUI and TUI apps.

use serde::{Deserialize, Serialize};

use crate::recording_types::RecordingResult;

/// Room EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomEqStep {
    /// Step 1: Load/import measurement data
    #[default]
    LoadData,
    /// Step 2: Configure channels, mode, and optimizer settings
    Configure,
    /// Step 3: Run optimization (per-channel, then combined)
    Optimize,
    /// Step 4: Review results and visualizations
    Review,
    /// Step 5: Export DSP chain and apply
    Export,
}

impl RoomEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [RoomEqStep] {
        &[
            RoomEqStep::LoadData,
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
            RoomEqStep::Configure => 1,
            RoomEqStep::Optimize => 2,
            RoomEqStep::Review => 3,
            RoomEqStep::Export => 4,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            RoomEqStep::LoadData => "Load Data",
            RoomEqStep::Configure => "Configure",
            RoomEqStep::Optimize => "Optimize",
            RoomEqStep::Review => "Review",
            RoomEqStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => Some(RoomEqStep::Configure),
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
            RoomEqStep::Configure => Some(RoomEqStep::LoadData),
            RoomEqStep::Optimize => Some(RoomEqStep::Configure),
            RoomEqStep::Review => Some(RoomEqStep::Optimize),
            RoomEqStep::Export => Some(RoomEqStep::Review),
        }
    }
}

/// Source of measurement data for Room EQ
#[derive(Debug, Clone, PartialEq)]
pub enum RoomEqDataSource {
    /// Use recordings from current session
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
    pub playback_device_name: String,
    pub playback_device_id: String,
    pub playback_sample_rate: u32,
    pub playback_channels: usize,
    pub speaker_configuration: String,
    pub channel_names: Vec<String>,
    pub recording_device_name: String,
    pub recording_device_id: String,
    pub recording_sample_rate: u32,
    pub recording_channels: usize,
    pub mic_calibration_path: Option<String>,
    pub recording_directory: Option<String>,
    pub signal_type: String,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    #[serde(default)]
    pub sweep_start_freq: Option<f32>,
    #[serde(default)]
    pub sweep_end_freq: Option<f32>,
}

/// File format for saving/loading room EQ measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqMeasurementsFile {
    pub version: u32,
    pub channels: Vec<ChannelMeasurement>,
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

    fn convert_v1_to_v2(mut value: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = value.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(2));
        }
        value
    }

    /// Load measurements from a JSON file, supporting both the legacy
    /// `RoomEqMeasurementsFile` format and the newer `autoeq::RoomConfig`
    /// format (with `speakers` map and string version).
    ///
    /// `base_dir` is used to resolve relative wav/csv paths in the RoomConfig
    /// format.  Pass the parent directory of the JSON file.
    pub fn load_from_json(
        json: &str,
        base_dir: Option<&std::path::Path>,
    ) -> Result<Vec<ChannelMeasurement>, String> {
        // Try the new RoomConfig format first (has string version like "1.1.0")
        if let Ok(room_config) = serde_json::from_str::<autoeq::RoomConfig>(json) {
            log::info!(
                "Loaded {} speakers (RoomConfig format)",
                room_config.speakers.len(),
            );
            let channels = Self::channels_from_room_config(room_config, base_dir);
            if !channels.is_empty() {
                return Ok(channels);
            }
            // Empty result from RoomConfig → fall through to legacy format
        }

        // Fall back to legacy RoomEqMeasurementsFile format
        match Self::from_json_str(json) {
            Ok(file) => Ok(file.channels),
            Err(e) => Err(format!("Parse error: {}", e)),
        }
    }

    /// Convert an `autoeq::RoomConfig` into `Vec<ChannelMeasurement>`.
    fn channels_from_room_config(
        room_config: autoeq::RoomConfig,
        base_dir: Option<&std::path::Path>,
    ) -> Vec<ChannelMeasurement> {
        room_config
            .speakers
            .into_iter()
            .enumerate()
            .filter_map(|(idx, (channel_name, speaker_config))| {
                let inline = match speaker_config {
                    autoeq::SpeakerConfig::Single(source) => match source {
                        autoeq::MeasurementSource::Single(s) => {
                            s.measurement.inline_data().cloned()
                        }
                        autoeq::MeasurementSource::Multiple(m) => m
                            .measurements
                            .first()
                            .and_then(|r| r.inline_data())
                            .cloned(),
                        autoeq::MeasurementSource::InMemory(_) => None,
                    },
                    _ => None,
                };

                inline.map(|data| {
                    let resolve = |rel: &str| -> String {
                        match base_dir {
                            Some(dir) => {
                                let abs = dir.join(rel);
                                if abs.exists() {
                                    abs.to_string_lossy().to_string()
                                } else {
                                    rel.to_string()
                                }
                            }
                            None => rel.to_string(),
                        }
                    };

                    let wav_path = data.wav_path.as_deref().map(&resolve);
                    let csv_path = data.csv_path.as_deref().map(&resolve);

                    let (frequencies, magnitude_db, phase_deg) = if data.frequencies.is_empty() {
                        // Try loading from CSV
                        if let Some(ref csv) = csv_path {
                            let csv_full = std::path::PathBuf::from(csv);
                            if let Ok(curve) = autoeq::read::read_curve_from_csv(&csv_full) {
                                (
                                    curve.freq.iter().map(|&f| f as f32).collect(),
                                    curve.spl.iter().map(|&s| s as f32).collect(),
                                    curve
                                        .phase
                                        .map(|p| p.iter().map(|&v| v as f32).collect())
                                        .unwrap_or_default(),
                                )
                            } else {
                                (Vec::new(), Vec::new(), Vec::new())
                            }
                        } else {
                            (Vec::new(), Vec::new(), Vec::new())
                        }
                    } else {
                        (
                            data.frequencies.iter().map(|&f| f as f32).collect(),
                            data.magnitude_db.iter().map(|&m| m as f32).collect(),
                            data.phase_deg
                                .unwrap_or_default()
                                .iter()
                                .map(|&p| p as f32)
                                .collect(),
                        )
                    };

                    ChannelMeasurement {
                        channel_name,
                        measurement: RecordingResult {
                            channel: idx,
                            wav_path,
                            csv_path,
                            frequencies,
                            magnitude_db,
                            phase_deg,
                            impulse_response: None,
                            impulse_time_ms: None,
                            excess_group_delay_ms: None,
                            thd_percent: None,
                            harmonic_distortion_db: None,
                            rt60_ms: None,
                            clarity_c50_db: None,
                            clarity_c80_db: None,
                            spectrogram_db: None,
                        },
                        is_group: false,
                        group_drivers: Vec::new(),
                    }
                })
            })
            .filter(|ch| !ch.measurement.frequencies.is_empty())
            .collect()
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

/// Speaker configuration type (duplicated from autoeq::types for UI use)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqSpeakerConfigType {
    /// Single full-range driver or measurement
    #[default]
    Single,
    /// Multi-driver with active crossover
    MultiDriver,
}

/// Crossover type for multi-driver speakers (UI version with Butterworth24)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqCrossoverType {
    LR12,
    #[default]
    LR24,
    LR48,
    Butterworth12,
    Butterworth24,
}

impl RoomEqCrossoverType {
    pub fn all() -> &'static [RoomEqCrossoverType] {
        &[
            RoomEqCrossoverType::LR12,
            RoomEqCrossoverType::LR24,
            RoomEqCrossoverType::LR48,
            RoomEqCrossoverType::Butterworth12,
            RoomEqCrossoverType::Butterworth24,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqCrossoverType::LR12 => "Linkwitz-Riley 12dB",
            RoomEqCrossoverType::LR24 => "Linkwitz-Riley 24dB",
            RoomEqCrossoverType::LR48 => "Linkwitz-Riley 48dB",
            RoomEqCrossoverType::Butterworth12 => "Butterworth 12dB",
            RoomEqCrossoverType::Butterworth24 => "Butterworth 24dB",
        }
    }
}

/// Configuration for a speaker channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqSpeakerConfig {
    pub channel_name: String,
    pub config_type: RoomEqSpeakerConfigType,
    pub crossover_type: RoomEqCrossoverType,
    pub driver_names: Vec<String>,
    pub crossover_freq_hints: Vec<f64>,
}

impl Default for RoomEqSpeakerConfig {
    fn default() -> Self {
        Self {
            channel_name: String::new(),
            config_type: RoomEqSpeakerConfigType::Single,
            crossover_type: RoomEqCrossoverType::LR24,
            driver_names: Vec::new(),
            crossover_freq_hints: Vec::new(),
        }
    }
}

/// Multi-speaker optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MultiSpeakerMode {
    #[default]
    Sequential,
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
}

/// Optimization algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqAlgorithm {
    #[default]
    Cobyla,
    DifferentialEvolution,
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

/// Optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqOptimizationMode {
    #[default]
    Iir,
    Fir,
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
    pub taps: usize,
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
    pub tilt_type: String,
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
    pub filter_type: String,
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
    pub strategy: String,
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

/// Group Delay Optimization configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupDelayOptConfig {
    pub enabled: bool,
    pub target_ms: f64,
}

/// Voice of God (timbre matching) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoGConfig {
    pub enabled: bool,
    pub reference_channel: String,
}

impl Default for VoGConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reference_channel: "C".to_string(),
        }
    }
}

/// Broadband target matching configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BroadbandTargetMatchingConfig {
    pub enabled: bool,
}

/// Mixed mode (IIR+FIR) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedModeUiConfig {
    pub crossover_freq: f64,
    pub crossover_type: String,
    pub fir_band: String,
}

impl Default for MixedModeUiConfig {
    fn default() -> Self {
        Self {
            crossover_freq: 300.0,
            crossover_type: "LR24".to_string(),
            fir_band: "low".to_string(),
        }
    }
}

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    #[serde(default)]
    pub mode: RoomEqOptimizationMode,
    #[serde(default)]
    pub fir: RoomEqFirConfig,
    pub multi_speaker_mode: MultiSpeakerMode,
    pub algorithm: String,
    pub num_filters: usize,
    pub min_q: f64,
    pub max_q: f64,
    pub min_db: f64,
    pub max_db: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub max_iter: usize,
    pub peq_model: String,
    pub population: usize,
    pub refine: bool,
    pub local_algo: String,
    pub loss_type: String,
    pub psychoacoustic: bool,
    pub asymmetric_loss: bool,
    pub target_curve: String,
    pub system_type: String,
    #[serde(default)]
    pub allow_delay: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub gd_opt: GroupDelayOptConfig,
    #[serde(default)]
    pub vog: VoGConfig,
    #[serde(default)]
    pub broadband_target_matching: BroadbandTargetMatchingConfig,
    #[serde(default)]
    pub mixed_config: MixedModeUiConfig,
    #[serde(default)]
    pub target_tilt: TargetTiltConfig,
    #[serde(default)]
    pub excursion_protection: ExcursionProtectionConfig,
    #[serde(default)]
    pub schroeder_split: SchroederSplitConfig,
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
            algorithm: "autoeq:de".to_string(),
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
            refine: false,
            local_algo: "cobyla".to_string(),
            loss_type: "flat".to_string(),
            psychoacoustic: true,
            asymmetric_loss: true,
            target_curve: "flat".to_string(),
            system_type: "stereo".to_string(),
            allow_delay: false,
            seed: None,
            gd_opt: GroupDelayOptConfig::default(),
            vog: VoGConfig::default(),
            broadband_target_matching: BroadbandTargetMatchingConfig::default(),
            mixed_config: MixedModeUiConfig::default(),
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
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Field identifiers for AutoEQ form editing
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

/// EQ filter configuration (for display and export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqFilterConfig {
    pub filter_type: String,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
}

/// Optimization result for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOptResult {
    pub channel_name: String,
    pub pre_score: f64,
    pub post_score: f64,
    pub eq_filters: Vec<EqFilterConfig>,
    pub crossover_freqs: Option<Vec<f64>>,
    pub driver_gains: Option<Vec<f64>>,
    pub original_response: Option<Vec<(f64, f64)>>,
    pub corrected_response: Option<Vec<(f64, f64)>>,
    pub normalized_response: Option<Vec<(f64, f64)>>,
}

/// DSP chain output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainOutput {
    pub channels: std::collections::HashMap<String, ChannelDspChain>,
    pub metadata: Option<DspChainMetadata>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDspChain {
    pub channel: String,
    pub plugins: Vec<DspPluginConfig>,
    pub drivers: Option<Vec<DriverDspChain>>,
}

/// DSP chain for a driver in multi-driver setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDspChain {
    pub name: String,
    pub index: usize,
    pub plugins: Vec<DspPluginConfig>,
}

/// DSP plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspPluginConfig {
    pub plugin_type: String,
    pub parameters: serde_json::Value,
}

/// DSP chain metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainMetadata {
    pub pre_score: f64,
    pub post_score: f64,
    pub algorithm: String,
    pub iterations: usize,
    pub timestamp: String,
}

/// A control point for custom target curve editing
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TargetCurveControlPoint {
    pub frequency: f64,
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
    pub control_points: Vec<TargetCurveControlPoint>,
}

impl CustomTargetCurve {
    pub fn new_flat() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(20000.0, 0.0),
            ],
        }
    }

    pub fn add_point(&mut self, point: TargetCurveControlPoint) {
        self.control_points.push(point);
        self.control_points
            .sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    }

    pub fn remove_point(&mut self, index: usize) {
        if self.control_points.len() > 2 && index < self.control_points.len() {
            self.control_points.remove(index);
        }
    }

    pub fn update_point(&mut self, index: usize, frequency: f64, level_db: f64) {
        if let Some(point) = self.control_points.get_mut(index) {
            point.frequency = frequency.clamp(20.0, 20000.0);
            point.level_db = level_db.clamp(-24.0, 24.0);
        }
        self.control_points
            .sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    }

    /// Generate the target curve as 200 log-spaced points
    pub fn generate_curve(&self) -> Vec<(f64, f64)> {
        const NUM_POINTS: usize = 200;
        const MIN_FREQ: f64 = 20.0;
        const MAX_FREQ: f64 = 20000.0;

        if self.control_points.len() < 2 {
            return (0..NUM_POINTS)
                .map(|i| {
                    let t = i as f64 / (NUM_POINTS - 1) as f64;
                    let freq = (MIN_FREQ.ln() + t * (MAX_FREQ.ln() - MIN_FREQ.ln())).exp();
                    (freq, 0.0)
                })
                .collect();
        }

        let frequencies: Vec<f64> = (0..NUM_POINTS)
            .map(|i| {
                let t = i as f64 / (NUM_POINTS - 1) as f64;
                (MIN_FREQ.ln() + t * (MAX_FREQ.ln() - MIN_FREQ.ln())).exp()
            })
            .collect();

        frequencies
            .iter()
            .map(|&freq| {
                let level = self.interpolate_at(freq);
                (freq, level)
            })
            .collect()
    }

    fn interpolate_at(&self, freq: f64) -> f64 {
        if self.control_points.is_empty() {
            return 0.0;
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_json_legacy_format() {
        let json = r#"{
            "version": 2,
            "channels": [{
                "channel_name": "L",
                "measurement": {
                    "channel": 0,
                    "frequencies": [100.0, 1000.0],
                    "magnitude_db": [-3.0, 0.0],
                    "phase_deg": [10.0, 20.0]
                },
                "is_group": false,
                "group_drivers": []
            }]
        }"#;
        let channels = RoomEqMeasurementsFile::load_from_json(json, None).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].channel_name, "L");
        assert_eq!(channels[0].measurement.frequencies.len(), 2);
    }

    #[test]
    fn load_from_json_room_config_format() {
        let json = r#"{
            "version": "1.1.0",
            "speakers": {
                "R": {
                    "frequencies": [20.0, 100.0, 1000.0],
                    "magnitude_db": [-10.0, -3.0, 0.0],
                    "phase_deg": [5.0, 10.0, 15.0],
                    "name": "R"
                },
                "L": {
                    "frequencies": [20.0, 100.0, 1000.0],
                    "magnitude_db": [-9.0, -2.0, 1.0],
                    "name": "L"
                }
            },
            "optimizer": {}
        }"#;
        let channels = RoomEqMeasurementsFile::load_from_json(json, None).unwrap();
        assert_eq!(channels.len(), 2);
        for ch in &channels {
            assert_eq!(ch.measurement.frequencies.len(), 3);
            assert_eq!(ch.measurement.magnitude_db.len(), 3);
        }
    }

    #[test]
    fn load_from_json_room_config_real_file() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data_generated/recording-adam-20260114-142539/recordings.json");
        if !path.exists() {
            // Skip if test data not available
            return;
        }
        let json = std::fs::read_to_string(&path).unwrap();
        let base_dir = path.parent();
        let channels = RoomEqMeasurementsFile::load_from_json(&json, base_dir).unwrap();
        assert!(
            !channels.is_empty(),
            "Should load at least one channel from real recording file"
        );
        for ch in &channels {
            assert!(
                !ch.measurement.frequencies.is_empty(),
                "Channel '{}' should have frequency data",
                ch.channel_name
            );
        }
    }
}
