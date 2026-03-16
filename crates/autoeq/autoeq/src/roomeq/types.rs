//! Room EQ - Multi-channel room equalization optimizer
//!
//! Copyright (C) 2025-2026 Pierre Aubert pierre(at)spinorama(dot)org
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

use crate::Curve;
pub use crate::{MeasurementSingle, MeasurementSource};
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
    /// Phase in degrees (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<Vec<f64>>,
    /// Optional frequency range used for normalization
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub norm_range: Option<(f64, f64)>,
}

impl From<Curve> for CurveData {
    fn from(curve: Curve) -> Self {
        CurveData {
            freq: curve.freq.to_vec(),
            spl: curve.spl.to_vec(),
            phase: curve.phase.map(|p| p.to_vec()),
            norm_range: None,
        }
    }
}

impl From<&Curve> for CurveData {
    fn from(curve: &Curve) -> Self {
        CurveData {
            freq: curve.freq.to_vec(),
            spl: curve.spl.to_vec(),
            phase: curve.phase.as_ref().map(|p| p.to_vec()),
            norm_range: None,
        }
    }
}

impl From<CurveData> for Curve {
    fn from(data: CurveData) -> Self {
        Curve {
            freq: ndarray::Array1::from(data.freq),
            spl: ndarray::Array1::from(data.spl),
            phase: data.phase.map(ndarray::Array1::from),
        }
    }
}

// ============================================================================
// Configuration Data Structures
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
    /// Per-channel microphone calibration file paths
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_calibration_paths: Option<Vec<Option<String>>>,
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

// ============================================================================
// RoomEQ v2 Configuration
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

/// Configuration for Group Delay Optimization (GD-Opt)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupDelayOptimizationConfig {
    /// Enable Group Delay Optimization
    #[serde(default)]
    pub enabled: bool,

    /// Target group delay at crossover (ms)
    /// Default: 0.0 ms (perfect alignment)
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

/// Configuration for Voice of God (Timbre Matching)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VoiceOfGodConfig {
    /// Enable Voice of God optimization
    #[serde(default)]
    pub enabled: bool,

    /// Reference channel name (e.g., "Center" or "Left")
    pub reference_channel: String,
}

// ============================================================================
// System Configuration (v2.1 Refactor)
// ============================================================================

/// System topology model
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SystemModel {
    Stereo,
    HomeCinema,
    #[default]
    Custom,
}

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
    /// Key: Subwoofer measurement name (e.g., "sub0")
    /// Value: Logical main channel role to align with (e.g., "L")
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
    /// Key: Logical role (e.g., "L", "R", "C", "LFE")
    /// Value: Key in the `speakers` measurement map (e.g., "left", "right")
    pub speakers: HashMap<String, String>,

    /// Subwoofer configuration and mapping
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subwoofers: Option<SubwooferSystemConfig>,
}

/// Complete room configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoomConfig {
    /// Configuration version (semantic versioning, e.g., "1.0.0")
    #[serde(default = "default_config_version")]
    pub version: String,

    /// System configuration (v2.1) - Decouples logical roles from measurements
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemConfig>,

    /// Map of channel name to speaker configuration
    /// In v2.1 with `system` config, keys here are "measurement keys" referenced by `system.speakers`.
    /// In legacy mode, keys are logical channel names.
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
        if let Some(TargetCurveConfig::Path(ref mut path)) = self.target_curve
            && path.is_relative()
        {
            *path = base_dir.join(&*path);
        }
    }
}

/// Default configuration version
pub fn default_config_version() -> String {
    "1.3.0".to_string()
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
    /// Returns the optional speaker name associated with this configuration
    pub fn speaker_name(&self) -> Option<&str> {
        match self {
            SpeakerConfig::Single(source) => source.speaker_name(),
            SpeakerConfig::Group(group) => group.speaker_name.as_deref(),
            SpeakerConfig::MultiSub(ms) => ms.speaker_name.as_deref(),
            SpeakerConfig::Dba(dba) => dba.speaker_name.as_deref(),
            SpeakerConfig::Cardioid(c) => c.speaker_name.as_deref(),
        }
    }

    /// Resolve relative paths in this speaker configuration against a base directory.
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

    /// Optional speaker model name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,

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
    /// Resolve relative paths in this cardioid config against a base directory.
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
    /// Predefined target (e.g., "flat", "harman")
    Predefined(String),

    /// Path to CSV file (freq, spl columns)
    Path(PathBuf),
}

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
    /// When true, corrects both magnitude and excess phase (requires clean phase measurements).
    /// When false (default), only corrects magnitude (produces linear-phase FIR, more robust).
    #[serde(default)]
    pub correct_excess_phase: bool,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    /// Applied via group delay smoothing when excess phase correction is enabled.
    /// Smoothing reduces noise artifacts in phase measurements.
    /// Set to 0.0 to disable smoothing.
    #[serde(default = "default_phase_smoothing")]
    pub phase_smoothing: f64,
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

// ============================================================================
// Target Curve Configuration
// ============================================================================

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

/// Target curve tilt configuration
///
/// Applies a frequency-dependent tilt to the target curve instead of flat.
/// Harman-style tilt (-0.8 dB/octave) is psychoacoustically preferred for in-room listening.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TargetTiltConfig {
    /// Tilt type: flat, harman, or custom
    #[serde(default)]
    pub tilt_type: TiltType,

    /// Slope in dB per octave (negative = downward tilt towards high frequencies)
    /// Default: -0.8 for Harman-style tilt
    #[serde(default = "default_tilt_slope")]
    pub slope_db_per_octave: f64,

    /// Reference frequency where tilt equals 0 dB (Hz)
    /// Default: 1000.0 Hz
    #[serde(default = "default_tilt_reference_freq")]
    pub reference_freq: f64,

    /// Bass shelf boost in dB (applied below bass_shelf_freq)
    /// Default: 0.0 (no additional bass boost)
    #[serde(default)]
    pub bass_shelf_db: f64,

    /// Bass shelf frequency in Hz
    /// Default: 200.0 Hz
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
            slope_db_per_octave: default_tilt_slope(),
            reference_freq: default_tilt_reference_freq(),
            bass_shelf_db: 0.0,
            bass_shelf_freq: default_bass_shelf_freq(),
        }
    }
}

// ============================================================================
// Excursion Protection Configuration
// ============================================================================

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

/// Excursion protection configuration
///
/// Detects the speaker's F3 rolloff point and automatically generates a highpass filter
/// to prevent dangerous over-boost of bass frequencies on bookshelf speakers.
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
    /// Default: 0.25 (HPF placed at F3 * 2^(-0.25))
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

// ============================================================================
// Schroeder Split Configuration
// ============================================================================

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
    /// Default: false (cuts only for low frequencies)
    #[serde(default)]
    pub allow_boost: bool,
}

fn default_low_freq_max_q() -> f64 {
    10.0
}

impl Default for LowFreqFilterConfig {
    fn default() -> Self {
        Self {
            max_q: default_low_freq_max_q(),
            min_q: default_min_q(),
            allow_boost: false,
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

impl RoomDimensions {
    /// Calculate Schroeder frequency from room dimensions
    /// Formula: fs = 2000 * sqrt(RT60 / V) where V = volume
    /// Simplified: fs ≈ 11885 / sqrt(V) for typical rooms (RT60 ≈ 0.5s)
    pub fn schroeder_frequency(&self) -> f64 {
        let volume = self.length * self.width * self.height;
        // Using simplified formula for typical domestic room (RT60 ≈ 0.5s)
        11885.0 / volume.sqrt()
    }
}

/// Schroeder frequency split configuration
///
/// Different Q constraints below and above the Schroeder frequency.
/// Below Schroeder: high-Q narrow filters to address room modes
/// Above Schroeder: low-Q broad filters for gentle tone control
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchroederSplitConfig {
    /// Enable Schroeder split optimization
    #[serde(default)]
    pub enabled: bool,

    /// Schroeder frequency in Hz
    /// Default: 300.0 Hz (typical for small/medium rooms)
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
// Phase Alignment Configuration
// ============================================================================

/// Phase alignment configuration for subwoofer integration
///
/// Optimizes delay and polarity to maximize energy sum in the crossover region
/// between subwoofer and main speakers.
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

// ============================================================================
// Multi-Seat Configuration
// ============================================================================

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

/// Multi-seat measurement configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatMeasurement {
    /// Name of this multi-seat configuration
    pub name: String,

    /// Measurements at each seat position
    pub seat_measurements: Vec<MeasurementSource>,
}

/// Multi-seat optimization configuration
///
/// Optimizes subwoofer delays and gains to minimize response variance
/// across multiple listening positions.
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

    /// Maximum allowed deviation at non-primary seats (dB, used with PrimaryWithConstraints)
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

// ============================================================================
// Optimizer Configuration
// ============================================================================

/// Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OptimizerConfig {
    /// Optimization mode: "iir" (default), "fir", "mixed"
    /// (Legacy field, prefer `processing_mode` in v2)
    #[serde(default = "default_opt_mode")]
    pub mode: String,

    /// Processing mode for RoomEQ v2
    #[serde(default)]
    pub processing_mode: ProcessingMode,

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

    /// Whether to run local refinement after global optimization (hybrid optimization)
    /// When true, runs a two-stage optimization:
    ///   Stage 1 (Global): DE finds approximate solution
    ///   Stage 2 (Local): COBYLA fine-tunes the result
    /// Default: true (recommended for best results)
    #[serde(default = "default_refine")]
    pub refine: bool,

    /// Local optimizer algorithm for refinement stage
    /// Used when `refine` is true. Default: "cobyla"
    #[serde(default = "default_local_algo")]
    pub local_algo: String,

    /// Enable psychoacoustic preprocessing
    /// When true, applies variable smoothing before optimization:
    /// - 1/48 octave smoothing < 100 Hz (preserve room modes)
    /// - 1/6 octave smoothing > 1 kHz (ignore comb filtering)
    ///
    /// Default: true (recommended for room correction)
    #[serde(default = "default_psychoacoustic")]
    pub psychoacoustic: bool,

    /// Enable asymmetric loss (peaks penalized 2x more than dips)
    /// When true, the optimizer will prioritize reducing peaks over filling dips,
    /// which is psychoacoustically correct since nulls cannot be fixed with EQ.
    /// Default: true (recommended for room correction)
    #[serde(default = "default_asymmetric_loss")]
    pub asymmetric_loss: bool,

    /// Optimization convergence tolerance (relative)
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,

    /// Optimization convergence tolerance (absolute)
    #[serde(default = "default_atolerance")]
    pub atolerance: f64,

    /// Allow inter-speaker delay optimization
    /// When true, the optimizer generates delay plugins to align speakers in time.
    /// This includes time alignment from WAV measurements, phase alignment, and group delay optimization.
    /// Default: false for IIR mode (low-latency), true for FIR and mixed modes.
    /// When None (omitted from JSON), the default is inferred from the mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_delay: Option<bool>,

    // ========================================================================
    // Scenario B (WITHOUT Subwoofers) Configuration
    // ========================================================================
    /// Target curve tilt configuration
    /// Default: flat (no tilt)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_tilt: Option<TargetTiltConfig>,

    /// Excursion protection configuration
    /// Auto-generates HPF to prevent over-boost on bookshelf speakers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excursion_protection: Option<ExcursionProtectionConfig>,

    /// Schroeder frequency split configuration
    /// Different Q constraints below/above Schroeder frequency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schroeder_split: Option<SchroederSplitConfig>,

    // ========================================================================
    // Scenario A (WITH Subwoofers) Configuration
    // ========================================================================
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
    /// Fits shelf filters to match target curve across full spectrum before fine EQ
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadband_target_matching: Option<BroadbandTargetMatchingConfig>,

    /// Multi-measurement optimization configuration
    /// When a speaker has multiple measurements (different listening positions),
    /// controls how they are combined during optimization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_measurement: Option<MultiMeasurementConfig>,
}

// ============================================================================
// Broadband Target Matching Configuration
// ============================================================================

/// Configuration for broadband target matching
///
/// Fits Low Shelf, High Shelf, and Gain filters to match the target curve
/// across the full frequency range (20Hz-20kHz) before fine-grained PEQ optimization.
/// This provides broad tonal balance correction even if fine EQ is limited to a smaller range.
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

// ============================================================================
// Multi-Measurement Configuration
// ============================================================================

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
}

impl Default for MultiMeasurementConfig {
    fn default() -> Self {
        Self {
            strategy: MultiMeasurementStrategy::default(),
            weights: None,
            variance_lambda: default_variance_lambda(),
        }
    }
}

fn default_variance_lambda() -> f64 {
    1.0
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
fn default_phase_smoothing() -> f64 {
    0.167 // 1/6 octave
}
fn default_num_filters() -> usize {
    7
}
fn default_min_q() -> f64 {
    0.5
}
fn default_max_q() -> f64 {
    6.0
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
    50
}
fn default_refine() -> bool {
    true // Enable hybrid optimization by default for best results
}
fn default_local_algo() -> String {
    "cobyla".to_string()
}
fn default_psychoacoustic() -> bool {
    true // Enable psychoacoustic smoothing by default
}
fn default_asymmetric_loss() -> bool {
    true // Enable asymmetric loss by default (peaks penalized more than dips)
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
            processing_mode: ProcessingMode::LowLatency,
            fir: None,
            mixed_config: None,
            seed: None,
            refine: default_refine(),
            local_algo: default_local_algo(),
            psychoacoustic: default_psychoacoustic(),
            asymmetric_loss: default_asymmetric_loss(),
            tolerance: default_tolerance(),
            atolerance: default_atolerance(),
            allow_delay: None,
            // Scenario B configs
            target_tilt: None,
            excursion_protection: None,
            schroeder_split: None,
            // Scenario A configs
            phase_alignment: None,
            multi_seat: None,
            // V2 Configs
            gd_opt: None,
            vog: None,
            broadband_target_matching: None,
            multi_measurement: None,
        }
    }
}

impl OptimizerConfig {
    /// Resolve the effective `allow_delay` value based on the mode.
    /// - Explicit `Some(true/false)` takes precedence
    /// - Default: false for IIR mode, true for FIR and mixed modes
    pub fn allow_delay(&self) -> bool {
        self.allow_delay.unwrap_or(self.mode != "iir")
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

/// Impulse response waveform (time-domain)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IrWaveform {
    /// Time axis in milliseconds
    pub time_ms: Vec<f64>,
    /// Amplitude (normalized so pre-IR peak = 1.0)
    pub amplitude: Vec<f64>,
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

    /// EQ filter response curve (correction magnitude in dB) (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eq_response: Option<CurveData>,

    /// Impulse response before correction (optional, requires phase data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_ir: Option<IrWaveform>,

    /// Impulse response after correction (optional, requires phase data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_ir: Option<IrWaveform>,
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

    /// Initial frequency response curve for this driver before optimization (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_curve: Option<CurveData>,
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
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(PathBuf::from("left.csv")),
                speaker_name: None,
            })),
        );

        let config = RoomConfig {
            version: default_config_version(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
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
            speaker_name: None,
            measurements: vec![
                MeasurementSource::Single(MeasurementSingle {
                    measurement: MeasurementRef::Path(PathBuf::from("woofer.csv")),
                    speaker_name: None,
                }),
                MeasurementSource::Single(MeasurementSingle {
                    measurement: MeasurementRef::Path(PathBuf::from("tweeter.csv")),
                    speaker_name: None,
                }),
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

    #[test]
    fn test_speaker_name_validation() {
        let valid_names = vec!["Genelec 8361A", "Neumann KH-120", "Sub-1"];
        let invalid_names = vec!["Genelec @ 8361A", "Neumann_KH_120"];

        let is_valid = |name: &str| {
            name.chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
        };

        for name in valid_names {
            assert!(is_valid(name), "Should be valid: {}", name);
        }
        for name in invalid_names {
            assert!(!is_valid(name), "Should be invalid: {}", name);
        }
    }
}
