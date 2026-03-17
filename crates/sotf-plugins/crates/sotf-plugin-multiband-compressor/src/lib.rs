// ============================================================================
// Multiband Compressor Plugin
// ============================================================================

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::{
    find_by_key as pk,
    multiband_compressor::{BAND_TEMPLATE as MCB, GLOBAL_PARAMS as MC},
};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LogSmoother, Smoother};
use std::any::Any;
use std::sync::Arc;

pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0),
    (100.0, 3000.0, 8000.0, 12000.0),
    (250.0, 4000.0, 10000.0, 14000.0),
];

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
            active: true,
            solo: false,
            bypass: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub bands: Vec<BandCompressorParams>,
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

struct CrossoverPoint {
    lowpass: Vec<Vec<Biquad>>,
    highpass: Vec<Vec<Biquad>>,
    freq: f32,
}

impl CrossoverPoint {
    fn new(channels: usize, freq: f32, sr: u32) -> Self {
        let q = 1.0 / std::f64::consts::SQRT_2;
        let mut lp = Vec::with_capacity(channels);
        let mut hp = Vec::with_capacity(channels);
        for _ in 0..channels {
            lp.push(vec![
                Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr as f64, q, 0.0),
                Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr as f64, q, 0.0),
            ]);
            hp.push(vec![
                Biquad::new(BiquadFilterType::Highpass, freq as f64, sr as f64, q, 0.0),
                Biquad::new(BiquadFilterType::Highpass, freq as f64, sr as f64, q, 0.0),
            ]);
        }
        Self {
            lowpass: lp,
            highpass: hp,
            freq,
        }
    }
    fn process_lowpass(&mut self, ch: usize, mut s: f32) -> f32 {
        for f in &mut self.lowpass[ch] {
            s = f.process(s as f64) as f32;
        }
        s
    }
    fn process_highpass(&mut self, ch: usize, mut s: f32) -> f32 {
        for f in &mut self.highpass[ch] {
            s = f.process(s as f64) as f32;
        }
        s
    }
    fn reset(&mut self, sr: u32) {
        let q = 1.0 / std::f64::consts::SQRT_2;
        for ch in 0..self.lowpass.len() {
            for f in &mut self.lowpass[ch] {
                *f = Biquad::new(
                    BiquadFilterType::Lowpass,
                    self.freq as f64,
                    sr as f64,
                    q,
                    0.0,
                );
            }
            for f in &mut self.highpass[ch] {
                *f = Biquad::new(
                    BiquadFilterType::Highpass,
                    self.freq as f64,
                    sr as f64,
                    q,
                    0.0,
                );
            }
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
    band_params: Vec<BandCompressorParams>,
    crossover_points: Vec<CrossoverPoint>,
    band_compressors: Vec<BandCompressor>,
    band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>,
    dry_buffer: Vec<f32>,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    xover_smoothers: Vec<LogSmoother>,

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
            band_params,
            crossover_points: Vec::new(),
            band_compressors: bcomps,
            band_buffers: Vec::new(),
            band_levels_db: vec![0.0; nb],
            dry_buffer: Vec::new(),
            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sr),
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            xover_smoothers: xfs.iter().map(|&f| LogSmoother::new(f, 50.0, sr)).collect(),
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

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_int(
                "num_bands",
                "Bands",
                self.num_bands as i32,
                pk(MC, "num_bands").min_f64() as i32,
                pk(MC, "num_bands").max_f64() as i32,
            )
            .with_group("General")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_group("General")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(MC, "mix").min_f64() as f32,
                pk(MC, "mix").max_f64() as f32,
            )
            .with_group("General")
            .with_importance(ParameterImportance::Useful),
        ];

        // Crossover frequencies
        for i in 0..(self.num_bands - 1) {
            let name = format!("crossover_freq_{}", i + 1);
            let label = format!("X-Over {}", i + 1);
            params.push(
                Parameter::new_float(&name, &label, self.crossover_frequencies[i], 20.0, 20000.0)
                    .with_group("Crossover"),
            );
        }

        // Global dynamics (defaults for bands)
        params.extend(vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(MC, "threshold").min_f64() as f32,
                pk(MC, "threshold").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(MC, "ratio").min_f64() as f32,
                pk(MC, "ratio").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(MC, "attack").min_f64() as f32,
                pk(MC, "attack").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(MC, "release").min_f64() as f32,
                pk(MC, "release").max_f64() as f32,
            )
            .with_group("Global Dynamics"),
        ]);

        // Per-band dynamics
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

    fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        for i in 0..(self.num_bands - 1) {
            let f = self.xover_smoothers[i].target();
            self.crossover_points
                .push(CrossoverPoint::new(self.channels, f, self.sample_rate));
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
        let name = &id.0;

        if name == "num_bands" {
            let nb = value
                .as_int()
                .ok_or_else(|| "Bands must be an integer".to_string())?
                as usize;
            let nb = nb.clamp(
                pk(MC, "num_bands").min_f64() as usize,
                pk(MC, "num_bands").max_f64() as usize,
            );
            if nb != self.num_bands {
                self.num_bands = nb;
                self.build_crossovers();
                while self.band_params.len() < self.num_bands {
                    self.band_params.push(BandCompressorParams::default());
                }
                while self.band_compressors.len() < self.num_bands {
                    self.band_compressors.push(BandCompressor {
                        envelope: vec![0.0; self.channels],
                        attack_coeff: 0.0,
                        release_coeff: 0.0,
                    });
                }
                self.band_levels_db.resize(self.num_bands, -100.0);
                self.gain_reduction_flattened
                    .resize(self.num_bands * self.channels, 0.0);
                self.update_coefficients();
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        self.validate_parameter(&id, &value)?;

        if name == "link_channels" {
            self.link_channels = value
                .as_bool()
                .ok_or_else(|| "link_channels must be a boolean".to_string())?;
        } else if name == "mix" {
            let v = value
                .as_float()
                .ok_or_else(|| "mix must be a float".to_string())?;
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if name.starts_with("crossover_freq_") {
            let idx = name
                .replace("crossover_freq_", "")
                .parse::<usize>()
                .map_err(|e| format!("Invalid crossover index: {}", e))?
                .checked_sub(1)
                .ok_or_else(|| "Crossover index must be at least 1".to_string())?;

            if idx < self.xover_smoothers.len() {
                let f = value
                    .as_float()
                    .ok_or_else(|| format!("{} must be a float", name))?;
                if f.is_finite() {
                    self.crossover_frequencies[idx] = f;
                    self.xover_smoothers[idx].set_target(f);
                }
            } else {
                return Err(format!("Crossover index {} out of range", idx + 1));
            }
        } else if name == "threshold" {
            let v = value
                .as_float()
                .ok_or_else(|| "threshold must be a float".to_string())?;
            if v.is_finite() {
                self.threshold_db = v;
                self.threshold_smoother.set_target(self.threshold_db);
            }
        } else if name == "ratio" {
            let v = value
                .as_float()
                .ok_or_else(|| "ratio must be a float".to_string())?;
            if v.is_finite() {
                self.ratio = v.max(1.0);
            }
        } else if name == "attack" {
            let v = value
                .as_float()
                .ok_or_else(|| "attack must be a float".to_string())?;
            if v.is_finite() {
                self.attack_ms = v;
                self.update_coefficients();
            }
        } else if name == "release" {
            let v = value
                .as_float()
                .ok_or_else(|| "release must be a float".to_string())?;
            if v.is_finite() {
                self.release_ms = v;
                self.update_coefficients();
            }
        } else if name.starts_with("band_") {
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
            match name.as_str() {
                "knee" => {
                    let v = value
                        .as_float()
                        .ok_or_else(|| "knee must be a float".to_string())?;
                    if v.is_finite() {
                        self.knee_db = v;
                    }
                }
                _ => return Err(format!("Unknown parameter: {}", id)),
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = &id.0;
        if name == "num_bands" {
            Some(ParameterValue::Int(self.num_bands as i32))
        } else if name == "link_channels" {
            Some(ParameterValue::Bool(self.link_channels))
        } else if name == "mix" {
            Some(ParameterValue::Float(self.mix))
        } else if name == "threshold" {
            Some(ParameterValue::Float(self.threshold_db))
        } else if name == "ratio" {
            Some(ParameterValue::Float(self.ratio))
        } else if name == "attack" {
            Some(ParameterValue::Float(self.attack_ms))
        } else if name == "release" {
            Some(ParameterValue::Float(self.release_ms))
        } else if name.starts_with("crossover_freq_") {
            let idx = name
                .replace("crossover_freq_", "")
                .parse::<usize>()
                .map(|i| i - 1)
                .unwrap_or(0);
            self.crossover_frequencies
                .get(idx)
                .map(|&f| ParameterValue::Float(f))
        } else if name.starts_with("band_") {
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
            match name.as_str() {
                "knee" => Some(ParameterValue::Float(self.knee_db)),
                _ => None,
            }
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.build_crossovers();
        self.update_coefficients();
        self.threshold_smoother.set_time(20.0, sr);
        self.mix_smoother.set_time(20.0, sr);
        for s in &mut self.xover_smoothers {
            *s = LogSmoother::new(s.target(), 50.0, sr);
        }

        // Pre-allocate buffers for real-time safety
        let max_frames = 4096;
        let stride = max_frames * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(max_frames * self.channels, 0.0);

        Ok(())
    }
    fn reset(&mut self) {
        for x in &mut self.crossover_points {
            x.reset(self.sample_rate);
        }
        for b in &mut self.band_compressors {
            b.envelope.fill(0.0);
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

        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        if self.band_buffers.len() < self.num_bands * stride {
            self.band_buffers.resize(self.num_bands * stride, 0.0);
        }

        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        for i in 0..(self.num_bands - 1) {
            let freq = self.xover_smoothers[i].next_n(nf);
            if (freq - self.crossover_points[i].freq).abs() > 1.0 {
                self.crossover_points[i].freq = freq;
                let q = 1.0 / std::f64::consts::SQRT_2;
                let sr = self.sample_rate as f64;
                for ch in 0..self.channels {
                    for f in &mut self.crossover_points[i].lowpass[ch] {
                        *f = Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr, q, 0.0);
                    }
                    for f in &mut self.crossover_points[i].highpass[ch] {
                        *f = Biquad::new(BiquadFilterType::Highpass, freq as f64, sr, q, 0.0);
                    }
                }
            }
        }

        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut rem = buffer[idx];
                for xidx in 0..(self.num_bands - 1) {
                    let low = self.crossover_points[xidx].process_lowpass(ch, rem);
                    self.band_buffers[xidx * stride + idx] = low;
                    rem = self.crossover_points[xidx].process_highpass(ch, rem);
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
            let mk = if bp.map(|p| p.auto_makeup).unwrap_or(false) {
                let ratio = rat.max(1.0);
                let slope = 1.0 - 1.0 / ratio;
                let overshoot = (-th).max(0.0) * 0.5;
                fast_pow10((overshoot * slope) / 20.0)
            } else {
                fast_pow10(bp.map(|p| p.makeup_gain_db).unwrap_or(0.0) / 20.0)
            };

            let bcomp = &mut self.band_compressors[b];
            let off = b * stride;
            let mut band_max_abs = 0.0f32;

            for frame in 0..nf {
                let mut det = 0.0f32;
                if self.link_channels {
                    for ch in 0..self.channels {
                        det = det.max(self.band_buffers[off + frame * self.channels + ch].abs());
                    }
                }

                let idb_shared = if self.link_channels {
                    20.0 * fast_log10(det.max(1e-10))
                } else {
                    0.0
                };

                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let sample_abs = self.band_buffers[idx].abs();
                    band_max_abs = band_max_abs.max(sample_abs);

                    let idb = if self.link_channels {
                        idb_shared
                    } else {
                        20.0 * fast_log10(sample_abs.max(1e-10))
                    };
                    let tgr = Self::calculate_gain_reduction(idb, th, rat, kn);

                    let c = if tgr > bcomp.envelope[ch] {
                        bcomp.attack_coeff
                    } else {
                        bcomp.release_coeff
                    };
                    bcomp.envelope[ch] = tgr + c * (bcomp.envelope[ch] - tgr);
                    self.band_buffers[idx] *= fast_pow10(-bcomp.envelope[ch] / 20.0) * mk;
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
            (0.7..1.3).contains(&ratio),
            "With ratio=1 (no compression), crossover should reconstruct signal. \
             RMS ratio={ratio:.3} (in={rms_in:.4}, out={rms_out:.4})"
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
