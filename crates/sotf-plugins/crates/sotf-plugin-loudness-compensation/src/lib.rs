// ============================================================================
// Loudness Compensation Plugin
// ============================================================================
//
// Two modes of operation:
//
// **Manual mode** (default, backward compatible):
//   Uses 3 bands (5 biquads): lowshelf at 100Hz, peak at 3.5kHz, highshelf at
//   8kHz. Filter frequencies are derived from ISO 226:2003 equal-loudness
//   contour inflection points. Gains are user-controlled.
//
// **ISO 226 mode**:
//   Full ISO 226:2003 equal-loudness contour lookup table. Computes a
//   compensation curve from the delta between the reference level (default 83
//   dB SPL) and the playback level, then fits 7 parametric EQ bands to match
//   the delta curve. All filter computation happens at parameter-change time;
//   the hot path only applies pre-computed biquads.
// ============================================================================

pub mod iso226;
pub mod params;

use crate::params::PARAMS as LC;
use iso226::{ISO226_NUM_FREQS, compute_iso226_delta, interpolate_delta};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Controls where auto-gain measurement and compensation are applied
/// relative to the EQ filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoGainPosition {
    /// Measure input before filters, apply compensation after filters (current default)
    Post,
    /// Measure and apply compensation before filters (pre-filter gain matching)
    Pre,
    /// Auto-gain disabled
    Disabled,
}

impl fmt::Display for AutoGainPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutoGainPosition::Post => write!(f, "post"),
            AutoGainPosition::Pre => write!(f, "pre"),
            AutoGainPosition::Disabled => write!(f, "disabled"),
        }
    }
}

impl AutoGainPosition {
    fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pre" => AutoGainPosition::Pre,
            "disabled" | "off" => AutoGainPosition::Disabled,
            _ => AutoGainPosition::Post,
        }
    }
}

// ============================================================================
// ISO 226:2003 Equal-Loudness Contour Reference Frequencies
// ============================================================================
//
// Default filter frequencies are derived from ISO 226:2003 equal-loudness
// contour data (see param_specs::loudness_compensation). At 83 dB SPL
// reference level, the contour shape dictates where compensation filters
// should be placed:
//
//   - Low shelf at ~100 Hz: ISO 226 shows a steep rise in threshold below
//     100 Hz. The 100-phon contour inflects here, making it the optimal
//     center frequency for bass compensation.
//
//   - Midrange peak at ~3.5 kHz: The ear canal resonance creates maximum
//     sensitivity near 3.5 kHz (ISO 226 shows the deepest dip in the
//     equal-loudness contour at this frequency). A peak here compensates
//     for the ear's natural sensitivity advantage.
//
//   - High shelf at ~8 kHz: ISO 226 shows sensitivity declining above
//     ~8 kHz. The 80-phon contour begins rising steeply above 8 kHz,
//     making this the optimal center for high-frequency compensation.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoudnessCompensation {
    pub reference_level: f64,
    pub low_boost: f64,
    pub high_boost: f64,
    #[serde(default)]
    pub attenuate_mid: bool,
}

impl LoudnessCompensation {
    pub fn new(reference_level: f64, low_boost: f64, high_boost: f64) -> Result<Self, String> {
        Ok(Self {
            reference_level,
            low_boost,
            high_boost,
            attenuate_mid: false,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLoudnessParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
    #[serde(default = "default_mid_freq")]
    pub mid_freq: f32,
    #[serde(default = "default_mid_gain")]
    pub mid_gain: f32,
    #[serde(default = "default_mid_q")]
    pub mid_q: f32,
}

fn default_low_freq() -> f32 {
    pk(LC, "low_freq").default_f32()
}
fn default_low_gain() -> f32 {
    pk(LC, "low_gain").default_f32()
}
fn default_high_freq() -> f32 {
    pk(LC, "high_freq").default_f32()
}
fn default_high_gain() -> f32 {
    pk(LC, "high_gain").default_f32()
}
fn default_mid_freq() -> f32 {
    pk(LC, "mid_freq").default_f32()
}
fn default_mid_gain() -> f32 {
    pk(LC, "mid_gain").default_f32()
}
fn default_mid_q() -> f32 {
    pk(LC, "mid_q").default_f32()
}
fn default_mid_enabled() -> bool {
    true
}

impl Default for ChannelLoudnessParams {
    fn default() -> Self {
        Self {
            low_freq: default_low_freq(),
            low_gain: default_low_gain(),
            high_freq: default_high_freq(),
            high_gain: default_high_gain(),
            mid_freq: default_mid_freq(),
            mid_gain: default_mid_gain(),
            mid_q: default_mid_q(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoudnessCompensationPluginParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
    #[serde(default = "default_mid_enabled")]
    pub mid_enabled: bool,
    #[serde(default = "default_mid_freq")]
    pub mid_freq: f32,
    #[serde(default = "default_mid_gain")]
    pub mid_gain: f32,
    #[serde(default = "default_mid_q")]
    pub mid_q: f32,
    #[serde(default)]
    pub channel_params: Vec<ChannelLoudnessParams>,
    #[serde(default)]
    pub auto_gain_enabled: bool,
    #[serde(default)]
    pub auto_gain_max_db: f32,
    #[serde(default)]
    pub auto_gain_smoothing_ms: f32,
    /// Auto-gain position: "pre", "post" (default), or "disabled"
    #[serde(default = "default_auto_gain_position")]
    pub auto_gain_position: String,
    /// 0 = Manual (default), 1 = ISO 226, 2 = Auto
    #[serde(default)]
    pub mode: usize,
    #[serde(default = "default_playback_level_db")]
    pub playback_level_db: f32,
    #[serde(default = "default_reference_level_db")]
    pub reference_level_db: f32,
    /// Engine playback volume in dB (used in Auto mode)
    #[serde(default)]
    pub playback_volume_db: f32,
}

fn default_auto_gain_position() -> String {
    "post".to_string()
}
fn default_playback_level_db() -> f32 {
    pk(LC, "playback_level_db").default_f32()
}
fn default_reference_level_db() -> f32 {
    pk(LC, "reference_level_db").default_f32()
}

// ============================================================================
// Fletcher-Munson backward compatibility
// ============================================================================

/// Backward-compatible deserialization of old FletcherMunson configs.
/// When the factory receives a `FletcherMunson` plugin type, it deserializes
/// into this struct and then converts to `LoudnessCompensationPluginParams`
/// with `mode = 2` (Auto).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FletcherMunsonCompat {
    #[serde(default)]
    pub playback_volume_db: f32,
    #[serde(default = "default_fm_compat_reference")]
    pub reference_level_db: f32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_gain_enabled: bool,
    #[serde(default)]
    pub smoothing_ms: f32,
}

fn default_fm_compat_reference() -> f32 {
    -14.0
}

impl FletcherMunsonCompat {
    /// Convert this backward-compat struct to a LoudnessCompensation params in Auto mode.
    pub fn into_loudness_compensation_params(self) -> LoudnessCompensationPluginParams {
        LoudnessCompensationPluginParams {
            mode: 2, // Auto
            playback_volume_db: self.playback_volume_db,
            // Convert relative reference_level_db to absolute SPL estimate for ISO 226.
            // Old FM used relative dB (e.g. -14). Map to SPL: 83 + reference_level_db.
            reference_level_db: 83.0 + self.reference_level_db,
            auto_gain_enabled: self.auto_gain_enabled,
            ..Default::default()
        }
    }
}

/// Type alias for backward compatibility.
pub type FletcherMunsonPlugin = LoudnessCompensationPlugin;
/// Type alias for backward compatibility.
pub type FletcherMunsonPluginParams = LoudnessCompensationPluginParams;

// ============================================================================
// ISO 226 Filter Bank
// ============================================================================

/// Number of ISO 226 compensation filters per channel.
const ISO_FILTER_COUNT: usize = 7;

/// Center frequencies for the 7 ISO 226 compensation bands.
const ISO_BAND_FREQS: [f64; ISO_FILTER_COUNT] =
    [50.0, 150.0, 500.0, 1500.0, 3500.0, 7000.0, 10000.0];

/// Q factors for the 7 ISO 226 compensation bands.
const ISO_BAND_QS: [f64; ISO_FILTER_COUNT] = [0.7, 0.8, 1.0, 1.2, 1.5, 1.2, 0.8];

pub struct LoudnessCompensationPlugin {
    num_channels: usize,
    sample_rate: u32,
    // -- Manual mode fields --
    low_freq: f32,
    low_gain: f32,
    high_freq: f32,
    high_gain: f32,
    mid_enabled: bool,
    mid_freq: f32,
    mid_gain: f32,
    mid_q: f32,
    /// Manual mode filters: [channel][filter_index], 5 biquads per channel.
    filters: Vec<Vec<Biquad>>,
    // -- ISO 226 / Auto mode fields --
    /// 0 = Manual, 1 = ISO 226, 2 = Auto
    mode_index: usize,
    playback_level_db: f32,
    reference_level_db: f32,
    /// Engine playback volume in dB (relative, set externally). Used in Auto mode.
    playback_volume_db: f32,
    /// Last volume at which ISO filters were rebuilt (Auto mode). Prevents
    /// per-frame rebuilds; filters are only rebuilt when volume changes by >0.5 dB.
    last_auto_volume_db: f32,
    /// ISO 226 mode filters: [channel][band_index], 7 biquads per channel.
    /// Pre-allocated in `new()`, coefficients updated in `rebuild_iso_filters()`.
    iso_filters: Vec<Vec<Biquad>>,
    /// Cached ISO 226 delta curve for the current playback/reference levels.
    iso_deltas: [(f64, f64); ISO226_NUM_FREQS],
    // -- Common fields --
    auto_gain: Option<AutoGain>,
    auto_gain_enabled: bool,
    auto_gain_max_db: f32,
    auto_gain_smoothing_ms: f32,
    auto_gain_position: AutoGainPosition,
    comp_gain_smoother: Vec<Smoother>,
    cache: RealTimeCache<AutoGainData>,
    cached_parameters: Vec<Parameter>,
}

impl LoudnessCompensationPlugin {
    pub fn new(
        num_channels: usize,
        low_freq: f32,
        low_gain: f32,
        high_freq: f32,
        high_gain: f32,
    ) -> Self {
        let sr = 48000;
        let playback_db = default_playback_level_db();
        let reference_db = default_reference_level_db();
        let mut p = Self {
            num_channels,
            sample_rate: sr,
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            mid_enabled: default_mid_enabled(),
            mid_freq: default_mid_freq(),
            mid_gain: default_mid_gain(),
            mid_q: default_mid_q(),
            filters: vec![Vec::new(); num_channels],
            mode_index: 0, // Manual by default
            playback_level_db: playback_db,
            reference_level_db: reference_db,
            playback_volume_db: 0.0,
            last_auto_volume_db: 0.0,
            iso_filters: vec![Vec::new(); num_channels],
            iso_deltas: compute_iso226_delta(playback_db as f64, reference_db as f64),
            auto_gain: None,
            auto_gain_enabled: false,
            auto_gain_max_db: pk(LC, "auto_gain_max_db").default_f32(),
            auto_gain_smoothing_ms: pk(LC, "auto_gain_smoothing_ms").default_f32(),
            auto_gain_position: AutoGainPosition::Post,
            comp_gain_smoother: (0..num_channels)
                .map(|_| Smoother::new(1.0, 20.0, sr))
                .collect(),
            cache: RealTimeCache::new(AutoGainData::default()),
            cached_parameters: Vec::new(),
        };
        p.rebuild_filters();
        p.rebuild_iso_filters();
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "low_gain",
                "Bass Boost",
                self.low_gain,
                pk(LC, "low_gain").min_f64() as f32,
                pk(LC, "low_gain").max_f64() as f32,
            )
            .with_description("Low-frequency shelf gain (dB)")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "high_gain",
                "Treble Boost",
                self.high_gain,
                pk(LC, "high_gain").min_f64() as f32,
                pk(LC, "high_gain").max_f64() as f32,
            )
            .with_description("High-frequency shelf gain (dB)")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "low_freq",
                "Low Frequency",
                self.low_freq,
                pk(LC, "low_freq").min_f64() as f32,
                pk(LC, "low_freq").max_f64() as f32,
            )
            .with_description("Low shelf center frequency (Hz)")
            .with_group("Frequency")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "high_freq",
                "High Frequency",
                self.high_freq,
                pk(LC, "high_freq").min_f64() as f32,
                pk(LC, "high_freq").max_f64() as f32,
            )
            .with_description("High shelf center frequency (Hz)")
            .with_group("Frequency")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("mid_enabled", "Mid Enabled", self.mid_enabled)
                .with_description("Enable midrange peak band")
                .with_group("Mid"),
            Parameter::new_float(
                "mid_freq",
                "Mid Frequency",
                self.mid_freq,
                pk(LC, "mid_freq").min_f64() as f32,
                pk(LC, "mid_freq").max_f64() as f32,
            )
            .with_description("Midrange peak center frequency (Hz)")
            .with_group("Mid"),
            Parameter::new_float(
                "mid_gain",
                "Mid Gain",
                self.mid_gain,
                pk(LC, "mid_gain").min_f64() as f32,
                pk(LC, "mid_gain").max_f64() as f32,
            )
            .with_description("Midrange peak gain (dB)")
            .with_group("Mid"),
            Parameter::new_float(
                "mid_q",
                "Mid Q",
                self.mid_q,
                pk(LC, "mid_q").min_f64() as f32,
                pk(LC, "mid_q").max_f64() as f32,
            )
            .with_description("Midrange peak Q factor")
            .with_group("Mid"),
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", self.auto_gain_enabled)
                .with_group("Auto Gain"),
            Parameter::new_float(
                "auto_gain_max_db",
                "AG Max",
                self.auto_gain_max_db,
                pk(LC, "auto_gain_max_db").min_f64() as f32,
                pk(LC, "auto_gain_max_db").max_f64() as f32,
            )
            .with_group("Auto Gain"),
            Parameter::new_float(
                "auto_gain_smoothing_ms",
                "AG Smoothing",
                self.auto_gain_smoothing_ms,
                pk(LC, "auto_gain_smoothing_ms").min_f64() as f32,
                pk(LC, "auto_gain_smoothing_ms").max_f64() as f32,
            )
            .with_group("Auto Gain"),
            Parameter::new_string(
                "auto_gain_position",
                "AG Position",
                self.auto_gain_position.to_string(),
            )
            .with_description("Auto-gain position: pre, post, or disabled")
            .with_group("Auto Gain"),
            Parameter::new_int("mode", "Mode", self.mode_index as i32, 0, 2)
                .with_description("0 = Manual, 1 = ISO 226, 2 = Auto")
                .with_group("Compensation"),
            Parameter::new_float(
                "playback_level_db",
                "Playback Level",
                self.playback_level_db,
                pk(LC, "playback_level_db").min_f64() as f32,
                pk(LC, "playback_level_db").max_f64() as f32,
            )
            .with_description("Current playback level (dB SPL)")
            .with_group("Compensation"),
            Parameter::new_float(
                "reference_level_db",
                "Reference Level",
                self.reference_level_db,
                pk(LC, "reference_level_db").min_f64() as f32,
                pk(LC, "reference_level_db").max_f64() as f32,
            )
            .with_description("Reference listening level (dB SPL)")
            .with_group("Compensation"),
            Parameter::new_float(
                "playback_volume_db",
                "Playback Volume",
                self.playback_volume_db,
                pk(LC, "playback_volume_db").min_f64() as f32,
                pk(LC, "playback_volume_db").max_f64() as f32,
            )
            .with_description("Engine playback volume (dB, set automatically)")
            .with_group("Auto"),
        ];
    }

    /// Expected filter count per channel for manual mode: 2x lowshelf + 1x mid peak + 2x highshelf = 5
    const FILTER_COUNT: usize = 5;

    /// Rebuild the manual-mode 3-band filters (5 biquads per channel).
    fn rebuild_filters(&mut self) {
        let q = 0.707;
        let sr = self.sample_rate as f64;
        // Manual mode intentionally uses two cascaded shelves at half the requested
        // gain. This gives a steeper transition than a single shelf, but the
        // combined response around the corner is an approximation rather than an
        // exact additive `low_gain`/`high_gain` curve.
        let lg = self.low_gain / 2.0;
        let hg = self.high_gain / 2.0;
        // When midrange is disabled, set gain to 0 dB so the peak filter is a no-op
        let mg = if self.mid_enabled { self.mid_gain } else { 0.0 };
        for ch in 0..self.num_channels {
            if self.filters[ch].len() == Self::FILTER_COUNT {
                // Update coefficients in place — preserves filter delay state
                // (x1/x2/y1/y2) so parameter changes are click-free.
                self.filters[ch][0].update_params(
                    BiquadFilterType::Lowshelf,
                    self.low_freq as f64,
                    sr,
                    q,
                    lg as f64,
                );
                self.filters[ch][1].update_params(
                    BiquadFilterType::Lowshelf,
                    self.low_freq as f64,
                    sr,
                    q,
                    lg as f64,
                );
                self.filters[ch][2].update_params(
                    BiquadFilterType::Peak,
                    self.mid_freq as f64,
                    sr,
                    self.mid_q as f64,
                    mg as f64,
                );
                self.filters[ch][3].update_params(
                    BiquadFilterType::Highshelf,
                    self.high_freq as f64,
                    sr,
                    q,
                    hg as f64,
                );
                self.filters[ch][4].update_params(
                    BiquadFilterType::Highshelf,
                    self.high_freq as f64,
                    sr,
                    q,
                    hg as f64,
                );
            } else {
                // First initialization — create filters from scratch
                self.filters[ch] = vec![
                    Biquad::new(
                        BiquadFilterType::Lowshelf,
                        self.low_freq as f64,
                        sr,
                        q,
                        lg as f64,
                    ),
                    Biquad::new(
                        BiquadFilterType::Lowshelf,
                        self.low_freq as f64,
                        sr,
                        q,
                        lg as f64,
                    ),
                    Biquad::new(
                        BiquadFilterType::Peak,
                        self.mid_freq as f64,
                        sr,
                        self.mid_q as f64,
                        mg as f64,
                    ),
                    Biquad::new(
                        BiquadFilterType::Highshelf,
                        self.high_freq as f64,
                        sr,
                        q,
                        hg as f64,
                    ),
                    Biquad::new(
                        BiquadFilterType::Highshelf,
                        self.high_freq as f64,
                        sr,
                        q,
                        hg as f64,
                    ),
                ];
            }
        }
        self.update_comp_gain_smoother();
    }

    /// Rebuild ISO 226 filters based on current playback/reference levels.
    ///
    /// Fits 7 parametric EQ bands to the ISO 226 delta contour.
    /// Called at parameter-change time only, never in the hot path.
    fn rebuild_iso_filters(&mut self) {
        let sr = self.sample_rate as f64;
        self.iso_deltas = compute_iso226_delta(
            self.playback_level_db as f64,
            self.reference_level_db as f64,
        );

        for ch in 0..self.num_channels {
            if self.iso_filters[ch].len() == ISO_FILTER_COUNT {
                // Update in place — preserves filter delay state for click-free transitions
                for (band_idx, &freq) in ISO_BAND_FREQS.iter().enumerate() {
                    let gain = interpolate_delta(&self.iso_deltas, freq);
                    let q = ISO_BAND_QS[band_idx];
                    let filter_type = if band_idx == 0 {
                        BiquadFilterType::Lowshelf
                    } else if band_idx == ISO_FILTER_COUNT - 1 {
                        BiquadFilterType::Highshelf
                    } else {
                        BiquadFilterType::Peak
                    };
                    self.iso_filters[ch][band_idx].update_params(filter_type, freq, sr, q, gain);
                }
            } else {
                // First initialization — create from scratch
                self.iso_filters[ch] = Vec::with_capacity(ISO_FILTER_COUNT);
                for (band_idx, &freq) in ISO_BAND_FREQS.iter().enumerate() {
                    let gain = interpolate_delta(&self.iso_deltas, freq);
                    let q = ISO_BAND_QS[band_idx];
                    let filter_type = if band_idx == 0 {
                        BiquadFilterType::Lowshelf
                    } else if band_idx == ISO_FILTER_COUNT - 1 {
                        BiquadFilterType::Highshelf
                    } else {
                        BiquadFilterType::Peak
                    };
                    self.iso_filters[ch].push(Biquad::new(filter_type, freq, sr, q, gain));
                }
            }
        }
        self.update_comp_gain_smoother();
    }

    /// Number of log-spaced frequency points used to evaluate the combined
    /// ISO filter chain peak gain for comp-gain calculation (Bug #1 fix).
    const COMP_GAIN_GRID_POINTS: usize = 128;

    /// Update the compensation gain smoother targets based on the active mode.
    ///
    /// For ISO 226 / Auto modes, the combined response of 7 parametric EQ bands
    /// is evaluated on a 128-point log-spaced grid (20 Hz – 20 kHz) to capture
    /// constructive interference (ripple peaks) that occur between band centres.
    /// Evaluating only at the 7 band-centre frequencies can underestimate the
    /// true peak by several dB, causing under-attenuation and potential clipping.
    fn update_comp_gain_smoother(&mut self) {
        for ch in 0..self.num_channels {
            let max_gain = if self.mode_index == 1 || self.mode_index == 2 {
                // ISO 226 / Auto mode: evaluate combined filter response on a dense
                // log-spaced grid and find the true peak gain in dB.
                // Use channel 0 filters — all channels share the same coefficients.
                let ref_ch = if self.iso_filters.is_empty() { ch } else { 0 };
                if self.iso_filters[ref_ch].is_empty() {
                    0.0_f32
                } else {
                    let f_lo = 20.0_f64;
                    let f_hi = 20000.0_f64;
                    let log_lo = f_lo.ln();
                    let log_hi = f_hi.ln();
                    let n = Self::COMP_GAIN_GRID_POINTS;
                    let mut peak_db = 0.0_f64;
                    for k in 0..n {
                        let t = k as f64 / (n - 1) as f64;
                        let freq = (log_lo + t * (log_hi - log_lo)).exp();
                        // Sum log-magnitude responses of all bands (dB is additive for cascades)
                        let combined_db: f64 = self.iso_filters[ref_ch]
                            .iter()
                            .map(|f| f.log_result(freq))
                            .sum();
                        if combined_db.abs() > peak_db {
                            peak_db = combined_db.abs();
                        }
                    }
                    peak_db as f32
                }
            } else {
                // Manual mode: abs gain (shelves and peak are already in dB at their centres)
                let low_abs = self.low_gain.abs();
                let high_abs = self.high_gain.abs();
                if self.mid_enabled {
                    low_abs.max(high_abs).max(self.mid_gain.abs())
                } else {
                    low_abs.max(high_abs)
                }
            };
            let target = 10.0_f32.powf(-max_gain / 20.0);
            self.comp_gain_smoother[ch].set_target(target);
        }
    }

    pub fn from_params(
        num_channels: usize,
        params: LoudnessCompensationPluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(
            num_channels,
            params.low_freq,
            params.low_gain,
            params.high_freq,
            params.high_gain,
        );
        p.mid_enabled = params.mid_enabled;
        p.mid_freq = params.mid_freq;
        p.mid_gain = params.mid_gain;
        p.mid_q = params.mid_q;
        p.mode_index = params.mode;
        p.playback_level_db = params.playback_level_db;
        p.reference_level_db = params.reference_level_db;
        p.playback_volume_db = params.playback_volume_db;
        p.last_auto_volume_db = params.playback_volume_db;
        p.auto_gain_position = AutoGainPosition::from_str_lossy(&params.auto_gain_position);
        // auto_gain_enabled overrides position: if explicitly disabled, position becomes Disabled
        if !params.auto_gain_enabled {
            p.auto_gain_position = AutoGainPosition::Disabled;
        }
        p.auto_gain_enabled = p.auto_gain_position != AutoGainPosition::Disabled;
        p.auto_gain_max_db = params.auto_gain_max_db;
        p.auto_gain_smoothing_ms = params.auto_gain_smoothing_ms;
        if p.auto_gain_enabled {
            p.auto_gain = Some(AutoGain::new(
                num_channels,
                p.sample_rate,
                AutoGainParams {
                    enabled: true,
                    loudness_type: AutoGainLoudnessType::Momentary,
                    max_gain_db: params.auto_gain_max_db,
                    smoothing_ms: params.auto_gain_smoothing_ms,
                },
            )?);
        }
        p.rebuild_filters();
        p.rebuild_iso_filters();
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Process a single frame through the active filter bank.
    /// Returns the processed sample value.
    #[inline(always)]
    fn process_sample(&mut self, ch: usize, sample: f32) -> f32 {
        let mut s = sample as f64;
        if self.mode_index == 1 || self.mode_index == 2 {
            // ISO 226 / Auto mode — both use the iso_filters bank
            for f in &mut self.iso_filters[ch] {
                s = f.process(s);
            }
        } else {
            // Manual mode
            for f in &mut self.filters[ch] {
                s = f.process(s);
            }
        }
        (s as f32) * self.comp_gain_smoother[ch].advance()
    }

    /// In Auto mode, rebuild ISO 226 filters based on engine volume.
    /// Converts relative `playback_volume_db` to absolute SPL estimate:
    ///   estimated_spl = reference_level_db + playback_volume_db
    /// Only rebuilds if volume changed by >0.5 dB since last rebuild.
    fn maybe_rebuild_auto_filters(&mut self) {
        if self.mode_index != 2 {
            return;
        }
        let delta = (self.playback_volume_db - self.last_auto_volume_db).abs();
        if delta < 0.5 {
            return;
        }
        self.last_auto_volume_db = self.playback_volume_db;
        // Compute effective SPL: 0 dB volume = reference_level_db SPL
        let estimated_spl = self.reference_level_db + self.playback_volume_db;
        // Clamp to valid ISO 226 range (20-90 phon)
        let estimated_phon = (estimated_spl as f64).clamp(20.0, 90.0);
        let reference_phon = (self.reference_level_db as f64).clamp(20.0, 90.0);
        // Temporarily set playback_level_db for rebuild_iso_filters
        let saved_playback = self.playback_level_db;
        self.playback_level_db = estimated_phon as f32;
        let saved_reference = self.reference_level_db;
        self.reference_level_db = reference_phon as f32;
        self.rebuild_iso_filters();
        self.playback_level_db = saved_playback;
        self.reference_level_db = saved_reference;
    }
}

impl InPlacePlugin for LoudnessCompensationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Compensation", "3.0.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id.0 == "low_gain" {
            let v = value.as_float().unwrap_or(pk(LC, "low_gain").default_f32());
            if v.is_finite() {
                self.low_gain = v;
                self.rebuild_filters();
            }
        } else if id.0 == "high_gain" {
            let v = value
                .as_float()
                .unwrap_or(pk(LC, "high_gain").default_f32());
            if v.is_finite() {
                self.high_gain = v;
                self.rebuild_filters();
            }
        } else if id.0 == "low_freq" {
            let v = value.as_float().unwrap_or(pk(LC, "low_freq").default_f32());
            if v.is_finite() {
                self.low_freq = v;
                self.rebuild_filters();
            }
        } else if id.0 == "high_freq" {
            let v = value
                .as_float()
                .unwrap_or(pk(LC, "high_freq").default_f32());
            if v.is_finite() {
                self.high_freq = v;
                self.rebuild_filters();
            }
        } else if id.0 == "mid_enabled" {
            let v = value
                .as_bool()
                .ok_or_else(|| "mid_enabled must be a boolean".to_string())?;
            self.mid_enabled = v;
            self.rebuild_filters();
        } else if id.0 == "mid_freq" {
            let v = value.as_float().unwrap_or(pk(LC, "mid_freq").default_f32());
            if v.is_finite() {
                self.mid_freq = v;
                self.rebuild_filters();
            }
        } else if id.0 == "mid_gain" {
            let v = value.as_float().unwrap_or(pk(LC, "mid_gain").default_f32());
            if v.is_finite() {
                self.mid_gain = v;
                self.rebuild_filters();
            }
        } else if id.0 == "mid_q" {
            let v = value.as_float().unwrap_or(pk(LC, "mid_q").default_f32());
            if v.is_finite() {
                self.mid_q = v;
                self.rebuild_filters();
            }
        } else if id.0 == "auto_gain_enabled" {
            let v = value
                .as_bool()
                .ok_or_else(|| "auto_gain_enabled must be a boolean".to_string())?;
            self.auto_gain_enabled = v;
            if v && self.auto_gain.is_none() {
                self.auto_gain = Some(AutoGain::new(
                    self.num_channels,
                    self.sample_rate,
                    AutoGainParams {
                        enabled: true,
                        loudness_type: AutoGainLoudnessType::Momentary,
                        max_gain_db: self.auto_gain_max_db,
                        smoothing_ms: self.auto_gain_smoothing_ms,
                    },
                )?);
            } else if !v {
                self.auto_gain = None;
            }
        } else if id.0 == "auto_gain_max_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "auto_gain_max_db must be a float".to_string())?;
            if v.is_finite() {
                self.auto_gain_max_db = v;
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_max_gain_db(v);
                }
            }
        } else if id.0 == "auto_gain_smoothing_ms" {
            let v = value
                .as_float()
                .ok_or_else(|| "auto_gain_smoothing_ms must be a float".to_string())?;
            if v.is_finite() {
                self.auto_gain_smoothing_ms = v;
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_smoothing_ms(v);
                }
            }
        } else if id.0 == "auto_gain_position" {
            let s = value
                .as_string()
                .ok_or_else(|| "auto_gain_position must be a string".to_string())?;
            let pos = AutoGainPosition::from_str_lossy(s);
            self.auto_gain_position = pos;
            let want_enabled = pos != AutoGainPosition::Disabled;
            if want_enabled && self.auto_gain.is_none() {
                self.auto_gain = Some(AutoGain::new(
                    self.num_channels,
                    self.sample_rate,
                    AutoGainParams {
                        enabled: true,
                        loudness_type: AutoGainLoudnessType::Momentary,
                        max_gain_db: self.auto_gain_max_db,
                        smoothing_ms: self.auto_gain_smoothing_ms,
                    },
                )?);
            } else if !want_enabled {
                self.auto_gain = None;
            }
            self.auto_gain_enabled = want_enabled;
        } else if id.0 == "mode" {
            // Accept both int and float representations
            let v = match &value {
                ParameterValue::Int(i) => *i as usize,
                ParameterValue::Float(f) => *f as usize,
                _ => return Err(format!("mode must be numeric, got {:?}", value)),
            };
            if v <= 2 {
                self.mode_index = v;
                if v == 2 {
                    // Auto mode: force an initial rebuild
                    self.last_auto_volume_db = f32::MIN;
                    self.maybe_rebuild_auto_filters();
                }
                self.update_comp_gain_smoother();
            }
        } else if id.0 == "playback_volume_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "playback_volume_db must be a float".to_string())?;
            if v.is_finite() {
                self.playback_volume_db = v;
                self.maybe_rebuild_auto_filters();
            }
        } else if id.0 == "playback_level_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "playback_level_db must be a float".to_string())?;
            if v.is_finite() {
                self.playback_level_db = v;
                if self.mode_index == 2 {
                    // Auto mode: force rebuild with updated reference
                    self.last_auto_volume_db = f32::MIN;
                    self.maybe_rebuild_auto_filters();
                } else if self.mode_index == 1 {
                    // ISO 226 mode only — no-op in Manual mode (Bug #5 fix)
                    self.rebuild_iso_filters();
                }
            }
        } else if id.0 == "reference_level_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "reference_level_db must be a float".to_string())?;
            if v.is_finite() {
                self.reference_level_db = v;
                if self.mode_index == 2 {
                    // Auto mode: force rebuild with updated reference
                    self.last_auto_volume_db = f32::MIN;
                    self.maybe_rebuild_auto_filters();
                } else if self.mode_index == 1 {
                    // ISO 226 mode only — no-op in Manual mode (Bug #5 fix)
                    self.rebuild_iso_filters();
                }
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "low_gain" {
            Some(ParameterValue::Float(self.low_gain))
        } else if id.0 == "high_gain" {
            Some(ParameterValue::Float(self.high_gain))
        } else if id.0 == "low_freq" {
            Some(ParameterValue::Float(self.low_freq))
        } else if id.0 == "high_freq" {
            Some(ParameterValue::Float(self.high_freq))
        } else if id.0 == "mid_enabled" {
            Some(ParameterValue::Bool(self.mid_enabled))
        } else if id.0 == "mid_freq" {
            Some(ParameterValue::Float(self.mid_freq))
        } else if id.0 == "mid_gain" {
            Some(ParameterValue::Float(self.mid_gain))
        } else if id.0 == "mid_q" {
            Some(ParameterValue::Float(self.mid_q))
        } else if id.0 == "auto_gain_enabled" {
            Some(ParameterValue::Bool(self.auto_gain_enabled))
        } else if id.0 == "auto_gain_max_db" {
            Some(ParameterValue::Float(self.auto_gain_max_db))
        } else if id.0 == "auto_gain_smoothing_ms" {
            Some(ParameterValue::Float(self.auto_gain_smoothing_ms))
        } else if id.0 == "auto_gain_position" {
            Some(ParameterValue::String(self.auto_gain_position.to_string()))
        } else if id.0 == "mode" {
            Some(ParameterValue::Int(self.mode_index as i32))
        } else if id.0 == "playback_level_db" {
            Some(ParameterValue::Float(self.playback_level_db))
        } else if id.0 == "reference_level_db" {
            Some(ParameterValue::Float(self.reference_level_db))
        } else if id.0 == "playback_volume_db" {
            Some(ParameterValue::Float(self.playback_volume_db))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        for s in &mut self.comp_gain_smoother {
            s.set_time(20.0, sr);
        }
        self.rebuild_filters();
        self.rebuild_iso_filters();
        Ok(())
    }
    fn reset(&mut self) {
        // Clear filter delay state (x1/x2/y1/y2) for clean restart.
        // rebuild_filters() uses update_params() which preserves state — wrong for reset.
        let sr = self.sample_rate as f64;
        let q = 0.707;
        let lg = self.low_gain / 2.0;
        let hg = self.high_gain / 2.0;
        let mg = if self.mid_enabled { self.mid_gain } else { 0.0 };
        for ch in 0..self.num_channels {
            // Reset manual filters
            self.filters[ch].clear();
            self.filters[ch].push(Biquad::new(
                BiquadFilterType::Lowshelf,
                self.low_freq as f64,
                sr,
                q,
                lg as f64,
            ));
            self.filters[ch].push(Biquad::new(
                BiquadFilterType::Lowshelf,
                self.low_freq as f64,
                sr,
                q,
                lg as f64,
            ));
            self.filters[ch].push(Biquad::new(
                BiquadFilterType::Peak,
                self.mid_freq as f64,
                sr,
                self.mid_q as f64,
                mg as f64,
            ));
            self.filters[ch].push(Biquad::new(
                BiquadFilterType::Highshelf,
                self.high_freq as f64,
                sr,
                q,
                hg as f64,
            ));
            self.filters[ch].push(Biquad::new(
                BiquadFilterType::Highshelf,
                self.high_freq as f64,
                sr,
                q,
                hg as f64,
            ));

            // Reset ISO 226 filters
            self.iso_filters[ch].clear();
            for (band_idx, &freq) in ISO_BAND_FREQS.iter().enumerate() {
                let gain = interpolate_delta(&self.iso_deltas, freq);
                let bq = ISO_BAND_QS[band_idx];
                let filter_type = if band_idx == 0 {
                    BiquadFilterType::Lowshelf
                } else if band_idx == ISO_FILTER_COUNT - 1 {
                    BiquadFilterType::Highshelf
                } else {
                    BiquadFilterType::Peak
                };
                self.iso_filters[ch].push(Biquad::new(filter_type, freq, sr, bq, gain));
            }
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;

        // Auto mode only: rebuild filters if volume changed significantly (>0.5 dB).
        // Skip the call entirely in Manual/ISO modes to avoid per-block overhead (Bug #7 fix).
        if self.mode_index == 2 {
            self.maybe_rebuild_auto_filters();
        }

        // Measurement (input + output LUFS) and cache update happen every block
        // for fresh auto-gain data (Bug #2 fix: previously throttled to every
        // 10 blocks, causing up to ~107 ms of stale data at 512-sample / 48 kHz).
        let do_cache_update = true;

        match self.auto_gain_position {
            AutoGainPosition::Pre => {
                // Pre mode: measure input, apply gain compensation, then run filters.
                // Output measurement happens after compensation (correct level reported).
                if let Some(ag) = &mut self.auto_gain {
                    let _ = ag.measure_input(buffer);
                    // Apply compensation before filters
                    ag.apply_compensation(buffer, nf);
                    let _ = ag.measure_output(buffer);
                    if do_cache_update {
                        let data = ag.get_data();
                        self.cache.update(|d| {
                            *d = data;
                        });
                    }
                }

                // Process through filters
                for frame in 0..nf {
                    for ch in 0..self.num_channels {
                        let idx = frame * self.num_channels + ch;
                        buffer[idx] = self.process_sample(ch, buffer[idx]);
                    }
                }
            }
            AutoGainPosition::Post => {
                // Post mode (default): measure input, run EQ filters, apply
                // compensation, then measure output.
                // Measuring output AFTER apply_compensation ensures output_lufs
                // reflects the actual compensated signal level (Bug #3 fix).
                if let Some(ag) = &mut self.auto_gain {
                    let _ = ag.measure_input(buffer);
                }

                for frame in 0..nf {
                    for ch in 0..self.num_channels {
                        let idx = frame * self.num_channels + ch;
                        buffer[idx] = self.process_sample(ch, buffer[idx]);
                    }
                }

                if let Some(ag) = &mut self.auto_gain {
                    // Apply compensation first, then measure the actual output level.
                    ag.apply_compensation(buffer, nf);
                    let _ = ag.measure_output(buffer);
                    if do_cache_update {
                        let data = ag.get_data();
                        self.cache.update(|d| {
                            *d = data;
                        });
                    }
                }
            }
            AutoGainPosition::Disabled => {
                // No auto-gain, just filters
                for frame in 0..nf {
                    for ch in 0..self.num_channels {
                        let idx = frame * self.num_channels + ch;
                        buffer[idx] = self.process_sample(ch, buffer[idx]);
                    }
                }
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        if self.auto_gain.is_some() {
            Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_loudness_basic() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        let mut b = vec![0.5; 1000];
        p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
            .unwrap();
        assert!(b[999] > 0.0);
    }

    /// Regression: rebuild_filters() used to call Biquad::new() which resets
    /// filter delay state (x1/x2/y1/y2), causing a click artifact on every
    /// parameter change. Now it uses update_params() to preserve state.
    #[test]
    fn test_param_change_no_click() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        // Process a block to establish filter state
        let mut b = vec![0.3f32; 4800];
        let ctx = ProcessContext::new(48000, 4800);
        p.process_in_place(&mut b, &ctx).unwrap();
        let last_before = b[4799];

        // Change gain parameter — this should NOT reset filter state
        p.set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(7.0))
            .unwrap();

        // Process another block of the same signal
        let mut b2 = vec![0.3f32; 480];
        p.process_in_place(&mut b2, &ProcessContext::new(48000, 480))
            .unwrap();

        // The first sample after param change should be close to the last
        // sample before the change. A filter state reset would cause a
        // transient (click) where the output jumps to near-zero.
        let first_after = b2[0];
        let jump = (first_after - last_before).abs();
        assert!(
            jump < 0.2,
            "Parameter change caused discontinuity: last={last_before:.4}, first={first_after:.4}, \
             jump={jump:.4}. Filter state may have been reset."
        );
    }

    /// Verify 3-band topology: 2 lowshelf + 1 peak + 2 highshelf = 5 filters.
    /// When mid_enabled is toggled off, the peak filter gain becomes 0 dB (passthrough).
    #[test]
    fn test_three_band_topology_filter_count() {
        let p = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
        // Each channel should have exactly 5 filters
        assert_eq!(
            p.filters[0].len(),
            LoudnessCompensationPlugin::FILTER_COUNT,
            "Channel 0 should have {} filters",
            LoudnessCompensationPlugin::FILTER_COUNT
        );
        assert_eq!(
            p.filters[1].len(),
            LoudnessCompensationPlugin::FILTER_COUNT,
            "Channel 1 should have {} filters",
            LoudnessCompensationPlugin::FILTER_COUNT
        );
    }

    #[test]
    fn test_manual_cascaded_shelf_approximates_requested_passband_gain() {
        let mut p = LoudnessCompensationPlugin::new(1, 200.0, 12.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        let gain_db: f64 = p.filters[0]
            .iter()
            .map(|filter| filter.log_result(40.0))
            .sum();
        assert!(
            (8.0..=14.0).contains(&(gain_db as f32)),
            "two half-gain shelves should approximate the requested low passband gain; got {gain_db:.2} dB"
        );
    }

    #[test]
    fn test_mid_disabled_sets_peak_gain_zero() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        // Confirm mid is enabled by default
        assert!(p.mid_enabled);

        // Disable mid band
        p.set_parameter(
            ParameterId::from("mid_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
        assert!(!p.mid_enabled);

        // The peak filter (index 2) should have gain_db == 0.0.
        // We verify by processing: with mid disabled, a mid-frequency signal
        // should see the same behavior as if the peak band didn't exist.
        // Process two paths: one with mid_enabled=false, one with mid_gain=0.
        let nf = 4800;
        let ctx = ProcessContext::new(48000, nf);

        let signal: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 3500.0 * i as f32 / 48000.0).sin())
            .collect();

        // Path A: mid disabled
        let mut buf_a = signal.clone();
        p.process_in_place(&mut buf_a, &ctx).unwrap();

        // Path B: mid enabled but gain=0
        let mut p2 = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p2, 48000).unwrap();
        p2.set_parameter(ParameterId::from("mid_gain"), ParameterValue::Float(0.0))
            .unwrap();
        let mut buf_b = signal.clone();
        p2.process_in_place(&mut buf_b, &ctx).unwrap();

        // RMS of both should be very close
        let rms_a: f32 =
            (buf_a[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let rms_b: f32 =
            (buf_b[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let diff_db = 20.0 * (rms_a / rms_b).log10();
        assert!(
            diff_db.abs() < 0.5,
            "mid_enabled=false should behave like mid_gain=0, but RMS diff is {diff_db:.2} dB"
        );
    }

    /// Verify that the plugin actually applies gain when configured.
    /// With shelving filters active, a low-frequency signal should be processed
    /// differently than a mid-frequency signal (spectral shaping occurs).
    #[test]
    fn test_loudness_comp_applies_gain() {
        // Process a low-frequency signal (within the low shelf)
        let mut p_low = LoudnessCompensationPlugin::new(1, 100.0, 12.0, 10000.0, 12.0);
        InPlacePlugin::initialize(&mut p_low, 48000).unwrap();

        // Process a mid-frequency signal (outside both shelves)
        let mut p_mid = LoudnessCompensationPlugin::new(1, 100.0, 12.0, 10000.0, 12.0);
        InPlacePlugin::initialize(&mut p_mid, 48000).unwrap();

        let nf = 9600;
        let sr = 48000.0f32;

        let mut low_buf: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin())
            .collect();
        let mut mid_buf: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
            .collect();

        let ctx = ProcessContext::new(48000, nf);
        p_low.process_in_place(&mut low_buf, &ctx).unwrap();
        p_mid.process_in_place(&mut mid_buf, &ctx).unwrap();

        // Measure RMS in the settled second half
        let low_rms: f32 =
            (low_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let mid_rms: f32 =
            (mid_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();

        // The low-freq signal should be louder relative to mid-freq due to shelf boost
        assert!(
            low_rms > mid_rms * 1.3,
            "Loudness compensation should boost 50 Hz relative to 1 kHz, \
             but low RMS {low_rms:.4} is not significantly greater than mid RMS {mid_rms:.4}"
        );
    }

    // ==========================================================================
    // ISO 226 mode tests
    // ==========================================================================

    #[test]
    fn test_default_mode_is_manual() {
        let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        assert_eq!(p.mode_index, 0, "Default mode should be Manual (0)");
    }

    #[test]
    fn test_iso226_mode_has_seven_filters_per_channel() {
        let p = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
        for ch in 0..2 {
            assert_eq!(
                p.iso_filters[ch].len(),
                ISO_FILTER_COUNT,
                "Channel {ch} should have {ISO_FILTER_COUNT} ISO filters"
            );
        }
    }

    #[test]
    fn test_iso226_mode_equal_levels_passthrough() {
        // When playback_level == reference_level, ISO 226 mode should be near-passthrough
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
            .unwrap();
        p.set_parameter(
            ParameterId::from("playback_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();

        let nf = 4800;
        let ctx = ProcessContext::new(48000, nf);
        let signal: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect();

        let mut buf = signal.clone();
        p.process_in_place(&mut buf, &ctx).unwrap();

        // Measure RMS in the settled half
        let input_rms: f32 =
            (signal[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let output_rms: f32 =
            (buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let diff_db = 20.0 * (output_rms / input_rms).log10();
        assert!(
            diff_db.abs() < 1.0,
            "Equal playback and reference levels should be near-passthrough, got {diff_db:.2} dB difference"
        );
    }

    #[test]
    fn test_iso226_mode_low_volume_boosts_bass() {
        // At lower playback level, bass should be boosted relative to mid
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
            .unwrap();
        p.set_parameter(
            ParameterId::from("playback_level_db"),
            ParameterValue::Float(60.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();

        let nf = 9600;
        let sr = 48000.0f32;
        let ctx = ProcessContext::new(48000, nf);

        // Process a 50 Hz signal
        let mut low_buf: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin())
            .collect();
        p.process_in_place(&mut low_buf, &ctx).unwrap();

        // Process a 1 kHz signal with a fresh plugin at same settings
        let mut p2 = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p2, 48000).unwrap();
        p2.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
            .unwrap();
        p2.set_parameter(
            ParameterId::from("playback_level_db"),
            ParameterValue::Float(60.0),
        )
        .unwrap();
        p2.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();

        let mut mid_buf: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
            .collect();
        p2.process_in_place(&mut mid_buf, &ctx).unwrap();

        let low_rms: f32 =
            (low_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let mid_rms: f32 =
            (mid_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();

        assert!(
            low_rms > mid_rms * 1.2,
            "ISO 226 at low volume should boost bass: low RMS={low_rms:.4} should be > mid RMS={mid_rms:.4} * 1.2"
        );
    }

    #[test]
    fn test_mode_switch_via_set_parameter() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        assert_eq!(p.mode_index, 0);

        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
            .unwrap();
        assert_eq!(p.mode_index, 1);

        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(0))
            .unwrap();
        assert_eq!(p.mode_index, 0);
    }

    #[test]
    fn test_get_parameter_new_fields() {
        let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        assert_eq!(
            p.get_parameter(&ParameterId::from("mode")),
            Some(ParameterValue::Int(0))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("playback_level_db")),
            Some(ParameterValue::Float(70.0))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("reference_level_db")),
            Some(ParameterValue::Float(83.0))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("playback_volume_db")),
            Some(ParameterValue::Float(0.0))
        );
    }

    // ==========================================================================
    // Auto mode tests
    // ==========================================================================

    #[test]
    fn test_auto_mode_applies_compensation() {
        // Auto mode with volume=-20 should produce bass boost
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
            .unwrap();
        p.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("playback_volume_db"),
            ParameterValue::Float(-20.0),
        )
        .unwrap();

        let nf = 9600;
        let sr = 48000.0f32;
        let ctx = ProcessContext::new(48000, nf);

        // Process a 50 Hz signal
        let mut low_buf: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin())
            .collect();
        p.process_in_place(&mut low_buf, &ctx).unwrap();

        // Process a 1 kHz signal with a fresh plugin at same settings
        let mut p2 = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p2, 48000).unwrap();
        p2.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
            .unwrap();
        p2.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();
        p2.set_parameter(
            ParameterId::from("playback_volume_db"),
            ParameterValue::Float(-20.0),
        )
        .unwrap();

        let mut mid_buf: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
            .collect();
        p2.process_in_place(&mut mid_buf, &ctx).unwrap();

        let low_rms: f32 =
            (low_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let mid_rms: f32 =
            (mid_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();

        assert!(
            low_rms > mid_rms * 1.2,
            "Auto mode at -20dB volume should boost bass: low RMS={low_rms:.4} should be > mid RMS={mid_rms:.4} * 1.2"
        );
    }

    #[test]
    fn test_auto_mode_zero_volume_flat_response() {
        // Auto mode with volume=0 and reference=83 means estimated_spl = 83 = reference
        // => no compensation (flat response)
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
            .unwrap();
        p.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("playback_volume_db"),
            ParameterValue::Float(0.0),
        )
        .unwrap();

        let nf = 4800;
        let ctx = ProcessContext::new(48000, nf);
        let signal: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect();

        let mut buf = signal.clone();
        p.process_in_place(&mut buf, &ctx).unwrap();

        let input_rms: f32 =
            (signal[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let output_rms: f32 =
            (buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let diff_db = 20.0 * (output_rms / input_rms).log10();
        assert!(
            diff_db.abs() < 1.0,
            "Auto mode at 0dB volume should be near-passthrough, got {diff_db:.2} dB difference"
        );
    }

    #[test]
    fn test_auto_mode_switch_via_set_parameter() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        assert_eq!(p.mode_index, 0);

        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
            .unwrap();
        assert_eq!(p.mode_index, 2);

        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(0))
            .unwrap();
        assert_eq!(p.mode_index, 0);
    }

    // ==========================================================================
    // Bug fix tests
    // ==========================================================================

    /// Bug #1: comp gain must account for inter-band constructive interference.
    ///
    /// When all 7 ISO bands have large gains (e.g. 10 dB each), the combined
    /// frequency response can produce ripples above the maximum band-centre gain.
    /// The comp smoother target must attenuate enough so the combined peak never
    /// exceeds 0 dBFS when the input is at 0 dBFS.
    ///
    /// Specifically, comp_gain_target = 10^(-max_combined_db / 20).  If we only
    /// sample at the 7 band centres, we miss the ripple peak, and the smoother
    /// target is set too high (not enough attenuation), allowing output > 0 dBFS.
    #[test]
    fn test_comp_gain_does_not_allow_clipping_in_iso_mode() {
        // Use a large bass boost scenario: playback=40, reference=83 -> big delta at bass
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
            .unwrap();
        p.set_parameter(
            ParameterId::from("playback_level_db"),
            ParameterValue::Float(40.0), // extreme low: large ISO delta
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();

        // Warm-up pass: let the smoother settle from its initial value (1.0) to the
        // compensated target over several blocks.  At 20 ms time constant, 10 time
        // constants (200 ms = 9600 samples) is enough for >99.99% convergence.
        let nf = 9600;
        let ctx = ProcessContext::new(48000, nf);
        let warmup: Vec<f32> = (0..nf)
            .map(|i| (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
            .collect();
        let mut warm_buf = warmup.clone();
        p.process_in_place(&mut warm_buf, &ctx).unwrap();

        // Second pass with smoother fully settled: peak must not exceed 0 dBFS
        let mut buf: Vec<f32> = (0..nf)
            .map(|i| (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
            .collect();
        p.process_in_place(&mut buf, &ctx).unwrap();

        // With proper comp gain the peak must not exceed 1.0 (0 dBFS) once settled
        let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            peak <= 1.0 + 1e-3,
            "comp_gain under-attenuated: peak = {peak:.4} > 1.0 (clipping) after smoother settled"
        );
    }

    /// Bug #2: auto-gain measurement must happen every block, not every 10.
    ///
    /// With the old bug, `do_measure` was only true every 10 blocks.  All auto-gain
    /// measurement and cache writes were skipped otherwise.  So for 9 blocks after
    /// each measurement cycle, the cache held stale data.
    ///
    /// The fix makes measurement happen every block.  To observe a measurable difference
    /// we compare what happens after 9 blocks of silence followed by 1 block of loud
    /// signal vs 10 blocks of loud signal.  With the old code the 10th block overwrites;
    /// with the fix every block updates.
    ///
    /// Practical test: process enough audio for EBU R128 momentary measurement to
    /// accumulate (≥400 ms = ~19200 samples), then verify that `input_lufs` reflects
    /// the actual signal level — not the plugin default (-120.0).
    #[test]
    fn test_auto_gain_measurement_not_stale_after_one_block() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        p.set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(true),
        )
        .unwrap();

        // Process 9 small blocks (9 * 512 = 4608 samples) of silence.
        // With the old bug, the cache update counter fires on the 10th block only.
        let nf = 512;
        let ctx = ProcessContext::new(48000, nf);
        for _ in 0..9 {
            let mut buf = vec![0.0_f32; nf];
            p.process_in_place(&mut buf, &ctx).unwrap();
        }

        // Now feed loud audio for enough blocks to fill the EBU R128 400ms window
        // (~19200 samples = 38 blocks of 512).  With the fix, measurement and cache
        // update happen on every block, so after these blocks input_lufs is live.
        let loud_nf = 19200;
        let loud_ctx = ProcessContext::new(48000, loud_nf);
        let mut loud_buf: Vec<f32> = (0..loud_nf)
            .map(|i| if i % 2 == 0 { 0.5_f32 } else { -0.5_f32 })
            .collect();
        p.process_in_place(&mut loud_buf, &loud_ctx).unwrap();

        let data_arc = p.get_data().expect("auto_gain should produce data");
        let ag_data = data_arc
            .downcast_ref::<sotf_host::auto_gain::AutoGainData>()
            .expect("data should be AutoGainData");
        // After 400ms of loud audio the EBU momentary measurement must be well above
        // the plugin default of -120.0 dB.  With the old throttled code the cache
        // holds the measurement from block 10 (still all silence), so input_lufs
        // would remain near -inf / -120.
        assert!(
            ag_data.input_lufs > -40.0,
            "input_lufs should reflect loud signal after 400ms, got {:.2} dB (still default/stale?)",
            ag_data.input_lufs
        );
    }

    /// Bug #3: in Post mode, output measurement must see post-compensation level.
    ///
    /// The AutoGain feedback loop sets its next gain target via:
    ///   `target = input_lufs - output_lufs`
    ///
    /// If `ag.measure_output` is called BEFORE `ag.apply_compensation` (the bug),
    /// `output_lufs` reflects the signal BEFORE the AutoGain's own gain is applied.
    /// When the AutoGain is boosting (gain_linear > 1.0), the measurement will be
    /// lower than the actual output, causing the feedback loop to increase gain further.
    /// This positive feedback drives gain to `max_gain_db` → audible pumping.
    ///
    /// The fix applies `ag.apply_compensation` first, then calls `ag.measure_output`.
    ///
    /// This test verifies:
    ///   (a) Both `input_lufs` and `output_lufs` are finite after sufficient audio.
    ///   (b) The difference is bounded by the AutoGain's max_gain_db range.
    ///
    /// Full regression of the feedback instability requires fine-grained control over
    /// the EBU R128 internal state, which is out of scope here.  The code fix is
    /// verified by code review (apply then measure).
    #[test]
    fn test_post_mode_output_measurement_after_compensation() {
        let params = crate::LoudnessCompensationPluginParams {
            auto_gain_enabled: true,
            auto_gain_position: "post".to_string(),
            auto_gain_max_db: 12.0,
            auto_gain_smoothing_ms: 5.0,
            ..Default::default()
        };
        let mut p = LoudnessCompensationPlugin::from_params(1, params).unwrap();
        InPlacePlugin::initialize(&mut p, 48000).unwrap();

        let nf = 4800; // 100ms per block
        let ctx = ProcessContext::new(48000, nf);
        let signal: Vec<f32> = (0..nf)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect();

        // 10 blocks = 1 second; enough for EBU R128 momentary window to fill
        for _ in 0..10 {
            let mut buf = signal.clone();
            p.process_in_place(&mut buf, &ctx).unwrap();
        }

        let data_arc = p.get_data().expect("auto_gain should produce data");
        let ag_data = data_arc
            .downcast_ref::<sotf_host::auto_gain::AutoGainData>()
            .expect("data should be AutoGainData");

        // Both measurements must be finite after 1 second
        assert!(
            ag_data.input_lufs.is_finite(),
            "input_lufs must be finite after 1s, got {}",
            ag_data.input_lufs
        );
        assert!(
            ag_data.output_lufs.is_finite(),
            "output_lufs must be finite after 1s, got {}",
            ag_data.output_lufs
        );

        // In steady state, |output_lufs - input_lufs| must be within AutoGain's range.
        // This bound would be violated if the feedback loop ran away due to the
        // wrong measurement order.
        let diff = (ag_data.output_lufs - ag_data.input_lufs).abs();
        let max_gain_db = 12.0_f64;
        assert!(
            diff <= max_gain_db + 1.0,
            "output_lufs ({:.2}) and input_lufs ({:.2}) should be within {:.1} dB \
             (Post mode, Bug #3 fix: measure AFTER compensation); diff = {:.2}",
            ag_data.output_lufs,
            ag_data.input_lufs,
            max_gain_db + 1.0,
            diff
        );
    }

    /// Bug #5 + #7: manual mode should not rebuild ISO filters or call
    /// `maybe_rebuild_auto_filters` on every block.
    ///
    /// Indirect verification: setting `playback_level_db` in manual mode (mode=0)
    /// must not panic or corrupt internal state.  If it incorrectly rebuilt ISO
    /// filters AND mode were 0, the iso_filters would be recomputed — harmless but
    /// indicates the guard is absent.  We test stability by processing after the
    /// parameter change.
    #[test]
    fn test_manual_mode_level_change_does_not_corrupt() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        assert_eq!(p.mode_index, 0);

        // Change ISO-related params in manual mode — must be a no-op for filter bank
        p.set_parameter(
            ParameterId::from("playback_level_db"),
            ParameterValue::Float(60.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("reference_level_db"),
            ParameterValue::Float(83.0),
        )
        .unwrap();

        // Process must succeed and produce finite output
        let nf = 480;
        let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 / 48.0).sin()).collect();
        p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
            .unwrap();
        assert!(
            buf.iter().all(|s| s.is_finite()),
            "output should be finite after parameter change in manual mode"
        );
    }
}
