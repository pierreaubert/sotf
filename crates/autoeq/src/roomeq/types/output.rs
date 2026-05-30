//! Room EQ Output Types
//!
//! Types for returning optimization results and DSP chain outputs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export Curve for reference in output docs
pub use crate::Curve;

// ============================================================================
// Frequency Response Curve Data
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
            ..Default::default()
        }
    }
}

// ============================================================================
// Impulse Response Waveform
// ============================================================================

/// Impulse response waveform (time-domain)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IrWaveform {
    /// Time axis in milliseconds
    pub time_ms: Vec<f64>,
    /// Amplitude (normalized so pre-IR peak = 1.0)
    pub amplitude: Vec<f64>,
}

// ============================================================================
// DSP Chain Types
// ============================================================================

/// DSP chain output (AudioEngine PluginConfig format)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DspChainOutput {
    /// Output version
    #[serde(default = "crate::roomeq::types::default_config_version")]
    pub version: String,
    /// Global graph-level plugins, e.g. matrix routing that combines several
    /// programme inputs before per-output correction chains.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub global_plugins: Vec<PluginConfigWrapper>,
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
    /// EQ filter response curve (correction magnitude in dB) (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eq_response: Option<CurveData>,
    /// Effective target curve the optimizer worked against (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_curve: Option<CurveData>,
    /// Impulse response before correction (optional, requires phase data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_ir: Option<IrWaveform>,
    /// Impulse response after correction (optional, requires phase data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_ir: Option<IrWaveform>,
    /// FIR impulse-response temporal masking metrics (optional, FIR/phase modes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fir_temporal_masking: Option<crate::loss::epa::score::TemporalIrMaskingMetrics>,
    /// Direct/early/late correction-energy metrics (optional, FIR/phase modes
    /// or any channel with phase-derived IRs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_early_late_correction: Option<DirectEarlyLateCorrectionMetrics>,
}

/// DSP chain for an individual driver in a multi-driver speaker
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DriverDspChain {
    /// Driver name (e.g. "woofer", "tweeter")
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

/// Per-channel EPA psychoacoustic metrics computed on the initial
/// (pre-EQ) and final (post-EQ) frequency responses.
///
/// See [`crate::loss::epa::score::EpaScore`] for the individual fields.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EpaChannelMetrics {
    /// EPA score computed from the initial (pre-EQ) response.
    pub pre: crate::loss::epa::score::EpaScore,
    /// EPA score computed from the final (post-EQ) response.
    pub post: crate::loss::epa::score::EpaScore,
}

/// Aggregate EPA metrics for the whole reproduced channel set.
///
/// Channels are combined with BS.1770-style energy weights before EPA scoring:
/// front/main channels use unit weight, surround channels use +1.5 dB energy
/// weight, and LFE/subwoofer channels are excluded.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EpaMultichannelMetrics {
    /// EPA score computed from the aggregate initial (pre-EQ) response.
    pub pre: crate::loss::epa::score::EpaScore,
    /// EPA score computed from the aggregate final (post-EQ) response.
    pub post: crate::loss::epa::score::EpaScore,
    /// Human-readable aggregation standard/approximation.
    pub standard: String,
}

/// Correction-energy split across direct, early, and late IR windows.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DirectEarlyLateCorrectionMetrics {
    /// Direct-sound window end in milliseconds.
    pub direct_window_ms: f64,
    /// Early-reflection window end in milliseconds.
    pub early_window_ms: f64,
    /// Late summary window end in milliseconds.
    pub late_window_ms: f64,
    /// Correction energy in the direct window, dB relative to total correction energy.
    pub direct_energy_db: f64,
    /// Correction energy in the early window, dB relative to total correction energy.
    pub early_energy_db: f64,
    /// Correction energy in the late window, dB relative to total correction energy.
    pub late_energy_db: f64,
    /// Direct + early correction energy, dB relative to total correction energy.
    pub direct_plus_early_energy_db: f64,
    /// Advisory when FIR/mixed-phase correction may be altering direct/early cues.
    pub advisory: String,
}

/// Resolved perceptual policy metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerceptualPolicyReport {
    /// Policy preset requested by the user or UI.
    pub preset: crate::roomeq::PerceptualPolicyPreset,
    /// Effective loss type after policy resolution.
    pub loss_type: String,
    /// Effective target response after policy resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_response: Option<crate::roomeq::TargetResponseConfig>,
    /// Effective audibility deadband.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audibility_deadband: Option<crate::roomeq::AudibilityDeadbandConfig>,
    /// Effective high-frequency safeguard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_frequency_correction: Option<crate::roomeq::HighFrequencyCorrectionConfig>,
}

/// Bootstrap uncertainty reporting summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BootstrapUncertaintyReport {
    /// Number of case-bootstrap resamples.
    pub num_resamples: usize,
    /// Two-sided alpha used for confidence bands.
    pub alpha: f64,
    /// Scalarisation used for uncertainty-aware optimization.
    pub scalarisation: crate::roomeq::BootstrapScalarisation,
    /// CVaR tail fraction when scalarisation is CVaR.
    pub cvar_alpha: f64,
    /// Whether bootstrap confidence width was folded into correction-depth masks.
    pub used_for_correction_depth_mask: bool,
}

/// Validation/listening-test bundle descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationBundleReport {
    /// JSON artifact path.
    pub artifact: String,
    /// Target loudness for matched validation assets.
    pub target_lufs: f64,
    /// ABX descriptor included.
    pub abx: bool,
    /// MUSHRA descriptor included.
    pub mushra: bool,
    /// Perceptual regression summary included.
    pub perceptual_regression_summary: bool,
    /// Bundle advisories.
    pub advisories: Vec<String>,
}

/// Compact perceptual scorecard for downstream QA and UIs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerceptualMetrics {
    /// Average EPA preference before correction.
    pub epa_preference_pre: f64,
    /// Average EPA preference after correction.
    pub epa_preference_post: f64,
    /// EPA preference delta, positive means perceptual improvement.
    pub epa_preference_delta: f64,
    /// Midrange inter-channel deviation in dB when more than one comparable
    /// channel exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_matching_midrange_rms_db: Option<f64>,
    /// Role-aware channel matching RMS, computed only inside comparable
    /// channel groups such as L/R, surrounds, or matching height pairs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_channel_matching_rms_db: Option<f64>,
    /// Bass-seat/output consistency RMS across sub/LFE outputs in the
    /// modal/crossover band. Lower is better.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_consistency_rms_db: Option<f64>,
    /// Center-channel dialog-band roughness after removing the local mean.
    /// Lower is better for speech intelligibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialog_band_roughness_rms_db: Option<f64>,
    /// Peak positive gain requested by exported gain/EQ plugins. High values
    /// are a clipping/headroom risk even when the final curve looks smooth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_peak_boost_db: Option<f64>,
    /// Advisory derived from `headroom_peak_boost_db`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_risk: Option<String>,
    /// Human-readable timing/GD confidence label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_confidence: Option<String>,
    /// Maximum FIR pre-ringing audible energy across channels, dB relative to
    /// each FIR's main impulse peak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fir_pre_ringing_audible_db: Option<f64>,
    /// Maximum FIR post-ringing audible energy across channels, dB relative to
    /// each FIR's main impulse peak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fir_post_ringing_audible_db: Option<f64>,
    /// Maximum FIR temporal masking penalty across channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fir_temporal_masking_penalty: Option<f64>,
    /// Maximum direct + early correction energy across channels, dB relative
    /// to total correction energy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_plus_early_correction_energy_db: Option<f64>,
    /// Worst direct/early cue advisory across channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_cue_advisory: Option<String>,
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
    /// Loss function that the optimizer minimized.
    /// One of `"flat"`, `"score"`, `"epa"`.
    ///
    /// Note: `pre_score` and `post_score` are *not* values of this loss
    /// function — they are always computed by
    /// `crate::roomeq::workflows::compute_flat_loss` over the
    /// `[min_freq, max_freq]` evaluation window so that runs with
    /// different `loss_type` values stay on the same scale and can be
    /// compared directly. To compare *perceptual* outcomes across
    /// loss types use `epa_per_channel.{pre,post}.preference` instead,
    /// which is computed identically for every run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_type: Option<String>,
    /// Number of iterations
    pub iterations: usize,
    /// Timestamp
    pub timestamp: String,
    /// Inter-channel deviation metric (computed when >1 channel)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_channel_deviation: Option<crate::roomeq::types::InterChannelDeviation>,
    /// Per-channel EPA psychoacoustic metrics (pre-EQ and post-EQ).
    /// Computed from each channel's initial and final frequency responses
    /// using the configured `EpaConfig` (or defaults when unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epa_per_channel: Option<HashMap<String, EpaChannelMetrics>>,
    /// Whole-system EPA psychoacoustic metrics computed after BS.1770-style
    /// channel-energy aggregation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epa_multichannel: Option<EpaMultichannelMetrics>,
    /// Group delay optimisation summary (GD-Opt v2, Phase GD-4).
    /// Present when GD-Opt was attempted (success or skip with advisory).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_delay: Option<crate::roomeq::gd_opt::GroupDelayOptSummary>,
    /// Perceptual scorecard computed from final exported curves/DSP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perceptual_metrics: Option<PerceptualMetrics>,
    /// Home-cinema role/layout interpretation used by role-aware scoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_cinema_layout: Option<crate::roomeq::home_cinema::HomeCinemaLayoutReport>,
    /// Coverage summary for multi-position measurements beyond sub-only MSO.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_seat_coverage: Option<crate::roomeq::home_cinema::MultiSeatCoverageReport>,
    /// All-channel multi-seat correction summary for non-sub home-cinema channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_seat_correction: Option<crate::roomeq::home_cinema::MultiSeatCorrectionReport>,
    /// Bass-management policy and applied trim/headroom summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_management: Option<crate::roomeq::home_cinema::BassManagementReport>,
    /// Timing/localization diagnostics derived from measured arrivals and
    /// final exported delay plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_diagnostics: Option<crate::roomeq::home_cinema::TimingDiagnosticsReport>,
    /// Cross-talk cancellation / binaural-aware correction artifact summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctc: Option<crate::roomeq::ctc::CtcReport>,
    /// Resolved perceptual policy metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perceptual_policy: Option<PerceptualPolicyReport>,
    /// Bootstrap uncertainty summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_uncertainty: Option<BootstrapUncertaintyReport>,
    /// Validation/listening-test bundle descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_bundle: Option<ValidationBundleReport>,
}
