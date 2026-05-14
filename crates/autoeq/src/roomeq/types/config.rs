//! Room EQ Configuration Types
//!
//! All configuration structs for room equalization.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::MeasurementSource;
use crate::optim::SmoothnessPenaltyConfig;

/// Configuration version (semantic versioning)
pub fn default_config_version() -> String {
    "2.0.0".to_string()
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
    /// Physical room dimensions (metric — length/width/height in meters)
    /// collected from the user at save time. When present these are
    /// reused by the optimizer's Schroeder-frequency auto-detection; see
    /// [`RoomDimensions::schroeder_frequency_with_rt60`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_dimensions: Option<RoomDimensions>,
    /// Free-form description of the listening setup (treatment,
    /// seating, notes about speaker placement, etc.). Not consumed by
    /// the optimizer — stored purely for session reproducibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_description: Option<String>,
    /// Per-channel speaker identity (brand + model) as free-form
    /// strings, ideally autocompleted from the spinorama.org catalog.
    /// Keyed by channel name so it round-trips through reorder/rename.
    /// Not consumed by the optimizer — metadata only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_speakers: Option<HashMap<String, String>>,
    /// Tone-burst delay probe results captured during the Recording
    /// wizard's Probe step. Stored here so the `autoeq::roomeq`
    /// pipeline can pick them up at config-load time without requiring
    /// a live measurement. Mirrors the shape of the engine's
    /// `ProbeDelayResults` for cross-crate serde compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_results: Option<ProbeResultsLegacy>,
    /// Relative path (within the recording directory) of the raw
    /// probe WAV persisted by `probe_channel_delays_with_recording`.
    /// `None` for sessions that skipped the Probe step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_wav_relative: Option<String>,

    /// Bass anchor results captured during the GD-1e BassAnchor
    /// wizard step — per-channel phase of a low-frequency tone burst
    /// at `bass_probe_freq_hz`. Populated after the wizard finishes;
    /// absent when the user skipped the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_anchor_results: Option<BassAnchorResultsLegacy>,

    /// Relative path (within the recording directory) of the raw
    /// bass-anchor WAV. `None` when the BassAnchor step was skipped
    /// or when recording persistence was disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_anchor_wav_relative: Option<String>,

    // ------------------------------------------------------------------
    // GD-Opt v2 recording extensions (see `docs/gd_opt_v2_plan.md` §2).
    // All optional; absent values degrade the GD confidence gate but
    // do not break the wider recording pipeline.
    // ------------------------------------------------------------------
    /// Per-octave bass sweep duration in seconds. Defaults to 3.0; the
    /// sweep generator scales total duration so that the band below
    /// 100 Hz receives `bass_octave_duration_s` seconds per octave.
    /// Clamped to `[1.0 .. 10.0]` at load time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_octave_duration_s: Option<f32>,
    /// Pre-sweep silence window in seconds. Used by the coherence
    /// averager to estimate the noise-floor. Default 2.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_silence_s: Option<f32>,
    /// Post-sweep silence window in seconds. Default
    /// `schroeder_rt60 + 1.0`; falls back to `2.0` if no RT60 estimate
    /// is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_silence_s: Option<f32>,
    /// Target sweep level at the listening position in dBSPL. Requires
    /// [`spl_calibration`](Self::spl_calibration) to be populated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_level_db_spl: Option<f32>,
    /// Number of sweeps recorded back-to-back for coherence averaging.
    /// Default 4. Clamped to `[1 .. 8]` at load time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_sweeps: Option<u8>,
    /// Coherence threshold below which the GD confidence gate declares
    /// bass phase untrustworthy. Default 0.9. Clamped to `[0.5 .. 0.99]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coherence_threshold: Option<f32>,
    /// Centre frequency of the bass tone burst captured by the
    /// BassAnchor wizard step. Default 20.0 Hz (or
    /// `1.25 * min_freq`, whichever is higher).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_probe_freq_hz: Option<f32>,
    /// Total length of the steady-state bass-anchor tone in seconds
    /// (steady portion + fades). Default 2.0.
    ///
    /// The pre-v2 schema stored `bass_probe_cycles: u16` here — it's
    /// dropped silently at load (the units differ and converting
    /// requires `bass_probe_freq_hz` which serde sees per-field). The
    /// per-result `BassAnchorResultsLegacy::bass_duration_s` migration
    /// handles legacy run-output files (where the conversion is well
    /// defined).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_probe_duration_s: Option<f32>,
    /// Path to the microphone phase calibration CSV (4 columns:
    /// `freq, mag_db, phase_deg, coherence`). Magnitude calibration
    /// already lives under [`mic_calibration_path`](Self::mic_calibration_path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_phase_calibration_path: Option<String>,
    /// Per-channel mic phase calibration files, aligned with
    /// [`mic_calibration_paths`](Self::mic_calibration_paths).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_phase_calibration_paths: Option<Vec<Option<String>>>,
    /// SPL calibration anchor captured from a pre-sweep reference tone.
    /// Required on new recordings by the SplCalibration wizard step;
    /// stored here so that replayed recordings can re-derive
    /// `sweep_level_db_spl`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spl_calibration: Option<SplCalibration>,
    /// Deterministic seed for the sweep / probe generators. QA sets it;
    /// the UI hides it. When `None`, the generators use their internal
    /// fixed seed constants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_seed: Option<u64>,
    /// Number of measurement positions (seats) the user captured. `None`
    /// or `Some(1)` means a single-position session; `Some(n)` with
    /// `n >= 2` means each `ChannelMeasurement.multi_mic_measurements`
    /// holds `n * num_mics - 1` entries in `(position, mic)` order
    /// (primary measurement is `(0, 0)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_positions: Option<usize>,
}

/// SPL calibration anchor captured from a pre-sweep reference tone.
///
/// Maps the peak sample value observed on the recording ADC during a
/// reference tone to a dBSPL reading taken by the user at the listening
/// position with an external meter. Once captured, the sweep playback
/// level is chosen so the in-band energy at the listening position hits
/// [`RecordingConfiguration::sweep_level_db_spl`](RecordingConfiguration::sweep_level_db_spl)
/// deterministically — avoiding the subwoofer driver over-excursion
/// that would otherwise contaminate bass phase with harmonic distortion.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SplCalibration {
    /// User-reported dBSPL at the listening position during calibration.
    pub reported_db_spl: f32,
    /// Frequency of the calibration tone in Hz (default 1000).
    pub reference_freq_hz: f32,
    /// Peak sample value observed on the recording ADC during the tone,
    /// in the range `[0.0 .. 1.0]`.
    pub peak_sample_level: f32,
    /// Offset mapping ADC peak level to dBSPL at the mic:
    /// `dbspl_at_mic = 20 * log10(peak_sample_value) + spl_offset_db`.
    pub spl_offset_db: f32,
}

impl SplCalibration {
    /// Convenience: compute the expected dBSPL at the mic for a given
    /// peak sample value, using this calibration's offset.
    pub fn dbspl_for_peak_level(&self, peak_sample_value: f32) -> f32 {
        20.0 * peak_sample_value.max(f32::EPSILON).log10() + self.spl_offset_db
    }

    /// Convenience: compute the peak sample value required to hit a
    /// target dBSPL at the mic. Returns `0.0` if the target is below
    /// the representable range.
    pub fn peak_level_for_dbspl(&self, target_db_spl: f32) -> f32 {
        10.0f32
            .powf((target_db_spl - self.spl_offset_db) / 20.0)
            .clamp(0.0, 1.0)
    }
}

/// Serializable mirror of the engine's `ProbeDelayResults`. Kept
/// in-crate (rather than depending on `sotf-engine` or `sotf-player`)
/// so the autoeq crate remains lean. Fields match the engine type
/// 1:1 so round-trip through serde is lossless.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProbeResultsLegacy {
    pub channels: Vec<ProbeChannelResultLegacy>,
    pub sample_rate: u32,
    pub alignment_delays_ms: Vec<f64>,
}

/// Per-channel probe result (mirror of `ProbeDelayChannelResult`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProbeChannelResultLegacy {
    pub channel_name: String,
    pub channel_index: usize,
    pub arrival_ms: f64,
    pub gain_db: f64,
    pub snr_db: f64,
}

/// Serializable mirror of the engine's `BassAnchorResults`. Captures
/// the per-channel phase at the bass anchor frequency so that GD-Opt
/// v2's confidence gate (§3.5 of `docs/gd_opt_v2_plan.md`) and
/// optimiser (§3.2) can ingest it at config-load time without
/// depending on `sotf-engine`.
///
/// Implements a custom `Deserialize` that also accepts the pre-v2
/// schema field name `bass_cycles: u16`; when present (and
/// `bass_duration_s` is absent) it is converted to seconds via
/// `cycles / bass_freq_hz` so older recordings.json files load without
/// re-recording.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct BassAnchorResultsLegacy {
    /// Per-channel phase + quality metrics.
    pub channels: Vec<BassAnchorChannelResultLegacy>,
    /// Sample rate used for the capture (Hz).
    pub sample_rate: u32,
    /// Centre frequency of the steady-state tone in Hz (nominal 30 Hz).
    pub bass_freq_hz: f32,
    /// Total tone length in seconds (steady portion + fades). Nominal 2.0.
    pub bass_duration_s: f32,
}

impl<'de> Deserialize<'de> for BassAnchorResultsLegacy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(default)]
            channels: Vec<BassAnchorChannelResultLegacy>,
            #[serde(default)]
            sample_rate: u32,
            #[serde(default)]
            bass_freq_hz: f32,
            #[serde(default)]
            bass_duration_s: Option<f32>,
            // Legacy v1 field — preserved here for migration only.
            #[serde(default)]
            bass_cycles: Option<u16>,
        }
        let h = Helper::deserialize(deserializer)?;
        let bass_duration_s = match (h.bass_duration_s, h.bass_cycles, h.bass_freq_hz) {
            (Some(d), _, _) => d,
            (None, Some(cycles), freq) if freq > 0.0 => cycles as f32 / freq,
            _ => 0.0,
        };
        Ok(BassAnchorResultsLegacy {
            channels: h.channels,
            sample_rate: h.sample_rate,
            bass_freq_hz: h.bass_freq_hz,
            bass_duration_s,
        })
    }
}

/// Per-channel bass-anchor result (mirror of `BassAnchorChannelResult`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct BassAnchorChannelResultLegacy {
    /// Channel name (e.g. "L", "R", "Sub").
    pub channel_name: String,
    /// Channel output index used during playback.
    pub channel_index: usize,
    /// Reported phase of the bass tone at the listening position,
    /// degrees in `(−180°, 180°]`, sin-referenced. When a loopback
    /// reference channel was recorded this is the loopback-corrected
    /// acoustic phase (`phase_mic − phase_loopback`); see
    /// `bass_anchor_loopback_phase_deg` for the raw loopback value.
    pub bass_anchor_phase_deg: f64,
    /// Linear magnitude of the lock-in I/Q estimator at
    /// `bass_freq_hz` on the mic. SNR proxy.
    pub bass_anchor_magnitude: f64,
    /// Circular standard deviation of phase across the sub-window
    /// lock-in estimates, in degrees. Values above the
    /// `"bass_anchor_unreliable"` advisory threshold (§2.8, 20°) mean
    /// the GD confidence gate should discard this channel's anchor.
    pub bass_anchor_stability_deg: f64,
    /// Raw loopback reference phase in degrees (sin-referenced).
    /// `None` when no loopback channel was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_anchor_loopback_phase_deg: Option<f64>,
    /// Magnitude-squared coherence γ² ∈ [0, 1] between the mic and the
    /// loopback per-window phasors at `bass_freq_hz`. Conventional QA
    /// threshold is γ² > 0.9. `None` when no loopback channel was
    /// recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_anchor_coherence: Option<f64>,
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
    /// Warped IIR mode — exports RoomEQ filters as warped biquads using a
    /// Bark-scale lambda. The optimizer currently uses the same biquad
    /// placement/scoring path as low_latency, then serializes the runtime
    /// topology as `warped_biquad`.
    WarpedIir,
    /// Kautz modal mode — pole-tuned filter targeting detected room modes.
    /// Uses room mode analysis to place filter poles at resonance frequencies.
    /// Gain optimization via linear least-squares (very fast, no DE needed).
    /// Exports the runtime topology as `kautz_filter` with modal sections.
    /// Best for small, highly resonant rooms with clear modal problems.
    /// Returns an error if no room modes are detected.
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

/// Target response shape preset
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
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
    /// Derive slope from the input measurement curve at optimization time
    FromMeasurement,
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
    /// Complex modal-basis sound-field management across seats
    ModalBasis,
    /// Continuous listening-area prior: integrate the variance / mean / worst-case
    /// objective over a probability density over positions, instead of the
    /// discrete seat slots. Requires `MultiSeatConfig::continuous_area` to be set.
    ContinuousArea,
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
    /// Measurement-uncertainty-aware robust optimization. Generates B
    /// case-bootstrap resamples of the input curves at setup time, then
    /// scalarises losses across the resampled targets per the configured
    /// `BootstrapUncertaintyConfig::scalarisation` (worst-case or CVaR).
    /// Drives the optimizer toward a solution that is robust to which
    /// resample of the measurement set is "true".
    MinimaxUncertainty,
}

/// Correction mode for CEA2034 speaker pre-correction
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Cea2034CorrectionMode {
    /// Correct Listening Window toward flat (best for nearfield <2m)
    Flat,
    /// Unsupported in roomeq; Harman speaker score is anechoic-only
    Score,
    /// Auto-select the supported roomeq pre-correction from listening distance
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

fn default_bass_management_enabled() -> bool {
    true
}
fn default_redirect_bass() -> bool {
    true
}
fn default_lfe_channel() -> String {
    "LFE".to_string()
}
fn default_lfe_playback_gain_db() -> f64 {
    10.0
}
fn default_sub_headroom_margin_db() -> f64 {
    6.0
}
fn default_max_sub_boost_db() -> f64 {
    6.0
}
fn default_optimize_bass_groups() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BassHeadroomModelKind {
    CinemaCorrelated,
}

fn default_bass_headroom_model_kind() -> BassHeadroomModelKind {
    BassHeadroomModelKind::CinemaCorrelated
}
fn default_lr_correlation() -> f64 {
    0.75
}
fn default_lcr_correlation() -> f64 {
    0.5
}
fn default_surround_height_correlation() -> f64 {
    0.35
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct BassHeadroomModelConfig {
    #[serde(default = "default_bass_headroom_model_kind")]
    pub model: BassHeadroomModelKind,
    #[serde(default = "default_lr_correlation")]
    pub lr_correlation: f64,
    #[serde(default = "default_lcr_correlation")]
    pub lcr_correlation: f64,
    #[serde(default = "default_surround_height_correlation")]
    pub surround_height_correlation: f64,
}

impl Default for BassHeadroomModelConfig {
    fn default() -> Self {
        Self {
            model: default_bass_headroom_model_kind(),
            lr_correlation: default_lr_correlation(),
            lcr_correlation: default_lcr_correlation(),
            surround_height_correlation: default_surround_height_correlation(),
        }
    }
}

/// Home-cinema bass-management policy.
///
/// This describes how RoomEQ should reason about the main high-pass, sub
/// low-pass, redirected bass, and the LFE programme path. The current DSP
/// exporter still emits per-output-channel correction chains, so LFE +10 dB
/// is reported but not inserted by default: applying it to the physical sub
/// chain would also boost redirected bass.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementConfig {
    /// Enable bass-management semantics for home-cinema workflows.
    #[serde(default = "default_bass_management_enabled")]
    pub enabled: bool,
    /// Whether small-speaker bass is redirected to the subwoofer output.
    #[serde(default = "default_redirect_bass")]
    pub redirect_bass: bool,
    /// Logical LFE programme channel name.
    #[serde(default = "default_lfe_channel")]
    pub lfe_channel: String,
    /// Cinema LFE playback calibration gain. Reported in metadata; not applied
    /// to the sub correction chain unless `apply_lfe_gain_to_chain` is set.
    #[serde(default = "default_lfe_playback_gain_db")]
    pub lfe_playback_gain_db: f64,
    /// Explicitly insert LFE gain in the exported physical sub chain. This is
    /// normally false because redirected bass and LFE share the same sub output.
    #[serde(default)]
    pub apply_lfe_gain_to_chain: bool,
    /// User sub trim applied after crossover/alignment optimization.
    #[serde(default)]
    pub sub_trim_db: f64,
    /// Maximum allowed positive sub boost from RoomEQ bass management.
    #[serde(default = "default_max_sub_boost_db")]
    pub max_sub_boost_db: f64,
    /// Headroom reserve expected downstream for bass-managed playback.
    #[serde(default = "default_sub_headroom_margin_db")]
    pub headroom_margin_db: f64,
    /// Optional role-group to crossover-key mapping. Supported built-in group
    /// ids are `lcr`, `surround`, `height`, and `wide`; unknown ids are kept as
    /// custom metadata groups.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub group_crossovers: HashMap<String, String>,
    /// Optimize crossover settings independently per speaker group.
    #[serde(default = "default_optimize_bass_groups")]
    pub optimize_groups: bool,
    /// Programme-correlation model used for bass-bus headroom simulation.
    #[serde(default)]
    pub headroom_model: BassHeadroomModelConfig,
}

impl Default for BassManagementConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            redirect_bass: true,
            lfe_channel: default_lfe_channel(),
            lfe_playback_gain_db: default_lfe_playback_gain_db(),
            apply_lfe_gain_to_chain: false,
            sub_trim_db: 0.0,
            max_sub_boost_db: default_max_sub_boost_db(),
            headroom_margin_db: default_sub_headroom_margin_db(),
            group_crossovers: HashMap::new(),
            optimize_groups: true,
            headroom_model: BassHeadroomModelConfig::default(),
        }
    }
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
    /// Home-cinema bass-management policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_management: Option<BassManagementConfig>,
}

/// Speaker configuration (can be single measurement or group)
///
/// Variant order matters for serde untagged deserialization: serde tries each variant
/// in order. Group/MultiSub/Dba all require a `name` field that `MeasurementSource`
/// doesn't have, so they are tried first. `Single` is last as a catch-all.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[allow(
    clippy::large_enum_variant,
    reason = "SpeakerConfig::Single is the dominant variant in 100+ call sites; boxing would create churn for marginal memory savings"
)]
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

fn default_tilt_slope() -> f64 {
    -0.8
}
fn default_tilt_reference_freq() -> f64 {
    1000.0
}
fn default_bass_shelf_freq() -> f64 {
    200.0
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

fn default_role_targets_enabled() -> bool {
    true
}
fn default_center_dialog_low_hz() -> f64 {
    300.0
}
fn default_center_dialog_high_hz() -> f64 {
    4_000.0
}
fn default_cinema_reference_distance_m() -> f64 {
    3.0
}
fn default_cinema_x_curve_start_hz() -> f64 {
    2_000.0
}

/// Optional role-aware target adjustments for home-cinema layouts.
///
/// These are deliberately explicit and default to zero change: enabling the
/// block makes the target semantics role-aware without silently changing
/// existing RoomEQ output. The adjustments layer on top of
/// [`TargetResponseConfig::preference`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleTargetConfig {
    /// Enable role-aware target adjustment.
    #[serde(default = "default_role_targets_enabled")]
    pub enabled: bool,
    /// Extra broadband slope for front L/R channels, in dB/octave.
    #[serde(default)]
    pub front_slope_offset_db_per_octave: f64,
    /// Extra broadband slope for the center channel, in dB/octave.
    #[serde(default)]
    pub center_slope_offset_db_per_octave: f64,
    /// Extra broadband slope for surround and wide channels, in dB/octave.
    #[serde(default)]
    pub surround_slope_offset_db_per_octave: f64,
    /// Extra broadband slope for height channels, in dB/octave.
    #[serde(default)]
    pub height_slope_offset_db_per_octave: f64,
    /// Extra broadband slope for subwoofer channels, in dB/octave.
    #[serde(default)]
    pub subwoofer_slope_offset_db_per_octave: f64,
    /// Extra broadband slope for LFE channels, in dB/octave.
    #[serde(default)]
    pub lfe_slope_offset_db_per_octave: f64,
    /// Additional treble shelf applied only to the centre channel.
    #[serde(default)]
    pub center_treble_shelf_db: f64,
    /// Additional treble shelf applied to side/rear/wide surrounds.
    #[serde(default)]
    pub surround_treble_shelf_db: f64,
    /// Additional treble shelf applied to height channels.
    #[serde(default)]
    pub height_treble_shelf_db: f64,
    /// Additional bass shelf applied to subwoofer channels.
    #[serde(default)]
    pub subwoofer_bass_shelf_db: f64,
    /// Additional bass shelf applied to LFE channels.
    #[serde(default)]
    pub lfe_bass_shelf_db: f64,
    /// Broad, smooth center-channel dialog-band target lift/cut in dB.
    #[serde(default)]
    pub center_dialog_boost_db: f64,
    /// Lower edge of the center dialog emphasis band.
    #[serde(default = "default_center_dialog_low_hz")]
    pub center_dialog_low_hz: f64,
    /// Upper edge of the center dialog emphasis band.
    #[serde(default = "default_center_dialog_high_hz")]
    pub center_dialog_high_hz: f64,
    /// Enable cinema/X-curve style high-frequency rolloff shaping.
    #[serde(default)]
    pub cinema_x_curve_enabled: bool,
    /// Additional high-frequency slope above `cinema_x_curve_start_hz`, in dB/octave.
    #[serde(default)]
    pub cinema_x_curve_db_per_octave: f64,
    /// Frequency where cinema/X-curve high-frequency shaping starts.
    #[serde(default = "default_cinema_x_curve_start_hz")]
    pub cinema_x_curve_start_hz: f64,
    /// Listening distance used for optional distance-compensated treble rolloff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening_distance_m: Option<f64>,
    /// Reference distance for distance-compensated treble rolloff.
    #[serde(default = "default_cinema_reference_distance_m")]
    pub cinema_reference_distance_m: f64,
    /// Additional HF rolloff per distance doubling beyond the reference distance.
    #[serde(default)]
    pub distance_treble_rolloff_db_per_doubling: f64,
}

impl Default for RoleTargetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            front_slope_offset_db_per_octave: 0.0,
            center_slope_offset_db_per_octave: 0.0,
            surround_slope_offset_db_per_octave: 0.0,
            height_slope_offset_db_per_octave: 0.0,
            subwoofer_slope_offset_db_per_octave: 0.0,
            lfe_slope_offset_db_per_octave: 0.0,
            center_treble_shelf_db: 0.0,
            surround_treble_shelf_db: 0.0,
            height_treble_shelf_db: 0.0,
            subwoofer_bass_shelf_db: 0.0,
            lfe_bass_shelf_db: 0.0,
            center_dialog_boost_db: 0.0,
            center_dialog_low_hz: default_center_dialog_low_hz(),
            center_dialog_high_hz: default_center_dialog_high_hz(),
            cinema_x_curve_enabled: false,
            cinema_x_curve_db_per_octave: 0.0,
            cinema_x_curve_start_hz: default_cinema_x_curve_start_hz(),
            listening_distance_m: None,
            cinema_reference_distance_m: default_cinema_reference_distance_m(),
            distance_treble_rolloff_db_per_doubling: 0.0,
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
    /// Optional home-cinema role-aware target adjustments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_targets: Option<RoleTargetConfig>,
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
            role_targets: None,
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
    /// Enable per-sub polarity search for MSO.
    #[serde(default)]
    pub optimize_polarity: bool,
    /// Number of per-sub all-pass filters allowed during MSO.
    #[serde(default)]
    pub allpass_filters_per_sub: usize,
    /// Optimize a per-subwoofer PEQ from that sub's measurements across all seats
    /// before the gain/delay/polarity/all-pass MSO pass.
    #[serde(default = "default_multiseat_per_sub_peq")]
    pub per_sub_peq: bool,
    /// Optimize a shared EQ on the post-MSO combined response across all seats.
    #[serde(default = "default_multiseat_global_eq")]
    pub global_eq: bool,
    /// Enable all-channel multi-seat correction for non-sub home-cinema channels.
    #[serde(default = "default_all_channel_multiseat_enabled")]
    pub all_channel_enabled: bool,
    /// Strategy used when deriving per-channel multi-measurement correction.
    #[serde(default = "default_all_channel_multiseat_strategy")]
    pub all_channel_strategy: MultiMeasurementStrategy,
    /// Optional seat weights for all-channel multi-seat correction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat_weights: Option<Vec<f64>>,
    /// Relative primary-seat weight used with PrimaryWithConstraints.
    #[serde(default = "default_primary_seat_weight")]
    pub primary_seat_weight: f64,
    /// Continuous listening-area prior. Required (and only consulted) when
    /// `strategy = ContinuousArea`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuous_area: Option<ContinuousListeningAreaConfig>,
}

fn default_max_deviation_db() -> f64 {
    6.0
}
fn default_all_channel_multiseat_enabled() -> bool {
    true
}
fn default_all_channel_multiseat_strategy() -> MultiMeasurementStrategy {
    MultiMeasurementStrategy::SpatialRobustness
}
fn default_primary_seat_weight() -> f64 {
    2.0
}
fn default_multiseat_per_sub_peq() -> bool {
    true
}
fn default_multiseat_global_eq() -> bool {
    true
}

impl Default for MultiSeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: MultiSeatStrategy::MinimizeVariance,
            primary_seat: 0,
            max_deviation_db: default_max_deviation_db(),
            optimize_polarity: false,
            allpass_filters_per_sub: 0,
            per_sub_peq: default_multiseat_per_sub_peq(),
            global_eq: default_multiseat_global_eq(),
            all_channel_enabled: default_all_channel_multiseat_enabled(),
            all_channel_strategy: default_all_channel_multiseat_strategy(),
            seat_weights: None,
            primary_seat_weight: default_primary_seat_weight(),
            continuous_area: None,
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

/// Group-delay optimization configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupDelayOptimizationConfig {
    /// Enable GD optimization and apply the resulting phase-only DSP.
    #[serde(default)]
    pub enabled: bool,
    /// Maximum per-channel delay in ms.
    #[serde(default = "default_gd_max_delay_ms")]
    pub max_delay_ms: f64,
    /// Maximum all-pass filters per channel. Production may downgrade this to
    /// zero when bootstrap evidence is unavailable.
    #[serde(default = "default_gd_ap_per_channel")]
    pub ap_per_channel: usize,
    /// Minimum all-pass Q.
    #[serde(default = "default_gd_ap_min_q")]
    pub ap_min_q: f64,
    /// Maximum all-pass Q.
    #[serde(default = "default_gd_ap_max_q")]
    pub ap_max_q: f64,
    /// Whether polarity may be optimized when coherence is present.
    #[serde(default = "default_true")]
    pub optimize_polarity: bool,
    /// Minimum in-band mean coherence for polarity/AP optimization.
    #[serde(default = "default_gd_coherence_threshold")]
    pub coherence_threshold: f64,
    /// Minimum improvement required before applying GD DSP.
    #[serde(default = "default_gd_min_improvement_db")]
    pub min_improvement_db: f64,
    /// DE maximum iterations.
    #[serde(default = "default_gd_max_iter")]
    pub max_iter: usize,
    /// DE population size multiplier.
    #[serde(default = "default_gd_popsize")]
    pub popsize: usize,
    /// DE convergence tolerance.
    #[serde(default = "default_gd_tol")]
    pub tol: f64,
    /// Require adaptive AP bootstrap before emitting all-pass filters.
    #[serde(default = "default_true")]
    pub adaptive_allpass: bool,
}

fn default_gd_max_delay_ms() -> f64 {
    20.0
}
fn default_gd_ap_per_channel() -> usize {
    2
}
fn default_gd_ap_min_q() -> f64 {
    0.3
}
fn default_gd_ap_max_q() -> f64 {
    10.0
}
fn default_gd_coherence_threshold() -> f64 {
    0.8
}
fn default_gd_min_improvement_db() -> f64 {
    1.0
}
fn default_gd_max_iter() -> usize {
    2000
}
fn default_gd_popsize() -> usize {
    20
}
fn default_gd_tol() -> f64 {
    1e-8
}

impl Default for GroupDelayOptimizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_delay_ms: default_gd_max_delay_ms(),
            ap_per_channel: default_gd_ap_per_channel(),
            ap_min_q: default_gd_ap_min_q(),
            ap_max_q: default_gd_ap_max_q(),
            optimize_polarity: true,
            coherence_threshold: default_gd_coherence_threshold(),
            min_improvement_db: default_gd_min_improvement_db(),
            max_iter: default_gd_max_iter(),
            popsize: default_gd_popsize(),
            tol: default_gd_tol(),
            adaptive_allpass: true,
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

/// Serializable smoothness-penalty configuration for JSON config files.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SmoothnessPenaltyConfigSerde {
    /// Penalty weight in loss units per (dB/decade^2)^exponent.
    #[serde(default)]
    pub tv2_weight: f64,
    /// Optional Schroeder cutoff in Hz for reduced modal-region penalty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schroeder_hz: Option<f64>,
    /// Multiplier below `schroeder_hz` (0 = modal region exempt).
    #[serde(default = "default_modal_weight_scale")]
    pub modal_weight_scale: f64,
    /// L_p exponent. 1.0 = TV^2-like sparse curvature, 2.0 = L2 smoothing.
    #[serde(default = "default_smoothness_exponent")]
    pub exponent: f64,
}

fn default_modal_weight_scale() -> f64 {
    0.1
}
fn default_smoothness_exponent() -> f64 {
    1.0
}

impl From<&SmoothnessPenaltyConfigSerde> for SmoothnessPenaltyConfig {
    fn from(value: &SmoothnessPenaltyConfigSerde) -> Self {
        Self {
            tv2_weight: value.tv2_weight,
            schroeder_hz: value.schroeder_hz,
            modal_weight_scale: value.modal_weight_scale,
            exponent: value.exponent,
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

/// How to scalarise the per-bootstrap-resample losses into one outer-loop loss.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapScalarisation {
    /// Pure worst-case: max loss across the B resamples. Most conservative; can be
    /// driven by a single outlier resample.
    #[default]
    WorstCase,
    /// Mean of the worst α-fraction of resamples (CVaR). Smoother, less sensitive
    /// to a single freak resample than `WorstCase`.
    Cvar,
}

/// Serializable bootstrap uncertainty configuration for JSON config files.
///
/// Drives `MultiMeasurementStrategy::MinimaxUncertainty`. At optimizer-setup
/// time, the input N measurement curves are case-bootstrap resampled B times;
/// each resampled mean becomes its own per-measurement objective. The outer
/// optimizer then scalarises the B objectives per `scalarisation`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct BootstrapUncertaintyConfig {
    /// Number of bootstrap resamples B. Typical: 200..1000. Default: 400.
    #[serde(default = "default_bootstrap_num_resamples")]
    pub num_resamples: usize,
    /// Two-sided confidence level α; band covers `[α/2, 1-α/2]`. Default: 0.10.
    #[serde(default = "default_bootstrap_alpha")]
    pub alpha: f64,
    /// PRNG seed for determinism.
    #[serde(default = "default_bootstrap_seed")]
    pub seed: u64,
    /// Scalarisation across the B resamples.
    #[serde(default)]
    pub scalarisation: BootstrapScalarisation,
    /// Tail fraction for CVaR (only used when `scalarisation = Cvar`). Default 0.20.
    #[serde(default = "default_bootstrap_cvar_alpha")]
    pub cvar_alpha: f64,
}

fn default_bootstrap_num_resamples() -> usize {
    400
}
fn default_bootstrap_alpha() -> f64 {
    0.10
}
fn default_bootstrap_seed() -> u64 {
    0xC0FFEE
}
fn default_bootstrap_cvar_alpha() -> f64 {
    0.20
}

impl Default for BootstrapUncertaintyConfig {
    fn default() -> Self {
        Self {
            num_resamples: default_bootstrap_num_resamples(),
            alpha: default_bootstrap_alpha(),
            seed: default_bootstrap_seed(),
            scalarisation: BootstrapScalarisation::default(),
            cvar_alpha: default_bootstrap_cvar_alpha(),
        }
    }
}

/// Probability density shape over positions, JSON-serialisable.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AreaPriorKind {
    /// Uniform density over the configured `bounds`.
    #[default]
    Uniform,
    /// Axis-aligned Gaussian density. `mean` and `cov_diag` must each have
    /// length equal to the number of dimensions; truncated at ±k·σ.
    Gaussian {
        /// Per-axis means.
        mean: Vec<f64>,
        /// Per-axis variances (must be > 0).
        cov_diag: Vec<f64>,
        /// Truncation in standard deviations. Default 4.0.
        #[serde(default = "default_gaussian_truncation_sigmas")]
        truncation_sigmas: f64,
    },
}

fn default_gaussian_truncation_sigmas() -> f64 {
    4.0
}

/// Quadrature scheme for discretising the prior integral, JSON-serialisable.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AreaQuadratureKind {
    /// Sobol low-discrepancy QMC with `num_points` samples.
    Sobol {
        /// Number of quadrature points (powers of two are most efficient).
        num_points: usize,
        /// PRNG seed.
        #[serde(default)]
        seed: u64,
    },
    /// Latin-Hypercube sampling.
    LatinHypercube {
        /// Number of quadrature points.
        num_points: usize,
        /// PRNG seed.
        #[serde(default)]
        seed: u64,
    },
    /// Gauss–Legendre tensor product. Total points = `points_per_axis^D`.
    GaussLegendre {
        /// Nodes per axis.
        points_per_axis: usize,
    },
}

impl Default for AreaQuadratureKind {
    fn default() -> Self {
        AreaQuadratureKind::Sobol {
            num_points: 64,
            seed: 0xC0FFEE,
        }
    }
}

/// How to scalarise per-quadrature-point losses, JSON-serialisable.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AreaScalarisationKind {
    /// Probability-weighted mean (expected loss over the listening area).
    #[default]
    ExpectedValue,
    /// Worst-case (max) over the area's bounding box. Inner DE search.
    WorstCase {
        /// Inner-search budget. Default 50.
        #[serde(default = "default_area_inner_maxiter")]
        inner_maxiter: usize,
        /// Inner-search seed.
        #[serde(default)]
        inner_seed: u64,
    },
    /// CVaR at level α — mean of the worst α-fraction of points.
    Cvar {
        /// Tail fraction in (0, 1].
        #[serde(default = "default_area_cvar_alpha")]
        alpha: f64,
    },
}

fn default_area_inner_maxiter() -> usize {
    50
}
fn default_area_cvar_alpha() -> f64 {
    0.20
}

/// Serializable continuous listening-area configuration for JSON config files.
///
/// Drives `MultiSeatStrategy::ContinuousArea`. The optimizer integrates the
/// per-position objective over a continuous prior π(p) defined over a
/// `dimensions`-dimensional axis-aligned box, replacing the discrete seats
/// array with a continuous probability density.
///
/// `bounds.len()` must equal `dimensions`. For Gaussian priors,
/// `mean.len()` and `cov_diag.len()` must also equal `dimensions`.
/// `seat_positions.len()` must equal the number of discrete seats in the
/// calibration `MultiSeatMeasurements` and each row's length must equal
/// `dimensions`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ContinuousListeningAreaConfig {
    /// Number of spatial dimensions (typical: 1 for a couch line, 2 for an
    /// MLP rectangle, 3 for a head-volume sweep). Currently 1, 2, and 3 are
    /// supported by the runtime dispatcher.
    pub dimensions: usize,
    /// Per-axis bounding-box bounds `(lo, hi)`. Always required; even for
    /// Gaussian priors the bounds determine the truncation rectangle.
    pub bounds: Vec<(f64, f64)>,
    /// Spatial coordinates of each calibration seat in
    /// `MultiSeatMeasurements`. Outer length = number of seats, inner length =
    /// `dimensions`. Order must match the seat index in the measurements.
    pub seat_positions: Vec<Vec<f64>>,
    /// Probability density shape.
    #[serde(default)]
    pub prior: AreaPriorKind,
    /// Quadrature scheme.
    #[serde(default)]
    pub quadrature: AreaQuadratureKind,
    /// How to scalarise the Q per-point losses.
    #[serde(default)]
    pub scalarisation: AreaScalarisationKind,
    /// IDW power exponent for spatial interpolation (default 2.0).
    #[serde(default = "default_idw_power")]
    pub idw_power: f64,
}

fn default_idw_power() -> f64 {
    2.0
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
    /// Bootstrap uncertainty configuration (used when strategy = MinimaxUncertainty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_uncertainty: Option<BootstrapUncertaintyConfig>,
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
            bootstrap_uncertainty: None,
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
    /// Enable Frequency-Dependent Windowing when `ssir_wav_path` provides an IR.
    #[serde(default = "default_true")]
    pub fdw_enabled: bool,
    /// FDW window length in cycles before min/max clamping. Default: 8.0
    #[serde(default = "default_fdw_cycles")]
    pub fdw_cycles: f64,
    /// Minimum FDW window length in milliseconds. Default: 3.0
    #[serde(default = "default_fdw_min_window_ms")]
    pub fdw_min_window_ms: f64,
    /// Maximum FDW window length in milliseconds. Default: 500.0
    #[serde(default = "default_fdw_max_window_ms")]
    pub fdw_max_window_ms: f64,
    /// FDW smoothing width in octaves. Default: 1/24 octave
    #[serde(default = "default_fdw_smoothing_octaves")]
    pub fdw_smoothing_octaves: f64,
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
fn default_fdw_cycles() -> f64 {
    8.0
}
fn default_fdw_min_window_ms() -> f64 {
    3.0
}
fn default_fdw_max_window_ms() -> f64 {
    500.0
}
fn default_fdw_smoothing_octaves() -> f64 {
    1.0 / 24.0
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
            fdw_enabled: true,
            fdw_cycles: default_fdw_cycles(),
            fdw_min_window_ms: default_fdw_min_window_ms(),
            fdw_max_window_ms: default_fdw_max_window_ms(),
            fdw_smoothing_octaves: default_fdw_smoothing_octaves(),
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
    /// Correction mode: flat, score (unsupported in roomeq), auto (distance-aware flat)
    #[serde(default)]
    pub correction_mode: Cea2034CorrectionMode,
    /// Manual listening distance override in meters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening_distance_m: Option<f64>,
    /// System round-trip latency in ms (for distance computation from impulse response)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_latency_ms: Option<f64>,
    /// Distance threshold in meters for auto-mode diagnostics (default: 2.0m)
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

/// Opt-in automatic room EQ optimizer bound selection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AutoOptimizerConfig {
    /// Enable automatic selection for the fields below.
    #[serde(default)]
    pub enabled: bool,
    /// Automatically choose the maximum PEQ filter count.
    #[serde(default = "default_true")]
    pub filter_count: bool,
    /// Automatically choose Q bounds.
    #[serde(default = "default_true")]
    pub q_bounds: bool,
    /// Automatically choose gain bounds and boost envelopes.
    #[serde(default = "default_true")]
    pub gain_bounds: bool,
    /// Minimum filter count when automatic count selection is enabled.
    #[serde(default = "default_auto_min_filters")]
    pub min_filters: usize,
    /// Maximum filter count when automatic count selection is enabled.
    #[serde(default = "default_auto_max_filters")]
    pub max_filters: usize,
}

fn default_auto_min_filters() -> usize {
    1
}

fn default_auto_max_filters() -> usize {
    12
}

impl Default for AutoOptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            filter_count: true,
            q_bounds: true,
            gain_bounds: true,
            min_filters: default_auto_min_filters(),
            max_filters: default_auto_max_filters(),
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
// Main OptimizerConfig
// ============================================================================

/// Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OptimizerConfig {
    /// Processing mode — selects the filter class used for correction.
    #[serde(default)]
    pub processing_mode: ProcessingMode,
    /// FIR configuration (used when `processing_mode` requires FIR filters)
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
    /// Population size for population-based optimizers
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
    /// Bayesian optimization Sobol hot-start samples. `None` uses an automatic default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bo_initial_samples: Option<usize>,
    /// Bayesian optimization batch size. `None` uses the backend default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bo_batch_size: Option<usize>,
    /// Posterior standard-deviation threshold for BO local-refiner handoff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bo_posterior_std_threshold: Option<f64>,
    /// Bayesian optimization acquisition: `"ei"`, `"qei"`, or `"thompson"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bo_acquisition: Option<String>,
    /// Use Monte-Carlo qEHVI Bayesian optimization for multi-objective data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bo_ehvi: Option<bool>,
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
    /// Unified target response configuration (shape + preference shelves + broadband pre-correction)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_response: Option<TargetResponseConfig>,
    /// Excursion protection configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excursion_protection: Option<ExcursionProtectionConfig>,
    /// Schroeder frequency split configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schroeder_split: Option<SchroederSplitConfig>,
    /// Automatic selection of filter count and optimizer bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_optimizer: Option<AutoOptimizerConfig>,
    /// Smoothness regularizer on the correction curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothness_penalty: Option<SmoothnessPenaltyConfigSerde>,
    /// Phase alignment configuration for subwoofer integration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_alignment: Option<PhaseAlignmentConfig>,
    /// Group-delay optimization configuration. Disabled by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_delay: Option<GroupDelayOptimizationConfig>,
    /// Multi-seat optimization configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_seat: Option<MultiSeatConfig>,
    /// Voice of God optimization configuration (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vog: Option<VoiceOfGodConfig>,
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

    /// Runtime-only: system-wide slope (dB/octave) resolved once for
    /// `TargetShape::FromMeasurement`. When `Some`, every channel reuses
    /// this slope instead of re-running the regression on its own curve.
    /// Lifted to room level so that band-limited channels (LFE, sub) do
    /// not derive a junk slope from their own rolled-off skirts.
    #[serde(skip)]
    pub from_measurement_slope_override: Option<f64>,
}

// Default values for OptimizerConfig
fn default_loss_type() -> String {
    "flat".to_string()
}
fn default_algorithm() -> String {
    "autoeq:cmaes".to_string()
}
fn default_strategy() -> String {
    "lshade".to_string()
}
fn default_peq_model() -> String {
    "pk".to_string()
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
            processing_mode: ProcessingMode::LowLatency,
            fir: None,
            mixed_config: None,
            mixed_phase: None,
            phase_correction: None,
            seed: None,
            refine: default_refine(),
            local_algo: default_local_algo(),
            bo_initial_samples: None,
            bo_batch_size: None,
            bo_posterior_std_threshold: None,
            bo_acquisition: None,
            bo_ehvi: None,
            psychoacoustic: default_psychoacoustic(),
            smooth_n: default_smooth_n(),
            asymmetric_loss: default_asymmetric_loss(),
            tolerance: default_tolerance(),
            atolerance: default_atolerance(),
            allow_delay: None,
            target_response: None,
            excursion_protection: None,
            schroeder_split: None,
            auto_optimizer: None,
            smoothness_penalty: None,
            phase_alignment: None,
            group_delay: None,
            multi_seat: None,
            vog: None,
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
            from_measurement_slope_override: None,
        }
    }
}

impl OptimizerConfig {
    /// Resolve the effective `allow_delay` value.
    ///
    /// Defaults to `true` whenever `processing_mode` introduces any non-zero
    /// base latency (everything except `LowLatency`). Callers can override
    /// explicitly via the `allow_delay` field.
    pub fn allow_delay(&self) -> bool {
        self.allow_delay
            .unwrap_or(self.processing_mode != ProcessingMode::LowLatency)
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
}

// ============================================================================
// CTC / binaural transfer-matrix configuration
// ============================================================================

fn default_ctc_matrix_source() -> String {
    "measured".to_string()
}
fn default_ctc_window_type() -> String {
    "ctc_direct".to_string()
}
fn default_ctc_window_start_ms() -> f64 {
    0.0
}
fn default_ctc_window_length_ms() -> f64 {
    6.0
}
fn default_ctc_window_fade_ms() -> f64 {
    1.0
}
fn default_ctc_beta_db() -> f64 {
    -30.0
}
fn default_ctc_max_gain_db() -> f64 {
    12.0
}
fn default_ctc_fir_taps() -> usize {
    4096
}
fn default_ctc_robustness() -> String {
    "average".to_string()
}

fn default_ctc_include_room_eq_dsp() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcHeadPositionConfig {
    pub id: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub yaw_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcMeasurementFileConfig {
    pub head_position: String,
    pub speaker: String,
    /// Processed/deconvolved two-channel ear IR WAV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir: Option<PathBuf>,
    /// Raw recorded two-ear sweep WAV. Channel 1 = left ear, channel 2 = right ear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_sweep: Option<PathBuf>,
    /// Raw loopback/reference recording WAV used to align the take.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loopback: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcMeasurementConfig {
    pub speakers: Vec<String>,
    #[serde(default)]
    pub mics: Vec<String>,
    #[serde(default)]
    pub head_positions: Vec<CtcHeadPositionConfig>,
    pub files: Vec<CtcMeasurementFileConfig>,
}

impl CtcMeasurementConfig {
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        for file in &mut self.files {
            if let Some(ir) = &mut file.ir
                && ir.is_relative()
            {
                *ir = base_dir.join(&*ir);
            }
            if let Some(raw_sweep) = &mut file.raw_sweep
                && raw_sweep.is_relative()
            {
                *raw_sweep = base_dir.join(&*raw_sweep);
            }
            if let Some(loopback) = &mut file.loopback
                && loopback.is_relative()
            {
                *loopback = base_dir.join(&*loopback);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcHrtfSpeakerConfig {
    pub speaker: String,
    pub azimuth_deg: f64,
    #[serde(default)]
    pub elevation_deg: f64,
    #[serde(default = "default_ctc_hrtf_distance_m")]
    pub distance_m: f64,
}

fn default_ctc_hrtf_distance_m() -> f64 {
    2.0
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcHrtfConfig {
    pub hrtf_file: PathBuf,
    pub speakers: Vec<CtcHrtfSpeakerConfig>,
}

impl CtcHrtfConfig {
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        if self.hrtf_file.is_relative() {
            self.hrtf_file = base_dir.join(&self.hrtf_file);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcWindowConfig {
    #[serde(default = "default_ctc_window_type")]
    pub window_type: String,
    #[serde(default = "default_ctc_window_start_ms")]
    pub start_ms: f64,
    #[serde(default = "default_ctc_window_length_ms")]
    pub length_ms: f64,
    #[serde(default = "default_ctc_window_fade_ms")]
    pub fade_ms: f64,
    #[serde(default = "default_ctc_fdw_cycles")]
    pub fdw_cycles: f64,
    #[serde(default = "default_ctc_fdw_min_ms")]
    pub fdw_min_ms: f64,
    #[serde(default = "default_ctc_fdw_max_ms")]
    pub fdw_max_ms: f64,
}

fn default_ctc_fdw_cycles() -> f64 {
    8.0
}
fn default_ctc_fdw_min_ms() -> f64 {
    3.0
}
fn default_ctc_fdw_max_ms() -> f64 {
    200.0
}

impl Default for CtcWindowConfig {
    fn default() -> Self {
        Self {
            window_type: default_ctc_window_type(),
            start_ms: default_ctc_window_start_ms(),
            length_ms: default_ctc_window_length_ms(),
            fade_ms: default_ctc_window_fade_ms(),
            fdw_cycles: default_ctc_fdw_cycles(),
            fdw_min_ms: default_ctc_fdw_min_ms(),
            fdw_max_ms: default_ctc_fdw_max_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcRegularizationConfig {
    #[serde(default = "default_ctc_beta_db")]
    pub beta_db: f64,
    #[serde(default = "default_ctc_beta_db")]
    pub beta_lf_db: f64,
    #[serde(default = "default_ctc_beta_db")]
    pub beta_hf_db: f64,
    #[serde(default = "default_ctc_max_gain_db")]
    pub max_gain_db: f64,
}

impl Default for CtcRegularizationConfig {
    fn default() -> Self {
        Self {
            beta_db: default_ctc_beta_db(),
            beta_lf_db: default_ctc_beta_db(),
            beta_hf_db: default_ctc_beta_db(),
            max_gain_db: default_ctc_max_gain_db(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CtcConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ctc_matrix_source")]
    pub matrix_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurements: Option<CtcMeasurementConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hrtf: Option<CtcHrtfConfig>,
    #[serde(default)]
    pub window: CtcWindowConfig,
    #[serde(default)]
    pub regularization: CtcRegularizationConfig,
    #[serde(default = "default_ctc_robustness")]
    pub robustness: String,
    /// Include the exported per-channel RoomEQ gain/EQ/delay response in the
    /// acoustic plant before solving the CTC matrix. This matches the runtime
    /// order where the global XTC matrix feeds the per-channel correction
    /// chains.
    #[serde(default = "default_ctc_include_room_eq_dsp")]
    pub include_room_eq_dsp: bool,
    #[serde(default = "default_ctc_fir_taps")]
    pub fir_taps: usize,
    /// Optional emitted sweep WAV used for raw sweep deconvolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_sweep: Option<PathBuf>,
    /// Sweep duration in seconds, used to suppress log-sweep harmonic residues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_duration_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_start_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_end_hz: Option<f64>,
    #[serde(default = "default_ctc_max_harmonic")]
    pub harmonic_suppression_harmonics: usize,
    #[serde(default = "default_ctc_harmonic_window_ms")]
    pub harmonic_suppression_window_ms: f64,
    #[serde(default = "default_ctc_minimax_iterations")]
    pub minimax_iterations: usize,
}

fn default_ctc_max_harmonic() -> usize {
    5
}
fn default_ctc_harmonic_window_ms() -> f64 {
    2.0
}
fn default_ctc_minimax_iterations() -> usize {
    8
}

impl Default for CtcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            matrix_source: default_ctc_matrix_source(),
            measurements: None,
            hrtf: None,
            window: CtcWindowConfig::default(),
            regularization: CtcRegularizationConfig::default(),
            robustness: default_ctc_robustness(),
            include_room_eq_dsp: default_ctc_include_room_eq_dsp(),
            fir_taps: default_ctc_fir_taps(),
            reference_sweep: None,
            sweep_duration_s: None,
            sweep_start_hz: None,
            sweep_end_hz: None,
            harmonic_suppression_harmonics: default_ctc_max_harmonic(),
            harmonic_suppression_window_ms: default_ctc_harmonic_window_ms(),
            minimax_iterations: default_ctc_minimax_iterations(),
        }
    }
}

impl CtcConfig {
    pub fn resolve_paths(&mut self, base_dir: &std::path::Path) {
        if let Some(measurements) = &mut self.measurements {
            measurements.resolve_paths(base_dir);
        }
        if let Some(hrtf) = &mut self.hrtf {
            hrtf.resolve_paths(base_dir);
        }
        if let Some(reference_sweep) = &mut self.reference_sweep
            && reference_sweep.is_relative()
        {
            *reference_sweep = base_dir.join(&*reference_sweep);
        }
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
    /// Cross-talk cancellation / binaural-aware correction configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctc: Option<CtcConfig>,
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
        if let Some(ctc) = &mut self.ctc {
            ctc.resolve_paths(base_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spl_calibration_roundtrip_and_helpers() {
        let cal = SplCalibration {
            reported_db_spl: 85.0,
            reference_freq_hz: 1000.0,
            peak_sample_level: 0.5,
            spl_offset_db: 85.0 - 20.0 * 0.5_f32.log10(),
        };
        // Round-trip through JSON.
        let json = serde_json::to_string(&cal).unwrap();
        let back: SplCalibration = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cal);
        // `dbspl_for_peak_level` at the calibration peak must return the
        // reported dBSPL (within f32 rounding).
        let recovered = cal.dbspl_for_peak_level(cal.peak_sample_level);
        assert!((recovered - cal.reported_db_spl).abs() < 1e-3);
        // `peak_level_for_dbspl` is the inverse.
        let peak = cal.peak_level_for_dbspl(cal.reported_db_spl);
        assert!((peak - cal.peak_sample_level).abs() < 1e-5);
        // Clamp: asking for a dBSPL below the representable range
        // should return 0.0 rather than a negative peak level.
        assert_eq!(cal.peak_level_for_dbspl(-1000.0), 0.0);
    }

    #[test]
    fn recording_configuration_accepts_gd_v2_fields() {
        let cfg_json = serde_json::json!({
            "bass_octave_duration_s": 3.0,
            "pre_silence_s": 2.0,
            "post_silence_s": 4.0,
            "sweep_level_db_spl": 85.0,
            "num_sweeps": 4,
            "coherence_threshold": 0.9,
            "bass_probe_freq_hz": 30.0,
            "bass_probe_duration_s": 2.0,
            "mic_phase_calibration_path": "/tmp/mic_phase.csv",
            "spl_calibration": {
                "reported_db_spl": 85.0,
                "reference_freq_hz": 1000.0,
                "peak_sample_level": 0.5,
                "spl_offset_db": 91.02
            },
            "recording_seed": 42
        });
        let cfg: RecordingConfiguration = serde_json::from_value(cfg_json).unwrap();
        assert_eq!(cfg.bass_octave_duration_s, Some(3.0));
        assert_eq!(cfg.num_sweeps, Some(4));
        assert_eq!(cfg.coherence_threshold, Some(0.9));
        assert_eq!(cfg.bass_probe_freq_hz, Some(30.0));
        assert_eq!(cfg.bass_probe_duration_s, Some(2.0));
        assert_eq!(
            cfg.mic_phase_calibration_path.as_deref(),
            Some("/tmp/mic_phase.csv")
        );
        let cal = cfg.spl_calibration.expect("spl_calibration populated");
        assert!((cal.reported_db_spl - 85.0).abs() < 1e-6);
        assert_eq!(cfg.recording_seed, Some(42));
    }

    #[test]
    fn recording_configuration_legacy_json_still_loads() {
        // A session written before GD-1a only knows the pre-existing
        // fields. GD-Opt v2 metadata must default to `None` rather
        // than failing deserialization — the confidence gate is what
        // converts `None` into `Advisory::GdOptDegradedPhase` at
        // optimization time.
        let legacy_json = serde_json::json!({
            "signal_type": "Sweep",
            "signal_duration_secs": 10.0,
            "sweep_start_freq": 20.0,
            "sweep_end_freq": 20000.0
        });
        let cfg: RecordingConfiguration = serde_json::from_value(legacy_json).unwrap();
        assert!(cfg.bass_octave_duration_s.is_none());
        assert!(cfg.num_sweeps.is_none());
        assert!(cfg.coherence_threshold.is_none());
        assert!(cfg.bass_probe_freq_hz.is_none());
        assert!(cfg.bass_probe_duration_s.is_none());
        assert!(cfg.mic_phase_calibration_path.is_none());
        assert!(cfg.mic_phase_calibration_paths.is_none());
        assert!(cfg.spl_calibration.is_none());
        assert!(cfg.recording_seed.is_none());
    }

    #[test]
    fn bass_anchor_results_legacy_migrates_v1_bass_cycles_to_duration() {
        // Pre-v2 schema: bass_cycles + bass_freq_hz, no bass_duration_s.
        let legacy = serde_json::json!({
            "channels": [],
            "sample_rate": 48_000_u32,
            "bass_freq_hz": 30.0_f32,
            "bass_cycles": 6_u16,
        });
        let r: BassAnchorResultsLegacy = serde_json::from_value(legacy).unwrap();
        // 6 cycles at 30 Hz = 0.2 s.
        assert!(
            (r.bass_duration_s - 0.2).abs() < 1e-6,
            "expected 0.2 s migrated from 6 cycles @ 30 Hz, got {}",
            r.bass_duration_s
        );
        assert_eq!(r.sample_rate, 48_000);
        assert_eq!(r.bass_freq_hz, 30.0);
    }

    #[test]
    fn bass_anchor_results_prefers_explicit_duration_when_both_present() {
        let mixed = serde_json::json!({
            "channels": [],
            "sample_rate": 48_000_u32,
            "bass_freq_hz": 30.0_f32,
            "bass_cycles": 6_u16,
            "bass_duration_s": 2.5_f32,
        });
        let r: BassAnchorResultsLegacy = serde_json::from_value(mixed).unwrap();
        assert!((r.bass_duration_s - 2.5).abs() < 1e-6);
    }

    #[test]
    fn bass_anchor_results_v2_round_trips() {
        let v2 = serde_json::json!({
            "channels": [],
            "sample_rate": 48_000_u32,
            "bass_freq_hz": 30.0_f32,
            "bass_duration_s": 2.0_f32,
        });
        let r: BassAnchorResultsLegacy = serde_json::from_value(v2).unwrap();
        assert_eq!(r.bass_duration_s, 2.0);
    }

    #[test]
    fn target_shape_canonical_wire_format() {
        // Pins the on-the-wire string for every TargetShape variant.
        // `from_measurement` (underscore) is the sole canonical form,
        // matching bin/roomeq/input_schema.json and INPUT_FORMAT.md.
        let cases = [
            (TargetShape::Flat, "\"flat\""),
            (TargetShape::Harman, "\"harman\""),
            (TargetShape::Custom, "\"custom\""),
            (TargetShape::File, "\"file\""),
            (TargetShape::FromMeasurement, "\"from_measurement\""),
        ];
        for (variant, expected) in cases {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(serialized, expected, "serialize {variant:?}");
            let round_tripped: TargetShape = serde_json::from_str(&serialized).unwrap();
            assert_eq!(round_tripped, variant, "round-trip {variant:?}");
        }
        // Old canonical form used before the snake_case switch must no
        // longer deserialize — a paranoid guard against accidental
        // reintroduction of the `#[serde(alias = "from_measurement")]`
        // back-compat shim.
        assert!(serde_json::from_str::<TargetShape>("\"frommeasurement\"").is_err());
    }

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
    fn test_optimizer_config_default_algorithm_is_cmaes() {
        let config = OptimizerConfig::default();
        assert_eq!(config.algorithm, "autoeq:cmaes");
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
        assert!(dc.fdw_enabled);
        assert_eq!(dc.fdw_cycles, 8.0);
        assert_eq!(dc.fdw_min_window_ms, 3.0);
        assert_eq!(dc.fdw_max_window_ms, 500.0);
        assert_eq!(dc.fdw_smoothing_octaves, 1.0 / 24.0);
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
