use super::auto_gain_position::AutoGainPosition;
use super::consts::ISO_BAND_FREQS;
use super::consts::ISO_BAND_QS;
use super::consts::ISO_FILTER_COUNT;
use super::default::default_mid_enabled;
use super::default::default_mid_freq;
use super::default::default_mid_gain;
use super::default::default_mid_q;
use super::default::default_playback_level_db;
use super::default::default_reference_level_db;
use super::iso_fit::{band_type, fit_iso_gains, safe_frequency};
use super::iso226::{ISO226_NUM_FREQS, compute_iso226_delta};
use super::types::LoudnessCompensationPluginParams;
use crate::params::PARAMS as LC;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
use sotf_host::param_bridge::apply_spec_update_modes;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::flush_denormals_inplace;
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

const FILTER_CROSSFADE_SAMPLES: usize = 256;

struct FilterTransition {
    old_filters: Vec<Vec<Biquad>>,
    old_mode: usize,
    remaining: usize,
}

impl FilterTransition {
    fn new(num_channels: usize) -> Self {
        Self {
            old_filters: (0..num_channels)
                .map(|_| Vec::with_capacity(ISO_FILTER_COUNT))
                .collect(),
            old_mode: 0,
            remaining: 0,
        }
    }
}

pub struct LoudnessCompensationPlugin {
    pub(super) num_channels: usize,
    pub(super) sample_rate: u32,
    // -- Manual mode fields --
    pub(super) low_freq: f32,
    pub(super) low_gain: f32,
    pub(super) high_freq: f32,
    pub(super) high_gain: f32,
    pub(super) mid_enabled: bool,
    pub(super) mid_freq: f32,
    pub(super) mid_gain: f32,
    pub(super) mid_q: f32,
    /// Manual mode filters: [channel][filter_index], 5 biquads per channel.
    pub(super) filters: Vec<Vec<Biquad>>,
    // -- ISO 226 / Auto mode fields --
    /// 0 = Manual, 1 = ISO 226, 2 = Auto
    pub(super) mode_index: usize,
    pub(super) playback_level_db: f32,
    pub(super) reference_level_db: f32,
    /// Engine playback volume in dB (relative, set externally). Used in Auto mode.
    pub(super) playback_volume_db: f32,
    /// Last volume at which ISO filters were rebuilt (Auto mode). Prevents
    /// per-frame rebuilds; filters are only rebuilt when volume changes by >0.5 dB.
    pub(super) last_auto_volume_db: f32,
    /// ISO 226 mode filters: [channel][band_index], jointly fitted biquad bank.
    /// Pre-allocated in `new()`, coefficients updated in `rebuild_iso_filters()`.
    pub(super) iso_filters: Vec<Vec<Biquad>>,
    /// Cached ISO 226 delta curve for the current playback/reference levels.
    pub(super) iso_deltas: [(f64, f64); ISO226_NUM_FREQS],
    // -- Common fields --
    pub(super) auto_gain: Option<AutoGain>,
    pub(super) auto_gain_max_db: f32,
    pub(super) auto_gain_smoothing_ms: f32,
    pub(super) auto_gain_position: AutoGainPosition,
    pub(super) headroom_normalized: bool,
    pub(super) auto_calibrated: bool,
    pub(super) comp_gain_smoother: Vec<Smoother>,
    transition: FilterTransition,
    pub(super) cache: RealTimeCache<AutoGainData>,
    pub(super) cached_parameters: Vec<Parameter>,
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
            auto_gain_max_db: pk(LC, "auto_gain_max_db").default_f32(),
            auto_gain_smoothing_ms: pk(LC, "auto_gain_smoothing_ms").default_f32(),
            auto_gain_position: AutoGainPosition::Disabled,
            headroom_normalized: false,
            auto_calibrated: false,
            comp_gain_smoother: (0..num_channels)
                .map(|_| Smoother::new(1.0, 20.0, sr))
                .collect(),
            transition: FilterTransition::new(num_channels),
            cache: RealTimeCache::new(AutoGainData::default()),
            cached_parameters: Vec::new(),
        };
        p.rebuild_filters();
        p.rebuild_iso_filters();
        p.rebuild_cached_parameters();
        p
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
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
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", self.auto_gain_enabled())
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
            Parameter::new_int(
                "auto_gain_position",
                "AG Position",
                match self.auto_gain_position {
                    AutoGainPosition::Disabled => 0,
                    AutoGainPosition::Pre => 1,
                    AutoGainPosition::Post => 2,
                },
                0,
                2,
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
            Parameter::new_bool(
                "headroom_normalized",
                "Headroom Normalized",
                self.headroom_normalized,
            )
            .with_description("Attenuate by the realized positive cascade peak"),
            Parameter::new_bool("auto_calibrated", "SPL Calibrated", self.auto_calibrated)
                .with_description("Reference level is a measured SPL at volume 0 dB"),
        ];
        apply_spec_update_modes(&mut self.cached_parameters, LC);
    }

    /// Expected filter count per channel for manual mode: 2x lowshelf + 1x mid peak + 2x highshelf = 5
    pub(super) const FILTER_COUNT: usize = 5;

    pub(super) fn auto_gain_enabled(&self) -> bool {
        self.auto_gain_position != AutoGainPosition::Disabled
    }

    fn checked_float(key: &str, value: &ParameterValue) -> PluginResult<f32> {
        let value = value
            .as_float()
            .ok_or_else(|| format!("{key} must be a float"))?;
        let spec = pk(LC, key);
        let min = spec.min_f64() as f32;
        let max = spec.max_f64() as f32;
        if !value.is_finite() || !(min..=max).contains(&value) {
            return Err(format!(
                "{key} must be finite and in [{min}, {max}], got {value}"
            ));
        }
        Ok(value)
    }

    fn set_auto_gain_position(&mut self, position: AutoGainPosition) -> PluginResult<()> {
        self.auto_gain_position = position;
        if position == AutoGainPosition::Disabled {
            self.auto_gain = None;
        } else if self.auto_gain.is_none() {
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
        }
        Ok(())
    }

    fn begin_transition(&mut self, old_mode: usize) {
        if self.transition.remaining != 0 {
            return;
        }
        let source = if old_mode == 0 {
            &self.filters
        } else {
            &self.iso_filters
        };
        if source.first().is_none_or(Vec::is_empty) {
            return;
        }
        for (destination, source) in self.transition.old_filters.iter_mut().zip(source) {
            destination.clear();
            destination.extend(source.iter().cloned());
        }
        self.transition.old_mode = old_mode;
        self.transition.remaining = FILTER_CROSSFADE_SAMPLES;
    }

    /// Rebuild the manual-mode 3-band filters (5 biquads per channel).
    pub(super) fn rebuild_filters(&mut self) {
        if self.mode_index == 0 {
            self.begin_transition(0);
        }
        let q = 0.707;
        let sr = self.sample_rate as f64;
        let low_freq = safe_frequency(self.low_freq as f64, sr);
        let mid_freq = safe_frequency(self.mid_freq as f64, sr);
        let high_freq = safe_frequency(self.high_freq as f64, sr);
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
                    low_freq,
                    sr,
                    q,
                    lg as f64,
                );
                self.filters[ch][1].update_params(
                    BiquadFilterType::Lowshelf,
                    low_freq,
                    sr,
                    q,
                    lg as f64,
                );
                self.filters[ch][2].update_params(
                    BiquadFilterType::Peak,
                    mid_freq,
                    sr,
                    self.mid_q as f64,
                    mg as f64,
                );
                self.filters[ch][3].update_params(
                    BiquadFilterType::Highshelf,
                    high_freq,
                    sr,
                    q,
                    hg as f64,
                );
                self.filters[ch][4].update_params(
                    BiquadFilterType::Highshelf,
                    high_freq,
                    sr,
                    q,
                    hg as f64,
                );
            } else {
                // First initialization — create filters from scratch
                self.filters[ch] = vec![
                    Biquad::new(BiquadFilterType::Lowshelf, low_freq, sr, q, lg as f64),
                    Biquad::new(BiquadFilterType::Lowshelf, low_freq, sr, q, lg as f64),
                    Biquad::new(
                        BiquadFilterType::Peak,
                        mid_freq,
                        sr,
                        self.mid_q as f64,
                        mg as f64,
                    ),
                    Biquad::new(BiquadFilterType::Highshelf, high_freq, sr, q, hg as f64),
                    Biquad::new(BiquadFilterType::Highshelf, high_freq, sr, q, hg as f64),
                ];
            }
        }
        self.update_comp_gain_smoother();
    }

    /// Rebuild ISO 226 filters based on current playback/reference levels.
    ///
    /// Fits 7 parametric EQ bands to the ISO 226 delta contour.
    /// Called at parameter-change time only, never in the hot path.
    pub(super) fn rebuild_iso_filters(&mut self) {
        if self.mode_index != 0 {
            self.begin_transition(self.mode_index);
        }
        let sr = self.sample_rate as f64;
        let playback_phon = (self.playback_level_db as f64).clamp(20.0, 90.0);
        let reference_phon = (self.reference_level_db as f64).clamp(20.0, 90.0);
        self.iso_deltas = compute_iso226_delta(playback_phon, reference_phon);
        let fitted_gains = fit_iso_gains(&self.iso_deltas, sr);

        for ch in 0..self.num_channels {
            if self.iso_filters[ch].len() == ISO_FILTER_COUNT {
                // Update in place — preserves filter delay state for click-free transitions
                for (band_idx, &freq) in ISO_BAND_FREQS.iter().enumerate() {
                    let gain = fitted_gains[band_idx];
                    let q = ISO_BAND_QS[band_idx];
                    self.iso_filters[ch][band_idx].update_params(
                        band_type(band_idx),
                        safe_frequency(freq, sr),
                        sr,
                        q,
                        gain,
                    );
                }
            } else {
                // First initialization — create from scratch
                self.iso_filters[ch] = Vec::with_capacity(ISO_FILTER_COUNT);
                for (band_idx, &freq) in ISO_BAND_FREQS.iter().enumerate() {
                    let gain = fitted_gains[band_idx];
                    let q = ISO_BAND_QS[band_idx];
                    self.iso_filters[ch].push(Biquad::new(
                        band_type(band_idx),
                        safe_frequency(freq, sr),
                        sr,
                        q,
                        gain,
                    ));
                }
            }
        }
        self.update_comp_gain_smoother();
    }

    /// Number of log-spaced frequency points used to evaluate the combined
    /// ISO filter chain peak gain for comp-gain calculation (Bug #1 fix).
    pub(super) const COMP_GAIN_GRID_POINTS: usize = 128;

    /// Update the compensation gain smoother targets based on the active mode.
    ///
    /// For ISO 226 / Auto modes, the combined response of 7 parametric EQ bands
    /// is evaluated on a 128-point log-spaced grid (20 Hz – 20 kHz) to capture
    /// constructive interference (ripple peaks) that occur between band centres.
    /// Evaluating only at the 7 band-centre frequencies can underestimate the
    /// true peak by several dB, causing under-attenuation and potential clipping.
    pub(super) fn update_comp_gain_smoother(&mut self) {
        let active = if self.mode_index == 0 {
            self.filters.first()
        } else {
            self.iso_filters.first()
        };
        let max_gain = if !self.headroom_normalized {
            0.0
        } else if let Some(filters) = active.filter(|filters| !filters.is_empty()) {
            let f_lo = 20.0_f64.min(self.sample_rate as f64 * 0.1);
            let f_hi = self.sample_rate as f64 * 0.499;
            let log_lo = f_lo.ln();
            let log_hi = f_hi.ln();
            let mut peak_db = 0.0_f64;
            for k in 0..Self::COMP_GAIN_GRID_POINTS {
                let t = k as f64 / (Self::COMP_GAIN_GRID_POINTS - 1) as f64;
                let freq = (log_lo + t * (log_hi - log_lo)).exp();
                let combined_db: f64 = filters.iter().map(|f| f.log_result(freq)).sum();
                peak_db = peak_db.max(combined_db.max(0.0));
            }
            peak_db as f32
        } else {
            0.0
        };
        for ch in 0..self.num_channels {
            let target = 10.0_f32.powf(-max_gain / 20.0);
            self.comp_gain_smoother[ch].set_target(target);
        }
    }

    pub fn from_params(
        num_channels: usize,
        params: LoudnessCompensationPluginParams,
    ) -> Result<Self, String> {
        if num_channels == 0 || num_channels > 32 {
            return Err(format!(
                "Loudness Compensation channels must be in 1..=32, got {num_channels}"
            ));
        }
        if !params.channel_params.is_empty() {
            return Err("Loudness Compensation per-channel curves are unsupported; channel_params must be empty".into());
        }
        let values = [
            ("low_freq", params.low_freq, 20.0, 500.0),
            ("low_gain", params.low_gain, -20.0, 20.0),
            ("high_freq", params.high_freq, 2000.0, 20000.0),
            ("high_gain", params.high_gain, -20.0, 20.0),
            ("mid_freq", params.mid_freq, 500.0, 8000.0),
            ("mid_gain", params.mid_gain, -20.0, 20.0),
            ("mid_q", params.mid_q, 0.1, 5.0),
            ("auto_gain_max_db", params.auto_gain_max_db, 0.0, 24.0),
            (
                "auto_gain_smoothing_ms",
                params.auto_gain_smoothing_ms,
                1.0,
                1000.0,
            ),
            ("playback_level_db", params.playback_level_db, 40.0, 90.0),
            ("reference_level_db", params.reference_level_db, 60.0, 100.0),
            ("playback_volume_db", params.playback_volume_db, -80.0, 0.0),
        ];
        for (name, value, min, max) in values {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "{name} must be finite and in [{min}, {max}], got {value}"
                ));
            }
        }
        if params.mode > 2 {
            return Err(format!("mode must be 0, 1, or 2, got {}", params.mode));
        }
        if params.mode == 2 && !params.auto_calibrated {
            return Err("Auto mode requires an explicit measured SPL calibration".into());
        }
        let requested_position = AutoGainPosition::parse(&params.auto_gain_position)?;
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
        p.headroom_normalized = params.headroom_normalized;
        p.auto_calibrated = params.auto_calibrated;
        p.last_auto_volume_db = params.playback_volume_db;
        // `auto_gain_enabled` is retained only as a serialized compatibility
        // input. Runtime state has one source of truth: the typed position.
        p.auto_gain_position = if params.auto_gain_enabled {
            if requested_position == AutoGainPosition::Disabled {
                AutoGainPosition::Post
            } else {
                requested_position
            }
        } else {
            AutoGainPosition::Disabled
        };
        p.auto_gain_max_db = params.auto_gain_max_db;
        p.auto_gain_smoothing_ms = params.auto_gain_smoothing_ms;
        if p.auto_gain_enabled() {
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
        if p.mode_index == 2 {
            p.last_auto_volume_db = f32::MIN;
            p.maybe_rebuild_auto_filters();
        }
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Process a single frame through the active filter bank.
    /// Returns the processed sample value.
    #[inline(always)]
    pub(super) fn process_sample(&mut self, ch: usize, sample: f32) -> f32 {
        let mut current = sample as f64;
        if self.mode_index == 1 || self.mode_index == 2 {
            for f in &mut self.iso_filters[ch] {
                current = f.process(current);
            }
        } else {
            for f in &mut self.filters[ch] {
                current = f.process(current);
            }
        }
        let output = if self.transition.remaining != 0 {
            let mut old = sample as f64;
            for filter in &mut self.transition.old_filters[ch] {
                old = filter.process(old);
            }
            let alpha = 1.0 - self.transition.remaining as f64 / FILTER_CROSSFADE_SAMPLES as f64;
            old + (current - old) * alpha
        } else {
            current
        };
        if ch + 1 == self.num_channels && self.transition.remaining != 0 {
            self.transition.remaining -= 1;
        }
        (output as f32) * self.comp_gain_smoother[ch].advance()
    }

    /// In Auto mode, rebuild ISO 226 filters based on engine volume.
    /// Converts relative `playback_volume_db` to absolute SPL estimate:
    ///   estimated_spl = reference_level_db + playback_volume_db
    /// This is called only from construction/control updates. `process_in_place`
    /// consumes the prepared bank and never designs coefficients.
    pub(super) fn maybe_rebuild_auto_filters(&mut self) {
        if self.mode_index != 2 {
            return;
        }
        let delta = (self.playback_volume_db - self.last_auto_volume_db).abs();
        if delta <= f32::EPSILON {
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

impl ParametricInPlacePlugin for LoudnessCompensationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Compensation", env!("CARGO_PKG_VERSION"), "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Iir
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        if self.auto_gain_enabled() {
            return PluginCompileMetadata::boundary(PluginCostClass::Iir, 0);
        }
        PluginCompileMetadata::linear_transform(PluginCostClass::Iir, None, 0, false, true, true)
    }

    fn channels(&self) -> usize {
        self.num_channels
    }
    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }
    fn current_values(&self) -> ParameterSet {
        self.cached_parameters
            .iter()
            .map(|p| (p.id.clone(), self.parametric_get_parameter(&p.id).unwrap()))
            .collect()
    }
    fn parametric_get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        Some(match id.as_str() {
            "low_gain" => ParameterValue::Float(self.low_gain),
            "high_gain" => ParameterValue::Float(self.high_gain),
            "low_freq" => ParameterValue::Float(self.low_freq),
            "high_freq" => ParameterValue::Float(self.high_freq),
            "mid_enabled" => ParameterValue::Bool(self.mid_enabled),
            "mid_freq" => ParameterValue::Float(self.mid_freq),
            "mid_gain" => ParameterValue::Float(self.mid_gain),
            "mid_q" => ParameterValue::Float(self.mid_q),
            "auto_gain_enabled" => ParameterValue::Bool(self.auto_gain_enabled()),
            "auto_gain_max_db" => ParameterValue::Float(self.auto_gain_max_db),
            "auto_gain_smoothing_ms" => ParameterValue::Float(self.auto_gain_smoothing_ms),
            "auto_gain_position" => ParameterValue::Int(match self.auto_gain_position {
                AutoGainPosition::Disabled => 0,
                AutoGainPosition::Pre => 1,
                AutoGainPosition::Post => 2,
            }),
            "mode" => ParameterValue::Int(self.mode_index as i32),
            "playback_level_db" => ParameterValue::Float(self.playback_level_db),
            "reference_level_db" => ParameterValue::Float(self.reference_level_db),
            "playback_volume_db" => ParameterValue::Float(self.playback_volume_db),
            "headroom_normalized" => ParameterValue::Bool(self.headroom_normalized),
            "auto_calibrated" => ParameterValue::Bool(self.auto_calibrated),
            _ => return None,
        })
    }
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        let mut rebuild_manual = false;
        let mut rebuild_iso = false;
        let mut rebuild_auto = false;
        let mut update_headroom = false;
        for (id, value) in values {
            let key = id.as_str();
            if matches!(
                key,
                "low_gain"
                    | "high_gain"
                    | "low_freq"
                    | "high_freq"
                    | "mid_freq"
                    | "mid_gain"
                    | "mid_q"
            ) {
                let v = Self::checked_float(key, &value)?;
                match key {
                    "low_gain" => self.low_gain = v,
                    "high_gain" => self.high_gain = v,
                    "low_freq" => self.low_freq = v,
                    "high_freq" => self.high_freq = v,
                    "mid_freq" => self.mid_freq = v,
                    "mid_gain" => self.mid_gain = v,
                    "mid_q" => self.mid_q = v,
                    _ => unreachable!(),
                }
                rebuild_manual = true;
            } else if key == "mid_enabled" {
                self.mid_enabled = value
                    .as_bool()
                    .ok_or_else(|| "mid_enabled must be a boolean".to_string())?;
                rebuild_manual = true;
            } else if key == "auto_gain_enabled" {
                let v = value
                    .as_bool()
                    .ok_or_else(|| "auto_gain_enabled must be a boolean".to_string())?;
                let position = if v {
                    if self.auto_gain_position == AutoGainPosition::Disabled {
                        AutoGainPosition::Post
                    } else {
                        self.auto_gain_position
                    }
                } else {
                    AutoGainPosition::Disabled
                };
                self.set_auto_gain_position(position)?;
            } else if key == "auto_gain_max_db" {
                let v = Self::checked_float(key, &value)?;
                self.auto_gain_max_db = v;
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_max_gain_db(v);
                }
            } else if key == "auto_gain_smoothing_ms" {
                let v = Self::checked_float(key, &value)?;
                self.auto_gain_smoothing_ms = v;
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_smoothing_ms(v);
                }
            } else if key == "auto_gain_position" {
                let position = match &value {
                    ParameterValue::String(s) => AutoGainPosition::parse(s)?,
                    ParameterValue::Int(0) => AutoGainPosition::Disabled,
                    ParameterValue::Int(1) => AutoGainPosition::Pre,
                    ParameterValue::Int(2) => AutoGainPosition::Post,
                    _ => {
                        return Err(
                            "auto_gain_position must be disabled/pre/post or choice 0/1/2".into(),
                        );
                    }
                };
                self.set_auto_gain_position(position)?;
            } else if key == "mode" {
                let v = match value {
                    ParameterValue::Int(i) if (0..=2).contains(&i) => i as usize,
                    ParameterValue::Float(f)
                        if f.is_finite() && f.fract() == 0.0 && (0.0..=2.0).contains(&f) =>
                    {
                        f as usize
                    }
                    _ => return Err(format!("mode must be numeric, got {:?}", value)),
                };
                if v == 2 && !self.auto_calibrated {
                    return Err("Auto mode requires an explicit measured SPL calibration".into());
                }
                if v != self.mode_index {
                    self.begin_transition(self.mode_index);
                    self.mode_index = v;
                    rebuild_auto |= v == 2;
                    update_headroom = true;
                }
            } else if key == "playback_volume_db" {
                self.playback_volume_db = Self::checked_float(key, &value)?;
                rebuild_auto = self.mode_index == 2;
            } else if key == "playback_level_db" {
                self.playback_level_db = Self::checked_float(key, &value)?;
                rebuild_auto |= self.mode_index == 2;
                rebuild_iso |= self.mode_index == 1;
            } else if key == "reference_level_db" {
                self.reference_level_db = Self::checked_float(key, &value)?;
                rebuild_auto |= self.mode_index == 2;
                rebuild_iso |= self.mode_index == 1;
            } else if key == "headroom_normalized" {
                self.headroom_normalized = value
                    .as_bool()
                    .ok_or_else(|| "headroom_normalized must be a boolean".to_string())?;
                update_headroom = true;
            } else if key == "auto_calibrated" {
                self.auto_calibrated = value
                    .as_bool()
                    .ok_or_else(|| "auto_calibrated must be a boolean".to_string())?;
                if !self.auto_calibrated && self.mode_index == 2 {
                    return Err("cannot remove SPL calibration while Auto mode is active".into());
                }
            } else {
                return Err(format!("Unknown parameter: {}", id));
            }
        }
        if rebuild_manual {
            self.rebuild_filters();
        }
        if rebuild_auto {
            self.last_auto_volume_db = f32::MIN;
            self.maybe_rebuild_auto_filters();
        } else if rebuild_iso {
            self.rebuild_iso_filters();
        } else if update_headroom {
            self.update_comp_gain_smoother();
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        if sr == 0 {
            return Err("Loudness Compensation sample rate must be greater than zero".into());
        }
        self.sample_rate = sr;
        if let Some(auto_gain) = &mut self.auto_gain {
            auto_gain.set_sample_rate(sr)?;
        }
        for s in &mut self.comp_gain_smoother {
            s.set_time(20.0, sr);
        }
        self.transition.remaining = 0;
        self.rebuild_filters();
        self.rebuild_iso_filters();
        self.transition.remaining = 0;
        Ok(())
    }
    fn reset(&mut self) {
        for ch in 0..self.num_channels {
            for filter in &mut self.filters[ch] {
                filter.reset();
            }
            for filter in &mut self.iso_filters[ch] {
                filter.reset();
            }
            let target = self.comp_gain_smoother[ch].target();
            self.comp_gain_smoother[ch].reset(target);
        }
        if let Some(auto_gain) = &mut self.auto_gain {
            auto_gain.reset();
        }
        self.transition.remaining = 0;
        for filters in &mut self.transition.old_filters {
            for filter in filters {
                filter.reset();
            }
        }
        self.cache.update(|data| *data = AutoGainData::default());
        self.last_auto_volume_db = self.playback_volume_db;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        let expected_len = nf
            .checked_mul(self.num_channels)
            .ok_or_else(|| "Loudness Compensation buffer length overflow".to_string())?;
        if buffer.len() != expected_len {
            return Err(format!(
                "Loudness Compensation expected {expected_len} samples for {nf} frames and {} channels, got {}",
                self.num_channels,
                buffer.len()
            ));
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Loudness Compensation context sample rate {} does not match initialized rate {}",
                context.sample_rate, self.sample_rate
            ));
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
                }

                // Process through filters
                for frame in 0..nf {
                    for ch in 0..self.num_channels {
                        let idx = frame * self.num_channels + ch;
                        buffer[idx] = self.process_sample(ch, buffer[idx]);
                    }
                }
                if let Some(ag) = &mut self.auto_gain {
                    let _ = ag.measure_output(buffer);
                    if do_cache_update {
                        let data = ag.get_data();
                        self.cache.update(|d| *d = data);
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
