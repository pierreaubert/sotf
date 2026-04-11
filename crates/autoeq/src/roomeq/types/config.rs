//! Room EQ Configuration Types
//!
//! All configuration structs for room equalization.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::MeasurementSource;

/// Configuration version (semantic versioning)
pub fn default_config_version() -> String {
    "1.3.0".to_string()
}

// ============================================================================
// Recording Configuration
// ============================================================================

/// Recording configuration stored with measurements
/// Contains device settings and signal parameters used during measurement capture
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
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
    /// Speaker configuration (e.g. "5.1", "7.1.4", "Stereo")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_configuration: Option<String>,
    /// Channel names in order (e.g. ["L", "R", "C", "LFE", "SL", "SR"])
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
    /// Per-channel microphone calibration file paths
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_calibration_paths: Option<Vec<Option<String>>>,
    /// Recording output directory
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_directory: Option<String>,
    /// Signal type used for measurements (e.g. "Sweep", "Pink Noise")
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

// ============================================================================
// Processing Mode & Strategy Enums
// ============================================================================

/// Processing mode for the optimization engine
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingMode {
    /// Low-latency mode (IIR filters only) - < 5ms latency
    #[default]
    LowLatency,
    /// Phase-linear mode (FIR filters only) - High latency allowed
    PhaseLinear,
    /// Hybrid mode (IIR for bass, FIR for mids/highs) - Variable latency
    Hybrid,
    /// Mixed-phase mode (IIR for minimum-phase + excess phase FIR)
    /// Requires phase data in measurements. Low latency (~10ms).
    MixedPhase,
    /// Warped IIR mode — biquads designed on a Bark-scale warped frequency axis.
    /// Concentrates filter resolution in bass/low-mid where room modes live.
    /// Same latency as LowLatency but perceptually-weighted correction.
    WarpedIir,
    /// Kautz modal mode — pole-tuned filter targeting detected room modes.
    /// Uses room mode analysis to place filter poles at resonance frequencies.
    /// Gain optimization via linear least-squares (very fast, no DE needed).
    /// Best for small, highly resonant rooms with clear modal problems.
    KautzModal,
}

/// Strategy for subwoofer optimization
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubwooferStrategy {
    /// Single subwoofer optimization (default)
    #[default]
    Single,
    /// Multi-Sub Optimizer (minimize seat-to-seat variance)
    Mso,
    /// Double Bass Array (active cancellation)
    Dba,
}

/// System topology model
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SystemModel {
    Stereo,
    HomeCinema,
    #[default]
    Custom,
}

/// Target curve tilt type for room correction
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TiltType {
    /// Flat target (no tilt)
    #[default]
    Flat,
    /// Harman-style tilt (-0.8 dB/octave with bass shelf)
    Harman,
    /// Custom tilt with user-specified parameters
    Custom,
}

/// Target response shape preset
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TargetShape {
    /// Flat in-room response (no tilt)
    #[default]
    Flat,
    /// Harman preferred in-room curve (-0.8 dB/octave from 1 kHz reference)
    Harman,
    /// Custom slope specified by `slope_db_per_octave`
    Custom,
    /// Load target curve from external CSV file (`curve_path` must be set)
    File,
}

/// Highpass filter type for excursion protection
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HighpassType {
    /// Linkwitz-Riley (4th order = 24dB/oct)
    #[default]
    LinkwitzRiley,
    /// Butterworth
    Butterworth,
}

/// Strategy for multi-seat optimization
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MultiSeatStrategy {
    /// Minimize standard deviation across all seats (default)
    #[default]
    MinimizeVariance,
    /// Optimize for primary seat with constraints on others
    PrimaryWithConstraints,
    /// Optimize for average response across all seats
    Average,
}

/// Strategy for handling multiple measurements per speaker
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MultiMeasurementStrategy {
    /// RMS-average curves, optimize on average (existing behavior)
    #[default]
    Average,
    /// loss = Σ w_i * loss_i — weighted sum of per-measurement losses
    WeightedSum,
    /// loss = max(loss_i) — optimize worst case across all measurements
    Minimax,
    /// loss = mean(loss_i) + λ * var(loss_i) — balance quality + consistency
    VariancePenalized,
    /// Spatial robustness: RMS-average + correction depth mask based on spatial variance.
    /// Only corrects features consistent across positions.
    SpatialRobustness,
}

/// Correction mode for CEA2034 speaker pre-correction
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Cea2034CorrectionMode {
    /// Correct Listening Window toward flat (best for nearfield <2m)
    Flat,
    /// Optimize full Harman speaker preference score using all CEA2034 curves
    Score,
    /// Auto-select based on estimated listening distance from impulse response
    #[default]
    Auto,
}

// ============================================================================
// Subwoofer & Speaker Configs
// ============================================================================

/// Subwoofer system configuration (part of SystemConfig)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubwooferSystemConfig {
    /// Strategy for subwoofer optimization
    #[serde(default)]
    pub config: SubwooferStrategy,
    /// Crossover reference key (points to entry in `crossovers` map)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossover: Option<String>,
    /// Mapping of subwoofer measurement key to main speaker logical role
    #[serde(flatten)]
    pub mapping: HashMap<String, String>,
}

/// Explicit system configuration mapping logical roles to measurements
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SystemConfig {
    /// System topology model
    #[serde(default)]
    pub model: SystemModel,
    /// Map of logical role to measurement key
    pub speakers: HashMap<String, String>,
    /// Subwoofer configuration and mapping
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subwoofers: Option<SubwooferSystemConfig>,
}

/// Speaker configuration (can be single measurement or group)
///
/// Variant order matters for serde untagged deserialization: serde tries each variant
/// in order. Group/MultiSub/Dba all require a `name` field that `MeasurementSource`
/// doesn't have, so they are tried first. `Single` is last as a catch-all.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SpeakerConfig {
    /// Group of measurements (multi-driver case)
    Group(SpeakerGroup),
    /// Multiple subwoofers optimization
    MultiSub(MultiSubGroup),
    /// Double Bass Array (DBA) optimization
    Dba(DBAConfig),
    /// Gradient Cardioid subwoofer optimization
    Cardioid(Box<CardioidConfig>),
    /// Single channel (simple case)
    Single(MeasurementSource),
}

impl SpeakerConfig {
    pub fn speaker_name(&self) -> Option<&str> {
        match self {
            SpeakerConfig::Single(source) => source.speaker_name(),
            SpeakerConfig::Group(group) => group.speaker_name.as_deref(),
            SpeakerConfig::MultiSub(ms) => ms.speaker_name.as_deref(),
            SpeakerConfig::Dba(dba) => dba.speaker_name.as_deref(),
            SpeakerConfig::Cardioid(c) => c.speaker_name.as_deref(),
        }
    }

    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        match self {
            SpeakerConfig::Single(source) => source.resolve_paths(base_dir),
            SpeakerConfig::Group(group) => group.resolve_paths(base_dir),
            SpeakerConfig::MultiSub(group) => group.resolve_paths(base_dir),
            SpeakerConfig::Dba(config) => config.resolve_paths(base_dir),
            SpeakerConfig::Cardioid(config) => config.resolve_paths(base_dir),
        }
    }
}

/// Group of measurements for a single speaker (multi-driver)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerGroup {
    /// Name of the group
    pub name: String,
    /// Optional speaker model name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// Measurements in this group
    pub measurements: Vec<MeasurementSource>,
    /// Crossover configuration for this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossover: Option<String>,
}

impl SpeakerGroup {
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
    /// Optional speaker model name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// Measurements for each subwoofer
    pub subwoofers: Vec<MeasurementSource>,
    /// Enable per-subwoofer all-pass filter optimization
    #[serde(default)]
    pub allpass_optimization: bool,
}

impl MultiSubGroup {
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for m in &mut self.subwoofers {
            m.resolve_paths(base_dir);
        }
    }
}

/// Configuration for Gradient Cardioid Subwoofer (2 subwoofers)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CardioidConfig {
    /// Name of the cardioid system
    pub name: String,
    /// Optional speaker model name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// Measurement for the front (primary) subwoofer
    pub front: MeasurementSource,
    /// Measurement for the rear (cancellation) subwoofer
    pub rear: MeasurementSource,
    /// Physical separation distance in meters (between acoustic centers)
    pub separation_meters: f64,
}

impl CardioidConfig {
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        self.front.resolve_paths(base_dir);
        self.rear.resolve_paths(base_dir);
    }
}

/// Configuration for Double Bass Array (DBA)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DBAConfig {
    /// Name of the DBA system
    pub name: String,
    /// Optional speaker model name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// Measurements for the front array
    pub front: Vec<MeasurementSource>,
    /// Measurements for the rear array
    pub rear: Vec<MeasurementSource>,
}

impl DBAConfig {
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for m in &mut self.front {
            m.resolve_paths(base_dir);
        }
        for m in &mut self.rear {
            m.resolve_paths(base_dir);
        }
    }
}

// ============================================================================
// Crossover & Target Configs
// ============================================================================

/// Crossover configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossoverConfig {
    /// Crossover type (e.g. "LR24", "LR48", "Butterworth24")
    #[serde(rename = "type")]
    pub crossover_type: String,
    /// Crossover frequency in Hz (for 2-way speakers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<f64>,
    /// Crossover frequencies in Hz (for 3-way and above)
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
    /// Predefined target (e.g. "flat", "harman")
    Predefined(String),
    /// Path to CSV file (freq, spl columns)
    Path(PathBuf),
}

/// Target curve tilt configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TargetTiltConfig {
    /// Tilt type: flat, harman, or custom
    #[serde(default)]
    pub tilt_type: TiltType,
    /// Slope in dB per octave (negative = downward tilt towards high frequencies)
    #[serde(default = "default_tilt_slope")]
    pub slope_db_per_octave: f64,
    /// Reference frequency where tilt equals 0 dB (Hz)
    #[serde(default = "default_tilt_reference_freq")]
    pub reference_freq: f64,
    /// Bass shelf boost in dB (applied below bass_shelf_freq)
    #[serde(default)]
    pub bass_shelf_db: f64,
    /// Bass shelf frequency in Hz
    #[serde(default = "default_bass_shelf_freq")]
    pub bass_shelf_freq: f64,
}

fn default_tilt_slope() -> f64 {
    -0.8
}
fn default_tilt_reference_freq() -> f64 {
    1000.0
}
fn default_bass_shelf_freq() -> f64 {
    200.0
}

impl Default for TargetTiltConfig {
    fn default() -> Self {
        Self {
            tilt_type: TiltType::Flat,
            slope_db_per_octave: 0.0,
            reference_freq: default_tilt_reference_freq(),
            bass_shelf_db: 0.0,
            bass_shelf_freq: default_bass_shelf_freq(),
        }
    }
}

/// User preference adjustments layered on top of the target shape
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserPreference {
    /// Bass shelf boost/cut in dB (applied below `bass_shelf_freq`)
    #[serde(default)]
    pub bass_shelf_db: f64,
    /// Bass shelf frequency in Hz
    #[serde(default = "default_bass_shelf_freq")]
    pub bass_shelf_freq: f64,
    /// Treble shelf boost/cut in dB (applied above `treble_shelf_freq`)
    #[serde(default)]
    pub treble_shelf_db: f64,
    /// Treble shelf frequency in Hz
    #[serde(default = "default_treble_shelf_freq")]
    pub treble_shelf_freq: f64,
}

fn default_treble_shelf_freq() -> f64 {
    8000.0
}

impl Default for UserPreference {
    fn default() -> Self {
        Self {
            bass_shelf_db: 0.0,
            bass_shelf_freq: default_bass_shelf_freq(),
            treble_shelf_db: 0.0,
            treble_shelf_freq: default_treble_shelf_freq(),
        }
    }
}

/// Unified target response configuration for room correction
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TargetResponseConfig {
    /// Target shape preset
    #[serde(default)]
    pub shape: TargetShape,
    /// Slope in dB per octave (used when shape == Custom)
    #[serde(default = "default_tilt_slope")]
    pub slope_db_per_octave: f64,
    /// Reference frequency where target shape equals 0 dB (Hz)
    #[serde(default = "default_tilt_reference_freq")]
    pub reference_freq: f64,
    /// Path to custom target curve CSV (used when shape == File)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_path: Option<std::path::PathBuf>,
    /// User preference adjustments (layered ON TOP of the target shape)
    #[serde(default)]
    pub preference: UserPreference,
    /// Enable broadband pre-correction (shelf+gain fit before fine EQ)
    #[serde(default)]
    pub broadband_precorrection: bool,
}

impl Default for TargetResponseConfig {
    fn default() -> Self {
        Self {
            shape: TargetShape::Flat,
            slope_db_per_octave: 0.0,
            reference_freq: default_tilt_reference_freq(),
            curve_path: None,
            preference: UserPreference::default(),
            broadband_precorrection: false,
        }
    }
}

// ============================================================================
// FIR & Mixed-Phase Configs
// ============================================================================

/// FIR filter configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FirConfig {
    /// Number of taps (coefficients)
    #[serde(default = "default_fir_taps")]
    pub taps: usize,
    /// Phase response type: "linear" or "kirkeby"
    #[serde(default = "default_fir_phase")]
    pub phase: String,
    /// Whether to correct excess phase (only applies to kirkeby mode)
    #[serde(default)]
    pub correct_excess_phase: bool,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    #[serde(default = "default_phase_smoothing")]
    pub phase_smoothing: f64,
    /// Pre-ringing suppression configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_ringing: Option<PreRingingSerdeConfig>,
}

/// Serializable pre-ringing configuration for JSON config files
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreRingingSerdeConfig {
    /// Maximum pre-ringing level in dB relative to main tap. Default: -30.0
    #[serde(default = "default_pre_ringing_threshold")]
    pub threshold_db: f64,
    /// Maximum pre-ringing time in seconds. Default: 0.005 (5 ms)
    #[serde(default = "default_pre_ringing_time")]
    pub max_time_s: f64,
}

fn default_pre_ringing_threshold() -> f64 {
    -30.0
}
fn default_pre_ringing_time() -> f64 {
    0.005
}
fn default_fir_taps() -> usize {
    4096
}
fn default_fir_phase() -> String {
    "kirkeby".to_string()
}
fn default_phase_smoothing() -> f64 {
    0.167
}

/// Serializable mixed-phase correction configuration for JSON config files
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixedPhaseSerdeConfig {
    /// Maximum FIR length in milliseconds for excess phase correction. Default: 10.0
    #[serde(default = "default_mixed_phase_fir_length")]
    pub max_fir_length_ms: f64,
    /// Pre-ringing threshold in dB. Default: -30.0
    #[serde(default = "default_pre_ringing_threshold")]
    pub pre_ringing_threshold_db: f64,
    /// Minimum spatial correction depth for excess phase correction. Default: 0.5
    #[serde(default = "default_mixed_phase_spatial_depth")]
    pub min_spatial_depth: f64,
    /// Phase smoothing width in octaves. Default: 1/6 octave
    #[serde(default = "default_mask_smoothing")]
    pub phase_smoothing_octaves: f64,
}

fn default_mixed_phase_fir_length() -> f64 {
    10.0
}
fn default_mixed_phase_spatial_depth() -> f64 {
    0.5
}
fn default_mask_smoothing() -> f64 {
    1.0 / 6.0
}

/// Configuration for frequency-based mixed mode crossover
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixedModeConfig {
    /// Crossover frequency dividing IIR and FIR bands (Hz)
    #[serde(default = "default_crossover_freq")]
    pub crossover_freq: f64,
    /// Crossover filter type: "LR24", "LR48"
    #[serde(default = "default_crossover_type")]
    pub crossover_type: String,
    /// Which band uses FIR: "low" or "high" (default: "low")
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

// ============================================================================
// Excursion & Schroeder Split Configs
// ============================================================================

/// Excursion protection configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExcursionProtectionConfig {
    /// Enable excursion protection
    #[serde(default)]
    pub enabled: bool,
    /// Auto-detect F3 from measurement
    #[serde(default = "default_true")]
    pub auto_detect_f3: bool,
    /// Manual F3 override in Hz (used if auto_detect_f3 is false)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_f3_hz: Option<f64>,
    /// Filter order (2 = 12dB/oct, 4 = 24dB/oct)
    #[serde(default = "default_filter_order")]
    pub filter_order: usize,
    /// Highpass filter type
    #[serde(default)]
    pub filter_type: HighpassType,
    /// Safety margin in octaves below F3 for HPF placement
    #[serde(default = "default_margin_octaves")]
    pub margin_octaves: f64,
}

fn default_true() -> bool {
    true
}
fn default_filter_order() -> usize {
    4
}
fn default_margin_octaves() -> f64 {
    0.25
}

impl Default for ExcursionProtectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_detect_f3: true,
            manual_f3_hz: None,
            filter_order: default_filter_order(),
            filter_type: HighpassType::LinkwitzRiley,
            margin_octaves: default_margin_octaves(),
        }
    }
}

/// Low frequency filter configuration for Schroeder split
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LowFreqFilterConfig {
    /// Maximum Q factor for low frequency filters (allow high-Q for modes)
    #[serde(default = "default_low_freq_max_q")]
    pub max_q: f64,
    /// Minimum Q factor
    #[serde(default = "default_min_q")]
    pub min_q: f64,
    /// Allow boost (true) or cuts only (false)
    #[serde(default)]
    pub allow_boost: bool,
    /// Maximum boost/cut in dB for below-Schroeder filters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_db: Option<f64>,
}

fn default_low_freq_max_q() -> f64 {
    5.0
}

impl Default for LowFreqFilterConfig {
    fn default() -> Self {
        Self {
            max_q: default_low_freq_max_q(),
            min_q: default_min_q(),
            allow_boost: false,
            max_db: None,
        }
    }
}

/// High frequency filter configuration for Schroeder split
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HighFreqFilterConfig {
    /// Maximum Q factor for high frequency filters (tone controls only)
    #[serde(default = "default_high_freq_max_q")]
    pub max_q: f64,
    /// Use shelving filters only
    #[serde(default)]
    pub shelving_only: bool,
}

fn default_high_freq_max_q() -> f64 {
    1.0
}

impl Default for HighFreqFilterConfig {
    fn default() -> Self {
        Self {
            max_q: default_high_freq_max_q(),
            shelving_only: false,
        }
    }
}

/// Room dimensions for automatic Schroeder frequency calculation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoomDimensions {
    /// Length in meters
    pub length: f64,
    /// Width in meters
    pub width: f64,
    /// Height in meters
    pub height: f64,
}

/// Default RT60 (seconds) used when computing the Schroeder frequency
/// from room dimensions without a measured reverberation time. 0.4 s
/// is representative of a typical small, moderately-furnished
/// listening room (carpet or rug, sofa, bookshelves). Rooms with a
/// very different character (bare-floor, untreated, or heavily
/// treated) should supply their own RT60 via
/// [`RoomDimensions::schroeder_frequency_with_rt60`].
pub const DEFAULT_LISTENING_ROOM_RT60_S: f64 = 0.4;

impl RoomDimensions {
    /// Calculate the Schroeder frequency from room dimensions using a
    /// default RT60 assumption of [`DEFAULT_LISTENING_ROOM_RT60_S`].
    ///
    /// See [`Self::schroeder_frequency_with_rt60`] for the underlying
    /// formula and the meaning of the Schroeder frequency. The previous
    /// implementation of this function used `11885 / √V`, which is
    /// equivalent to the correct formula `2000 · √(RT60 / V)` with an
    /// implicit RT60 of ~35 s — a value appropriate to a cathedral,
    /// not a listening room. That bug inflated the computed Schroeder
    /// frequency by roughly an order of magnitude for every small-room
    /// caller.
    pub fn schroeder_frequency(&self) -> f64 {
        self.schroeder_frequency_with_rt60(DEFAULT_LISTENING_ROOM_RT60_S)
    }

    /// Calculate the Schroeder frequency from room dimensions and a
    /// known RT60 (reverberation time to −60 dB, in seconds).
    ///
    /// Uses Schroeder's engineering formula
    /// `f_S ≈ 2000 · √(RT60 / V)` where V is the room volume in m³
    /// and the result is in Hz. This is the canonical crossover
    /// between the modal region (discrete resonances, where narrow EQ
    /// cuts are effective and boosts cannot fill nulls) and the
    /// diffuse region (statistical mode overlap, where broadband
    /// correction works).
    pub fn schroeder_frequency_with_rt60(&self, rt60_seconds: f64) -> f64 {
        let volume = self.length * self.width * self.height;
        if volume <= 0.0 || rt60_seconds <= 0.0 {
            return 0.0;
        }
        2000.0 * (rt60_seconds / volume).sqrt()
    }
}

/// Schroeder frequency split configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchroederSplitConfig {
    /// Enable Schroeder split optimization
    #[serde(default)]
    pub enabled: bool,
    /// Schroeder frequency in Hz
    #[serde(default = "default_schroeder_freq")]
    pub schroeder_freq: f64,
    /// Room dimensions for auto-calculation (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_dimensions: Option<RoomDimensions>,
    /// Low frequency filter configuration (below Schroeder)
    #[serde(default)]
    pub low_freq_config: LowFreqFilterConfig,
    /// High frequency filter configuration (above Schroeder)
    #[serde(default)]
    pub high_freq_config: HighFreqFilterConfig,
}

fn default_schroeder_freq() -> f64 {
    300.0
}

impl Default for SchroederSplitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schroeder_freq: default_schroeder_freq(),
            room_dimensions: None,
            low_freq_config: LowFreqFilterConfig::default(),
            high_freq_config: HighFreqFilterConfig::default(),
        }
    }
}

// ============================================================================
// Phase, Multi-Seat, Channel Matching Configs
// ============================================================================

/// Phase alignment configuration for subwoofer integration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PhaseAlignmentConfig {
    /// Enable phase alignment optimization
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Minimum frequency for optimization (Hz)
    #[serde(default = "default_phase_min_freq")]
    pub min_freq: f64,
    /// Maximum frequency for optimization (Hz)
    #[serde(default = "default_phase_max_freq")]
    pub max_freq: f64,
    /// Optimize polarity (normal vs inverted)
    #[serde(default = "default_true")]
    pub optimize_polarity: bool,
    /// Maximum delay in milliseconds
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: f64,
}

fn default_phase_min_freq() -> f64 {
    60.0
}
fn default_phase_max_freq() -> f64 {
    100.0
}
fn default_max_delay_ms() -> f64 {
    3.0
}

impl Default for PhaseAlignmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_freq: default_phase_min_freq(),
            max_freq: default_phase_max_freq(),
            optimize_polarity: true,
            max_delay_ms: default_max_delay_ms(),
        }
    }
}

/// Multi-seat measurement configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatMeasurement {
    /// Name of this multi-seat configuration
    pub name: String,
    /// Measurements at each seat position
    pub seat_measurements: Vec<MeasurementSource>,
}

/// Multi-seat optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatConfig {
    /// Enable multi-seat optimization
    #[serde(default)]
    pub enabled: bool,
    /// Optimization strategy
    #[serde(default)]
    pub strategy: MultiSeatStrategy,
    /// Index of primary seat (0-based, used with PrimaryWithConstraints strategy)
    #[serde(default)]
    pub primary_seat: usize,
    /// Maximum allowed deviation at non-primary seats (dB)
    #[serde(default = "default_max_deviation_db")]
    pub max_deviation_db: f64,
}

fn default_max_deviation_db() -> f64 {
    6.0
}

impl Default for MultiSeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: MultiSeatStrategy::MinimizeVariance,
            primary_seat: 0,
            max_deviation_db: default_max_deviation_db(),
        }
    }
}

/// Inter-channel consistency correction configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelMatchingConfig {
    /// Enable inter-channel matching correction
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// ICD RMS threshold in dB below which no correction is applied
    #[serde(default = "default_channel_matching_threshold")]
    pub threshold_db: f64,
    /// Maximum number of additional PEQ filters per channel for matching
    #[serde(default = "default_channel_matching_max_filters")]
    pub max_filters: usize,
}

fn default_channel_matching_threshold() -> f64 {
    0.75
}
fn default_channel_matching_max_filters() -> usize {
    5
}

impl Default for ChannelMatchingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: default_channel_matching_threshold(),
            max_filters: default_channel_matching_max_filters(),
        }
    }
}

/// Subwoofer-specific optimizer overrides
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubOptimizerConfig {
    /// Number of PEQ filters for subwoofer channels
    #[serde(default = "default_sub_num_filters")]
    pub num_filters: usize,
    /// Maximum boost in dB (room gain can be 15+ dB at resonances)
    #[serde(default = "default_sub_max_db")]
    pub max_db: f64,
    /// Maximum cut in dB
    #[serde(default = "default_sub_min_db")]
    pub min_db: f64,
    /// Minimum Q factor
    #[serde(default = "default_min_q")]
    pub min_q: f64,
    /// Maximum Q factor (higher Q for narrow room modes)
    #[serde(default = "default_sub_max_q")]
    pub max_q: f64,
}

fn default_sub_num_filters() -> usize {
    10
}
fn default_sub_max_db() -> f64 {
    18.0
}
fn default_sub_min_db() -> f64 {
    -18.0
}
fn default_sub_max_q() -> f64 {
    10.0
}

impl Default for SubOptimizerConfig {
    fn default() -> Self {
        Self {
            num_filters: default_sub_num_filters(),
            max_db: default_sub_max_db(),
            min_db: default_sub_min_db(),
            min_q: default_min_q(),
            max_q: default_sub_max_q(),
        }
    }
}

// ============================================================================
// Measurement & Deviation Types
// ============================================================================

/// Measurement of inter-channel SPL consistency after optimization
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InterChannelDeviation {
    /// Per-frequency max deviation (freq_hz, spread_db)
    pub deviation_per_freq: Vec<(f64, f64)>,
    /// RMS of deviation in the midrange (200-4000 Hz)
    pub midrange_rms_db: f64,
    /// RMS of deviation from F3 to 10 kHz
    pub passband_rms_db: f64,
    /// Maximum single-point deviation in midrange
    pub midrange_peak_db: f64,
    /// Frequency of maximum midrange deviation
    pub midrange_peak_freq: f64,
}

// ============================================================================
// Additional Configs (Broadband, Multi-Measurement, Spatial, Decomposed, CEA2034)
// ============================================================================

/// Configuration for broadband target matching
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BroadbandTargetMatchingConfig {
    /// Enable broadband target matching
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for BroadbandTargetMatchingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Serializable spatial robustness configuration for JSON config files
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpatialRobustnessSerdeConfig {
    /// Variance threshold (dB) below which full correction is allowed. Default: 3.0
    #[serde(default = "default_variance_threshold")]
    pub variance_threshold_db: f64,
    /// Transition width (dB) for sigmoid blending. Default: 2.0
    #[serde(default = "default_transition_width")]
    pub transition_width_db: f64,
    /// Minimum correction depth (0.0-1.0). Default: 0.1
    #[serde(default = "default_min_correction_depth")]
    pub min_correction_depth: f64,
    /// Smoothing width in octaves for the correction depth mask. Default: 1/6 octave.
    #[serde(default = "default_mask_smoothing_octaves")]
    pub mask_smoothing_octaves: f64,
}

fn default_variance_threshold() -> f64 {
    3.0
}
fn default_transition_width() -> f64 {
    2.0
}
fn default_min_correction_depth() -> f64 {
    0.1
}
fn default_mask_smoothing_octaves() -> f64 {
    1.0 / 6.0
}

/// Configuration for multi-measurement optimization
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiMeasurementConfig {
    /// Strategy for combining per-measurement losses
    #[serde(default)]
    pub strategy: MultiMeasurementStrategy,
    /// Weights for WeightedSum (normalized internally). Equal if omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<f64>>,
    /// Lambda for VariancePenalized (default 1.0). Higher = more consistent across positions.
    #[serde(default = "default_variance_lambda")]
    pub variance_lambda: f64,
    /// Spatial robustness configuration (used when strategy = SpatialRobustness)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_robustness: Option<SpatialRobustnessSerdeConfig>,
}

fn default_variance_lambda() -> f64 {
    1.0
}

impl Default for MultiMeasurementConfig {
    fn default() -> Self {
        Self {
            strategy: MultiMeasurementStrategy::default(),
            weights: None,
            variance_lambda: default_variance_lambda(),
            spatial_robustness: None,
        }
    }
}

/// Serializable decomposed correction configuration for JSON config files
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecomposedCorrectionSerdeConfig {
    /// Whether decomposed correction is enabled. Default: true
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Schroeder frequency (Hz). Below: modal, above: statistical.
    ///
    /// When `room_dimensions` is also provided AND an impulse response is
    /// available, this value is overridden at run time by a
    /// measurement-driven Schroeder frequency: the optimizer measures
    /// RT60 from the IR via Schroeder backward integration and plugs it
    /// into `f_S ≈ 2000 · √(RT60 / V)` with V from `room_dimensions`. In
    /// that case this field is used only as the fallback if the RT60 fit
    /// fails.
    #[serde(default = "default_decomposed_schroeder")]
    pub schroeder_freq: f64,
    /// Room dimensions (L × W × H in metres). When present together with
    /// a measured impulse response, enables a measurement-driven
    /// Schroeder frequency via `RoomDimensions::schroeder_frequency_with_rt60`
    /// using the RT60 measured from the IR. When absent, the optimizer
    /// falls back to the `schroeder_freq` field above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_dimensions: Option<RoomDimensions>,
    /// Minimum Q to qualify as a room mode. Default: 3.0
    #[serde(default = "default_decomposed_min_q")]
    pub min_mode_q: f64,
    /// Minimum prominence (dB) for mode detection. Default: 3.0
    #[serde(default = "default_decomposed_prominence")]
    pub min_mode_prominence_db: f64,
    /// Correction weight for detected room modes (0.0-1.0). Default: 1.0
    #[serde(default = "default_decomposed_mode_weight")]
    pub mode_correction_weight: f64,
    /// Correction weight for early reflections (0.0-1.0). Default: 0.3
    #[serde(default = "default_decomposed_reflection_weight")]
    pub early_reflection_weight: f64,
    /// Correction weight for steady-state above Schroeder (0.0-1.0). Default: 0.4
    #[serde(default = "default_decomposed_steady_weight")]
    pub steady_state_weight: f64,
}

fn default_decomposed_schroeder() -> f64 {
    250.0
}
fn default_decomposed_min_q() -> f64 {
    3.0
}
fn default_decomposed_prominence() -> f64 {
    3.0
}
fn default_decomposed_mode_weight() -> f64 {
    1.0
}
fn default_decomposed_reflection_weight() -> f64 {
    0.3
}
fn default_decomposed_steady_weight() -> f64 {
    0.4
}

impl Default for DecomposedCorrectionSerdeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            schroeder_freq: default_decomposed_schroeder(),
            room_dimensions: None,
            min_mode_q: default_decomposed_min_q(),
            min_mode_prominence_db: default_decomposed_prominence(),
            mode_correction_weight: default_decomposed_mode_weight(),
            early_reflection_weight: default_decomposed_reflection_weight(),
            steady_state_weight: default_decomposed_steady_weight(),
        }
    }
}

/// CEA2034 speaker pre-correction configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Cea2034CorrectionConfig {
    /// Enable CEA2034 speaker pre-correction
    #[serde(default)]
    pub enabled: bool,
    /// Speaker name on spinorama.org (overrides speaker_name from MeasurementSource)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// Measurement version on spinorama.org (default: "asr")
    #[serde(default = "default_cea2034_version")]
    pub version: String,
    /// Correction mode: flat (nearfield), score (farfield), auto (distance-based)
    #[serde(default)]
    pub correction_mode: Cea2034CorrectionMode,
    /// Manual listening distance override in meters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening_distance_m: Option<f64>,
    /// System round-trip latency in ms (for distance computation from impulse response)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_latency_ms: Option<f64>,
    /// Distance threshold in meters for auto mode switch (default: 2.0m)
    #[serde(default = "default_nearfield_threshold")]
    pub nearfield_threshold_m: f64,
    /// Override minimum correction frequency in Hz (Schroeder frequency)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_freq: Option<f64>,
    /// Number of PEQ filters for speaker correction (default: 5)
    #[serde(default = "default_cea2034_num_filters")]
    pub num_filters: usize,
    /// Maximum Q factor (default: 3.0)
    #[serde(default = "default_cea2034_max_q")]
    pub max_q: f64,
    /// Maximum boost in dB (default: 3.0)
    #[serde(default = "default_cea2034_max_db")]
    pub max_db: f64,
    /// Minimum gain in dB (default: -12.0)
    #[serde(default = "default_cea2034_min_db")]
    pub min_db: f64,
}

fn default_cea2034_version() -> String {
    "asr".to_string()
}
fn default_nearfield_threshold() -> f64 {
    2.0
}
fn default_cea2034_num_filters() -> usize {
    5
}
fn default_cea2034_max_q() -> f64 {
    3.0
}
fn default_cea2034_max_db() -> f64 {
    3.0
}
fn default_cea2034_min_db() -> f64 {
    -12.0
}

impl Default for Cea2034CorrectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            speaker_name: None,
            version: default_cea2034_version(),
            correction_mode: Cea2034CorrectionMode::default(),
            listening_distance_m: None,
            system_latency_ms: None,
            nearfield_threshold_m: default_nearfield_threshold(),
            min_freq: None,
            num_filters: default_cea2034_num_filters(),
            max_q: default_cea2034_max_q(),
            max_db: default_cea2034_max_db(),
            min_db: default_cea2034_min_db(),
        }
    }
}

// ============================================================================
// Configuration for Voice of God
// ============================================================================

/// Configuration for Voice of God (Timbre Matching)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VoiceOfGodConfig {
    /// Enable Voice of God optimization
    #[serde(default)]
    pub enabled: bool,
    /// Reference channel name (e.g. "Center" or "Left")
    pub reference_channel: String,
}

// ============================================================================
// Configuration for Group Delay Optimization (GD-Opt)
// ============================================================================

/// Configuration for Group Delay Optimization (GD-Opt)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupDelayOptimizationConfig {
    /// Enable Group Delay Optimization
    #[serde(default)]
    pub enabled: bool,
    /// Target group delay at crossover (ms)
    #[serde(default)]
    pub target_ms: f64,
}

impl Default for GroupDelayOptimizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target_ms: 0.0,
        }
    }
}

// ============================================================================
// Main OptimizerConfig
// ============================================================================

/// Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OptimizerConfig {
    /// Optimization mode: "iir" (default), "fir", "mixed"
    #[serde(default = "default_opt_mode")]
    pub mode: String,
    /// Processing mode for RoomEQ v2
    #[serde(default)]
    pub processing_mode: ProcessingMode,
    /// FIR configuration (if mode is "fir" or "mixed")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fir: Option<FirConfig>,
    /// Mixed mode configuration (frequency-based crossover)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixed_config: Option<MixedModeConfig>,
    /// Mixed-phase correction configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixed_phase: Option<MixedPhaseSerdeConfig>,
    /// Standalone phase correction (rePhase-style)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_correction: Option<MixedPhaseSerdeConfig>,
    /// Loss function type. Supported values:
    /// - `"flat"` — minimize deviation from target (default)
    /// - `"score"` — maximize Harman/Olive preference score
    /// - `"epa"` — EPA (Evaluation/Potency/Activity) psychoacoustic
    ///   loss combining spectral flatness with sharpness, roughness,
    ///   and loudness-balance penalties derived from Zwicker metrics.
    ///   When selected, the EPA penalty weights can be customized via
    ///   the [`epa_config`](Self::epa_config) field; otherwise the
    ///   defaults from [`EpaConfig::default`](crate::loss::epa::score::EpaConfig::default)
    ///   are used.
    #[serde(default = "default_loss_type")]
    pub loss_type: String,
    /// EPA loss configuration. Only used when `loss_type == "epa"`.
    /// When `None`, the optimizer falls back to
    /// [`EpaConfig::default`](crate::loss::epa::score::EpaConfig::default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epa_config: Option<crate::loss::epa::score::EpaConfig>,
    /// Optimization algorithm
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    /// DE mutation strategy (e.g. "currenttobest1bin", "lshade", "best1bin")
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Maximum number of PEQ filters per channel
    #[serde(default = "default_num_filters")]
    pub num_filters: usize,
    /// Minimum loss improvement to justify adding another filter
    #[serde(default = "default_min_filter_improvement")]
    pub min_filter_improvement: f64,
    /// Backward elimination threshold
    #[serde(default = "default_elimination_threshold")]
    pub elimination_threshold: f64,
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
    /// PEQ model (e.g. "pk", "ls-pk-hs", "free")
    #[serde(default = "default_peq_model")]
    pub peq_model: String,
    /// Random seed for reproducible results (None for random)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Whether to run local refinement after global optimization
    #[serde(default = "default_refine")]
    pub refine: bool,
    /// Local optimizer algorithm for refinement stage
    #[serde(default = "default_local_algo")]
    pub local_algo: String,
    /// Enable psychoacoustic preprocessing
    #[serde(default = "default_psychoacoustic")]
    pub psychoacoustic: bool,
    /// Loss function smoothing resolution as 1/N octave
    #[serde(default = "default_smooth_n")]
    pub smooth_n: usize,
    /// Enable asymmetric loss (peaks penalized 2x more than dips)
    #[serde(default = "default_asymmetric_loss")]
    pub asymmetric_loss: bool,
    /// Optimization convergence tolerance (relative)
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
    /// Optimization convergence tolerance (absolute)
    #[serde(default = "default_atolerance")]
    pub atolerance: f64,
    /// Allow inter-speaker delay optimization
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_delay: Option<bool>,
    /// Unified target response configuration (preferred over legacy target_tilt + broadband)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_response: Option<TargetResponseConfig>,
    /// Legacy target curve tilt configuration — migrated to `target_response` at load time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_tilt: Option<TargetTiltConfig>,
    /// Excursion protection configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excursion_protection: Option<ExcursionProtectionConfig>,
    /// Schroeder frequency split configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schroeder_split: Option<SchroederSplitConfig>,
    /// Phase alignment configuration for subwoofer integration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_alignment: Option<PhaseAlignmentConfig>,
    /// Multi-seat optimization configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_seat: Option<MultiSeatConfig>,
    /// Group Delay Optimization configuration (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gd_opt: Option<GroupDelayOptimizationConfig>,
    /// Voice of God optimization configuration (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vog: Option<VoiceOfGodConfig>,
    /// Broadband target matching configuration (v2.1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadband_target_matching: Option<BroadbandTargetMatchingConfig>,
    /// Multi-measurement optimization configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_measurement: Option<MultiMeasurementConfig>,
    /// Decomposed correction configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decomposed_correction: Option<DecomposedCorrectionSerdeConfig>,
    /// CEA2034 speaker pre-correction configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cea2034_correction: Option<Cea2034CorrectionConfig>,
    /// Subwoofer-specific optimizer overrides
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_config: Option<SubOptimizerConfig>,
    /// Inter-channel consistency correction configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_matching: Option<ChannelMatchingConfig>,
    /// Runtime-only: path to a measured room impulse response WAV file
    #[serde(skip)]
    pub ssir_wav_path: Option<std::path::PathBuf>,
    /// Frequency-dependent maximum boost envelope.
    /// Each entry is (frequency_hz, max_boost_db).
    /// Between points, linear interpolation in log-frequency.
    /// Default: None (use the existing flat `max_db` limit).
    /// When set, overrides `max_db` on a per-frequency basis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_boost_envelope: Option<Vec<(f64, f64)>>,

    /// CDT-aware minimum cut envelope: limits how deep the optimizer can cut
    /// at frequencies where the ear generates Cubic Distortion Tones.
    /// Each entry is (frequency_hz, max_cut_db) where max_cut_db is negative.
    /// Default: None (no CDT protection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cut_envelope: Option<Vec<(f64, f64)>>,
}

// Default values for OptimizerConfig
fn default_loss_type() -> String {
    "flat".to_string()
}
fn default_algorithm() -> String {
    "autoeq:de".to_string()
}
fn default_strategy() -> String {
    "lshade".to_string()
}
fn default_peq_model() -> String {
    "pk".to_string()
}
fn default_opt_mode() -> String {
    "iir".to_string()
}
fn default_num_filters() -> usize {
    7
}
fn default_min_filter_improvement() -> f64 {
    0.01
}
fn default_elimination_threshold() -> f64 {
    0.005
}
fn default_min_q() -> f64 {
    0.5
}
fn default_max_q() -> f64 {
    3.0
}
fn default_min_db() -> f64 {
    -12.0
}
fn default_max_db() -> f64 {
    4.0
}
fn default_min_freq() -> f64 {
    20.0
}
fn default_max_freq() -> f64 {
    1600.0
}
fn default_max_iter() -> usize {
    50000
}
fn default_population() -> usize {
    300
}
fn default_refine() -> bool {
    true
}
fn default_local_algo() -> String {
    "cobyla".to_string()
}
fn default_psychoacoustic() -> bool {
    true
}
fn default_smooth_n() -> usize {
    2
}
fn default_asymmetric_loss() -> bool {
    true
}
fn default_tolerance() -> f64 {
    1e-5
}
fn default_atolerance() -> f64 {
    1e-5
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            loss_type: default_loss_type(),
            algorithm: default_algorithm(),
            strategy: default_strategy(),
            num_filters: default_num_filters(),
            min_filter_improvement: default_min_filter_improvement(),
            elimination_threshold: default_elimination_threshold(),
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
            processing_mode: ProcessingMode::LowLatency,
            fir: None,
            mixed_config: None,
            mixed_phase: None,
            phase_correction: None,
            seed: None,
            refine: default_refine(),
            local_algo: default_local_algo(),
            psychoacoustic: default_psychoacoustic(),
            smooth_n: default_smooth_n(),
            asymmetric_loss: default_asymmetric_loss(),
            tolerance: default_tolerance(),
            atolerance: default_atolerance(),
            allow_delay: None,
            target_response: None,
            target_tilt: None,
            excursion_protection: None,
            schroeder_split: None,
            phase_alignment: None,
            multi_seat: None,
            gd_opt: None,
            vog: None,
            broadband_target_matching: None,
            multi_measurement: None,
            decomposed_correction: Some(DecomposedCorrectionSerdeConfig {
                enabled: true,
                ..Default::default()
            }),
            cea2034_correction: None,
            sub_config: None,
            channel_matching: None,
            ssir_wav_path: None,
            max_boost_envelope: None,
            min_cut_envelope: None,
            epa_config: None,
        }
    }
}

impl OptimizerConfig {
    /// Resolve the effective `allow_delay` value based on the mode
    pub fn allow_delay(&self) -> bool {
        self.allow_delay.unwrap_or(self.mode != "iir")
    }

    /// Get the maximum allowed boost at a given frequency.
    /// If `max_boost_envelope` is set, interpolate it in log-frequency space.
    /// Otherwise fall back to `self.max_db`.
    pub fn max_boost_at_freq(&self, freq_hz: f64) -> f64 {
        let envelope = match &self.max_boost_envelope {
            Some(env) if !env.is_empty() => env,
            _ => return self.max_db,
        };

        if freq_hz <= envelope[0].0 {
            return envelope[0].1;
        }
        let last = envelope.len() - 1;
        if freq_hz >= envelope[last].0 {
            return envelope[last].1;
        }

        for i in 0..last {
            let (f0, db0) = envelope[i];
            let (f1, db1) = envelope[i + 1];
            if freq_hz >= f0 && freq_hz <= f1 {
                let t = (freq_hz.ln() - f0.ln()) / (f1.ln() - f0.ln());
                return db0 + t * (db1 - db0);
            }
        }

        self.max_db
    }

    /// Migrate legacy `target_tilt` + `broadband_target_matching` into `target_response`
    pub fn migrate_target_config(&mut self) {
        if self.target_response.is_some() {
            if self.target_tilt.is_some() {
                log::warn!(
                    "Both target_response and target_tilt are set; target_tilt is ignored. Use target_response exclusively."
                );
            }
            if self
                .broadband_target_matching
                .as_ref()
                .is_some_and(|b| b.enabled)
            {
                log::warn!(
                    "Both target_response and broadband_target_matching are set; broadband_target_matching is ignored. Set target_response.broadband_precorrection instead."
                );
            }
            self.target_tilt = None;
            self.broadband_target_matching = None;
            return;
        }

        if self.target_tilt.is_none() && self.broadband_target_matching.is_none() {
            return;
        }

        let tilt = self.target_tilt.take();
        let bb = self.broadband_target_matching.take();

        let (shape, slope) = match tilt.as_ref() {
            Some(t) if t.tilt_type == TiltType::Harman => (TargetShape::Harman, -0.8),
            Some(t) if t.tilt_type == TiltType::Custom => {
                (TargetShape::Custom, t.slope_db_per_octave)
            }
            Some(t)
                if t.tilt_type == TiltType::Flat
                    && (t.slope_db_per_octave.abs() > 1e-6 || t.bass_shelf_db.abs() > 1e-6) =>
            {
                (TargetShape::Custom, t.slope_db_per_octave)
            }
            _ => (TargetShape::Flat, 0.0),
        };

        self.target_response = Some(TargetResponseConfig {
            shape,
            slope_db_per_octave: slope,
            reference_freq: tilt.as_ref().map(|t| t.reference_freq).unwrap_or(1000.0),
            curve_path: None,
            preference: UserPreference {
                bass_shelf_db: tilt.as_ref().map(|t| t.bass_shelf_db).unwrap_or(0.0),
                bass_shelf_freq: tilt.as_ref().map(|t| t.bass_shelf_freq).unwrap_or(200.0),
                treble_shelf_db: 0.0,
                treble_shelf_freq: 8000.0,
            },
            broadband_precorrection: bb.as_ref().map(|b| b.enabled).unwrap_or(false),
        });
    }
}

// ============================================================================
// RoomConfig
// ============================================================================

/// Complete room configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoomConfig {
    /// Configuration version (semantic versioning, e.g. "1.0.0")
    #[serde(default = "default_config_version")]
    pub version: String,
    /// System configuration (v2.1) - Decouples logical roles from measurements
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemConfig>,
    /// Map of channel name to speaker configuration
    pub speakers: HashMap<String, SpeakerConfig>,
    /// Optional crossover configuration for multi-driver groups
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossovers: Option<HashMap<String, CrossoverConfig>>,
    /// Optional target curve (freq, spl)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_curve: Option<TargetCurveConfig>,
    /// Optimizer configuration
    #[serde(default)]
    pub optimizer: OptimizerConfig,
    /// Recording configuration (device settings, signal parameters used during capture)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_config: Option<RecordingConfiguration>,
    /// Pre-fetched CEA2034 data (runtime only, not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub cea2034_cache: Option<HashMap<String, crate::read::Cea2034Data>>,
}

impl RoomConfig {
    /// Resolve relative paths in this room configuration against a base directory
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for speaker in self.speakers.values_mut() {
            speaker.resolve_paths(base_dir);
        }
        if let Some(TargetCurveConfig::Path(ref mut path)) = self.target_curve
            && path.is_relative()
        {
            *path = base_dir.join(&*path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_config_default_has_decomposed_correction_enabled() {
        let config = OptimizerConfig::default();
        let dc = config
            .decomposed_correction
            .expect("decomposed_correction should be Some by default");
        assert!(
            dc.enabled,
            "decomposed_correction should be enabled by default"
        );
        assert_eq!(dc.schroeder_freq, 250.0);
        assert_eq!(dc.steady_state_weight, 0.4);
    }

    #[test]
    fn test_decomposed_correction_serde_config_default() {
        let dc = DecomposedCorrectionSerdeConfig::default();
        assert!(dc.enabled);
        assert_eq!(dc.schroeder_freq, 250.0);
        assert_eq!(dc.steady_state_weight, 0.4);
        assert_eq!(dc.min_mode_q, 3.0);
        assert_eq!(dc.min_mode_prominence_db, 3.0);
        assert_eq!(dc.mode_correction_weight, 1.0);
        assert_eq!(dc.early_reflection_weight, 0.3);
    }

    #[test]
    fn test_channel_matching_config_defaults() {
        let cfg = ChannelMatchingConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.threshold_db, 0.75);
        assert_eq!(cfg.max_filters, 5);
    }

    #[test]
    fn test_max_boost_envelope_interpolation() {
        let mut config = OptimizerConfig::default();

        // Without envelope, falls back to max_db
        assert_eq!(config.max_boost_at_freq(100.0), config.max_db);

        // Set an envelope: generous bass boost tapering to zero
        config.max_boost_envelope = Some(vec![
            (20.0, 6.0),
            (200.0, 4.0),
            (1000.0, 2.0),
            (8000.0, 0.0),
        ]);

        // At exact envelope points
        assert!((config.max_boost_at_freq(20.0) - 6.0).abs() < 1e-10);
        assert!((config.max_boost_at_freq(200.0) - 4.0).abs() < 1e-10);
        assert!((config.max_boost_at_freq(1000.0) - 2.0).abs() < 1e-10);
        assert!((config.max_boost_at_freq(8000.0) - 0.0).abs() < 1e-10);

        // Below first point: clamp to first value
        assert!((config.max_boost_at_freq(10.0) - 6.0).abs() < 1e-10);

        // Above last point: clamp to last value
        assert!((config.max_boost_at_freq(16000.0) - 0.0).abs() < 1e-10);

        // Between 200Hz and 1000Hz: log-frequency interpolation
        // Geometric midpoint of 200 and 1000 is sqrt(200*1000) ~ 447Hz
        let mid_freq = (200.0_f64 * 1000.0).sqrt();
        let mid_boost = config.max_boost_at_freq(mid_freq);
        // At geometric midpoint, t = 0.5, so interpolated value = 4.0 + 0.5*(2.0-4.0) = 3.0
        assert!(
            (mid_boost - 3.0).abs() < 1e-6,
            "geometric midpoint should give 3.0 dB, got {:.6}",
            mid_boost
        );
    }
}
