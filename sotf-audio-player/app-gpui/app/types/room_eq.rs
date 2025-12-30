// ============================================================================
// Room EQ Screen Types
// ============================================================================

use serde::{Deserialize, Serialize};

use super::recording::{RecordingResult, RecordingState};

/// Room EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomEqStep {
    /// Step 1: Load/import measurement data
    #[default]
    LoadData,
    /// Step 2: Configure channels and optimizer settings
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

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    /// Optimization algorithm
    pub algorithm: RoomEqAlgorithm,
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
}

impl Default for RoomEqOptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 3.0,
            min_freq: 20.0,
            max_freq: 16000.0,
            max_iter: 10000,
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

/// UI state for Room EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct RoomEqDropdowns {
    pub data_source_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub crossover_type_open: bool,
    pub export_format_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
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

    // === Step 5: Export ===
    /// Generated DSP chain output
    pub dsp_output: Option<DspChainOutput>,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
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
            dsp_output: None,
            dropdowns: RoomEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
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
}

