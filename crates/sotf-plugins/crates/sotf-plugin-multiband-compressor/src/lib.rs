// ============================================================================
// Multiband Compressor Plugin
// ============================================================================

pub mod params;

use crate::params::{BAND_TEMPLATE as MCB, GLOBAL_PARAMS as MC};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_makeup::MeasuredMakeup;
use sotf_host::lookahead::LookaheadBuffer;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LogSmoother, Smoother};
use std::any::Any;
use std::sync::Arc;

pub use sotf_host::lr4_crossover::CROSSOVER_PRESETS;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandCompressorParams {
    pub threshold_db: Option<f32>,
    pub ratio: Option<f32>,
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    pub knee_db: Option<f32>,
    pub makeup_gain_db: f32,
    #[serde(default)]
    pub auto_makeup: bool,
    #[serde(default)]
    pub measured_auto_makeup: bool,
    #[serde(default = "default_true")]
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
}

impl Default for BandCompressorParams {
    fn default() -> Self {
        Self {
            threshold_db: None,
            ratio: None,
            attack_ms: None,
            release_ms: None,
            knee_db: None,
            makeup_gain_db: 0.0,
            auto_makeup: false,
            measured_auto_makeup: false,
            active: true,
            solo: false,
            bypass: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MultibandCompressorPluginParams {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_frequencies: Vec<f32>,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub link_channels: bool,
    pub mix: f32,
    #[serde(default)]
    pub per_band_lookahead_ms: f32,
    #[serde(default)]
    pub ms_mode: bool,
    pub bands: Vec<BandCompressorParams>,
    #[serde(default)]
    pub sidechain_tilt_db: f32,
    #[serde(default = "default_link_amount")]
    pub link_amount: f32,
}

fn default_link_amount() -> f32 {
    1.0
}

#[derive(Debug, Clone)]
pub struct MultibandCompressorData {
    /// Gain reduction per band and per channel (flattened: [band0_ch0, band0_ch1, ..., band1_ch0, ...])
    pub gain_reduction_db: Arc<Vec<f32>>,
    pub band_levels_db: Arc<Vec<f32>>,
    pub crossover_frequencies: Arc<Vec<f32>>,
}

impl Default for MultibandCompressorData {
    fn default() -> Self {
        Self {
            gain_reduction_db: Arc::new(Vec::new()),
            band_levels_db: Arc::new(Vec::new()),
            crossover_frequencies: Arc::new(Vec::new()),
        }
    }
}

impl MultibandCompressorData {
    pub fn new(num_bands: usize, channels: usize) -> Self {
        Self {
            gain_reduction_db: Arc::new(vec![0.0; num_bands * channels]),
            band_levels_db: Arc::new(vec![-120.0; num_bands]),
            crossover_frequencies: Arc::new(vec![0.0; num_bands.saturating_sub(1)]),
        }
    }

    pub fn update(&mut self, gains: &[f32], levels: &[f32], xovers: &[f32]) {
        if let Some(mut_gains) = Arc::get_mut(&mut self.gain_reduction_db)
            && mut_gains.len() == gains.len()
        {
            mut_gains.copy_from_slice(gains);
        }
        if let Some(mut_levels) = Arc::get_mut(&mut self.band_levels_db)
            && mut_levels.len() == levels.len()
        {
            mut_levels.copy_from_slice(levels);
        }
        if let Some(mut_xovers) = Arc::get_mut(&mut self.crossover_frequencies)
            && mut_xovers.len() == xovers.len()
        {
            mut_xovers.copy_from_slice(xovers);
        }
    }
}

struct BandCompressor {
    envelope: Vec<f32>,
    attack_coeff: f32,
    release_coeff: f32,
}

pub struct MultibandCompressorPlugin {
    channels: usize,
    sample_rate: u32,
    num_bands: usize,
    _crossover_preset: i32,
    crossover_frequencies: Vec<f32>,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    link_channels: bool,
    mix: f32,
    per_band_lookahead_ms: f32,
    ms_mode: bool,
    sidechain_tilt_db: f32,
    link_amount: f32,
    /// Per-band, per-channel high-shelf biquad for sidechain tilt.
    /// Layout: [band][channel]. Empty when tilt_db ≈ 0.
    sidechain_tilt_biquads: Vec<Vec<Biquad>>,
    band_params: Vec<BandCompressorParams>,
    crossover_points: Vec<Lr4Crossover<f32>>,
    band_compressors: Vec<BandCompressor>,
    band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>,
    dry_buffer: Vec<f32>,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    xover_smoothers: Vec<LogSmoother>,

    /// Per-band lookahead delay buffers (one per band, each with `channels` interleaved).
    lookahead_buffers: Vec<LookaheadBuffer>,
    /// Per-band measured auto-makeup gain trackers.
    measured_makeups: Vec<MeasuredMakeup>,
    /// Temporary frame buffer for lookahead processing.
    lookahead_frame_tmp: Vec<f32>,

    // Internal flattened monitoring buffer
    gain_reduction_flattened: Vec<f32>,
    cache: RealTimeCache<MultibandCompressorData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

impl MultibandCompressorPlugin {
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, Default::default())
    }
    pub fn with_params(channels: usize, params: MultibandCompressorPluginParams) -> Self {
        let nb = params.num_bands.clamp(
            pk(MC, "num_bands").min_f64() as usize,
            pk(MC, "num_bands").max_f64() as usize,
        );
        let sr = 44100;
        let mut xfs = params.crossover_frequencies.clone();
        while xfs.len() < 4 {
            xfs.push(1000.0);
        }
        let mut bcomps = Vec::with_capacity(nb);
        for _ in 0..nb {
            bcomps.push(BandCompressor {
                envelope: vec![0.0; channels],
                attack_coeff: 0.0,
                release_coeff: 0.0,
            });
        }

        let mut band_params = params.bands;
        while band_params.len() < nb {
            band_params.push(BandCompressorParams::default());
        }

        let la_ms = params.per_band_lookahead_ms.clamp(0.0, 10.0);
        let lookahead_buffers = (0..nb)
            .map(|_| {
                if la_ms > 0.0 {
                    LookaheadBuffer::from_ms(la_ms, sr, channels)
                } else {
                    LookaheadBuffer::new(1, channels)
                }
            })
            .collect();
        let measured_makeups = (0..nb).map(|_| MeasuredMakeup::new(1000.0, sr)).collect();

        let mut p = Self {
            channels,
            sample_rate: sr,
            num_bands: nb,
            _crossover_preset: params.crossover_preset,
            crossover_frequencies: xfs.clone(),
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            link_channels: params.link_channels,
            mix: params.mix,
            per_band_lookahead_ms: la_ms,
            ms_mode: params.ms_mode,
            sidechain_tilt_db: params.sidechain_tilt_db,
            link_amount: params.link_amount.clamp(0.0, 1.0),
            sidechain_tilt_biquads: Vec::new(),
            band_params,
            crossover_points: Vec::new(),
            band_compressors: bcomps,
            band_buffers: Vec::new(),
            band_levels_db: vec![0.0; nb],
            dry_buffer: Vec::new(),
            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sr),
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            xover_smoothers: xfs.iter().map(|&f| LogSmoother::new(f, 50.0, sr)).collect(),
            lookahead_buffers,
            measured_makeups,
            lookahead_frame_tmp: vec![0.0; channels],
            gain_reduction_flattened: vec![0.0; nb * channels],
            cache: RealTimeCache::new(MultibandCompressorData::new(nb, channels)),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.build_crossovers();
        p.update_coefficients();
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at GLOBAL_PARAMS index.
    /// Order must match params::GLOBAL_PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.num_bands as f64),                       // num_bands
            1 => Some(self._crossover_preset as f64),               // crossover_preset
            2 => Some(self.crossover_frequencies[0] as f64),        // crossover_freq_1
            3 => Some(self.crossover_frequencies[1] as f64),        // crossover_freq_2
            4 => Some(self.crossover_frequencies[2] as f64),        // crossover_freq_3
            5 => Some(self.crossover_frequencies[3] as f64),        // crossover_freq_4
            6 => Some(self.threshold_db as f64),                    // threshold
            7 => Some(self.ratio as f64),                           // ratio
            8 => Some(self.attack_ms as f64),                       // attack
            9 => Some(self.release_ms as f64),                      // release
            10 => Some(self.knee_db as f64),                        // knee
            11 => Some(self.mix as f64),                            // mix
            12 => Some(if self.link_channels { 1.0 } else { 0.0 }), // link_channels
            13 => Some(self.per_band_lookahead_ms as f64),          // per_band_lookahead_ms
            14 => Some(if self.ms_mode { 1.0 } else { 0.0 }),       // ms_mode
            15 => Some(self.sidechain_tilt_db as f64),              // sidechain_tilt_db
            16 => Some(self.link_amount as f64),                    // link_amount
            _ => None,
        }
    }

    /// Set the f64 value of parameter at GLOBAL_PARAMS index.
    /// Order must match params::GLOBAL_PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.num_bands = value as usize,              // num_bands
            1 => self._crossover_preset = value as i32,        // crossover_preset
            2 => self.crossover_frequencies[0] = value as f32, // crossover_freq_1
            3 => self.crossover_frequencies[1] = value as f32, // crossover_freq_2
            4 => self.crossover_frequencies[2] = value as f32, // crossover_freq_3
            5 => self.crossover_frequencies[3] = value as f32, // crossover_freq_4
            6 => self.threshold_db = value as f32,             // threshold
            7 => self.ratio = value as f32,                    // ratio
            8 => self.attack_ms = value as f32,                // attack
            9 => self.release_ms = value as f32,               // release
            10 => self.knee_db = value as f32,                 // knee
            11 => self.mix = value as f32,                     // mix
            12 => self.link_channels = value > 0.5,            // link_channels
            13 => self.per_band_lookahead_ms = value as f32,   // per_band_lookahead_ms
            14 => self.ms_mode = value > 0.5,                  // ms_mode
            15 => self.sidechain_tilt_db = value as f32,       // sidechain_tilt_db
            16 => self.link_amount = value as f32,             // link_amount
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = param_bridge::build_parameters(MC, |i| self.param_value(i));

        // Per-band dynamics (not covered by GLOBAL_PARAMS)
        for i in 0..self.num_bands {
            let group = format!("Band {}", i + 1);
            let bp = &self.band_params[i];

            params.push(
                Parameter::new_float(
                    &format!("band_{}_threshold", i),
                    "Threshold",
                    bp.threshold_db.unwrap_or(self.threshold_db),
                    pk(MCB, "threshold").min_f64() as f32,
                    pk(MCB, "threshold").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_ratio", i),
                    "Ratio",
                    bp.ratio.unwrap_or(self.ratio),
                    pk(MCB, "ratio").min_f64() as f32,
                    pk(MCB, "ratio").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_attack", i),
                    "Attack",
                    bp.attack_ms.unwrap_or(self.attack_ms),
                    pk(MCB, "attack").min_f64() as f32,
                    pk(MCB, "attack").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_release", i),
                    "Release",
                    bp.release_ms.unwrap_or(self.release_ms),
                    pk(MCB, "release").min_f64() as f32,
                    pk(MCB, "release").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_makeup", i),
                    "Makeup (dB)",
                    bp.makeup_gain_db,
                    -24.0,
                    24.0,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(
                    &format!("band_{}_auto_makeup", i),
                    "Auto Makeup",
                    bp.auto_makeup,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(
                    &format!("band_{}_measured_auto_makeup", i),
                    "Measured Auto Makeup",
                    bp.measured_auto_makeup,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_active", i), "Active", bp.active)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_solo", i), "Solo", bp.solo)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_bypass", i), "Bypass", bp.bypass)
                    .with_group(&group),
            );
        }

        self.cached_parameters = params;
    }
    pub fn from_params(channels: usize, params: MultibandCompressorPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    fn rebuild_sidechain_tilt(&mut self) {
        if self.sidechain_tilt_db.abs() < 0.01 || self.sample_rate == 0 {
            self.sidechain_tilt_biquads.clear();
            return;
        }
        // Create per-band, per-channel tilt biquads so each band has independent state
        self.sidechain_tilt_biquads = (0..self.num_bands)
            .map(|_| {
                (0..self.channels)
                    .map(|_| {
                        Biquad::new(
                            BiquadFilterType::Highshelf,
                            1000.0,
                            self.sample_rate as f64,
                            0.707,
                            self.sidechain_tilt_db as f64,
                        )
                    })
                    .collect()
            })
            .collect();
    }

    fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        for i in 0..(self.num_bands - 1) {
            let f = self.xover_smoothers[i].target();
            self.crossover_points.push(Lr4Crossover::new(
                f,
                self.sample_rate as f32,
                self.channels,
            ));
        }
    }

    fn update_coefficients(&mut self) {
        for (i, b) in self.band_compressors.iter_mut().enumerate() {
            let a = self
                .band_params
                .get(i)
                .and_then(|p| p.attack_ms)
                .unwrap_or(self.attack_ms);
            let r = self
                .band_params
                .get(i)
                .and_then(|p| p.release_ms)
                .unwrap_or(self.release_ms);
            b.attack_coeff = (-1.0 / (a * 0.001 * self.sample_rate as f32)).exp();
            b.release_coeff = (-1.0 / (r * 0.001 * self.sample_rate as f32)).exp();
        }
    }

    fn calculate_gain_reduction(idb: f32, th: f32, ratio: f32, knee: f32) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);
        if knee < 0.1 {
            if idb <= th { 0.0 } else { (idb - th) * slope }
        } else if idb < th - knee / 2.0 {
            0.0
        } else if idb > th + knee / 2.0 {
            (idb - th) * slope
        } else {
            let ov = idb - th + knee / 2.0;
            let kf = ov / knee;
            kf * kf * (knee / 2.0) * slope
        }
    }
}

impl InPlacePlugin for MultibandCompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Multiband Compressor", "2.0.0", "Sotf")
            .with_description("Phase-coherent multiband dynamics processor")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Try global params first via param_bridge
        if let Ok(idx) =
            param_bridge::set_parameter(MC, &id, &value, |i, v| self.set_param_value(i, v))
        {
            // Side effects for specific global params
            match idx {
                0 => {
                    // num_bands changed
                    let nb = self.num_bands;
                    self.build_crossovers();
                    while self.band_params.len() < nb {
                        self.band_params.push(BandCompressorParams::default());
                    }
                    while self.band_compressors.len() < nb {
                        self.band_compressors.push(BandCompressor {
                            envelope: vec![0.0; self.channels],
                            attack_coeff: 0.0,
                            release_coeff: 0.0,
                        });
                    }
                    while self.lookahead_buffers.len() < nb {
                        self.lookahead_buffers
                            .push(if self.per_band_lookahead_ms > 0.0 {
                                LookaheadBuffer::from_ms(
                                    self.per_band_lookahead_ms,
                                    self.sample_rate,
                                    self.channels,
                                )
                            } else {
                                LookaheadBuffer::new(1, self.channels)
                            });
                    }
                    while self.measured_makeups.len() < nb {
                        self.measured_makeups
                            .push(MeasuredMakeup::new(1000.0, self.sample_rate));
                    }
                    self.band_levels_db.resize(nb, -100.0);
                    self.gain_reduction_flattened
                        .resize(nb * self.channels, 0.0);
                    self.update_coefficients();
                }
                2..=5 => {
                    // crossover_freq_1..4 changed
                    let xover_idx = idx - 2;
                    if xover_idx < self.xover_smoothers.len() {
                        self.xover_smoothers[xover_idx]
                            .set_target(self.crossover_frequencies[xover_idx]);
                    }
                }
                6 => {
                    // threshold changed
                    self.threshold_smoother.set_target(self.threshold_db);
                }
                8 | 9 => {
                    // attack or release changed
                    self.update_coefficients();
                }
                11 => {
                    // mix changed
                    self.mix_smoother.set_target(self.mix);
                }
                13 => {
                    // per_band_lookahead_ms changed
                    for buf in &mut self.lookahead_buffers {
                        if self.per_band_lookahead_ms > 0.0 {
                            buf.set_delay_ms(self.per_band_lookahead_ms, self.sample_rate);
                        }
                    }
                }
                15 => {
                    // sidechain_tilt_db changed
                    self.rebuild_sidechain_tilt();
                }
                _ => {}
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        // Fall through to band-level param handling
        let name = &id.0;
        if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1]
                    .parse::<usize>()
                    .map_err(|e| format!("Invalid band index: {}", e))?;
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &mut self.band_params[b_idx];
                    match field {
                        "threshold" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.threshold_db = Some(v);
                            }
                        }
                        "ratio" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.ratio = Some(v);
                            }
                        }
                        "attack" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.attack_ms = Some(v);
                                self.update_coefficients();
                            }
                        }
                        "release" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.release_ms = Some(v);
                                self.update_coefficients();
                            }
                        }
                        "makeup" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.makeup_gain_db = v;
                            }
                        }
                        "auto" => {
                            bp.auto_makeup = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "measured" => {
                            bp.measured_auto_makeup = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "active" => {
                            bp.active = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "solo" => {
                            bp.solo = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "bypass" => {
                            bp.bypass = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        _ => return Err(format!("Unknown band field: {}", field)),
                    }
                } else {
                    return Err(format!("Band index {} out of range", b_idx));
                }
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Try global params first
        if let Some(v) = param_bridge::get_parameter(MC, id, |i| self.param_value(i)) {
            return Some(v);
        }
        // Fall through to band-level params
        let name = &id.0;
        if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &self.band_params[b_idx];
                    match field {
                        "threshold" => Some(ParameterValue::Float(
                            bp.threshold_db.unwrap_or(self.threshold_db),
                        )),
                        "ratio" => Some(ParameterValue::Float(bp.ratio.unwrap_or(self.ratio))),
                        "attack" => Some(ParameterValue::Float(
                            bp.attack_ms.unwrap_or(self.attack_ms),
                        )),
                        "release" => Some(ParameterValue::Float(
                            bp.release_ms.unwrap_or(self.release_ms),
                        )),
                        "makeup" => Some(ParameterValue::Float(bp.makeup_gain_db)),
                        "auto" => Some(ParameterValue::Bool(bp.auto_makeup)),
                        "measured" => Some(ParameterValue::Bool(bp.measured_auto_makeup)),
                        "active" => Some(ParameterValue::Bool(bp.active)),
                        "solo" => Some(ParameterValue::Bool(bp.solo)),
                        "bypass" => Some(ParameterValue::Bool(bp.bypass)),
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
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.build_crossovers();
        self.update_coefficients();
        self.rebuild_sidechain_tilt();
        self.threshold_smoother.set_time(20.0, sr);
        self.mix_smoother.set_time(20.0, sr);
        for s in &mut self.xover_smoothers {
            *s = LogSmoother::new(s.target(), 50.0, sr);
        }

        // Reinitialize lookahead buffers for new sample rate
        let la_ms = self.per_band_lookahead_ms;
        for buf in &mut self.lookahead_buffers {
            if la_ms > 0.0 {
                let max_samples = (10.0 * 0.001 * sr as f32).round() as usize;
                buf.resize(max_samples, self.channels);
                buf.set_delay_ms(la_ms, sr);
            }
        }
        // Reinitialize measured makeup smoothing for new sample rate
        for mm in &mut self.measured_makeups {
            mm.set_smoothing(1000.0, sr);
        }

        // Pre-allocate buffers for real-time safety
        let max_frames = 4096;
        let stride = max_frames * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(max_frames * self.channels, 0.0);
        self.lookahead_frame_tmp.resize(self.channels, 0.0);

        Ok(())
    }
    fn reset(&mut self) {
        for x in &mut self.crossover_points {
            x.reset();
        }
        for b in &mut self.band_compressors {
            b.envelope.fill(0.0);
        }
        for buf in &mut self.lookahead_buffers {
            buf.reset();
        }
        for mm in &mut self.measured_makeups {
            mm.reset();
        }
        self.band_buffers.fill(0.0);
        self.dry_buffer.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let stride = nf * self.channels;

        // Safety guard: resize if host sends larger blocks than initialize() pre-allocated (4096 frames).
        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        if self.band_buffers.len() < self.num_bands * stride {
            self.band_buffers.resize(self.num_bands * stride, 0.0);
        }

        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // M/S encode: convert L/R to Mid/Side before band splitting
        let use_ms = self.ms_mode && self.channels == 2;
        if use_ms {
            for frame in 0..nf {
                let idx = frame * 2;
                let l = buffer[idx];
                let r = buffer[idx + 1];
                buffer[idx] = (l + r) * 0.5; // Mid
                buffer[idx + 1] = (l - r) * 0.5; // Side
            }
        }

        for i in 0..(self.num_bands - 1) {
            let freq = self.xover_smoothers[i].next_n(nf);
            self.crossover_points[i].set_frequency(freq);
        }

        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut rem = buffer[idx];
                for xidx in 0..(self.num_bands - 1) {
                    let (low, high) = self.crossover_points[xidx].process(rem, ch);
                    self.band_buffers[xidx * stride + idx] = low;
                    rem = high;
                }
                self.band_buffers[(self.num_bands - 1) * stride + idx] = rem;
            }
        }

        let g_th = self.threshold_smoother.next_n(nf);
        let g_mix = self.mix_smoother.next_n(nf);

        let mut any_solo = false;
        for b in 0..self.num_bands {
            if let Some(p) = self.band_params.get(b)
                && p.solo
            {
                any_solo = true;
                break;
            }
        }

        for b in 0..self.num_bands {
            let bp = self.band_params.get(b);
            let is_bypassed = bp.map(|p| p.bypass).unwrap_or(false);
            let is_passive = !bp.map(|p| p.active).unwrap_or(true);
            let is_muted = any_solo && !bp.map(|p| p.solo).unwrap_or(false);

            if is_muted {
                let off = b * stride;
                self.band_buffers[off..off + stride].fill(0.0);
                self.band_levels_db[b] = -100.0;
                continue;
            }

            if is_bypassed || is_passive {
                let off = b * stride;
                let mut max_abs = 0.0f32;
                for i in 0..stride {
                    max_abs = max_abs.max(self.band_buffers[off + i].abs());
                }
                self.band_levels_db[b] = 20.0 * fast_log10(max_abs.max(1e-10));
                continue;
            }

            let th = bp.and_then(|p| p.threshold_db).unwrap_or(g_th);
            let rat = bp.and_then(|p| p.ratio).unwrap_or(self.ratio);
            let kn = bp.and_then(|p| p.knee_db).unwrap_or(self.knee_db);
            let use_measured_makeup = bp.map(|p| p.measured_auto_makeup).unwrap_or(false);
            let mk = if use_measured_makeup {
                // Measured makeup: will be computed per-frame below
                1.0
            } else if bp.map(|p| p.auto_makeup).unwrap_or(false) {
                let ratio = rat.max(1.0);
                let slope = 1.0 - 1.0 / ratio;
                let overshoot = (-th).max(0.0) * 0.5;
                fast_pow10((overshoot * slope) / 20.0)
            } else {
                fast_pow10(bp.map(|p| p.makeup_gain_db).unwrap_or(0.0) / 20.0)
            };

            let use_lookahead = self.per_band_lookahead_ms > 0.0;
            let bcomp = &mut self.band_compressors[b];
            let off = b * stride;
            let mut band_max_abs = 0.0f32;

            // Use link_amount for continuous blending (backward compat: link_channels overrides)
            let link = if self.link_channels {
                1.0f32
            } else {
                self.link_amount
            };

            for frame in 0..nf {
                // Detect max-of-channels level for linked detection
                // Apply per-band sidechain tilt filter if configured
                let has_tilt = b < self.sidechain_tilt_biquads.len();
                let mut max_det = 0.0f32;
                if use_ms {
                    let raw = self.band_buffers[off + frame * self.channels];
                    let filtered = if has_tilt {
                        self.sidechain_tilt_biquads[b][0].process(raw as f64) as f32
                    } else {
                        raw
                    };
                    max_det = filtered.abs();
                } else {
                    for ch in 0..self.channels {
                        let raw = self.band_buffers[off + frame * self.channels + ch];
                        let filtered = if has_tilt && ch < self.sidechain_tilt_biquads[b].len() {
                            self.sidechain_tilt_biquads[b][ch].process(raw as f64) as f32
                        } else {
                            raw
                        };
                        max_det = max_det.max(filtered.abs());
                    }
                }
                let max_idb = 20.0 * fast_log10(max_det.max(1e-10));

                // Apply lookahead delay: push current frame, get delayed frame
                if use_lookahead {
                    let f_off = off + frame * self.channels;
                    let input = &self.band_buffers[f_off..f_off + self.channels];
                    // Copy input into temp since process_frame borrows both
                    self.lookahead_frame_tmp.copy_from_slice(input);
                    self.lookahead_buffers[b].process_frame(
                        &self.lookahead_frame_tmp,
                        &mut self.band_buffers[f_off..f_off + self.channels],
                    );
                }

                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let detect_raw = if use_lookahead {
                        self.lookahead_frame_tmp[ch]
                    } else {
                        self.band_buffers[idx]
                    };
                    // Apply tilt filter to per-channel detection (reuses same biquads — OK for detection)
                    let detect_abs = detect_raw.abs();
                    band_max_abs = band_max_abs.max(self.band_buffers[idx].abs());

                    // Blend per-channel and linked detection using link_amount
                    let per_ch_idb = 20.0 * fast_log10(detect_abs.max(1e-10));
                    let idb = if link >= 1.0 {
                        max_idb
                    } else if link <= 0.0 {
                        per_ch_idb
                    } else {
                        per_ch_idb * (1.0 - link) + max_idb * link
                    };
                    let tgr = Self::calculate_gain_reduction(idb, th, rat, kn);

                    let c = if tgr > bcomp.envelope[ch] {
                        bcomp.attack_coeff
                    } else {
                        bcomp.release_coeff
                    };
                    bcomp.envelope[ch] = tgr + c * (bcomp.envelope[ch] - tgr);

                    // Update measured makeup tracker if enabled
                    if use_measured_makeup {
                        self.measured_makeups[b].update(bcomp.envelope[ch]);
                    }

                    let gain_linear = fast_pow10(-bcomp.envelope[ch] / 20.0);
                    let makeup = if use_measured_makeup {
                        self.measured_makeups[b].makeup_linear()
                    } else {
                        mk
                    };
                    self.band_buffers[idx] *= gain_linear * makeup;
                }
            }
            self.band_levels_db[b] = 20.0 * fast_log10(band_max_abs.max(1e-10));
        }

        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut s = 0.0f32;
                for b in 0..self.num_bands {
                    s += self.band_buffers[b * stride + idx];
                }
                buffer[idx] = self.dry_buffer[idx] * (1.0 - g_mix) + s * g_mix;
            }
        }

        // M/S decode: convert Mid/Side back to L/R
        if use_ms {
            for frame in 0..nf {
                let idx = frame * 2;
                let m = buffer[idx];
                let s = buffer[idx + 1];
                buffer[idx] = m + s; // L = M + S
                buffer[idx + 1] = m - s; // R = M - S
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            for b in 0..self.num_bands {
                for ch in 0..self.channels {
                    self.gain_reduction_flattened[b * self.channels + ch] =
                        self.band_compressors[b].envelope[ch];
                }
            }
            let levels = &self.band_levels_db;
            let xovers = &self.crossover_frequencies;
            self.cache.update(|d| {
                d.update(&self.gain_reduction_flattened, levels, xovers);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn test_mb_comp_basic() {
        let mut p = MultibandCompressorPlugin::new(1);
        p.initialize(48000).unwrap();
        let mut b = vec![0.5; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999].is_finite());
    }

    /// Unity passthrough: with ratio 1:1 on all bands (no compression),
    /// the crossover should reconstruct the signal without significant loss.
    #[test]
    fn test_mb_comp_crossover_reconstruction() {
        let mut params = MultibandCompressorPluginParams {
            num_bands: 3,
            ..Default::default()
        };
        for band in &mut params.bands {
            band.ratio = Some(1.0); // no compression
        }
        let mut p = MultibandCompressorPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Generate test signal (broadband)
        let nf = 4800;
        let mut input = Vec::with_capacity(nf);
        for i in 0..nf {
            input.push(0.3 * (i as f32 * 0.1).sin() + 0.1 * (i as f32 * 0.5).sin());
        }
        let mut output = input.clone();
        p.process_in_place(
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: nf,
            },
        )
        .unwrap();

        // After crossover filter settling, RMS should be close to input
        let half = nf / 2;
        let rms_in: f32 =
            (input[half..].iter().map(|s| s * s).sum::<f32>() / (nf - half) as f32).sqrt();
        let rms_out: f32 =
            (output[half..].iter().map(|s| s * s).sum::<f32>() / (nf - half) as f32).sqrt();
        let ratio = rms_out / rms_in;
        assert!(
            (0.85..1.15).contains(&ratio),
            "With ratio=1 (no compression), crossover should reconstruct signal. \
             RMS ratio={ratio:.3} (in={rms_in:.4}, out={rms_out:.4})"
        );
    }

    /// Changing a crossover frequency mid-stream should not produce clicks or
    /// non-finite values. The output must remain bounded.
    #[test]
    fn test_crossover_frequency_change_no_discontinuity() {
        let mut p = MultibandCompressorPlugin::new(1);
        p.initialize(48000).unwrap();

        let nf = 2400;
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: nf,
        };

        // Process a block with default crossover
        let mut b1: Vec<f32> = (0..nf).map(|i| 0.3 * (i as f32 * 0.1).sin()).collect();
        p.process_in_place(&mut b1, &ctx).unwrap();
        let last_before = b1[nf - 1];

        // Change crossover frequency mid-stream
        p.set_parameter(
            ParameterId::from("crossover_freq_1"),
            ParameterValue::Float(800.0),
        )
        .unwrap();

        // Process another block
        let mut b2: Vec<f32> = (0..nf)
            .map(|i| 0.3 * ((nf + i) as f32 * 0.1).sin())
            .collect();
        p.process_in_place(&mut b2, &ctx).unwrap();

        // All output must be finite and bounded
        for (i, &s) in b2.iter().enumerate() {
            assert!(
                s.is_finite() && s.abs() < 10.0,
                "Sample {} after crossover change is non-finite or unbounded: {}",
                i,
                s
            );
        }

        // The transition should not produce a large jump
        let jump = (b2[0] - last_before).abs();
        assert!(
            jump < 1.0,
            "Crossover frequency change caused discontinuity: jump={jump:.4}"
        );
    }

    /// Verify compression actually reduces loud signals.
    #[test]
    fn test_mb_comp_reduces_loud_signal() {
        let mut p = MultibandCompressorPlugin::new(1);
        p.initialize(48000).unwrap();

        // Set low threshold to ensure compression, and mix=1.0 for wet-only
        p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
            .unwrap();
        p.set_parameter(ParameterId::from("ratio"), ParameterValue::Float(8.0))
            .unwrap();
        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
            .unwrap();

        // Process enough to let smoothers settle (200ms)
        let nf = 9600;
        let input_val = 0.5f32; // -6 dBFS
        let mut b = vec![input_val; nf];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: nf,
            },
        )
        .unwrap();

        // After settling, output should be quieter than input
        let rms_out: f32 =
            (b[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        assert!(
            rms_out < input_val * 0.9,
            "Multiband compressor should reduce loud signal, but RMS {rms_out:.4} ≈ input {input_val}"
        );
    }
}
