//! Shared room EQ domain types used by both GPUI and TUI apps.

use serde::{Deserialize, Serialize};

use crate::EQFilter;
use crate::ReleaseChannel;
use crate::recording_types::{DelayProbeResults, RecordingResult};
use math_audio_iir_fir::BiquadFilterType;

/// (frequencies, magnitude_db, phase_deg, wav_path, csv_path)
type MeasurementData = (Vec<f32>, Vec<f32>, Vec<f32>, Option<String>, Option<String>);

/// Room EQ workflow step
///
/// Flow: LoadData → Delay → Process → Configure → Optimize → Review → Export
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomEqStep {
    /// Step 1: Load/import measurement data
    #[default]
    LoadData,
    /// Step 2: Per-channel alignment delay table. Shows arrival times
    /// from the recording session's probe results (or manual entry).
    /// The user can override delays; values < 0.3 ms get a "consider
    /// using 0" hint.
    Delay,
    /// Step 3: Choose between Simple Wizard (guided presets) and Full
    /// Wizard (all parameters in Acoustic + Optimizer blocks).
    Process,
    /// Step 4: Configure channels, mode, and optimizer settings.
    /// Layout depends on the wizard mode selected in the Process step.
    Configure,
    /// Step 5: Run optimization (per-channel, then combined)
    Optimize,
    /// Step 6: Review results and visualizations
    Review,
    /// Step 7: Export DSP chain and apply
    Export,
}

impl RoomEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [RoomEqStep] {
        &[
            RoomEqStep::LoadData,
            RoomEqStep::Delay,
            RoomEqStep::Process,
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
            RoomEqStep::Delay => 1,
            RoomEqStep::Process => 2,
            RoomEqStep::Configure => 3,
            RoomEqStep::Optimize => 4,
            RoomEqStep::Review => 5,
            RoomEqStep::Export => 6,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            RoomEqStep::LoadData => "Load Data",
            RoomEqStep::Delay => "Delay",
            RoomEqStep::Process => "Process",
            RoomEqStep::Configure => "Configure",
            RoomEqStep::Optimize => "Optimize",
            RoomEqStep::Review => "Review",
            RoomEqStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => Some(RoomEqStep::Delay),
            RoomEqStep::Delay => Some(RoomEqStep::Process),
            RoomEqStep::Process => Some(RoomEqStep::Configure),
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
            RoomEqStep::Delay => Some(RoomEqStep::LoadData),
            RoomEqStep::Process => Some(RoomEqStep::Delay),
            RoomEqStep::Configure => Some(RoomEqStep::Process),
            RoomEqStep::Optimize => Some(RoomEqStep::Configure),
            RoomEqStep::Review => Some(RoomEqStep::Optimize),
            RoomEqStep::Export => Some(RoomEqStep::Review),
        }
    }
}

/// Wizard mode selected in the Process step. Determines which
/// Configure layout renders: Simple shows guided presets, Full
/// shows all parameters in two organized blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RoomEqWizardMode {
    /// Guided preset selector adapted to the speaker configuration.
    #[default]
    Simple,
    /// Full parameter access split into Acoustic + Optimizer blocks.
    Full,
}

// Simple Wizard types — canonical definitions live in autoeq, re-exported here.
pub use autoeq::roomeq::{
    SimpleCrossoverChoice, SimpleLossChoice, SimplePresetConfig, SimpleProcessingChoice,
    SpeakerTier,
};

/// Apply the user's Simple Wizard choices to a flat UI optimizer config.
///
/// Fields not controlled by the preset keep their current values so the
/// user doesn't lose any manual tuning done in a previous Full Wizard
/// session.  This is the "mutate in place" path used when the full wizard
/// needs to incorporate simple-mode choices into an existing config.
pub fn apply_simple_preset(preset: &SimplePresetConfig, config: &mut RoomEqOptimizerConfig) {
    // Processing mode
    config.mode = match preset.processing {
        SimpleProcessingChoice::Iir => RoomEqOptimizationMode::Iir,
        SimpleProcessingChoice::MixedPhase => RoomEqOptimizationMode::MixedPhase,
    };

    // Loss function
    config.loss_type = match preset.loss {
        SimpleLossChoice::Flat => "flat".to_string(),
        SimpleLossChoice::Epa => "epa".to_string(),
    };

    // Target response derived from measurement
    config.target_response.enabled = true;
    config.target_response.shape = "from_measurement".to_string();
    config.target_response.slope_db_per_octave = 0.0;

    // Crossover (2.1+ only)
    if !preset.bass_management.is_empty() || matches!(preset.crossover, SimpleCrossoverChoice::Lr48)
    {
        config.schroeder_split.enabled = true;
    }

    // Sane defaults for params not exposed in Simple mode
    config.num_filters = 7;
    config.algorithm = "autoeq:de".to_string();
    config.population = 300;
    config.max_iter = 50_000;
    config.min_freq = 20.0;
    config.max_freq = 1600.0;
    config.min_db = -12.0;
    config.max_db = 4.0;
    config.min_q = 0.5;
    config.max_q = 6.0;
    config.peq_model = "pk".to_string();
    config.tolerance = 1e-5;
    config.atolerance = 1e-5;
    config.psychoacoustic = true;
    config.asymmetric_loss = true;
    config.refine = true;
    config.local_algo = "cobyla".to_string();

    // Multi-position strategy
    if !preset.multi_position_strategy.is_empty() {
        config.multi_measurement.enabled = true;
        config.multi_measurement.strategy = preset.multi_position_strategy.clone();
    }
}

/// Source of measurement data for Room EQ
#[derive(Debug, Clone, PartialEq, Default)]
pub enum RoomEqDataSource {
    /// Use recordings from current session
    #[default]
    FromRecording,
    /// Loaded from a JSON file
    FromFile(std::path::PathBuf),
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
    #[serde(default)]
    pub mic_calibration_paths: Vec<Option<String>>,
    pub recording_directory: Option<String>,
    pub signal_type: String,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    #[serde(default)]
    pub sweep_start_freq: Option<f32>,
    #[serde(default)]
    pub sweep_end_freq: Option<f32>,
    /// Physical room dimensions collected at save time (W × D × H, in
    /// meters). Optional — older files and hastily-saved sessions will
    /// not have this populated. When present the field round-trips
    /// through load/save unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_dimensions: Option<RoomDimensionsLegacy>,
    /// Free-form description of the listening setup (treatment,
    /// seating, notes). Empty strings are stored as `None` on save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_description: Option<String>,
    /// Per-channel speaker identity (brand + model) keyed by channel
    /// name so rename/reorder survives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_speakers: Option<std::collections::HashMap<String, String>>,
    /// Tone-burst delay probe results captured during the Recording
    /// wizard's Probe step. When present the Room EQ "Delay Detection"
    /// step can auto-populate from these instead of running a live
    /// measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_results: Option<DelayProbeResults>,
    /// Relative path (within the recording directory) of the raw
    /// probe WAV persisted by `probe_channel_delays_with_recording`.
    /// `None` for sessions that skipped the Probe step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_wav_relative: Option<String>,
    /// Number of measurement positions (seats) the user captured.
    /// 1 for legacy single-position sessions; ≥ 2 means the
    /// `multi_mic_measurements` arrays on each channel hold the
    /// per-(position, mic) sweeps in `(position, mic)` order.
    #[serde(default = "default_num_positions_one")]
    pub num_positions: usize,
}

fn default_num_positions_one() -> usize {
    1
}

/// Simple W/D/H triple in meters for the legacy
/// `RoomEqMeasurementsFile` format. Mirrors `autoeq::RoomDimensions`
/// but lives in the player crate so nothing here has to depend on
/// autoeq's roomeq types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomDimensionsLegacy {
    pub length: f64,
    pub width: f64,
    pub height: f64,
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

    /// Try to extract the delay-detection hints from a measurements JSON
    /// blob in either the legacy `RoomEqMeasurementsFile` format or the
    /// newer `autoeq::RoomConfig` format. Returns the canonical channel
    /// name order and sample rate the recording session was captured
    /// at, so the Delay Detection step can align its probe with the
    /// same device settings the user measured with.
    ///
    /// Returns `None` when neither format carries the config — that
    /// means the file was recorded without session metadata and the
    /// caller should fall back to defaults (0..N indices, 48 000 Hz).
    pub fn extract_delay_detection_hints(json: &str) -> Option<DelayDetectionHints> {
        // Newer RoomConfig format
        if let Ok(room_config) = serde_json::from_str::<autoeq::RoomConfig>(json)
            && let Some(rc) = room_config.recording_config
        {
            // The autoeq crate stores probe results as `ProbeResultsLegacy`
            // which is shape-compatible with the engine's `ProbeDelayResults`
            // (re-exported as `DelayProbeResults`). Translate via serde
            // round-trip so the player-layer type is what DelayDetectionState
            // expects.
            let probe_results = rc.probe_results.as_ref().and_then(|pr| {
                serde_json::to_string(pr)
                    .ok()
                    .and_then(|j| serde_json::from_str::<DelayProbeResults>(&j).ok())
            });
            return Some(DelayDetectionHints {
                channel_names: rc.channel_names.clone(),
                sample_rate: rc.recording_sample_rate,
                playback_device_name: rc.playback_device_name.clone(),
                recording_device_name: rc.recording_device_name.clone(),
                probe_results,
            });
        }
        // Legacy format — the player-layer `RecordingConfiguration`
        // stores the same fields but with stricter types.
        if let Ok(file) = Self::from_json_str(json)
            && let Some(cfg) = file.configuration
        {
            return Some(DelayDetectionHints {
                channel_names: Some(cfg.channel_names),
                sample_rate: Some(cfg.recording_sample_rate),
                playback_device_name: Some(cfg.playback_device_name),
                recording_device_name: Some(cfg.recording_device_name),
                probe_results: cfg.probe_results,
            });
        }
        None
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
        let resolve_path = |rel: &str| -> String {
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

        room_config
            .speakers
            .into_iter()
            .enumerate()
            .filter_map(|(idx, (channel_name, speaker_config))| {
                // Extract the primary MeasurementRef from the speaker config
                let measurement_ref = match speaker_config {
                    autoeq::SpeakerConfig::Single(source) => match source {
                        autoeq::MeasurementSource::Single(s) => Some(s.measurement),
                        autoeq::MeasurementSource::Multiple(m) => m.measurements.into_iter().next(),
                        autoeq::MeasurementSource::InMemory(_)
                        | autoeq::MeasurementSource::InMemoryMultiple(_) => None,
                    },
                    _ => None, // Groups not yet supported
                };

                let measurement_ref = measurement_ref?;

                // Build ChannelMeasurement from any MeasurementRef variant
                let (frequencies, magnitude_db, phase_deg, wav_path, csv_path) =
                    Self::load_measurement_ref(&measurement_ref, &resolve_path);

                Some(ChannelMeasurement {
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
                    multi_mic_measurements: Vec::new(),
                })
            })
            .filter(|ch| !ch.measurement.frequencies.is_empty())
            .collect()
    }

    /// Load measurement data from any MeasurementRef variant (inline, named path, or bare path).
    /// Returns (frequencies, magnitude_db, phase_deg, wav_path, csv_path).
    fn load_measurement_ref(
        measurement_ref: &autoeq::read::MeasurementRef,
        resolve_path: &dyn Fn(&str) -> String,
    ) -> MeasurementData {
        match measurement_ref {
            autoeq::read::MeasurementRef::Inline(data) => {
                let wav_path = data.wav_path.as_deref().map(resolve_path);
                let csv_path = data.csv_path.as_deref().map(resolve_path);

                if data.frequencies.is_empty() {
                    // Inline has no data — try loading from referenced CSV
                    if let Some(ref csv) = csv_path {
                        if let Some(loaded) = Self::load_curve_from_csv(csv) {
                            return (loaded.0, loaded.1, loaded.2, wav_path, csv_path);
                        }
                    }
                    (Vec::new(), Vec::new(), Vec::new(), wav_path, csv_path)
                } else {
                    (
                        data.frequencies.iter().map(|&f| f as f32).collect(),
                        data.magnitude_db.iter().map(|&m| m as f32).collect(),
                        data.phase_deg
                            .as_ref()
                            .map(|p| p.iter().map(|&v| v as f32).collect())
                            .unwrap_or_default(),
                        wav_path,
                        csv_path,
                    )
                }
            }
            autoeq::read::MeasurementRef::Named { path, .. } => {
                let csv_str = resolve_path(&path.to_string_lossy());
                let loaded = Self::load_curve_from_csv(&csv_str);
                let (freq, mag, phase) = loaded.unwrap_or_default();
                (freq, mag, phase, None, Some(csv_str))
            }
            autoeq::read::MeasurementRef::Path(path) => {
                let csv_str = resolve_path(&path.to_string_lossy());
                let loaded = Self::load_curve_from_csv(&csv_str);
                let (freq, mag, phase) = loaded.unwrap_or_default();
                (freq, mag, phase, None, Some(csv_str))
            }
        }
    }

    /// Load a curve from a CSV file, returning (frequencies, magnitude_db, phase_deg).
    fn load_curve_from_csv(csv_path: &str) -> Option<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        let path = std::path::PathBuf::from(csv_path);
        match autoeq::read::read_curve_from_csv(&path) {
            Ok(curve) => {
                log::info!(
                    "Loaded {} frequency points from CSV: {}",
                    curve.freq.len(),
                    csv_path
                );
                Some((
                    curve.freq.iter().map(|&f| f as f32).collect(),
                    curve.spl.iter().map(|&s| s as f32).collect(),
                    curve
                        .phase
                        .map(|p| p.iter().map(|&v| v as f32).collect())
                        .unwrap_or_default(),
                ))
            }
            Err(e) => {
                log::warn!("Failed to load CSV '{}': {}", csv_path, e);
                None
            }
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
    /// Additional mic measurements for multi-position optimization
    #[serde(default)]
    pub multi_mic_measurements: Vec<RecordingResult>,
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

/// Shorter alias for `RoomEqSpeakerConfigType`.
pub type SpeakerConfigType = RoomEqSpeakerConfigType;
/// Shorter alias for `RoomEqCrossoverType`.
pub type CrossoverType = RoomEqCrossoverType;

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
    MixedPhase,
}

impl RoomEqOptimizationMode {
    pub fn all() -> &'static [RoomEqOptimizationMode] {
        &[
            RoomEqOptimizationMode::Iir,
            RoomEqOptimizationMode::Fir,
            RoomEqOptimizationMode::Mixed,
            RoomEqOptimizationMode::MixedPhase,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "IIR (Parametric EQ)",
            RoomEqOptimizationMode::Fir => "FIR (Convolution)",
            RoomEqOptimizationMode::Mixed => "Mixed (IIR + FIR)",
            RoomEqOptimizationMode::MixedPhase => "Mixed-Phase (IIR + short FIR)",
        }
    }

    pub fn to_code(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "iir",
            RoomEqOptimizationMode::Fir => "fir",
            RoomEqOptimizationMode::Mixed => "mixed",
            RoomEqOptimizationMode::MixedPhase => "mixed_phase",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "fir" => RoomEqOptimizationMode::Fir,
            "mixed" => RoomEqOptimizationMode::Mixed,
            "mixed_phase" => RoomEqOptimizationMode::MixedPhase,
            _ => RoomEqOptimizationMode::Iir,
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
            RoomEqOptimizationMode::MixedPhase => {
                "IIR for minimum-phase + short FIR for excess phase. Low latency (~10ms)."
            }
        }
    }

    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            RoomEqOptimizationMode::Iir => ReleaseChannel::Beta,
            RoomEqOptimizationMode::Fir => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::Mixed => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::MixedPhase => ReleaseChannel::Alpha,
        }
    }

    pub fn available(channel: ReleaseChannel) -> Vec<Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|mode| channel.allows(mode.maturity()))
            .collect()
    }
}

/// Pre-ringing suppression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreRingingConfig {
    /// Maximum pre-ringing level in dB relative to main tap (default: -30.0)
    pub threshold_db: f64,
    /// Maximum pre-ringing time in seconds (default: 0.005 = 5 ms)
    pub max_time_s: f64,
}

impl Default for PreRingingConfig {
    fn default() -> Self {
        Self {
            threshold_db: -30.0,
            max_time_s: 0.005,
        }
    }
}

/// FIR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqFirConfig {
    pub taps: usize,
    pub phase: String,
    /// Whether to correct excess phase (only applies to kirkeby mode)
    #[serde(default)]
    pub correct_excess_phase: bool,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    #[serde(default = "default_phase_smoothing")]
    pub phase_smoothing: f64,
    /// Pre-ringing suppression configuration
    #[serde(default)]
    pub pre_ringing: Option<PreRingingConfig>,
}

fn default_phase_smoothing() -> f64 {
    0.167
}

impl Default for RoomEqFirConfig {
    fn default() -> Self {
        Self {
            taps: 4096,
            phase: "kirkeby".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        }
    }
}

/// Mixed-phase correction configuration (IIR for minimum-phase + short FIR for excess phase)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedPhaseUiConfig {
    /// Maximum FIR length in milliseconds for excess phase correction (default: 10.0)
    pub max_fir_length_ms: f64,
    /// Pre-ringing threshold in dB (default: -30.0)
    pub pre_ringing_threshold_db: f64,
    /// Minimum spatial correction depth (default: 0.5)
    pub min_spatial_depth: f64,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    pub phase_smoothing_octaves: f64,
}

impl Default for MixedPhaseUiConfig {
    fn default() -> Self {
        Self {
            max_fir_length_ms: 10.0,
            pre_ringing_threshold_db: -30.0,
            min_spatial_depth: 0.5,
            phase_smoothing_octaves: 0.167,
        }
    }
}

/// Target response configuration (UI-facing).
///
/// Mirrors the backend `autoeq::roomeq::TargetResponseConfig` but flattened
/// into a single struct for simpler binding in UI widgets. Covers the target
/// shape (flat / Harman / custom slope / file / derived-from-measurement),
/// the preference shelves (bass / treble), and the broadband pre-correction
/// toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetResponseUiConfig {
    /// Whether any target shaping is applied. When `false` the optimiser
    /// sees a flat target regardless of the other fields.
    pub enabled: bool,
    /// Target shape: "flat" | "harman" | "custom" | "file" | "from_measurement".
    pub shape: String,
    /// Slope in dB/octave (used when `shape == "custom"`).
    pub slope_db_per_octave: f64,
    /// Reference frequency where the slope passes through 0 dB.
    pub reference_freq: f64,
    /// Path to CSV target file (used when `shape == "file"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_path: Option<std::path::PathBuf>,
    /// Bass shelf preference in dB (layered on top of the target shape).
    pub bass_shelf_db: f64,
    /// Bass shelf frequency in Hz.
    pub bass_shelf_freq: f64,
    /// Treble shelf preference in dB.
    pub treble_shelf_db: f64,
    /// Treble shelf frequency in Hz.
    pub treble_shelf_freq: f64,
    /// Enable broadband pre-correction (shelf+gain fit before fine EQ).
    pub broadband_precorrection: bool,
}

impl Default for TargetResponseUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shape: "harman".to_string(),
            slope_db_per_octave: -0.8,
            reference_freq: 1000.0,
            curve_path: None,
            bass_shelf_db: 0.0,
            bass_shelf_freq: 200.0,
            treble_shelf_db: 0.0,
            treble_shelf_freq: 8000.0,
            broadband_precorrection: false,
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
    /// Maximum boost/cut in dB for below-Schroeder filters (None = use global max_db)
    #[serde(default)]
    pub low_freq_max_db: Option<f64>,
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
            low_freq_max_db: None,
            high_freq_max_q: 1.0,
            high_freq_shelving_only: false,
        }
    }
}

/// Subwoofer-specific optimizer overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubOptimizerUiConfig {
    pub enabled: bool,
    pub num_filters: usize,
    pub max_db: f64,
    pub min_db: f64,
    pub min_q: f64,
    pub max_q: f64,
}

impl Default for SubOptimizerUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            num_filters: 10,
            max_db: 18.0,
            min_db: -18.0,
            min_q: 0.5,
            max_q: 10.0,
        }
    }
}

/// Inter-channel consistency correction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMatchingUiConfig {
    pub enabled: bool,
    pub threshold_db: f64,
    pub max_filters: usize,
}

impl Default for ChannelMatchingUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: 1.5,
            max_filters: 3,
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
    #[serde(default = "default_all_channel_multiseat_enabled")]
    pub all_channel_enabled: bool,
    #[serde(default = "default_all_channel_multiseat_strategy")]
    pub all_channel_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat_weights: Option<Vec<f64>>,
    #[serde(default = "default_primary_seat_weight")]
    pub primary_seat_weight: f64,
}

fn default_all_channel_multiseat_enabled() -> bool {
    true
}

fn default_all_channel_multiseat_strategy() -> String {
    "spatial_robustness".to_string()
}

fn default_primary_seat_weight() -> f64 {
    2.0
}

impl Default for MultiSeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "variance".to_string(),
            primary_seat: 0,
            max_deviation_db: 6.0,
            all_channel_enabled: default_all_channel_multiseat_enabled(),
            all_channel_strategy: default_all_channel_multiseat_strategy(),
            seat_weights: None,
            primary_seat_weight: default_primary_seat_weight(),
        }
    }
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

/// Multi-measurement optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiMeasurementUiConfig {
    pub enabled: bool,
    pub strategy: String,
    pub variance_lambda: f64,
    pub weights: Vec<f64>,
}

impl Default for MultiMeasurementUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "average".to_string(),
            variance_lambda: 1.0,
            weights: Vec::new(),
        }
    }
}

fn default_room_smooth_n() -> usize {
    6 // 1/6 octave smoothing
}

fn default_room_strategy() -> String {
    "lshade".to_string()
}
fn default_de_f() -> f64 {
    0.8
}
fn default_de_cr() -> f64 {
    0.9
}
fn default_adaptive_weight_f() -> f64 {
    0.8
}
fn default_adaptive_weight_cr() -> f64 {
    0.7
}
fn default_spacing_weight() -> f64 {
    1.0
}
fn default_min_spacing_oct() -> f64 {
    0.08
}
fn default_sample_rate() -> usize {
    48000
}
fn default_room_tolerance() -> f64 {
    1e-5
}

/// Classical sample rates for audio (44.1k and 48k families, up to 8x).
pub const CLASSICAL_SAMPLE_RATES: &[usize] =
    &[44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];

/// Given a raw sample-rate value, snap to the next classical rate above it.
/// Returns the highest rate if already at or above the top.
pub fn next_sample_rate(current: usize) -> usize {
    for &rate in CLASSICAL_SAMPLE_RATES {
        if rate > current {
            return rate;
        }
    }
    *CLASSICAL_SAMPLE_RATES.last().unwrap()
}

/// Given a raw sample-rate value, snap to the previous classical rate below it.
/// Returns the lowest rate if already at or below the bottom.
pub fn prev_sample_rate(current: usize) -> usize {
    for &rate in CLASSICAL_SAMPLE_RATES.iter().rev() {
        if rate < current {
            return rate;
        }
    }
    *CLASSICAL_SAMPLE_RATES.first().unwrap()
}
fn default_room_atolerance() -> f64 {
    1e-5
}

// ---------------------------------------------------------------------------
// Channel metadata for smart defaults
// ---------------------------------------------------------------------------

/// Metadata about measurement channels, decoupled from UI state.
///
/// Used by [`RoomEqOptimizerConfig::apply_smart_defaults`] to infer system
/// configuration (stereo vs surround, subwoofer presence, height channels).
#[derive(Debug, Clone, Default)]
pub struct ChannelMetadata {
    pub channel_names: Vec<String>,
    pub playback_sample_rate: Option<u32>,
}

impl ChannelMetadata {
    /// Sub/LFE channel names.
    fn is_sub_name(name: &str) -> bool {
        let upper = name.to_uppercase();
        upper == "LFE" || upper == "SUB" || upper == "SW" || upper.starts_with("SUB")
    }

    /// Height channel names used for Voice of God detection.
    const HEIGHT_CHANNELS: &[&str] = &[
        "TFL", "TFR", "TSL", "TSR", "TBL", "TBR", "VOG", "TFC", "TBC", "TSC",
    ];

    /// Count of non-subwoofer channels.
    fn non_sub_count(&self) -> usize {
        self.channel_names
            .iter()
            .filter(|n| !Self::is_sub_name(n))
            .count()
    }

    pub fn has_subwoofer(&self) -> bool {
        self.channel_names.iter().any(|n| Self::is_sub_name(n))
    }

    pub fn is_surround(&self) -> bool {
        self.non_sub_count() >= 3
    }

    pub fn has_height_channels(&self) -> bool {
        self.channel_names.iter().any(|name| {
            let upper = name.to_uppercase();
            Self::HEIGHT_CHANNELS.iter().any(|&h| upper == h)
        })
    }

    pub fn is_home_cinema(&self) -> bool {
        self.non_sub_count() >= 5
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
    #[serde(default = "default_room_strategy")]
    pub strategy: String,
    #[serde(default = "default_de_f")]
    pub de_f: f64,
    #[serde(default = "default_de_cr")]
    pub de_cr: f64,
    #[serde(default = "default_adaptive_weight_f")]
    pub adaptive_weight_f: f64,
    #[serde(default = "default_adaptive_weight_cr")]
    pub adaptive_weight_cr: f64,
    #[serde(default = "default_spacing_weight")]
    pub spacing_weight: f64,
    #[serde(default = "default_min_spacing_oct")]
    pub min_spacing_oct: f64,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: usize,
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
    #[serde(default)]
    pub smooth: bool,
    #[serde(default = "default_room_smooth_n")]
    pub smooth_n: usize,
    #[serde(default = "default_room_tolerance")]
    pub tolerance: f64,
    #[serde(default = "default_room_atolerance")]
    pub atolerance: f64,
    pub target_curve: String,
    pub system_type: String,
    #[serde(default)]
    pub allow_delay: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub vog: VoGConfig,
    #[serde(default)]
    pub mixed_config: MixedModeUiConfig,
    #[serde(default)]
    pub mixed_phase: MixedPhaseUiConfig,
    /// Unified target response (shape + preference shelves + broadband pre-correction).
    #[serde(default)]
    pub target_response: TargetResponseUiConfig,
    #[serde(default)]
    pub excursion_protection: ExcursionProtectionConfig,
    #[serde(default)]
    pub schroeder_split: SchroederSplitConfig,
    #[serde(default)]
    pub phase_alignment: PhaseAlignmentConfig,
    #[serde(default)]
    pub multi_seat: MultiSeatConfig,
    #[serde(default)]
    pub multi_measurement: MultiMeasurementUiConfig,
    #[serde(default)]
    pub sub_config: SubOptimizerUiConfig,
    #[serde(default)]
    pub channel_matching: ChannelMatchingUiConfig,
    /// True when settings were imported from a backend config file (recordings.json).
    /// When set, `apply_smart_defaults()` skips overriding feature toggles.
    #[serde(default)]
    pub imported_from_file: bool,
}

impl Default for RoomEqOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: RoomEqOptimizationMode::default(),
            fir: RoomEqFirConfig::default(),
            multi_speaker_mode: MultiSpeakerMode::Combined,
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            de_f: default_de_f(),
            de_cr: default_de_cr(),
            adaptive_weight_f: default_adaptive_weight_f(),
            adaptive_weight_cr: default_adaptive_weight_cr(),
            spacing_weight: default_spacing_weight(),
            min_spacing_oct: default_min_spacing_oct(),
            sample_rate: default_sample_rate(),
            num_filters: 7,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 20.0,
            max_freq: 1600.0,
            max_iter: 50000,
            peq_model: "pk".to_string(),
            population: 300,
            refine: false,
            local_algo: "cobyla".to_string(),
            loss_type: "flat".to_string(),
            psychoacoustic: true,
            asymmetric_loss: true,
            smooth: false,
            smooth_n: default_room_smooth_n(),
            tolerance: 1e-5,
            atolerance: 1e-5,
            target_curve: "flat".to_string(),
            system_type: "stereo".to_string(),
            allow_delay: false,
            seed: None,
            vog: VoGConfig::default(),
            mixed_config: MixedModeUiConfig::default(),
            mixed_phase: MixedPhaseUiConfig::default(),
            target_response: TargetResponseUiConfig::default(),
            excursion_protection: ExcursionProtectionConfig::default(),
            schroeder_split: SchroederSplitConfig::default(),
            phase_alignment: PhaseAlignmentConfig::default(),
            multi_seat: MultiSeatConfig::default(),
            multi_measurement: MultiMeasurementUiConfig::default(),
            sub_config: SubOptimizerUiConfig::default(),
            channel_matching: ChannelMatchingUiConfig::default(),
            imported_from_file: false,
        }
    }
}

impl RoomEqOptimizerConfig {
    /// Import optimizer parameters and feature toggles from a backend `OptimizerConfig`.
    ///
    /// This is used when loading a RoomConfig JSON file so that the UI
    /// uses the same optimizer settings as the roomeq CLI.
    /// Sets `imported_from_file = true` so that `apply_smart_defaults()` will
    /// not override the imported feature toggle state.
    pub fn import_from_backend(&mut self, backend: &autoeq::roomeq::OptimizerConfig) {
        // Core optimizer parameters
        self.algorithm = backend.algorithm.clone();
        self.strategy = backend.strategy.clone();
        self.num_filters = backend.num_filters;
        self.min_q = backend.min_q;
        self.max_q = backend.max_q;
        self.min_db = backend.min_db;
        self.max_db = backend.max_db;
        self.min_freq = backend.min_freq;
        self.max_freq = backend.max_freq;
        self.max_iter = backend.max_iter;
        self.population = backend.population;
        self.peq_model = backend.peq_model.clone();
        self.loss_type = backend.loss_type.clone();
        self.psychoacoustic = backend.psychoacoustic;
        self.asymmetric_loss = backend.asymmetric_loss;
        self.tolerance = backend.tolerance;
        self.atolerance = backend.atolerance;
        self.refine = backend.refine;
        self.local_algo = backend.local_algo.clone();
        self.seed = backend.seed;

        // FIR configuration
        if let Some(ref fir) = backend.fir {
            self.fir.taps = fir.taps;
            self.fir.phase = fir.phase.clone();
            self.fir.correct_excess_phase = fir.correct_excess_phase;
            self.fir.phase_smoothing = fir.phase_smoothing;
            self.fir.pre_ringing = fir.pre_ringing.as_ref().map(|pr| PreRingingConfig {
                threshold_db: pr.threshold_db,
                max_time_s: pr.max_time_s,
            });
        }

        // Mixed-phase configuration
        if let Some(ref mp) = backend.mixed_phase {
            self.mixed_phase = MixedPhaseUiConfig {
                max_fir_length_ms: mp.max_fir_length_ms,
                pre_ringing_threshold_db: mp.pre_ringing_threshold_db,
                min_spatial_depth: mp.min_spatial_depth,
                phase_smoothing_octaves: mp.phase_smoothing_octaves,
            };
        }

        // Processing mode → optimization mode
        self.mode = match backend.processing_mode {
            autoeq::roomeq::ProcessingMode::LowLatency => RoomEqOptimizationMode::Iir,
            autoeq::roomeq::ProcessingMode::PhaseLinear => RoomEqOptimizationMode::Fir,
            autoeq::roomeq::ProcessingMode::Hybrid => RoomEqOptimizationMode::Mixed,
            autoeq::roomeq::ProcessingMode::MixedPhase => RoomEqOptimizationMode::MixedPhase,
            // WarpedIir and KautzModal are IIR-based modes
            autoeq::roomeq::ProcessingMode::WarpedIir
            | autoeq::roomeq::ProcessingMode::KautzModal => RoomEqOptimizationMode::Iir,
        };

        // Feature toggles: only override from backend when explicitly present.
        if let Some(ref tr) = backend.target_response {
            self.target_response.enabled = true;
            self.target_response.shape = match tr.shape {
                autoeq::roomeq::TargetShape::Flat => "flat".to_string(),
                autoeq::roomeq::TargetShape::Harman => "harman".to_string(),
                autoeq::roomeq::TargetShape::Custom => "custom".to_string(),
                autoeq::roomeq::TargetShape::File => "file".to_string(),
                autoeq::roomeq::TargetShape::FromMeasurement => "from_measurement".to_string(),
            };
            self.target_response.slope_db_per_octave = tr.slope_db_per_octave;
            self.target_response.reference_freq = tr.reference_freq;
            self.target_response.curve_path = tr.curve_path.clone();
            self.target_response.bass_shelf_db = tr.preference.bass_shelf_db;
            self.target_response.bass_shelf_freq = tr.preference.bass_shelf_freq;
            self.target_response.treble_shelf_db = tr.preference.treble_shelf_db;
            self.target_response.treble_shelf_freq = tr.preference.treble_shelf_freq;
            self.target_response.broadband_precorrection = tr.broadband_precorrection;
        } else {
            self.target_response.enabled = false;
        }

        self.excursion_protection.enabled = backend
            .excursion_protection
            .as_ref()
            .is_some_and(|e| e.enabled);
        if let Some(ref ep) = backend.excursion_protection {
            self.excursion_protection.auto_detect_f3 = ep.auto_detect_f3;
            self.excursion_protection.manual_f3_hz = ep.manual_f3_hz.unwrap_or(40.0);
            self.excursion_protection.filter_order = ep.filter_order;
            self.excursion_protection.filter_type = match ep.filter_type {
                autoeq::roomeq::HighpassType::Butterworth => "bw".to_string(),
                autoeq::roomeq::HighpassType::LinkwitzRiley => "lr".to_string(),
            };
            self.excursion_protection.margin_octaves = ep.margin_octaves;
        }

        self.schroeder_split.enabled = backend.schroeder_split.as_ref().is_some_and(|s| s.enabled);
        if let Some(ref ss) = backend.schroeder_split {
            self.schroeder_split.schroeder_freq = ss.schroeder_freq;
            self.schroeder_split.low_freq_max_q = ss.low_freq_config.max_q;
            self.schroeder_split.low_freq_allow_boost = ss.low_freq_config.allow_boost;
            self.schroeder_split.low_freq_max_db = ss.low_freq_config.max_db;
            self.schroeder_split.high_freq_max_q = ss.high_freq_config.max_q;
            self.schroeder_split.high_freq_shelving_only = ss.high_freq_config.shelving_only;
        }

        self.allow_delay = backend.allow_delay.unwrap_or(false);

        self.vog.enabled = backend.vog.as_ref().is_some_and(|v| v.enabled);
        if let Some(ref vog) = backend.vog {
            self.vog.reference_channel = vog.reference_channel.clone();
        }

        self.phase_alignment.enabled = backend.phase_alignment.as_ref().is_some_and(|p| p.enabled);
        if let Some(ref pa) = backend.phase_alignment {
            self.phase_alignment.min_freq = pa.min_freq;
            self.phase_alignment.max_freq = pa.max_freq;
            self.phase_alignment.optimize_polarity = pa.optimize_polarity;
            self.phase_alignment.max_delay_ms = pa.max_delay_ms;
        }

        self.multi_seat.enabled = backend.multi_seat.as_ref().is_some_and(|m| m.enabled);
        if let Some(ref ms) = backend.multi_seat {
            self.multi_seat.strategy = match ms.strategy {
                autoeq::roomeq::MultiSeatStrategy::MinimizeVariance => "variance".to_string(),
                autoeq::roomeq::MultiSeatStrategy::PrimaryWithConstraints => "primary".to_string(),
                autoeq::roomeq::MultiSeatStrategy::Average => "average".to_string(),
            };
            self.multi_seat.primary_seat = ms.primary_seat;
            self.multi_seat.max_deviation_db = ms.max_deviation_db;
        }

        if let Some(ref mm) = backend.multi_measurement {
            self.multi_measurement.enabled = true;
            self.multi_measurement.strategy = match mm.strategy {
                autoeq::roomeq::MultiMeasurementStrategy::Average => "average".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::WeightedSum => "weighted_sum".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::Minimax => "minimax".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized => {
                    "variance_penalized".to_string()
                }
                autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness => {
                    "spatial_robustness".to_string()
                }
            };
            self.multi_measurement.variance_lambda = mm.variance_lambda;
            self.multi_measurement.weights = mm.weights.clone().unwrap_or_default();
        } else {
            self.multi_measurement.enabled = false;
        }

        // Sub-specific optimizer overrides
        self.sub_config.enabled = backend.sub_config.is_some();
        if let Some(ref sc) = backend.sub_config {
            self.sub_config.num_filters = sc.num_filters;
            self.sub_config.max_db = sc.max_db;
            self.sub_config.min_db = sc.min_db;
            self.sub_config.min_q = sc.min_q;
            self.sub_config.max_q = sc.max_q;
        }

        // Channel matching correction
        self.channel_matching.enabled =
            backend.channel_matching.as_ref().is_some_and(|c| c.enabled);
        if let Some(ref cm) = backend.channel_matching {
            self.channel_matching.threshold_db = cm.threshold_db;
            self.channel_matching.max_filters = cm.max_filters;
        }

        self.imported_from_file = true;
    }

    /// Convert the flat UI optimizer config to a backend
    /// [`OptimizerConfig`](autoeq::roomeq::OptimizerConfig).
    ///
    /// This is the single canonical conversion used by both GPUI and TUI
    /// when building a `RoomConfig` for the optimizer.
    pub fn to_optimizer_config(&self) -> autoeq::roomeq::OptimizerConfig {
        use autoeq::roomeq::{
            ChannelMatchingConfig as BackendChannelMatchingConfig, DecomposedCorrectionSerdeConfig,
            ExcursionProtectionConfig as BackendExcursionProtectionConfig,
            FirConfig as BackendFirConfig, HighFreqFilterConfig, HighpassType, LowFreqFilterConfig,
            MixedModeConfig, MixedPhaseSerdeConfig as BackendMixedPhaseConfig,
            MultiMeasurementConfig, MultiMeasurementStrategy,
            MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
            OptimizerConfig as BackendOptimizerConfig,
            PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
            PreRingingSerdeConfig as BackendPreRingingConfig, ProcessingMode,
            SchroederSplitConfig as BackendSchroederSplitConfig, SubOptimizerConfig,
            TargetResponseConfig as BackendTargetResponseConfig, TargetShape, UserPreference,
            VoiceOfGodConfig,
        };

        let processing_mode = match self.mode {
            RoomEqOptimizationMode::Iir => ProcessingMode::LowLatency,
            RoomEqOptimizationMode::Fir => ProcessingMode::PhaseLinear,
            RoomEqOptimizationMode::Mixed => ProcessingMode::Hybrid,
            RoomEqOptimizationMode::MixedPhase => ProcessingMode::MixedPhase,
        };

        let fir = Some(BackendFirConfig {
            taps: self.fir.taps,
            phase: self.fir.phase.clone(),
            correct_excess_phase: self.fir.correct_excess_phase,
            phase_smoothing: self.fir.phase_smoothing,
            pre_ringing: self
                .fir
                .pre_ringing
                .as_ref()
                .map(|pr| BackendPreRingingConfig {
                    threshold_db: pr.threshold_db,
                    max_time_s: pr.max_time_s,
                }),
        });

        let mixed_phase = if self.mode == RoomEqOptimizationMode::MixedPhase {
            Some(BackendMixedPhaseConfig {
                max_fir_length_ms: self.mixed_phase.max_fir_length_ms,
                pre_ringing_threshold_db: self.mixed_phase.pre_ringing_threshold_db,
                min_spatial_depth: self.mixed_phase.min_spatial_depth,
                phase_smoothing_octaves: self.mixed_phase.phase_smoothing_octaves,
            })
        } else {
            None
        };

        let mixed_config = if self.mode == RoomEqOptimizationMode::Mixed {
            Some(MixedModeConfig {
                crossover_freq: self.mixed_config.crossover_freq,
                crossover_type: self.mixed_config.crossover_type.clone(),
                fir_band: self.mixed_config.fir_band.clone(),
            })
        } else {
            None
        };

        let target_response = if self.target_response.enabled {
            let tr = &self.target_response;
            let shape = match tr.shape.as_str() {
                "flat" => TargetShape::Flat,
                "harman" => TargetShape::Harman,
                "custom" => TargetShape::Custom,
                "file" => TargetShape::File,
                "from_measurement" => TargetShape::FromMeasurement,
                _ => TargetShape::Custom,
            };
            Some(BackendTargetResponseConfig {
                shape,
                slope_db_per_octave: tr.slope_db_per_octave,
                reference_freq: tr.reference_freq,
                curve_path: tr.curve_path.clone(),
                preference: UserPreference {
                    bass_shelf_db: tr.bass_shelf_db,
                    bass_shelf_freq: tr.bass_shelf_freq,
                    treble_shelf_db: tr.treble_shelf_db,
                    treble_shelf_freq: tr.treble_shelf_freq,
                },
                broadband_precorrection: tr.broadband_precorrection,
                role_targets: None,
            })
        } else {
            None
        };

        let excursion_protection = if self.excursion_protection.enabled {
            let filter_type = if self.excursion_protection.filter_type == "bw" {
                HighpassType::Butterworth
            } else {
                HighpassType::LinkwitzRiley
            };
            Some(BackendExcursionProtectionConfig {
                enabled: true,
                auto_detect_f3: self.excursion_protection.auto_detect_f3,
                manual_f3_hz: Some(self.excursion_protection.manual_f3_hz),
                filter_order: self.excursion_protection.filter_order,
                filter_type,
                margin_octaves: self.excursion_protection.margin_octaves,
            })
        } else {
            None
        };

        let schroeder_split = if self.schroeder_split.enabled {
            Some(BackendSchroederSplitConfig {
                enabled: true,
                schroeder_freq: self.schroeder_split.schroeder_freq,
                room_dimensions: None,
                low_freq_config: LowFreqFilterConfig {
                    max_q: self.schroeder_split.low_freq_max_q,
                    min_q: 0.5,
                    allow_boost: self.schroeder_split.low_freq_allow_boost,
                    max_db: self.schroeder_split.low_freq_max_db,
                },
                high_freq_config: HighFreqFilterConfig {
                    max_q: self.schroeder_split.high_freq_max_q,
                    shelving_only: self.schroeder_split.high_freq_shelving_only,
                },
            })
        } else {
            None
        };

        let phase_alignment = if self.phase_alignment.enabled {
            Some(BackendPhaseAlignmentConfig {
                enabled: true,
                min_freq: self.phase_alignment.min_freq,
                max_freq: self.phase_alignment.max_freq,
                optimize_polarity: self.phase_alignment.optimize_polarity,
                max_delay_ms: self.phase_alignment.max_delay_ms,
            })
        } else {
            None
        };

        let has_all_channel_multiseat_policy = !self.multi_seat.all_channel_enabled
            || self.multi_seat.all_channel_strategy != default_all_channel_multiseat_strategy()
            || self.multi_seat.seat_weights.is_some()
            || (self.multi_seat.primary_seat_weight - default_primary_seat_weight()).abs() > 1e-9
            || self.multi_seat.primary_seat != 0
            || (self.multi_seat.max_deviation_db - 6.0).abs() > 1e-9;
        let multi_seat = if self.multi_seat.enabled || has_all_channel_multiseat_policy {
            let strategy = match self.multi_seat.strategy.as_str() {
                "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                "average" => MultiSeatStrategy::Average,
                _ => MultiSeatStrategy::MinimizeVariance,
            };
            Some(BackendMultiSeatConfig {
                enabled: self.multi_seat.enabled,
                strategy,
                primary_seat: self.multi_seat.primary_seat,
                max_deviation_db: self.multi_seat.max_deviation_db,
                optimize_polarity: false,
                allpass_filters_per_sub: 0,
                all_channel_enabled: self.multi_seat.all_channel_enabled,
                all_channel_strategy: match self.multi_seat.all_channel_strategy.as_str() {
                    "weighted_sum" => autoeq::roomeq::MultiMeasurementStrategy::WeightedSum,
                    "minimax" => autoeq::roomeq::MultiMeasurementStrategy::Minimax,
                    "variance_penalized" => {
                        autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized
                    }
                    "average" => autoeq::roomeq::MultiMeasurementStrategy::Average,
                    _ => autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness,
                },
                seat_weights: self.multi_seat.seat_weights.clone(),
                primary_seat_weight: self.multi_seat.primary_seat_weight,
            })
        } else {
            None
        };

        let vog = if self.vog.enabled {
            Some(VoiceOfGodConfig {
                enabled: true,
                reference_channel: self.vog.reference_channel.clone(),
            })
        } else {
            None
        };

        let multi_measurement = if self.multi_measurement.enabled {
            let strategy = match self.multi_measurement.strategy.as_str() {
                "average" => MultiMeasurementStrategy::Average,
                "weighted_sum" => MultiMeasurementStrategy::WeightedSum,
                "minimax" => MultiMeasurementStrategy::Minimax,
                "variance_penalized" => MultiMeasurementStrategy::VariancePenalized,
                "spatial_robustness" => MultiMeasurementStrategy::SpatialRobustness,
                s => panic!("Unknown multi_measurement strategy: {s}"),
            };
            let weights = if self.multi_measurement.weights.is_empty() {
                None
            } else {
                Some(self.multi_measurement.weights.clone())
            };
            Some(MultiMeasurementConfig {
                strategy,
                weights,
                variance_lambda: self.multi_measurement.variance_lambda,
                spatial_robustness: None,
            })
        } else {
            None
        };

        let sub_config = if self.sub_config.enabled {
            Some(SubOptimizerConfig {
                num_filters: self.sub_config.num_filters,
                max_db: self.sub_config.max_db,
                min_db: self.sub_config.min_db,
                min_q: self.sub_config.min_q,
                max_q: self.sub_config.max_q,
            })
        } else {
            None
        };

        let channel_matching = if self.channel_matching.enabled {
            Some(BackendChannelMatchingConfig {
                enabled: true,
                threshold_db: self.channel_matching.threshold_db,
                max_filters: self.channel_matching.max_filters,
            })
        } else {
            None
        };

        BackendOptimizerConfig {
            loss_type: self.loss_type.clone(),
            algorithm: self.algorithm.clone(),
            strategy: self.strategy.clone(),
            num_filters: self.num_filters,
            min_q: self.min_q,
            max_q: self.max_q,
            min_db: self.min_db,
            max_db: self.max_db,
            min_freq: self.min_freq,
            max_freq: self.max_freq,
            max_iter: self.max_iter,
            population: self.population,
            peq_model: self.peq_model.clone(),
            processing_mode,
            fir,
            mixed_phase,
            mixed_config,
            seed: self.seed,
            refine: self.refine,
            local_algo: self.local_algo.clone(),
            psychoacoustic: self.psychoacoustic,
            asymmetric_loss: self.asymmetric_loss,
            tolerance: self.tolerance,
            atolerance: self.atolerance,
            allow_delay: Some(self.allow_delay),
            smooth_n: self.smooth_n,
            target_response,
            excursion_protection,
            schroeder_split,
            phase_alignment,
            multi_seat,
            vog,
            multi_measurement,
            sub_config,
            channel_matching,
            decomposed_correction: Some(DecomposedCorrectionSerdeConfig::default()),
            ..BackendOptimizerConfig::default()
        }
    }

    /// Apply smart defaults based on measurement channel metadata.
    ///
    /// Called after loading measurements to set sensible initial values.
    /// When `imported_from_file` is true, feature toggles are preserved.
    pub fn apply_smart_defaults(&mut self, meta: &ChannelMetadata) {
        // Seed sample rate from playback device when still at default
        if let Some(sr) = meta.playback_sample_rate
            && self.sample_rate == 48000
        {
            self.sample_rate = sr as usize;
        }

        // Loss type is always flat for room EQ
        self.loss_type = "flat".to_string();

        // Only override algorithm/seed defaults when not imported from file
        if !self.imported_from_file {
            self.local_algo = "cobyla".to_string();
            self.refine = true;
            self.seed = None;
        }

        // System type: auto-detect from channel count
        self.system_type = if meta.is_surround() {
            "multichannel".to_string()
        } else {
            "stereo".to_string()
        };

        // Feature flags: only auto-enable when NOT imported from file.
        // When imported, the file's feature state is authoritative
        // (None = disabled, Some = enabled with those params).
        if !self.imported_from_file {
            self.target_response.enabled = true;
            self.target_response.shape = "harman".to_string();
            self.excursion_protection.enabled = true;
            // Schroeder split only makes sense with a subwoofer
            self.schroeder_split.enabled = meta.has_subwoofer();
            self.allow_delay = true;
            self.target_response.broadband_precorrection = true;
            self.vog.enabled = meta.has_height_channels();
            self.vog.reference_channel = if meta.is_home_cinema() {
                "C".to_string()
            } else {
                "L".to_string()
            };
        }
    }
}

/// Compute the average slope for L and R channels in dB/octave.
///
/// Uses linear regression on the 200 Hz – 20 kHz range.
/// Returns `(slope, recommendation_min, recommendation_max)`.
pub fn compute_lr_slope(measurements: &[ChannelMeasurement]) -> Option<(f64, f64, f64)> {
    let lr_names = ["L", "R"];
    let mut slopes = Vec::new();

    for meas in measurements {
        let name_upper = meas.channel_name.to_uppercase();
        if !lr_names.iter().any(|&n| name_upper == n) {
            continue;
        }

        let freqs = &meas.measurement.frequencies;
        let spl = &meas.measurement.magnitude_db;

        let mut log_freqs = Vec::new();
        let mut dbs = Vec::new();

        for (i, &f) in freqs.iter().enumerate() {
            if (200.0..=20000.0).contains(&f)
                && let Some(&db) = spl.get(i)
            {
                log_freqs.push(f64::from(f).log10());
                dbs.push(f64::from(db));
            }
        }

        if log_freqs.len() < 2 {
            continue;
        }

        // Linear regression: db = slope * log_freq + intercept
        let n = log_freqs.len() as f64;
        let sum_x: f64 = log_freqs.iter().sum();
        let sum_y: f64 = dbs.iter().sum();
        let sum_xy: f64 = log_freqs.iter().zip(dbs.iter()).map(|(x, y)| x * y).sum();
        let sum_xx: f64 = log_freqs.iter().map(|x| x * x).sum();

        let denom = n * sum_xx - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            continue;
        }

        // slope in dB per log10(Hz) = dB/decade
        // Convert to dB/octave: 1 octave = log10(2) ≈ 0.301 in log10 space
        let slope_log10 = (n * sum_xy - sum_x * sum_y) / denom;
        let slope_db_per_octave = slope_log10 * std::f64::consts::LOG10_2;

        slopes.push(slope_db_per_octave);
    }

    if slopes.is_empty() {
        return None;
    }

    let avg_slope: f64 = slopes.iter().sum::<f64>() / slopes.len() as f64;
    let recommendation_min = avg_slope * 0.8;
    let recommendation_max = avg_slope * 1.1;

    Some((avg_slope, recommendation_min, recommendation_max))
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

/// Status of the tone-burst delay detection measurement.
///
/// The measurement runs on a background thread (kicked off from the UI).
/// `Running` carries the wall-clock start time in ms so the UI can
/// render a progress estimate as `elapsed / estimated_total` without
/// requiring the engine to surface a progress callback. The estimated
/// total is computed by the UI from `probe_duration_ms` and
/// `silence_duration_ms` × channel count.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DelayDetectionStatus {
    #[default]
    Idle,
    Running {
        /// Milliseconds since the Unix epoch when the measurement was
        /// spawned. Used purely for elapsed-time computation; if the
        /// system clock jumps backward the progress bar may briefly
        /// misreport but nothing else depends on this value.
        started_at_ms: u64,
    },
    Complete,
    Failed(String),
}

impl DelayDetectionStatus {
    /// Estimated fraction of the measurement completed, in `0.0..=1.0`.
    ///
    /// Returns `None` when the status is not `Running` or the estimated
    /// duration is zero. Callers should render a fallback (e.g. an
    /// indeterminate spinner) in that case.
    pub fn progress(&self, estimated_total_ms: u64, now_ms: u64) -> Option<f32> {
        match self {
            Self::Running { started_at_ms } if estimated_total_ms > 0 => {
                let elapsed = now_ms.saturating_sub(*started_at_ms);
                Some((elapsed as f32 / estimated_total_ms as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}

/// Session metadata extracted from a loaded measurements file, used to
/// pre-seed the Delay Detection form so the probe runs with the same
/// device settings the measurements were captured under.
///
/// Every field is optional because older files — and files migrated
/// from other tools — may not carry this metadata.
#[derive(Debug, Clone, Default)]
pub struct DelayDetectionHints {
    /// Canonical channel name order from the recording session, e.g.
    /// `["L", "R", "C", "LFE", "SL", "SR"]` for 5.1. Used by the UI to
    /// re-order or validate the playback channel map.
    pub channel_names: Option<Vec<String>>,
    /// Recording device sample rate in Hz (used for the probe).
    pub sample_rate: Option<u32>,
    /// Playback device name (None = system default).
    pub playback_device_name: Option<String>,
    /// Recording device name (None = system default).
    pub recording_device_name: Option<String>,
    /// Probe results captured during the Recording wizard's Probe step.
    /// When present, the Room EQ Delay step auto-populates arrival
    /// times from these instead of showing "no data".
    pub probe_results: Option<DelayProbeResults>,
}

/// Estimate the total duration of a probe sequence in milliseconds.
///
/// `num_channels` probes + (`num_channels - 1`) gaps + a ~1 s head/tail
/// budget for device startup and stream settling. Used by the UI to
/// turn `DelayDetectionStatus::Running { started_at_ms }` into a
/// progress estimate.
pub fn estimate_probe_sequence_ms(
    num_channels: usize,
    probe_duration_ms: f32,
    silence_duration_ms: f32,
) -> u64 {
    if num_channels == 0 {
        return 0;
    }
    let per_channel = probe_duration_ms as f64 + silence_duration_ms as f64;
    let total = per_channel * num_channels as f64 + 1_000.0;
    total.round().max(0.0) as u64
}

/// Shared state for the Room EQ "Delay Detection" wizard step.
///
/// The UI of both app-tui and app-gpui drives this struct: it carries the
/// probe/device form inputs, the background-measurement status, the raw
/// per-channel detection results from [`DelayProbeResults`], and the
/// user-editable override values that ultimately feed into
/// [`crate::autoeq::run_room_optimization_with_probe_arrivals`].
///
/// Channel identity (name, hardware index) always flows through
/// `results.channels`. We intentionally do **not** carry a parallel
/// `channel_names` vec on this struct: the earlier design had an
/// alignment bug where `probe_arrival_map` zipped `channel_names` with
/// `edited_arrival_ms` and silently truncated on length mismatch.
#[derive(Debug, Clone)]
pub struct DelayDetectionState {
    /// Duration of each narrowband tone-burst in milliseconds.
    /// The default (1000 ms) is long enough for robust cross-correlation
    /// in typical rooms without making the full sweep tediously slow.
    pub probe_duration_ms: f32,
    /// Silence gap between probes in milliseconds. Avoids overlap between
    /// late reflections of one channel and the onset of the next.
    pub silence_duration_ms: f32,
    /// Sample rate used for the probe in Hz. Populated from the loaded
    /// measurement's recording configuration when available, otherwise
    /// defaults to 48 000.
    pub sample_rate: u32,
    /// Playback device name (None = system default).
    pub output_device_name: Option<String>,
    /// Recording device name (None = system default).
    pub input_device_name: Option<String>,
    /// Microphone input channel index (0-based).
    pub input_channel: u16,
    /// Background-measurement status.
    pub status: DelayDetectionStatus,
    /// Raw detection results (populated on success). Cleared on Reset / new
    /// run. Contains per-channel arrival_ms, gain_db, snr_db, and the
    /// auto-computed `alignment_delays_ms` vector. This is the authority
    /// on channel identity — `edited_arrival_ms[i]` corresponds to
    /// `results.channels[i]`.
    pub results: Option<DelayProbeResults>,
    /// User-editable per-channel arrival times in milliseconds (seeded
    /// from `results.channels[i].arrival_ms` after a successful
    /// measurement). Indices mirror `results.channels` exactly. The
    /// optimizer consumes these values (not `alignment_delays_ms`) so the
    /// downstream speaker_eq path can compute consistent alignment.
    pub edited_arrival_ms: Vec<f64>,
}

impl Default for DelayDetectionState {
    fn default() -> Self {
        Self {
            probe_duration_ms: 1000.0,
            silence_duration_ms: 500.0,
            sample_rate: 48_000,
            output_device_name: None,
            input_device_name: None,
            input_channel: 0,
            status: DelayDetectionStatus::Idle,
            results: None,
            edited_arrival_ms: Vec::new(),
        }
    }
}

impl DelayDetectionState {
    /// Seed `edited_arrival_ms` from a fresh set of probe results.
    ///
    /// Called after a successful measurement so the override editor has
    /// sensible initial values. The UI may then let the user tweak these
    /// before they flow into `run_room_optimization_with_probe_arrivals`.
    pub fn apply_results(&mut self, results: DelayProbeResults) {
        self.edited_arrival_ms = results.channels.iter().map(|c| c.arrival_ms).collect();
        self.results = Some(results);
        self.status = DelayDetectionStatus::Complete;
    }

    /// Build the per-channel arrival-time map used by
    /// [`crate::autoeq::run_room_optimization_with_probe_arrivals`].
    ///
    /// Returns `None` if the measurement has not completed or the user
    /// has cleared it. Channels with non-finite overrides (e.g. the user
    /// blanked an entry) are skipped so the optimizer falls back to
    /// WAV-onset detection for them. Channel identity comes from
    /// `results.channels[i].channel_name` — this is the authoritative
    /// source — and `edited_arrival_ms[i]` is read by position.
    pub fn probe_arrival_map(&self) -> Option<std::collections::HashMap<String, f64>> {
        if !matches!(self.status, DelayDetectionStatus::Complete) {
            return None;
        }
        let results = self.results.as_ref()?;
        let mut map = std::collections::HashMap::with_capacity(results.channels.len());
        for (i, ch) in results.channels.iter().enumerate() {
            let arrival = self
                .edited_arrival_ms
                .get(i)
                .copied()
                .unwrap_or(ch.arrival_ms);
            if arrival.is_finite() {
                map.insert(ch.channel_name.clone(), arrival);
            }
        }
        if map.is_empty() { None } else { Some(map) }
    }

    /// Recompute per-channel alignment delays from the current
    /// `edited_arrival_ms`. Used by the UI to show a live "Align ms"
    /// column that reflects user overrides instead of the stale values
    /// the engine computed from the raw measurement.
    ///
    /// Returns a vector indexed the same way as `results.channels`.
    /// Empty when there are no results.
    pub fn edited_alignment_delays_ms(&self) -> Vec<f64> {
        let Some(results) = self.results.as_ref() else {
            return Vec::new();
        };
        let arrivals: Vec<f64> = results
            .channels
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                self.edited_arrival_ms
                    .get(i)
                    .copied()
                    .unwrap_or(ch.arrival_ms)
            })
            .collect();
        if arrivals.is_empty() {
            return Vec::new();
        }
        let max = arrivals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        arrivals.iter().map(|a| max - a).collect()
    }
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
    /// Broadband pre-correction filters (lowshelf/highshelf), separate from main EQ
    pub broadband_filters: Vec<EqFilterConfig>,
    pub crossover_freqs: Option<Vec<f64>>,
    pub driver_gains: Option<Vec<f64>>,
    pub original_response: Option<Vec<(f64, f64)>>,
    pub corrected_response: Option<Vec<(f64, f64)>>,
    pub normalized_response: Option<Vec<(f64, f64)>>,
    /// Target curve points (frequency_hz, level_db)
    pub target_curve: Option<Vec<(f64, f64)>>,
    /// Group delay before correction (frequency_hz, delay_ms)
    pub group_delay_before: Option<Vec<(f64, f64)>>,
    /// Group delay after correction (frequency_hz, delay_ms)
    pub group_delay_after: Option<Vec<(f64, f64)>>,
    /// Phase response before correction (frequency_hz, phase_radians)
    pub phase_response_before: Option<Vec<(f64, f64)>>,
    /// Phase response after correction (frequency_hz, phase_radians)
    pub phase_response_after: Option<Vec<(f64, f64)>>,
    /// Impulse response after correction (sample_index, amplitude)
    pub impulse_response: Option<Vec<(f64, f64)>>,
}

// DSP chain output types are the canonical `autoeq::roomeq` types — we
// re-export them here so downstream code keeps referring to
// `sotf_audio_player::room_eq_types::{DspChainOutput, ChannelDspChain, ...}`
// but we don't drop any fields on the floor (initial/final curves,
// target curve, pre/post IR, loss_type, inter-channel deviation, EPA
// metrics). Previously we had parallel stripped copies of these structs
// and a lossy field-by-field conversion in the Step-4 optimiser; that
// meant the Review step plot silently lost curves that the optimiser
// had already computed.

/// DSP plugin configuration (alias for `autoeq::roomeq::PluginConfigWrapper`).
pub type DspPluginConfig = autoeq::roomeq::PluginConfigWrapper;

/// DSP chain metadata (alias for `autoeq::roomeq::OptimizationMetadata`).
pub type DspChainMetadata = autoeq::roomeq::OptimizationMetadata;

pub use autoeq::roomeq::{ChannelDspChain, DriverDspChain, DspChainOutput};

/// Extension trait for `DspChainOutput` providing player-side helpers.
///
/// Lives here (not in `autoeq`) because it's a player concern: does this
/// chain correspond to a linear rack, or does it need a parallel
/// multi-driver graph? The autoeq crate doesn't know or care about the
/// player's rack model.
pub trait DspChainOutputExt {
    /// Returns true if the DSP output can be applied to a linear rack
    /// (no multi-driver crossovers requiring parallel paths).
    fn is_rack_compatible(&self) -> bool;

    /// Returns true when this output needs graph playback to preserve routing.
    fn requires_room_eq_graph(&self) -> bool;
}

impl DspChainOutputExt for DspChainOutput {
    fn is_rack_compatible(&self) -> bool {
        !self.requires_room_eq_graph()
    }

    fn requires_room_eq_graph(&self) -> bool {
        requires_room_eq_graph(self)
    }
}

/// Returns true when a RoomEQ result cannot be represented as a single linear rack.
pub fn requires_room_eq_graph(output: &DspChainOutput) -> bool {
    !output.global_plugins.is_empty()
        || output
            .channels
            .values()
            .any(|chain| chain.drivers.is_some())
        || output
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.bass_management.as_ref())
            .and_then(|report| report.routing_graph.as_ref())
            .is_some_and(|graph| !graph.routes.is_empty())
}

/// Build an engine graph that preserves RoomEQ routed bass management.
///
/// Routed bass management is represented as parallel route branches:
/// sparse matrix isolation, route crossover, route gain/polarity, and route
/// delay. Branch outputs are summed by the graph host. Per-output correction
/// EQ/convolution-style plugins are then isolated per physical output so the
/// exported RoomEQ curves and graph playback stay aligned.
pub fn build_room_eq_plugin_graph_config(
    output: &DspChainOutput,
    _sample_rate: f64,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    let routed_graph = output
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.bass_management.as_ref())
        .and_then(|report| report.routing_graph.as_ref())
        .filter(|graph| !graph.routes.is_empty());

    if let Some(graph) = routed_graph {
        return build_routed_room_eq_graph(output, graph);
    }

    build_linear_room_eq_graph(output)
}

fn build_routed_room_eq_graph(
    output: &DspChainOutput,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    let channel_count = routed_graph_channel_count(output, graph);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;

    let mut add_node = |plugin_type: String, parameters: serde_json::Value| -> usize {
        let id = next_id;
        next_id += 1;
        nodes.push(PluginGraphNodeConfig {
            id,
            plugin_type,
            parameters,
            input_channels: channel_count,
        });
        id
    };

    let mut global_tail = None;
    for plugin in output
        .global_plugins
        .iter()
        .filter(|plugin| !is_route_replaced_global_plugin(plugin))
    {
        let node = add_node(plugin.plugin_type.clone(), plugin.parameters.clone());
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
    }

    let mut route_tails = Vec::new();
    for route in &graph.routes {
        let matrix_gain = route_matrix_gain(route);
        let mut prev = add_node(
            "matrix".to_string(),
            serde_json::json!({
                "label": format!("room_eq_route_{}_{}_to_{}", route.route_kind, route.source_channel, route.destination),
                "input_channel_map": [route.source_index],
                "output_channel_map": [route.destination_index],
                "matrix": [matrix_gain as f32],
                "metadata": {
                    "route_kind": route.route_kind,
                    "group_id": route.group_id,
                    "source": route.source_channel,
                    "destination": route.destination,
                },
            }),
        );
        if let Some(global_tail) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: global_tail,
                to_node: prev,
            });
        }

        for plugin in pre_route_plugins_for_route(output, route, graph) {
            let node = add_node(plugin.plugin_type.clone(), plugin.parameters.clone());
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }

        if let Some(freq) = route.high_pass_hz {
            let node = add_node(
                "crossover".to_string(),
                serde_json::json!({
                    "type": route.crossover_type,
                    "frequency": freq,
                    "output": "high",
                    "label": "room_eq_route_highpass",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }
        if let Some(freq) = route.low_pass_hz {
            let node = add_node(
                "crossover".to_string(),
                serde_json::json!({
                    "type": route.crossover_type,
                    "frequency": freq,
                    "output": "low",
                    "label": "room_eq_route_lowpass",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }

        if route.polarity_inverted
            || (route.gain_db.abs() > 0.01 && (matrix_gain - 1.0).abs() < 1e-6)
        {
            let node = add_node(
                "gain".to_string(),
                serde_json::json!({
                    "gain_db": if (matrix_gain - 1.0).abs() < 1e-6 { route.gain_db } else { 0.0 },
                    "invert": route.polarity_inverted,
                    "label": "room_eq_route_gain_polarity",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }

        if route.delay_ms.abs() > 0.001 {
            let node = add_node(
                "delay".to_string(),
                serde_json::json!({
                    "delay_ms": route.delay_ms,
                    "label": "room_eq_route_delay",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }
        route_tails.push(prev);
    }

    let sum_anchor = add_node(
        "matrix".to_string(),
        identity_matrix_parameters(channel_count, "room_eq_route_sum_anchor"),
    );
    for route_tail in route_tails {
        edges.push(PluginGraphEdgeConfig {
            from_node: route_tail,
            to_node: sum_anchor,
        });
    }

    let output_order = if graph.output_channels.is_empty() {
        sorted_channel_names(output)
    } else {
        graph.output_channels.clone()
    };
    let mut correction_tails = Vec::new();
    for (channel_index, channel_name) in output_order.iter().enumerate() {
        let isolate = add_node(
            "matrix".to_string(),
            serde_json::json!({
                "label": format!("room_eq_output_isolate_{channel_name}"),
                "input_channel_map": [channel_index],
                "output_channel_map": [channel_index],
                "matrix": [1.0],
            }),
        );
        edges.push(PluginGraphEdgeConfig {
            from_node: sum_anchor,
            to_node: isolate,
        });
        let mut prev = isolate;
        for plugin in post_route_plugins_for_channel(output, channel_name, graph) {
            let node = add_node(plugin.plugin_type.clone(), plugin.parameters.clone());
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }
        correction_tails.push(prev);
    }

    if correction_tails.is_empty() {
        correction_tails.push(sum_anchor);
    }

    Ok(PluginGraphConfig { nodes, edges })
}

fn build_linear_room_eq_graph(
    output: &DspChainOutput,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    let channel_count = output.channels.len().max(2);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut prev = None;

    for (id, plugin) in output
        .global_plugins
        .iter()
        .chain(
            output
                .channels
                .values()
                .next()
                .into_iter()
                .flat_map(|ch| ch.plugins.iter()),
        )
        .enumerate()
    {
        nodes.push(PluginGraphNodeConfig {
            id,
            plugin_type: plugin.plugin_type.clone(),
            parameters: plugin.parameters.clone(),
            input_channels: channel_count,
        });
        if let Some(from_node) = prev {
            edges.push(PluginGraphEdgeConfig {
                from_node,
                to_node: id,
            });
        }
        prev = Some(id);
    }

    if nodes.is_empty() {
        anyhow::bail!("No plugins in DSP output");
    }

    Ok(PluginGraphConfig { nodes, edges })
}

fn routed_graph_channel_count(
    output: &DspChainOutput,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> usize {
    let route_max = graph
        .routes
        .iter()
        .flat_map(|route| [route.source_index, route.destination_index])
        .max()
        .map(|idx| idx + 1)
        .unwrap_or(0);
    route_max
        .max(graph.input_channels.len())
        .max(graph.output_channels.len())
        .max(output.channels.len())
        .max(1)
}

fn sorted_channel_names(output: &DspChainOutput) -> Vec<String> {
    let mut names: Vec<_> = output.channels.keys().cloned().collect();
    names.sort();
    names
}

fn route_matrix_gain(route: &autoeq::roomeq::BassManagementRoute) -> f64 {
    if route.matrix_gain.abs() <= f64::EPSILON && route.gain_linear.abs() > f64::EPSILON {
        route.gain_linear
    } else {
        route.matrix_gain
    }
}

fn identity_matrix_parameters(channel_count: usize, label: &str) -> serde_json::Value {
    serde_json::json!({
        "label": label,
        "input_channels": channel_count,
        "output_channels": channel_count,
        "matrix": identity_matrix(channel_count),
    })
}

fn identity_matrix(channel_count: usize) -> Vec<f32> {
    let mut matrix = vec![0.0; channel_count * channel_count];
    for idx in 0..channel_count {
        matrix[idx * channel_count + idx] = 1.0;
    }
    matrix
}

fn is_route_replaced_global_plugin(plugin: &DspPluginConfig) -> bool {
    plugin.plugin_type == "matrix"
        && (plugin
            .parameters
            .get("label")
            .and_then(|value| value.as_str())
            == Some("home_cinema_bass_management")
            || plugin
                .parameters
                .get("metadata")
                .and_then(|metadata| metadata.get("purpose"))
                .and_then(|value| value.as_str())
                == Some("home_cinema_bass_management"))
}

fn pre_route_plugins_for_route<'a>(
    output: &'a DspChainOutput,
    route: &autoeq::roomeq::BassManagementRoute,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> Vec<&'a DspPluginConfig> {
    let channel_name = route.pre_chain_channel.as_deref().unwrap_or_else(|| {
        if is_bass_route(route) {
            &graph.physical_sub_output
        } else {
            &route.source_channel
        }
    });
    let Some(chain) = output.channels.get(channel_name) else {
        return Vec::new();
    };
    let staged: Vec<_> = chain
        .plugins
        .iter()
        .filter(|plugin| plugin_stage(plugin) == Some("pre_route"))
        .collect();
    if !staged.is_empty() {
        return staged;
    }
    chain
        .plugins
        .iter()
        .take_while(|plugin| !is_route_owned_plugin(plugin) && plugin.plugin_type != "crossover")
        .collect()
}

fn post_route_plugins_for_channel<'a>(
    output: &'a DspChainOutput,
    channel_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> Vec<&'a DspPluginConfig> {
    let post_chain_name = graph
        .routes
        .iter()
        .find(|route| route.destination == channel_name)
        .and_then(|route| route.post_chain_channel.as_deref())
        .unwrap_or(channel_name);
    let mut staged = plugins_for_post_chain_name(output, post_chain_name);
    if staged.is_empty() && is_bass_output_channel(channel_name, graph) {
        staged = plugins_for_post_chain_name(output, &graph.physical_sub_output);
    }
    if !staged.is_empty() {
        return staged;
    }
    let chain = output.channels.get(post_chain_name).or_else(|| {
        is_bass_output_channel(channel_name, graph)
            .then(|| output.channels.get(&graph.physical_sub_output))
            .flatten()
    });
    let Some(chain) = chain else {
        return Vec::new();
    };
    let Some(split_idx) = chain
        .plugins
        .iter()
        .position(|plugin| is_route_owned_plugin(plugin) || plugin.plugin_type == "crossover")
    else {
        return chain.plugins.iter().collect();
    };

    let mut start = split_idx + 1;
    let mut skipped_route_gain = false;
    let mut skipped_route_delay = false;
    while let Some(plugin) = chain.plugins.get(start) {
        if is_route_owned_plugin(plugin) {
            start += 1;
            continue;
        }
        let route_owned = match plugin.plugin_type.as_str() {
            "crossover" => true,
            "gain" if !skipped_route_gain => {
                let owned = route_owns_gain_plugin(plugin, channel_name, post_chain_name, graph);
                skipped_route_gain = owned;
                owned
            }
            "delay" if !skipped_route_delay => {
                let owned = route_owns_delay_plugin(plugin, channel_name, post_chain_name, graph);
                skipped_route_delay = owned;
                owned
            }
            _ => false,
        };
        if !route_owned {
            break;
        }
        start += 1;
    }

    chain.plugins[start..].iter().collect()
}

fn is_route_owned_plugin(plugin: &DspPluginConfig) -> bool {
    plugin_stage(plugin) == Some("route_owned")
        || plugin
            .parameters
            .get("label")
            .and_then(|value| value.as_str())
            == Some("room_eq_route_owned")
}

fn plugin_stage(plugin: &DspPluginConfig) -> Option<&str> {
    plugin
        .parameters
        .get("room_eq_stage")
        .and_then(|value| value.as_str())
}

fn plugins_for_post_chain_name<'a>(
    output: &'a DspChainOutput,
    post_chain_name: &str,
) -> Vec<&'a DspPluginConfig> {
    output
        .channels
        .get(post_chain_name)
        .map(|chain| {
            chain
                .plugins
                .iter()
                .filter(|plugin| plugin_stage(plugin) == Some("post_route"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn is_bass_route(route: &autoeq::roomeq::BassManagementRoute) -> bool {
    route.route_kind == "redirected_bass_lowpass_to_sub" || route.route_kind == "lfe_lowpass_to_sub"
}

fn is_bass_output_channel(
    channel_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> bool {
    graph
        .routes
        .iter()
        .any(|route| is_bass_route(route) && route.destination == channel_name)
}

fn route_owns_gain_plugin(
    plugin: &DspPluginConfig,
    channel_name: &str,
    post_chain_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> bool {
    let gain_db = plugin
        .parameters
        .get("gain_db")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let invert = plugin
        .parameters
        .get("invert")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let bass_output = is_bass_output_channel(channel_name, graph);
    graph
        .routes
        .iter()
        .filter(|route| {
            route.destination == channel_name
                || route.post_chain_channel.as_deref() == Some(post_chain_name)
        })
        .any(|route| {
            let exact_route_match =
                (route.gain_db - gain_db).abs() <= 0.01 && route.polarity_inverted == invert;
            if exact_route_match {
                return true;
            }

            // Bass routes can encode the shared sub gain in the route matrix
            // instead of a separate gain node. Treat only the first
            // post-crossover gain as route-owned; later trims remain output
            // correction plugins and are preserved by the caller's state.
            bass_output
                && is_bass_route(route)
                && (route.gain_db.abs() > 0.01 || route.polarity_inverted)
                && (gain_db.abs() > 0.01 || invert)
        })
}

fn route_owns_delay_plugin(
    plugin: &DspPluginConfig,
    channel_name: &str,
    post_chain_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> bool {
    let Some(delay_ms) = plugin
        .parameters
        .get("delay_ms")
        .and_then(|value| value.as_f64())
    else {
        return false;
    };
    let bass_output = is_bass_output_channel(channel_name, graph);
    graph
        .routes
        .iter()
        .filter(|route| {
            route.destination == channel_name
                || route.post_chain_channel.as_deref() == Some(post_chain_name)
        })
        .any(|route| {
            (route.delay_ms - delay_ms).abs() <= 0.001
                || (bass_output
                    && is_bass_route(route)
                    && route.delay_ms.abs() > 0.001
                    && delay_ms.abs() > 0.001)
        })
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
                TargetCurveControlPoint::new(160.0, 0.5),
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
        const MIN_FREQ: f64 = math_audio_iir_fir::AUDIBLE_MIN_FREQ;
        const MAX_FREQ: f64 = math_audio_iir_fir::AUDIBLE_MAX_FREQ;

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

/// Parse EQ filters from JSON array.
///
/// Accepts both autoeq optimizer output format (`"freq"`, `"db_gain"`)
/// and engine format (`"frequency"`, `"gain_db"`).
pub fn parse_eq_filters_from_json(filters_json: &[serde_json::Value]) -> Vec<EQFilter> {
    filters_json
        .iter()
        .map(|filter| {
            let filter_type_str = filter
                .get("filter_type")
                .and_then(|t| t.as_str())
                .unwrap_or("peak");
            let filter_type = match filter_type_str.to_lowercase().as_str() {
                "peak" | "pk" => BiquadFilterType::Peak,
                "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                "highshelf" | "hs" => BiquadFilterType::Highshelf,
                "lowpass" | "lp" => BiquadFilterType::Lowpass,
                "highpass" | "hp" => BiquadFilterType::Highpass,
                "notch" => BiquadFilterType::Notch,
                _ => BiquadFilterType::Peak,
            };
            let frequency = filter
                .get("frequency")
                .or_else(|| filter.get("freq"))
                .and_then(|f| f.as_f64())
                .unwrap_or(1000.0);
            let q = filter.get("q").and_then(|q| q.as_f64()).unwrap_or(1.0);
            let gain_db = filter
                .get("gain_db")
                .or_else(|| filter.get("db_gain"))
                .and_then(|g| g.as_f64())
                .unwrap_or(0.0);
            EQFilter::new(filter_type, frequency, q, gain_db)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_types::DelayProbeChannelResult;

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

    #[test]
    fn multi_measurement_ui_config_serde_roundtrip() {
        let config = MultiMeasurementUiConfig {
            enabled: true,
            strategy: "weighted_sum".to_string(),
            variance_lambda: 2.5,
            weights: vec![0.3, 0.7],
        };
        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: MultiMeasurementUiConfig = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.enabled);
        assert_eq!(roundtrip.strategy, "weighted_sum");
        assert_eq!(roundtrip.variance_lambda, 2.5);
        assert_eq!(roundtrip.weights, vec![0.3, 0.7]);
    }

    #[test]
    fn multi_measurement_ui_config_default_deserialize() {
        // Ensure missing multi_measurement field in existing configs deserializes to default
        let json = r#"{
            "mode": "Iir",
            "multi_speaker_mode": "Combined",
            "algorithm": "autoeq:de",
            "num_filters": 7,
            "min_q": 0.5, "max_q": 6.0,
            "min_db": -12.0, "max_db": 4.0,
            "min_freq": 20.0, "max_freq": 1600.0,
            "max_iter": 50000,
            "peq_model": "pk",
            "population": 300,
            "refine": false,
            "local_algo": "cobyla",
            "loss_type": "flat",
            "psychoacoustic": true,
            "asymmetric_loss": true,
            "target_curve": "flat",
            "system_type": "stereo"
        }"#;
        let config: RoomEqOptimizerConfig = serde_json::from_str(json).unwrap();
        assert!(!config.multi_measurement.enabled);
        assert_eq!(config.multi_measurement.strategy, "average");
        assert_eq!(config.multi_measurement.variance_lambda, 1.0);
        assert!(config.multi_measurement.weights.is_empty());
    }

    #[test]
    fn multi_measurement_strategy_strings_match_constants() {
        let valid_strategies = ["average", "weighted_sum", "minimax", "variance_penalized"];
        let default = MultiMeasurementUiConfig::default();
        assert!(
            valid_strategies.contains(&default.strategy.as_str()),
            "Default strategy '{}' not in valid set",
            default.strategy
        );
    }

    // ── DelayDetectionState tests ────────────────────────────────────────

    fn make_results(entries: &[(&str, f64, f64, f64)]) -> DelayProbeResults {
        DelayProbeResults {
            channels: entries
                .iter()
                .enumerate()
                .map(|(i, (name, arrival, gain, snr))| DelayProbeChannelResult {
                    channel_name: (*name).to_string(),
                    channel_index: i,
                    arrival_ms: *arrival,
                    gain_db: *gain,
                    snr_db: *snr,
                })
                .collect(),
            sample_rate: 48_000,
            alignment_delays_ms: entries
                .iter()
                .map(|(_, a, _, _)| {
                    let max = entries
                        .iter()
                        .map(|(_, a, _, _)| *a)
                        .fold(f64::NEG_INFINITY, f64::max);
                    max - a
                })
                .collect(),
        }
    }

    #[test]
    fn probe_arrival_map_returns_none_when_idle() {
        let dd = DelayDetectionState::default();
        assert_eq!(dd.status, DelayDetectionStatus::Idle);
        assert!(dd.probe_arrival_map().is_none());
    }

    #[test]
    fn probe_arrival_map_returns_none_when_failed() {
        let mut dd = DelayDetectionState::default();
        dd.status = DelayDetectionStatus::Failed("mic unplugged".to_string());
        assert!(dd.probe_arrival_map().is_none());
    }

    #[test]
    fn apply_results_populates_edited_arrivals_and_sets_complete() {
        let mut dd = DelayDetectionState::default();
        let results = make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
            ("C", 6.0, -3.2, 12.0),
        ]);
        dd.apply_results(results);

        assert!(matches!(dd.status, DelayDetectionStatus::Complete));
        assert_eq!(dd.edited_arrival_ms, vec![5.0, 8.0, 6.0]);
        assert_eq!(dd.results.as_ref().unwrap().channels.len(), 3);
    }

    #[test]
    fn probe_arrival_map_uses_results_channels_as_source_of_truth() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
        ]));
        let map = dd.probe_arrival_map().expect("should produce map");
        assert_eq!(map.len(), 2);
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["R"], 8.0);
    }

    #[test]
    fn probe_arrival_map_respects_user_edits() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
        ]));
        dd.edited_arrival_ms[1] = 9.5; // user bumped R
        let map = dd.probe_arrival_map().unwrap();
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["R"], 9.5);
    }

    #[test]
    fn probe_arrival_map_skips_non_finite_values() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
            ("C", 6.0, -3.2, 12.0),
        ]));
        dd.edited_arrival_ms[1] = f64::NAN; // user cleared R
        let map = dd.probe_arrival_map().unwrap();
        assert!(!map.contains_key("R"));
        assert_eq!(map.len(), 2);
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["C"], 6.0);
    }

    #[test]
    fn probe_arrival_map_uses_raw_arrival_when_edited_vec_shorter() {
        // Simulate a corrupted state: edited_arrival_ms was cleared but
        // results still present. `probe_arrival_map` must fall back to
        // the raw measured arrival for rows past the edit cursor.
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
        ]));
        dd.edited_arrival_ms.truncate(1);
        let map = dd.probe_arrival_map().unwrap();
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["R"], 8.0);
    }

    #[test]
    fn edited_alignment_delays_track_user_overrides() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
            ("C", 6.0, -3.2, 12.0),
        ]));
        // Initially R is slowest (8.0) → L gets 3, R gets 0, C gets 2.
        let initial = dd.edited_alignment_delays_ms();
        assert!((initial[0] - 3.0).abs() < 1e-9);
        assert!((initial[1] - 0.0).abs() < 1e-9);
        assert!((initial[2] - 2.0).abs() < 1e-9);

        // User moves C to 10 ms → C is now slowest, all others rebase.
        dd.edited_arrival_ms[2] = 10.0;
        let updated = dd.edited_alignment_delays_ms();
        assert!((updated[0] - 5.0).abs() < 1e-9);
        assert!((updated[1] - 2.0).abs() < 1e-9);
        assert!((updated[2] - 0.0).abs() < 1e-9);
    }

    #[test]
    fn status_progress_returns_none_when_idle_or_failed() {
        let idle = DelayDetectionStatus::Idle;
        assert_eq!(idle.progress(10_000, 5_000), None);
        let failed = DelayDetectionStatus::Failed("x".to_string());
        assert_eq!(failed.progress(10_000, 5_000), None);
    }

    #[test]
    fn status_progress_computes_fraction_when_running() {
        let running = DelayDetectionStatus::Running {
            started_at_ms: 1_000,
        };
        // 3000 ms elapsed out of 10000 estimated = 30%
        let p = running.progress(10_000, 4_000).unwrap();
        assert!((p - 0.3).abs() < 1e-6);
        // Clamps to 1.0 after the estimated total elapses.
        let p = running.progress(10_000, 50_000).unwrap();
        assert_eq!(p, 1.0);
    }

    #[test]
    fn status_progress_returns_none_for_zero_total() {
        let running = DelayDetectionStatus::Running { started_at_ms: 0 };
        assert_eq!(running.progress(0, 1000), None);
    }

    #[test]
    fn estimate_probe_sequence_ms_sums_channels_gaps_and_headroom() {
        // 3 channels × (1000 ms probe + 500 ms gap) + 1000 ms head/tail
        let total = estimate_probe_sequence_ms(3, 1000.0, 500.0);
        assert_eq!(total, 3 * 1500 + 1000);
    }

    #[test]
    fn estimate_probe_sequence_ms_zero_channels_is_zero() {
        assert_eq!(estimate_probe_sequence_ms(0, 1000.0, 500.0), 0);
    }

    // =========================================================================
    // parse_eq_filters_from_json tests
    // =========================================================================

    #[test]
    fn test_parse_filters_autoeq_format() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak", "freq": 200.0, "q": 2.0, "db_gain": -5.0}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].frequency, 200.0);
        assert_eq!(filters[0].q, 2.0);
        assert_eq!(filters[0].gain_db, -5.0);
        assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
    }

    #[test]
    fn test_parse_filters_engine_format() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak", "frequency": 100.0, "q": 1.5, "gain_db": -3.0}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].frequency, 100.0);
        assert_eq!(filters[0].q, 1.5);
        assert_eq!(filters[0].gain_db, -3.0);
    }

    #[test]
    fn test_parse_filters_all_filter_types() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "pk", "freq": 200.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lowshelf", "freq": 300.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "ls", "freq": 400.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "highshelf", "freq": 500.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "hs", "freq": 600.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lowpass", "freq": 700.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lp", "freq": 800.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "highpass", "freq": 900.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "hp", "freq": 1000.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "notch", "freq": 1100.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "unknown_type", "freq": 1200.0, "q": 1.0, "db_gain": 0.0}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 12);
        assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
        assert_eq!(filters[1].filter_type, BiquadFilterType::Peak);
        assert_eq!(filters[2].filter_type, BiquadFilterType::Lowshelf);
        assert_eq!(filters[3].filter_type, BiquadFilterType::Lowshelf);
        assert_eq!(filters[4].filter_type, BiquadFilterType::Highshelf);
        assert_eq!(filters[5].filter_type, BiquadFilterType::Highshelf);
        assert_eq!(filters[6].filter_type, BiquadFilterType::Lowpass);
        assert_eq!(filters[7].filter_type, BiquadFilterType::Lowpass);
        assert_eq!(filters[8].filter_type, BiquadFilterType::Highpass);
        assert_eq!(filters[9].filter_type, BiquadFilterType::Highpass);
        assert_eq!(filters[10].filter_type, BiquadFilterType::Notch);
        assert_eq!(filters[11].filter_type, BiquadFilterType::Peak); // unknown → Peak
    }

    #[test]
    fn test_parse_filters_missing_fields_use_defaults() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak"}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].frequency, 1000.0);
        assert_eq!(filters[0].q, 1.0);
        assert_eq!(filters[0].gain_db, 0.0);
    }

    #[test]
    fn test_parse_filters_empty_array() {
        let json: Vec<serde_json::Value> = Vec::new();
        let filters = parse_eq_filters_from_json(&json);
        assert!(filters.is_empty());
    }

    // =========================================================================
    // is_rack_compatible tests
    // =========================================================================

    use super::{DspChainOutputExt, build_room_eq_plugin_graph_config};
    use autoeq::roomeq::{
        BassManagementReport, BassManagementRoute, BassManagementRoutingGraph,
        BassManagementSignalFlowEntry, HomeCinemaRole, OptimizationMetadata, PluginConfigWrapper,
    };

    /// Build a bare `ChannelDspChain` with all optional curve/IR fields
    /// defaulted to `None`. The `is_rack_compatible` check only looks at
    /// `drivers`, so the rest of the fields are irrelevant here and we
    /// don't want to repeat them at every call site.
    fn bare_chain(name: &str, drivers: Option<Vec<DriverDspChain>>) -> ChannelDspChain {
        ChannelDspChain {
            channel: name.to_string(),
            plugins: vec![],
            drivers,
            initial_curve: None,
            final_curve: None,
            eq_response: None,
            target_curve: None,
            pre_ir: None,
            post_ir: None,
        }
    }

    fn bare_driver(name: &str, index: usize) -> DriverDspChain {
        DriverDspChain {
            name: name.to_string(),
            index,
            plugins: vec![],
            initial_curve: None,
        }
    }

    fn bare_output(channels: Vec<(String, ChannelDspChain)>) -> DspChainOutput {
        DspChainOutput {
            version: "1.0.0".to_string(),
            global_plugins: Vec::new(),
            channels: channels.into_iter().collect(),
            metadata: None,
        }
    }

    #[test]
    fn test_is_rack_compatible_no_drivers() {
        let output = bare_output(vec![
            ("L".to_string(), bare_chain("L", None)),
            ("R".to_string(), bare_chain("R", None)),
        ]);
        assert!(output.is_rack_compatible());
    }

    #[test]
    fn test_is_rack_compatible_with_drivers() {
        let output = bare_output(vec![(
            "L".to_string(),
            bare_chain("L", Some(vec![bare_driver("woofer", 0)])),
        )]);
        assert!(!output.is_rack_compatible());
    }

    #[test]
    fn test_is_rack_compatible_mixed() {
        let output = bare_output(vec![
            ("L".to_string(), bare_chain("L", None)),
            (
                "R".to_string(),
                bare_chain("R", Some(vec![bare_driver("woofer", 0)])),
            ),
        ]);
        assert!(!output.is_rack_compatible());
    }

    #[test]
    fn test_is_rack_compatible_empty() {
        let output = bare_output(vec![]);
        assert!(output.is_rack_compatible());
    }

    fn routed_bass_output() -> DspChainOutput {
        let mut output = bare_output(vec![
            (
                "L".to_string(),
                ChannelDspChain {
                    plugins: vec![
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "gain_db": -1.0,
                                "room_eq_stage": "pre_route"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "pre_room_eq",
                                "room_eq_stage": "pre_route",
                                "filters": []
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({
                                "type": "LR24",
                                "frequency": 80.0,
                                "output": "high"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "delay".to_string(),
                            parameters: serde_json::json!({
                                "delay_ms": 2.0,
                                "room_eq_stage": "route_owned"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "label": "post_main_trim",
                                "room_eq_stage": "post_route",
                                "gain_db": -0.75
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "post_room_eq",
                                "room_eq_stage": "post_route",
                                "filters": []
                            }),
                        },
                    ],
                    ..bare_chain("L", None)
                },
            ),
            (
                "Sub".to_string(),
                ChannelDspChain {
                    plugins: vec![
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_pre_trim",
                                "room_eq_stage": "pre_route",
                                "gain_db": -0.5
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_pre_room_eq",
                                "room_eq_stage": "pre_route",
                                "filters": []
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({
                                "type": "LR24",
                                "frequency": 80.0,
                                "output": "low"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "room_eq_stage": "route_owned",
                                "gain_db": -3.0,
                                "invert": true
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "delay".to_string(),
                            parameters: serde_json::json!({
                                "delay_ms": 4.0,
                                "room_eq_stage": "route_owned"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_post_trim",
                                "room_eq_stage": "post_route",
                                "gain_db": -0.25
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_post_room_eq",
                                "room_eq_stage": "post_route",
                                "filters": []
                            }),
                        },
                    ],
                    ..bare_chain("Sub", None)
                },
            ),
        ]);
        output.metadata = Some(OptimizationMetadata {
            pre_score: 1.0,
            post_score: 0.5,
            algorithm: "test".to_string(),
            loss_type: None,
            iterations: 1,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: None,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: Some(BassManagementReport {
                enabled: true,
                crossover_type: "LR24".to_string(),
                crossover_frequency_hz: Some(80.0),
                redirected_bass_enabled: true,
                lfe_channel: "LFE".to_string(),
                lfe_playback_gain_db: 10.0,
                lfe_gain_applied_to_chain: false,
                sub_trim_db: 0.0,
                max_sub_boost_db: 6.0,
                headroom_margin_db: -3.0,
                applied_sub_gain_db: Some(0.0),
                gain_limited: false,
                physical_sub_output: "Sub".to_string(),
                redirected_bass_channel_count: 1,
                main_high_pass_hz: Some(80.0),
                sub_low_pass_hz: Some(80.0),
                lfe_headroom_required_db: 10.0,
                signal_flow: vec![BassManagementSignalFlowEntry {
                    source_channel: "L".to_string(),
                    role: HomeCinemaRole::FrontLeft,
                    destination: "Sub".to_string(),
                    high_pass_hz: None,
                    low_pass_hz: Some(80.0),
                    lfe_gain_db: 0.0,
                    redirects_bass: true,
                }],
                signal_flow_advisories: Vec::new(),
                routing_graph: Some(BassManagementRoutingGraph {
                    physical_sub_output: "Sub".to_string(),
                    input_channels: vec!["L".to_string(), "Sub".to_string()],
                    output_channels: vec!["L".to_string(), "Sub".to_string()],
                    routes: vec![
                        BassManagementRoute {
                            group_id: Some("lcr".to_string()),
                            source_channel: "L".to_string(),
                            source_index: 0,
                            destination: "L".to_string(),
                            destination_index: 0,
                            pre_chain_channel: Some("L".to_string()),
                            post_chain_channel: Some("L".to_string()),
                            route_kind: "main_highpass_to_self".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: Some(80.0),
                            low_pass_hz: None,
                            gain_db: 0.0,
                            gain_linear: 1.0,
                            matrix_gain: 1.0,
                            delay_ms: 2.0,
                            polarity_inverted: false,
                        },
                        BassManagementRoute {
                            group_id: Some("lcr".to_string()),
                            source_channel: "L".to_string(),
                            source_index: 0,
                            destination: "Sub".to_string(),
                            destination_index: 1,
                            pre_chain_channel: Some("Sub".to_string()),
                            post_chain_channel: Some("Sub".to_string()),
                            route_kind: "redirected_bass_lowpass_to_sub".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: None,
                            low_pass_hz: Some(80.0),
                            gain_db: -3.0,
                            gain_linear: 0.707945784,
                            matrix_gain: 1.0,
                            delay_ms: 4.0,
                            polarity_inverted: true,
                        },
                    ],
                    matrix: None,
                    advisories: Vec::new(),
                }),
                optimization: None,
                groups: Vec::new(),
                sub_outputs: Vec::new(),
                headroom_simulation: None,
                advisory: "ok".to_string(),
            }),
            timing_diagnostics: None,
        });
        output
    }

    fn routed_physical_sub_output() -> DspChainOutput {
        let mut output = routed_bass_output();
        let mut sub_chain = output.channels.remove("Sub").expect("sub chain");
        sub_chain.channel = "LFE".to_string();
        sub_chain.drivers = Some(vec![DriverDspChain {
            name: "SubA".to_string(),
            index: 0,
            plugins: vec![],
            initial_curve: None,
        }]);
        output.channels.insert("LFE".to_string(), sub_chain);

        let report = output
            .metadata
            .as_mut()
            .and_then(|metadata| metadata.bass_management.as_mut())
            .expect("bass management report");
        report.physical_sub_output = "LFE".to_string();
        let graph = report.routing_graph.as_mut().expect("routing graph");
        graph.physical_sub_output = "LFE".to_string();
        graph.input_channels = vec!["L".to_string(), "LFE".to_string(), "SubA".to_string()];
        graph.output_channels = vec!["L".to_string(), "LFE".to_string(), "SubA".to_string()];
        for route in &mut graph.routes {
            if route.route_kind == "redirected_bass_lowpass_to_sub" {
                route.destination = "SubA".to_string();
                route.destination_index = 2;
                route.pre_chain_channel = Some("LFE".to_string());
                route.post_chain_channel = Some("SubA".to_string());
            }
        }
        output
    }

    #[test]
    fn test_requires_room_eq_graph_with_routed_bass_management() {
        let output = routed_bass_output();
        assert!(output.requires_room_eq_graph());
        assert!(!output.is_rack_compatible());
    }

    #[test]
    fn test_build_room_eq_graph_emits_route_dsp_and_output_correction() {
        let output = routed_bass_output();
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let plugin_types: Vec<_> = graph
            .nodes
            .iter()
            .map(|node| node.plugin_type.as_str())
            .collect();
        assert!(
            plugin_types
                .iter()
                .filter(|&&kind| kind == "matrix")
                .count()
                >= 4
        );
        assert!(plugin_types.contains(&"crossover"));
        assert!(plugin_types.contains(&"delay"));
        assert!(plugin_types.iter().filter(|&&kind| kind == "gain").count() >= 3);
        assert!(plugin_types.contains(&"eq"));
        assert!(graph.nodes.iter().all(|node| node.input_channels == 2));

        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, label))
            })
            .collect();
        let pre_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "pre_room_eq")
            .map(|(id, _)| *id)
            .expect("pre-route EQ should be emitted");
        let first_route_crossover_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_route_highpass")
            .map(|(id, _)| *id)
            .expect("route highpass should be emitted");
        let post_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "post_room_eq")
            .map(|(id, _)| *id)
            .expect("post-route EQ should be emitted");
        let post_main_trim_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "post_main_trim")
            .map(|(id, _)| *id)
            .expect("post-route main trim should be emitted");
        let sub_pre_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "sub_pre_room_eq")
            .map(|(id, _)| *id)
            .expect("sub pre-crossover EQ should be emitted on the bass route");
        let lowpass_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_route_lowpass")
            .map(|(id, _)| *id)
            .expect("route lowpass should be emitted");
        let sub_post_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "sub_post_room_eq")
            .map(|(id, _)| *id)
            .expect("sub post-crossover EQ should be emitted after bass summation");
        let sub_post_trim_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "sub_post_trim")
            .map(|(id, _)| *id)
            .expect("post-route sub trim should be emitted after bass summation");
        let sum_anchor_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_route_sum_anchor")
            .map(|(id, _)| *id)
            .expect("route sum anchor should be emitted");
        assert!(
            pre_eq_id < first_route_crossover_id,
            "pre-crossover EQ must stay before route crossover"
        );
        assert!(
            post_eq_id > sum_anchor_id,
            "post-crossover EQ must stay after route summation"
        );
        assert!(
            post_main_trim_id > sum_anchor_id,
            "post-crossover main trims must stay after route summation"
        );
        assert!(
            sub_pre_eq_id < lowpass_id,
            "bass-route sub EQ must stay before the routed lowpass"
        );
        assert!(
            sub_post_eq_id > sum_anchor_id,
            "bass-output post EQ must stay after redirected-bass summation"
        );
        assert!(
            sub_post_trim_id > sum_anchor_id,
            "bass-output post trims must not be mistaken for route-owned gain"
        );
    }

    #[test]
    fn test_build_room_eq_graph_applies_shared_sub_chain_to_physical_sub_routes() {
        let output = routed_physical_sub_output();
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        assert!(graph.nodes.iter().all(|node| node.input_channels == 3));

        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, label))
            })
            .collect();
        let sub_pre_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "sub_pre_room_eq")
            .map(|(id, _)| *id)
            .expect("shared sub pre-EQ should be emitted for physical sub route");
        let lowpass_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_route_lowpass")
            .map(|(id, _)| *id)
            .expect("route lowpass should be emitted");
        let sub_post_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "sub_post_room_eq")
            .map(|(id, _)| *id)
            .expect("shared sub post-EQ should be emitted for physical sub output");
        let sum_anchor_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_route_sum_anchor")
            .map(|(id, _)| *id)
            .expect("route sum anchor should be emitted");

        assert!(sub_pre_eq_id < lowpass_id);
        assert!(sub_post_eq_id > sum_anchor_id);
    }

    #[test]
    fn test_build_room_eq_graph_preserves_non_routing_global_plugins() {
        let mut output = routed_bass_output();
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "eq".to_string(),
            parameters: serde_json::json!({
                "label": "global_room_eq",
                "filters": []
            }),
        });
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "matrix".to_string(),
            parameters: serde_json::json!({
                "label": "home_cinema_bass_management",
                "metadata": {
                    "purpose": "home_cinema_bass_management"
                },
                "input_channel_map": [0],
                "output_channel_map": [1],
                "matrix": [1.0]
            }),
        });

        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, label))
            })
            .collect();
        let global_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "global_room_eq")
            .map(|(id, _)| *id)
            .expect("non-routing global plugin should be preserved");
        assert!(
            labeled_nodes
                .iter()
                .all(|(_, label)| *label != "home_cinema_bass_management"),
            "legacy global bass matrix should be replaced by route branches"
        );
        let route_matrix_ids: Vec<_> = labeled_nodes
            .iter()
            .filter(|(_, label)| label.starts_with("room_eq_route_") && label.contains("_to_"))
            .map(|(id, _)| *id)
            .collect();
        assert!(!route_matrix_ids.is_empty());
        assert!(route_matrix_ids.iter().all(|id| *id > global_id));
        assert!(route_matrix_ids.iter().all(|route_id| {
            graph
                .edges
                .iter()
                .any(|edge| edge.from_node == global_id && edge.to_node == *route_id)
        }));
    }
}
