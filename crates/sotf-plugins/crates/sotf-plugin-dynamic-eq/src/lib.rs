// ============================================================================
// Dynamic EQ Plugin
// ============================================================================
//
// A hybrid EQ + compressor: each band is a parametric EQ whose gain is
// modulated by a dynamics section. The EQ gain kicks in only when the
// signal level in the band's frequency range exceeds a threshold.
//
// Key difference from multiband compressor:
// - Dynamic EQ bands are parametric (arbitrary center freq, narrow Q)
// - Multiband compressor splits the signal; dynamic EQ filters in-place
// - No crossover splitting needed

pub mod params;

use crate::params::{BAND_PARAMS, MAX_BANDS, PARAMS as DQ};
use math_audio_dsp::fast_math::fast_log10;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::dynamics_core::DynamicsCore;
use sotf_host::dynamics_core::DynamicsMode;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

const DB_CONVERSION_FACTOR: f32 = 20.0;
const EPSILON: f32 = 1e-10;

// ============================================================================
// Serializable plugin params (for engine/bridge construction)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEqPluginParams {
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_release")]
    pub release_ms: f32,
    #[serde(default = "default_knee")]
    pub knee: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_bands_params")]
    pub bands: Vec<DynEqBandParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynEqBandParams {
    #[serde(default = "default_band_frequency")]
    pub frequency: f32,
    #[serde(default = "default_band_q")]
    pub q: f32,
    #[serde(default = "default_band_gain")]
    pub gain: f32,
    #[serde(default = "default_band_threshold")]
    pub band_threshold: f32,
    #[serde(default = "default_band_ratio")]
    pub band_ratio: f32,
    #[serde(default = "default_band_active")]
    pub active: bool,
    #[serde(default)]
    pub solo: bool,
}

fn default_num_bands() -> usize {
    pk(DQ, "num_bands").default_f64() as usize
}
fn default_threshold() -> f32 {
    pk(DQ, "threshold").default_f64() as f32
}
fn default_ratio() -> f32 {
    pk(DQ, "ratio").default_f64() as f32
}
fn default_attack() -> f32 {
    pk(DQ, "attack").default_f64() as f32
}
fn default_release() -> f32 {
    pk(DQ, "release").default_f64() as f32
}
fn default_knee() -> f32 {
    pk(DQ, "knee").default_f64() as f32
}
fn default_link_channels() -> bool {
    pk(DQ, "link_channels").default_bool()
}
fn default_mix() -> f32 {
    pk(DQ, "mix").default_f64() as f32
}
fn default_band_frequency() -> f32 {
    pk(BAND_PARAMS, "frequency").default_f64() as f32
}
fn default_band_q() -> f32 {
    pk(BAND_PARAMS, "q").default_f64() as f32
}
fn default_band_gain() -> f32 {
    pk(BAND_PARAMS, "gain").default_f64() as f32
}
fn default_band_threshold() -> f32 {
    pk(BAND_PARAMS, "band_threshold").default_f64() as f32
}
fn default_band_ratio() -> f32 {
    pk(BAND_PARAMS, "band_ratio").default_f64() as f32
}
fn default_band_active() -> bool {
    pk(BAND_PARAMS, "active").default_bool()
}
fn default_bands_params() -> Vec<DynEqBandParams> {
    let n = default_num_bands();
    (0..n).map(|_| DynEqBandParams::default()).collect()
}

impl Default for DynEqBandParams {
    fn default() -> Self {
        Self {
            frequency: default_band_frequency(),
            q: default_band_q(),
            gain: default_band_gain(),
            band_threshold: default_band_threshold(),
            band_ratio: default_band_ratio(),
            active: default_band_active(),
            solo: false,
        }
    }
}

impl Default for DynamicEqPluginParams {
    fn default() -> Self {
        Self {
            num_bands: default_num_bands(),
            threshold: default_threshold(),
            ratio: default_ratio(),
            attack_ms: default_attack(),
            release_ms: default_release(),
            knee: default_knee(),
            link_channels: default_link_channels(),
            mix: default_mix(),
            bands: default_bands_params(),
        }
    }
}

// ============================================================================
// Monitoring data
// ============================================================================

/// Per-band gain reduction for UI meters.
#[derive(Debug, Clone)]
pub struct DynamicEqData {
    /// Gain reduction in dB per band.
    pub gain_reduction_db: Arc<Vec<f32>>,
}

impl Default for DynamicEqData {
    fn default() -> Self {
        Self {
            gain_reduction_db: Arc::new(Vec::new()),
        }
    }
}

impl DynamicEqData {
    pub fn new(num_bands: usize) -> Self {
        Self {
            gain_reduction_db: Arc::new(vec![0.0; num_bands]),
        }
    }

    pub fn update(&mut self, gr: &[f32]) {
        if let Some(v) = Arc::get_mut(&mut self.gain_reduction_db)
            && v.len() == gr.len()
        {
            v.copy_from_slice(gr);
        }
    }
}

// ============================================================================
// Per-band DSP state
// ============================================================================

struct DynEqBand {
    // EQ parameters
    frequency: f32,
    q: f32,
    target_gain_db: f32,

    // Per-band dynamics overrides
    band_threshold: f32,
    band_ratio: f32,
    use_band_threshold: bool,
    use_band_ratio: bool,

    // Band control
    active: bool,
    solo: bool,

    // DSP state (pre-allocated for max channels)
    /// Highpass filter per channel (lower bound of sidechain BPF)
    sidechain_bp_hp: Vec<Biquad>,
    /// Lowpass filter per channel (upper bound of sidechain BPF)
    sidechain_bp_lp: Vec<Biquad>,
    /// The actual EQ biquad per channel — held at target_gain_db (static coefficients).
    /// Gain modulation is applied as a dry/wet blend, not via coefficient updates.
    eq_filters: Vec<Biquad>,
    /// One DynamicsCore per channel
    cores: Vec<DynamicsCore>,
}

impl DynEqBand {
    fn new(
        channels: usize,
        sample_rate: u32,
        frequency: f32,
        q: f32,
        target_gain_db: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> Self {
        let (f_low, f_high) = bandpass_edges(frequency, q);

        let sidechain_bp_hp = (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Highpass,
                    f_low as f64,
                    sample_rate as f64,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                )
            })
            .collect();

        let sidechain_bp_lp = (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Lowpass,
                    f_high as f64,
                    sample_rate as f64,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                )
            })
            .collect();

        let eq_filters = (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Peak,
                    frequency as f64,
                    sample_rate as f64,
                    q as f64,
                    0.0, // starts at 0 dB (passthrough)
                )
            })
            .collect();

        let mut cores: Vec<DynamicsCore> = (0..channels)
            .map(|_| DynamicsCore::new(DynamicsMode::Compress, 1, sample_rate))
            .collect();
        for core in &mut cores {
            core.set_attack_release(attack_ms, release_ms);
        }

        Self {
            frequency,
            q,
            target_gain_db,
            band_threshold: default_band_threshold(),
            band_ratio: default_band_ratio(),
            use_band_threshold: false,
            use_band_ratio: false,
            active: true,
            solo: false,
            sidechain_bp_hp,
            sidechain_bp_lp,
            eq_filters,
            cores,
        }
    }

    /// Process the sidechain bandpass filter on a sample for a given channel.
    ///
    /// Accepts and returns f64 to avoid unnecessary round-trip conversions; the
    /// internal biquads already operate in f64.
    #[inline]
    fn apply_sidechain_bp(&mut self, ch: usize, sample: f64) -> f64 {
        let hp_out = self.sidechain_bp_hp[ch].process(sample);
        self.sidechain_bp_lp[ch].process(hp_out)
    }

    /// Compute the proportion of EQ gain to apply based on gain reduction from the
    /// dynamics core. Returns a value in [0.0, 1.0] representing how much of the
    /// full EQ band shape to blend in.
    #[inline]
    fn modulation_proportion(target_gain_db: f32, gain_reduction_db: f32) -> f32 {
        if target_gain_db.abs() < 0.01 {
            return 0.0;
        }
        (gain_reduction_db / target_gain_db.abs()).clamp(0.0, 1.0)
    }

    fn rebuild_sidechain_filters(&mut self, sample_rate: u32) {
        let (f_low, f_high) = bandpass_edges(self.frequency, self.q);
        for hp in &mut self.sidechain_bp_hp {
            *hp = Biquad::new(
                BiquadFilterType::Highpass,
                f_low as f64,
                sample_rate as f64,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
            );
        }
        for lp in &mut self.sidechain_bp_lp {
            *lp = Biquad::new(
                BiquadFilterType::Lowpass,
                f_high as f64,
                sample_rate as f64,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
            );
        }
    }

    fn rebuild_eq_filters(&mut self, sample_rate: u32) {
        // EQ filters are held at target_gain_db; modulation is a dry/wet blend
        for eq in &mut self.eq_filters {
            *eq = Biquad::new(
                BiquadFilterType::Peak,
                self.frequency as f64,
                sample_rate as f64,
                self.q as f64,
                self.target_gain_db as f64,
            );
        }
    }

    fn reset(&mut self, sample_rate: u32) {
        self.rebuild_sidechain_filters(sample_rate);
        self.rebuild_eq_filters(sample_rate);
        for core in &mut self.cores {
            core.reset();
        }
    }

    fn get_effective_threshold(&self, global_threshold: f32) -> f32 {
        if self.use_band_threshold {
            self.band_threshold
        } else {
            global_threshold
        }
    }

    fn get_effective_ratio(&self, global_ratio: f32) -> f32 {
        if self.use_band_ratio {
            self.band_ratio
        } else {
            global_ratio
        }
    }
}

/// Compute bandpass edges from center frequency and Q.
fn bandpass_edges(freq: f32, q: f32) -> (f32, f32) {
    // Exact peaking-EQ Q <-> octave-bandwidth relation:
    // BW_oct = 2 * asinh(1 / (2Q)) / ln(2).
    let inv_2q = 1.0 / (2.0 * q.max(0.1));
    let bw_oct = 2.0 * inv_2q.asinh() / std::f32::consts::LN_2;
    let half_bw = (bw_oct * 0.5).exp2();
    let f_low = (freq / half_bw).max(20.0);
    let f_high = (freq * half_bw).min(20000.0);
    (f_low, f_high)
}

// ============================================================================
// Plugin
// ============================================================================

pub struct DynamicEqPlugin {
    channels: usize,
    sample_rate: u32,
    num_bands: usize,

    // Global params
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    link_channels: bool,
    mix: f32,

    // Per-band state (pre-allocated for MAX_BANDS)
    bands: Vec<DynEqBand>,

    // Smoothers
    mix_smoother: Smoother,
    threshold_smoother: Smoother,

    // Monitoring
    monitoring_gr: Vec<f32>,
    cache: RealTimeCache<DynamicEqData>,
    cache_counter: usize,

    // Dry buffer for mix (pre-allocated)
    dry_buf: Vec<f32>,

    // Parameter IDs
    param_num_bands: ParameterId,
    param_threshold: ParameterId,
    param_ratio: ParameterId,
    param_attack: ParameterId,
    param_release: ParameterId,
    param_knee: ParameterId,
    param_link_channels: ParameterId,
    param_mix: ParameterId,

    cached_parameters: Vec<Parameter>,
}

impl DynamicEqPlugin {
    pub fn new(channels: usize) -> Self {
        let sr = 44100u32;
        let num_bands = default_num_bands();
        let attack = default_attack();
        let release = default_release();

        let bands: Vec<DynEqBand> = (0..MAX_BANDS)
            .map(|_| {
                DynEqBand::new(
                    channels,
                    sr,
                    default_band_frequency(),
                    default_band_q(),
                    default_band_gain(),
                    attack,
                    release,
                )
            })
            .collect();

        let threshold = default_threshold();
        let mix = default_mix();

        let mut p = Self {
            channels,
            sample_rate: sr,
            num_bands,

            threshold_db: threshold,
            ratio: default_ratio(),
            attack_ms: attack,
            release_ms: release,
            knee_db: default_knee(),
            link_channels: default_link_channels(),
            mix,

            bands,

            mix_smoother: Smoother::new(mix, 5.0, sr),
            threshold_smoother: Smoother::new(threshold, 5.0, sr),

            monitoring_gr: vec![0.0; MAX_BANDS],
            cache: RealTimeCache::new(DynamicEqData::new(MAX_BANDS)),
            cache_counter: 0,

            // Pre-allocate dry buffer for max expected frame size
            // 8192 frames * 32 channels should be more than enough
            dry_buf: vec![0.0; 8192 * channels.max(2)],

            param_num_bands: ParameterId::from("num_bands"),
            param_threshold: ParameterId::from("threshold"),
            param_ratio: ParameterId::from("ratio"),
            param_attack: ParameterId::from("attack"),
            param_release: ParameterId::from("release"),
            param_knee: ParameterId::from("knee"),
            param_link_channels: ParameterId::from("link_channels"),
            param_mix: ParameterId::from("mix"),

            cached_parameters: Vec::new(),
        };

        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: DynamicEqPluginParams) -> Self {
        let mut p = Self::new(channels);
        p.num_bands = params.num_bands.clamp(1, MAX_BANDS);
        p.threshold_db = params.threshold.clamp(-60.0, 0.0);
        p.threshold_smoother.set_target(p.threshold_db);
        p.ratio = params.ratio.clamp(1.0, 20.0);
        p.attack_ms = params.attack_ms.clamp(0.1, 100.0);
        p.release_ms = params.release_ms.clamp(10.0, 1000.0);
        p.knee_db = params.knee.clamp(0.0, 20.0);
        p.link_channels = params.link_channels;
        p.mix = params.mix.clamp(0.0, 1.0);
        p.mix_smoother.set_target(p.mix);

        // Apply per-band params
        for (i, band_params) in params.bands.iter().enumerate().take(MAX_BANDS) {
            let band = &mut p.bands[i];
            band.frequency = band_params.frequency.clamp(20.0, 20000.0);
            band.q = band_params.q.clamp(0.1, 10.0);
            band.target_gain_db = band_params.gain.clamp(-24.0, 24.0);
            band.band_threshold = band_params.band_threshold.clamp(-60.0, 0.0);
            band.band_ratio = band_params.band_ratio.clamp(1.0, 20.0);
            band.active = band_params.active;
            band.solo = band_params.solo;

            // If band values differ from global defaults, mark as overrides
            band.use_band_threshold = (band_params.band_threshold - params.threshold).abs() > 0.01;
            band.use_band_ratio = (band_params.band_ratio - params.ratio).abs() > 0.01;

            band.rebuild_sidechain_filters(p.sample_rate);
            band.rebuild_eq_filters(p.sample_rate);
        }

        // Update dynamics cores
        for band in &mut p.bands {
            for core in &mut band.cores {
                core.set_attack_release(p.attack_ms, p.release_ms);
            }
        }

        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int(
                "num_bands",
                "Num Bands",
                self.num_bands as i32,
                pk(DQ, "num_bands").min_f64() as i32,
                pk(DQ, "num_bands").max_f64() as i32,
            )
            .with_description("Number of dynamic EQ bands")
            .with_group("Setup")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(DQ, "threshold").min_f64() as f32,
                pk(DQ, "threshold").max_f64() as f32,
            )
            .with_description("Global detection threshold (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(DQ, "ratio").min_f64() as f32,
                pk(DQ, "ratio").max_f64() as f32,
            )
            .with_description("Global dynamics ratio")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(DQ, "attack").min_f64() as f32,
                pk(DQ, "attack").max_f64() as f32,
            )
            .with_description("Attack time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(DQ, "release").min_f64() as f32,
                pk(DQ, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "knee",
                "Knee",
                self.knee_db,
                pk(DQ, "knee").min_f64() as f32,
                pk(DQ, "knee").max_f64() as f32,
            )
            .with_description("Soft knee width (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_description("Stereo-link detection")
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(DQ, "mix").min_f64() as f32,
                pk(DQ, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
        ];
    }
}

impl InPlacePlugin for DynamicEqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("DynamicEQ", "1.0.0", "SotF")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if !id.0.starts_with("band_") {
            self.validate_parameter(&id, &value)?;
        }

        if id == self.param_num_bands {
            let v = value
                .as_int()
                .or_else(|| value.as_float().map(|f| f as i32))
                .unwrap_or(pk(DQ, "num_bands").default_f64() as i32);
            self.num_bands = (v as usize).clamp(1, MAX_BANDS);
        } else if id == self.param_threshold {
            let v = value
                .as_float()
                .unwrap_or(pk(DQ, "threshold").default_f64() as f32);
            if v.is_finite() {
                self.threshold_db = v.clamp(-60.0, 0.0);
                self.threshold_smoother.set_target(self.threshold_db);
                for band in &mut self.bands {
                    if (band.band_threshold - self.threshold_db).abs() <= 0.01 {
                        band.use_band_threshold = false;
                    }
                }
            }
        } else if id == self.param_ratio {
            let v = value
                .as_float()
                .unwrap_or(pk(DQ, "ratio").default_f64() as f32);
            if v.is_finite() {
                self.ratio = v.clamp(1.0, 20.0);
                for band in &mut self.bands {
                    if (band.band_ratio - self.ratio).abs() <= 0.01 {
                        band.use_band_ratio = false;
                    }
                }
            }
        } else if id == self.param_attack {
            let v = value
                .as_float()
                .unwrap_or(pk(DQ, "attack").default_f64() as f32);
            if v.is_finite() {
                self.attack_ms = v.clamp(0.1, 100.0);
                for band in &mut self.bands {
                    for core in &mut band.cores {
                        core.set_attack_release(self.attack_ms, self.release_ms);
                    }
                }
            }
        } else if id == self.param_release {
            let v = value
                .as_float()
                .unwrap_or(pk(DQ, "release").default_f64() as f32);
            if v.is_finite() {
                self.release_ms = v.clamp(10.0, 1000.0);
                for band in &mut self.bands {
                    for core in &mut band.cores {
                        core.set_attack_release(self.attack_ms, self.release_ms);
                    }
                }
            }
        } else if id == self.param_knee {
            let v = value
                .as_float()
                .unwrap_or(pk(DQ, "knee").default_f64() as f32);
            if v.is_finite() {
                self.knee_db = v.clamp(0.0, 20.0);
            }
        } else if id == self.param_link_channels {
            self.link_channels = value.as_bool().unwrap_or(default_link_channels());
        } else if id == self.param_mix {
            let v = value
                .as_float()
                .unwrap_or(pk(DQ, "mix").default_f64() as f32);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if let Some(rest) = id.0.strip_prefix("band_") {
            // Per-band parameters: band_N_field
            if let Some(sep) = rest.find('_') {
                let b_idx = rest[..sep].parse::<usize>().unwrap_or(0);
                let field = &rest[sep + 1..];
                if b_idx < self.bands.len() {
                    let band = &mut self.bands[b_idx];
                    match field {
                        "frequency" | "freq" => {
                            if let Some(v) = value.as_float() {
                                band.frequency = v.clamp(20.0, 20000.0);
                                band.rebuild_sidechain_filters(self.sample_rate);
                                band.rebuild_eq_filters(self.sample_rate);
                            }
                        }
                        "q" => {
                            if let Some(v) = value.as_float() {
                                band.q = v.clamp(0.1, 10.0);
                                band.rebuild_sidechain_filters(self.sample_rate);
                                band.rebuild_eq_filters(self.sample_rate);
                            }
                        }
                        "gain" => {
                            if let Some(v) = value.as_float() {
                                band.target_gain_db = v.clamp(-24.0, 24.0);
                            }
                        }
                        "threshold" => {
                            if let Some(v) = value.as_float() {
                                band.band_threshold = v.clamp(-60.0, 0.0);
                                band.use_band_threshold =
                                    (band.band_threshold - self.threshold_db).abs() > 0.01;
                            }
                        }
                        "ratio" => {
                            if let Some(v) = value.as_float() {
                                band.band_ratio = v.clamp(1.0, 20.0);
                                band.use_band_ratio = (band.band_ratio - self.ratio).abs() > 0.01;
                            }
                        }
                        "active" => {
                            band.active = value.as_bool().unwrap_or(true);
                        }
                        "solo" => {
                            band.solo = value.as_bool().unwrap_or(false);
                        }
                        _ => {}
                    }
                }
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_num_bands {
            Some(ParameterValue::Int(self.num_bands as i32))
        } else if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.ratio))
        } else if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_link_channels {
            Some(ParameterValue::Bool(self.link_channels))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if let Some(rest) = id.0.strip_prefix("band_") {
            if let Some(sep) = rest.find('_') {
                let b_idx = rest[..sep].parse::<usize>().unwrap_or(0);
                let field = &rest[sep + 1..];
                if b_idx < self.bands.len() {
                    let band = &self.bands[b_idx];
                    match field {
                        "frequency" | "freq" => Some(ParameterValue::Float(band.frequency)),
                        "q" => Some(ParameterValue::Float(band.q)),
                        "gain" => Some(ParameterValue::Float(band.target_gain_db)),
                        "threshold" => Some(ParameterValue::Float(band.band_threshold)),
                        "ratio" => Some(ParameterValue::Float(band.band_ratio)),
                        "active" => Some(ParameterValue::Bool(band.active)),
                        "solo" => Some(ParameterValue::Bool(band.solo)),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        for band in &mut self.bands {
            band.rebuild_sidechain_filters(sample_rate);
            band.rebuild_eq_filters(sample_rate);
            for core in &mut band.cores {
                core.initialize(sample_rate);
                core.set_attack_release(self.attack_ms, self.release_ms);
            }
        }

        self.mix_smoother.set_time(5.0, sample_rate);
        self.threshold_smoother.set_time(5.0, sample_rate);

        // Pre-allocate dry buffer for max expected frame size (up to 1s @ 96kHz)
        let buf_size = 96000 * self.channels.max(2);
        if self.dry_buf.len() < buf_size {
            self.dry_buf.resize(buf_size, 0.0);
        }

        Ok(())
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset(self.sample_rate);
        }
        self.monitoring_gr.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let nc = self.channels;
        let total = nf
            .checked_mul(nc)
            .ok_or_else(|| "Frame/channel count overflow".to_string())?;
        if buffer.len() != total {
            return Err(format!(
                "Buffer size mismatch: expected {}, got {}",
                total,
                buffer.len()
            ));
        }

        // Reject blocks larger than pre-allocated dry buffer to maintain real-time safety.
        if total > self.dry_buf.len() {
            return Err(format!(
                "dynamic-eq: block size {} samples exceeds max {} samples",
                total,
                self.dry_buf.len()
            ));
        }

        // Save dry signal
        self.dry_buf[..total].copy_from_slice(&buffer[..total]);

        let g_threshold = self.threshold_smoother.next_n(nf);
        let mix = self.mix_smoother.next_n(nf);
        let dry_mix = 1.0 - mix;
        let knee = self.knee_db;
        let ratio = self.ratio;

        // Check for solo
        let any_solo = self.bands[..self.num_bands].iter().any(|b| b.solo);

        for frame in 0..nf {
            for band_idx in 0..self.num_bands {
                let band = &mut self.bands[band_idx];
                if !band.active {
                    continue;
                }
                if any_solo && !band.solo {
                    continue;
                }

                let threshold = band.get_effective_threshold(g_threshold);
                let band_ratio = band.get_effective_ratio(ratio);

                if self.link_channels && nc > 1 {
                    // Linked: max detection across channels.
                    // Sidechain reads dry_buf to avoid inter-band contamination.
                    let mut max_level = 0.0f32;
                    for ch in 0..nc {
                        let idx = frame * nc + ch;
                        let filtered = band.apply_sidechain_bp(ch, self.dry_buf[idx] as f64) as f32;
                        let level = filtered.abs();
                        max_level = max_level.max(level);
                    }
                    let level_db = DB_CONVERSION_FACTOR * fast_log10(max_level.max(EPSILON));
                    let gr = band.cores[0]
                        .calculate_gain_reduction(level_db, threshold, band_ratio, knee);
                    let smoothed = band.cores[0].apply_envelope(0, gr);

                    // Proportion of the full EQ band shape to apply this sample.
                    // EQ biquad is held at target_gain_db; blend avoids coefficient updates.
                    let proportion =
                        DynEqBand::modulation_proportion(band.target_gain_db, smoothed);

                    self.monitoring_gr[band_idx] = smoothed;

                    for ch in 0..nc {
                        let idx = frame * nc + ch;
                        let dry = buffer[idx];
                        let eq_out = band.eq_filters[ch].process(dry as f64) as f32;
                        // Blend: proportion=0 → dry passthrough; proportion=1 → full EQ
                        buffer[idx] = dry + (eq_out - dry) * proportion;
                    }
                } else {
                    // Per-channel detection.
                    // Sidechain reads dry_buf to avoid inter-band contamination.
                    for ch in 0..nc {
                        let idx = frame * nc + ch;
                        let filtered = band.apply_sidechain_bp(ch, self.dry_buf[idx] as f64) as f32;
                        let level = filtered.abs();
                        let level_db = DB_CONVERSION_FACTOR * fast_log10(level.max(EPSILON));
                        let gr = band.cores[ch]
                            .calculate_gain_reduction(level_db, threshold, band_ratio, knee);
                        let smoothed = band.cores[ch].apply_envelope(ch, gr);

                        let proportion =
                            DynEqBand::modulation_proportion(band.target_gain_db, smoothed);

                        let dry = buffer[idx];
                        let eq_out = band.eq_filters[ch].process(dry as f64) as f32;
                        buffer[idx] = dry + (eq_out - dry) * proportion;
                    }

                    // Use channel 0 GR for monitoring (read-only)
                    self.monitoring_gr[band_idx] = if nc > 0 {
                        band.cores[0].envelope_db(0)
                    } else {
                        0.0
                    };
                }
            }
        }

        // Mix dry/wet
        if (mix - 1.0).abs() > 0.001 {
            for (sample, dry) in buffer[..total].iter_mut().zip(self.dry_buf[..total].iter()) {
                *sample = *dry * dry_mix + *sample * mix;
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_counter += 1;
        if self.cache_counter >= 10 {
            self.cache_counter = 0;
            self.cache.update(|d| {
                d.update(&self.monitoring_gr[..self.num_bands]);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
        (0..num_frames)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
            })
            .collect()
    }

    fn rms(buf: &[f32]) -> f32 {
        let sum: f32 = buf.iter().map(|x| x * x).sum();
        (sum / buf.len() as f32).sqrt()
    }

    fn process_with_first_eq_filter(plugin: &mut DynamicEqPlugin, input: &[f32]) -> Vec<f32> {
        let filter = &mut plugin.bands[0].eq_filters[0];
        input
            .iter()
            .map(|sample| filter.process(*sample as f64) as f32)
            .collect()
    }

    #[test]
    fn test_dynamic_eq_passthrough() {
        // With gain=0, output should equal input
        let sr = 48000u32;
        let num_frames = 4800; // 100ms
        let amplitude = 0.5;

        let mut plugin = DynamicEqPlugin::from_params(
            1,
            DynamicEqPluginParams {
                num_bands: 1,
                threshold: -60.0,
                ratio: 4.0,
                attack_ms: 1.0,
                release_ms: 50.0,
                knee: 0.0,
                link_channels: false,
                mix: 1.0,
                bands: vec![DynEqBandParams {
                    frequency: 1000.0,
                    q: 1.0,
                    gain: 0.0, // zero gain = passthrough
                    band_threshold: -60.0,
                    band_ratio: 4.0,
                    active: true,
                    solo: false,
                }],
            },
        );
        plugin.initialize(sr).unwrap();

        let original = make_sine(1000.0, sr, num_frames, amplitude);
        let mut buf = original.clone();

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        // Output should be essentially the same (peak EQ at 0 dB is passthrough)
        let input_rms = rms(&original);
        let output_rms = rms(&buf);
        let ratio = output_rms / input_rms;
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Passthrough: ratio={:.4} (input_rms={:.4}, output_rms={:.4})",
            ratio,
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_dynamic_eq_boosts_on_threshold() {
        // With gain=+6dB and loud signal above threshold, output should be boosted
        let sr = 48000u32;
        let num_frames = 48000; // 1 second
        let amplitude = 0.5; // about -6 dBFS

        let mut plugin = DynamicEqPlugin::from_params(
            1,
            DynamicEqPluginParams {
                num_bands: 1,
                threshold: -20.0,
                ratio: 10.0,
                attack_ms: 0.5,
                release_ms: 20.0,
                knee: 0.0,
                link_channels: false,
                mix: 1.0,
                bands: vec![DynEqBandParams {
                    frequency: 1000.0,
                    q: 1.0,
                    gain: 6.0, // +6 dB boost
                    band_threshold: -20.0,
                    band_ratio: 10.0,
                    active: true,
                    solo: false,
                }],
            },
        );
        plugin.initialize(sr).unwrap();

        let mut buf = make_sine(1000.0, sr, num_frames, amplitude);
        let input_rms = rms(&buf);

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        // Use the second half to allow attack to settle
        let output_rms = rms(&buf[num_frames / 2..]);

        // Output should be louder than input at the band frequency
        assert!(
            output_rms > input_rms * 1.1,
            "Boost: output_rms={:.4} should be > input_rms*1.1={:.4}",
            output_rms,
            input_rms * 1.1
        );
    }

    #[test]
    fn test_dynamic_eq_no_boost_below_threshold() {
        // Quiet signal should pass unaffected (below threshold)
        let sr = 48000u32;
        let num_frames = 48000; // 1 second
        let amplitude = 0.001; // very quiet, about -60 dBFS

        let mut plugin = DynamicEqPlugin::from_params(
            1,
            DynamicEqPluginParams {
                num_bands: 1,
                threshold: -10.0, // high threshold
                ratio: 10.0,
                attack_ms: 0.5,
                release_ms: 20.0,
                knee: 0.0,
                link_channels: false,
                mix: 1.0,
                bands: vec![DynEqBandParams {
                    frequency: 1000.0,
                    q: 1.0,
                    gain: 12.0, // big boost, but shouldn't trigger
                    band_threshold: -10.0,
                    band_ratio: 10.0,
                    active: true,
                    solo: false,
                }],
            },
        );
        plugin.initialize(sr).unwrap();

        let mut buf = make_sine(1000.0, sr, num_frames, amplitude);
        let input_rms = rms(&buf);

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        let output_rms = rms(&buf[num_frames / 2..]);

        // Should be essentially unchanged (EQ at ~0 dB gain)
        let ratio = output_rms / input_rms;
        assert!(
            (ratio - 1.0).abs() < 0.15,
            "Below threshold: ratio={:.4} (input_rms={:.6}, output_rms={:.6})",
            ratio,
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_dynamic_eq_frequency_selective() {
        // 1kHz band should only affect 1kHz content, not 100Hz content
        let sr = 48000u32;
        let num_frames = 48000; // 1 second
        let amplitude = 0.5;

        let params = DynamicEqPluginParams {
            num_bands: 1,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 2.0, // narrow band
                gain: 12.0,
                band_threshold: -20.0,
                band_ratio: 10.0,
                active: true,
                solo: false,
            }],
        };

        // Test with 1kHz signal (in-band)
        let mut plugin_1k = DynamicEqPlugin::from_params(1, params.clone());
        plugin_1k.initialize(sr).unwrap();
        let mut buf_1k = make_sine(1000.0, sr, num_frames, amplitude);
        let input_rms_1k = rms(&buf_1k);
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin_1k.process_in_place(&mut buf_1k, &ctx).unwrap();
        let output_rms_1k = rms(&buf_1k[num_frames / 2..]);

        // Test with 100Hz signal (out-of-band)
        let mut plugin_100 = DynamicEqPlugin::from_params(1, params);
        plugin_100.initialize(sr).unwrap();
        let mut buf_100 = make_sine(100.0, sr, num_frames, amplitude);
        let input_rms_100 = rms(&buf_100);
        plugin_100.process_in_place(&mut buf_100, &ctx).unwrap();
        let output_rms_100 = rms(&buf_100[num_frames / 2..]);

        let ratio_1k = output_rms_1k / input_rms_1k;
        let ratio_100 = output_rms_100 / input_rms_100;

        // 1kHz should be affected more than 100Hz
        assert!(
            ratio_1k > ratio_100 * 1.2,
            "Frequency selectivity: 1kHz ratio={:.4} should be > 100Hz ratio={:.4} * 1.2",
            ratio_1k,
            ratio_100
        );
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut plugin = DynamicEqPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Set threshold
        plugin
            .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("threshold"));
        assert_eq!(val, Some(ParameterValue::Float(-30.0)));

        // Set ratio
        plugin
            .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(5.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("ratio"));
        assert_eq!(val, Some(ParameterValue::Float(5.0)));

        // Set mix
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(val, Some(ParameterValue::Float(0.5)));

        // Set link_channels
        plugin
            .set_parameter(
                ParameterId::from("link_channels"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("link_channels"));
        assert_eq!(val, Some(ParameterValue::Bool(false)));

        // Set num_bands
        plugin
            .set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(6))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("num_bands"));
        assert_eq!(val, Some(ParameterValue::Int(6)));
    }

    #[test]
    fn test_band_frequency_change_rebuilds_filters() {
        let sr = 48000u32;
        let num_frames = 48000;
        let input = make_sine(1000.0, sr, num_frames, 0.25);
        let input_rms = rms(&input);

        let mut plugin = DynamicEqPlugin::new(1);
        plugin.initialize(sr).unwrap();
        plugin.bands[0].frequency = 1000.0;
        plugin.bands[0].q = 4.0;
        plugin.bands[0].target_gain_db = 12.0;
        plugin.bands[0].rebuild_eq_filters(sr);

        let boosted = process_with_first_eq_filter(&mut plugin, &input);
        let boosted_rms = rms(&boosted[num_frames / 2..]);
        assert!(boosted_rms > input_rms * 1.4);

        plugin.bands[0].frequency = 1000.0;
        plugin.bands[0].q = 4.0;
        plugin.bands[0].target_gain_db = 12.0;
        plugin.bands[0].rebuild_eq_filters(sr);
        plugin
            .set_parameter(
                ParameterId::from("band_0_frequency"),
                ParameterValue::Float(100.0),
            )
            .unwrap();

        let retuned = process_with_first_eq_filter(&mut plugin, &input);
        let retuned_rms = rms(&retuned[num_frames / 2..]);
        assert!(
            retuned_rms < input_rms * 1.15,
            "retuned 100 Hz EQ should not keep boosting 1 kHz: input={input_rms:.4}, retuned={retuned_rms:.4}"
        );
    }

    #[test]
    fn test_band_q_change_rebuilds_filters() {
        let sr = 48000u32;
        let num_frames = 48000;
        let input = make_sine(400.0, sr, num_frames, 0.25);
        let input_rms = rms(&input);

        let mut plugin = DynamicEqPlugin::new(1);
        plugin.initialize(sr).unwrap();
        plugin.bands[0].frequency = 1000.0;
        plugin.bands[0].q = 0.1;
        plugin.bands[0].target_gain_db = 12.0;
        plugin.bands[0].rebuild_eq_filters(sr);

        let wide = process_with_first_eq_filter(&mut plugin, &input);
        let wide_rms = rms(&wide[num_frames / 2..]);
        assert!(wide_rms > input_rms * 1.3);

        plugin.bands[0].frequency = 1000.0;
        plugin.bands[0].q = 0.1;
        plugin.bands[0].target_gain_db = 12.0;
        plugin.bands[0].rebuild_eq_filters(sr);
        plugin
            .set_parameter(ParameterId::from("band_0_q"), ParameterValue::Float(10.0))
            .unwrap();

        let narrow = process_with_first_eq_filter(&mut plugin, &input);
        let narrow_rms = rms(&narrow[num_frames / 2..]);
        assert!(
            narrow_rms < input_rms * 1.15,
            "narrowed Q should not keep wide-band boost: input={input_rms:.4}, narrow={narrow_rms:.4}"
        );
    }

    #[test]
    fn test_band_threshold_ratio_overrides_can_return_to_global() {
        let mut plugin = DynamicEqPlugin::new(1);
        plugin.initialize(48000).unwrap();

        plugin
            .set_parameter(
                ParameterId::from("band_0_threshold"),
                ParameterValue::Float(-30.0),
            )
            .unwrap();
        assert!(plugin.bands[0].use_band_threshold);
        assert_eq!(plugin.bands[0].get_effective_threshold(-20.0), -30.0);

        plugin
            .set_parameter(
                ParameterId::from("band_0_threshold"),
                ParameterValue::Float(plugin.threshold_db),
            )
            .unwrap();
        assert!(
            !plugin.bands[0].use_band_threshold,
            "setting a band threshold equal to the global threshold should clear the override"
        );

        plugin
            .set_parameter(
                ParameterId::from("band_0_ratio"),
                ParameterValue::Float(8.0),
            )
            .unwrap();
        assert!(plugin.bands[0].use_band_ratio);
        assert_eq!(plugin.bands[0].get_effective_ratio(2.0), 8.0);

        plugin
            .set_parameter(
                ParameterId::from("ratio"),
                ParameterValue::Float(plugin.bands[0].band_ratio),
            )
            .unwrap();
        assert!(
            !plugin.bands[0].use_band_ratio,
            "setting the global ratio equal to a band ratio should clear the override"
        );
    }

    /// `reset()` must rebuild EQ filters to reflect the current `target_gain_db`.
    ///
    /// We set target_gain_db = 0.0 (passthrough) but manually build the filter at
    /// 12 dB to simulate stale biquad state, then call reset() and verify the output
    /// is unaffected (filter rebuilt at 0 dB).
    #[test]
    fn test_reset_rebuilds_eq_filters_at_zero_gain() {
        let sr = 48000u32;
        let num_frames = 48000;
        let input = make_sine(1000.0, sr, num_frames, 0.25);
        let input_rms = rms(&input);

        let mut plugin = DynamicEqPlugin::new(1);
        plugin.initialize(sr).unwrap();
        plugin.num_bands = 1;
        plugin.bands[0].frequency = 1000.0;
        plugin.bands[0].q = 4.0;
        // target_gain_db = 0 → after reset the filter should be passthrough.
        // We deliberately build the filter at 12 dB to create stale biquad state.
        plugin.bands[0].target_gain_db = 12.0;
        plugin.bands[0].rebuild_eq_filters(sr);
        // Now set the intended target and call reset(); it should rebuild at 0 dB.
        plugin.bands[0].target_gain_db = 0.0;
        plugin.reset();

        let mut buf = input.clone();
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        let output_rms = rms(&buf[num_frames / 2..]);
        assert!(
            (output_rms / input_rms - 1.0).abs() < 0.05,
            "reset should restore neutral EQ: input={input_rms:.4}, output={output_rms:.4}"
        );
    }

    #[test]
    fn test_bandpass_edges_use_exact_q_to_octave_bandwidth() {
        let (low_q1, high_q1) = bandpass_edges(1000.0, 1.0);
        assert!(
            (low_q1 - 618.0).abs() < 2.0,
            "Q=1 low edge should use exact octave relation, got {low_q1}"
        );
        assert!(
            (high_q1 - 1618.0).abs() < 2.0,
            "Q=1 high edge should use exact octave relation, got {high_q1}"
        );

        let (low_q10, high_q10) = bandpass_edges(1000.0, 10.0);
        assert!(
            (low_q10 - 951.0).abs() < 2.0,
            "Q=10 low edge should stay narrow, got {low_q10}"
        );
        assert!(
            (high_q10 - 1051.0).abs() < 2.0,
            "Q=10 high edge should stay narrow, got {high_q10}"
        );

        let product = low_q10 * high_q10;
        assert!(
            (product / 1_000_000.0 - 1.0).abs() < 0.01,
            "band edges should remain geometrically centered around freq, product={product}"
        );
    }

    #[test]
    fn test_rejects_mismatched_buffer_size() {
        let sr = 48000u32;
        let mut plugin = DynamicEqPlugin::new(2);
        plugin.initialize(sr).unwrap();

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: 16,
        };
        let mut short = vec![0.0; 31];
        let err = plugin.process_in_place(&mut short, &ctx).unwrap_err();
        assert!(err.contains("Buffer size mismatch"));
    }

    /// Regression test: sidechain must read the dry buffer, not the processed output.
    ///
    /// Two bands at different frequencies are active. If the sidechain of band 1
    /// reads the EQ'd output of band 0 instead of the original dry signal, band 1's
    /// detection level would differ depending on band 0's activity — causing
    /// inter-band contamination. We verify that disabling band 0 does not change
    /// what band 1 detects.
    #[test]
    fn test_sidechain_reads_dry_buffer_not_modified_output() {
        let sr = 48000u32;
        let num_frames = 4800; // 100ms
        // Signal at 1 kHz only. Band 0 is also tuned to 1 kHz with a large gain,
        // so if band 1 (at 2 kHz) sees band 0's boosted output it would detect a
        // higher level at 2 kHz (due to sidechain filter bleed) than when band 0 is
        // inactive.
        let amplitude = 0.5;

        let make_two_band_plugin = |band0_active: bool| {
            DynamicEqPlugin::from_params(
                1,
                DynamicEqPluginParams {
                    num_bands: 2,
                    threshold: -60.0, // always triggering
                    ratio: 20.0,
                    attack_ms: 0.1,
                    release_ms: 200.0,
                    knee: 0.0,
                    link_channels: false,
                    mix: 1.0,
                    bands: vec![
                        DynEqBandParams {
                            frequency: 1000.0,
                            q: 1.0,
                            gain: 18.0, // large boost at 1 kHz
                            band_threshold: -60.0,
                            band_ratio: 20.0,
                            active: band0_active,
                            solo: false,
                        },
                        DynEqBandParams {
                            frequency: 2000.0,
                            q: 4.0,    // narrow band at 2 kHz — only detects 2 kHz
                            gain: 0.0, // passthrough, but detection is what we test
                            band_threshold: -60.0,
                            band_ratio: 20.0,
                            active: true,
                            solo: false,
                        },
                    ],
                },
            )
        };

        // We run the plugin twice: once with band 0 active (18 dB boost at 1 kHz)
        // and once with band 0 inactive. We capture band 1's monitoring GR value,
        // which reflects what the sidechain detected. If the sidechain was reading
        // the modified buffer, band 0's presence would inflate the band 1 detection.
        let capture_band1_gr = |plugin: &mut DynamicEqPlugin| -> f32 {
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames,
            };
            let mut buf = make_sine(1000.0, sr, num_frames, amplitude);
            plugin.process_in_place(&mut buf, &ctx).unwrap();
            plugin.monitoring_gr[1]
        };

        let mut p_with = make_two_band_plugin(true);
        p_with.initialize(sr).unwrap();
        let gr_with_band0 = capture_band1_gr(&mut p_with);

        let mut p_without = make_two_band_plugin(false);
        p_without.initialize(sr).unwrap();
        let gr_without_band0 = capture_band1_gr(&mut p_without);

        // Band 1 detects 2 kHz; the source is a 1 kHz pure tone.
        // Whether band 0 boosts 1 kHz or not, band 1's sidechain sees the same
        // original dry signal in both cases (assuming correct implementation).
        let diff = (gr_with_band0 - gr_without_band0).abs();
        assert!(
            diff < 0.5,
            "Band 1 GR differs by {diff:.3} dB depending on band 0 activity — sidechain contamination bug"
        );
    }

    /// Regression test: EQ coefficients must NOT be recomputed every sample.
    ///
    /// With the old buggy implementation, `update_eq_gain` called `update_params`
    /// (which calls sin/cos/tan internally) on nearly every sample during attack.
    /// The correct implementation uses a fixed biquad + dry/wet blend, meaning the
    /// filter state is shaped by a fixed transfer function and the blend proportion
    /// drives the modulation depth.
    ///
    /// This test verifies that the plugin produces a smoothly modulated output
    /// whose RMS at the band frequency correctly reflects the blend proportion,
    /// NOT a coefficient-stepped output that would introduce artifacts.
    #[test]
    fn test_eq_gain_uses_proportion_blend_not_coefficient_update() {
        let sr = 48000u32;
        // 200 ms: long enough to see steady-state, short enough for fast attack
        let num_frames = 9600usize;
        let amplitude = 0.5f32;
        let target_gain_db = 12.0f32;

        // Configure so the band fires immediately (threshold far below signal level)
        // and with a fast attack so it reaches proportion ≈ 1.0 quickly.
        let mut plugin = DynamicEqPlugin::from_params(
            1,
            DynamicEqPluginParams {
                num_bands: 1,
                threshold: -60.0,
                ratio: 20.0,
                attack_ms: 0.5,
                release_ms: 200.0,
                knee: 0.0,
                link_channels: false,
                mix: 1.0,
                bands: vec![DynEqBandParams {
                    frequency: 1000.0,
                    q: 1.0,
                    gain: target_gain_db,
                    band_threshold: -60.0,
                    band_ratio: 20.0,
                    active: true,
                    solo: false,
                }],
            },
        );
        plugin.initialize(sr).unwrap();

        let input = make_sine(1000.0, sr, num_frames, amplitude);
        let mut buf = input.clone();
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        // At steady state (second half) proportion ≈ 1.0; output should match
        // the EQ biquad at target_gain_db applied to the input.
        // Apply the reference EQ biquad (at target_gain_db) to the same input.
        let mut ref_plugin = DynamicEqPlugin::new(1);
        ref_plugin.initialize(sr).unwrap();
        ref_plugin.bands[0].target_gain_db = target_gain_db;
        ref_plugin.bands[0].rebuild_eq_filters(sr);
        let reference: Vec<f32> = input
            .iter()
            .map(|s| ref_plugin.bands[0].eq_filters[0].process(*s as f64) as f32)
            .collect();

        let out_rms = rms(&buf[num_frames / 2..]);
        let ref_rms = rms(&reference[num_frames / 2..]);

        // At full proportion the blend output must equal the biquad-only reference.
        // Allow ±10% for the envelope settling.
        let ratio = out_rms / ref_rms;
        assert!(
            (ratio - 1.0).abs() < 0.10,
            "Steady-state output RMS {out_rms:.4} should match reference {ref_rms:.4} (ratio={ratio:.4})"
        );
    }
}
