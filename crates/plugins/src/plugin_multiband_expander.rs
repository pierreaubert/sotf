// ============================================================================
// Multiband Expander Plugin
// ============================================================================

use super::param_specs::multiband_expander::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use super::smoothing::{LogSmoother, Smoother};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0),
    (100.0, 3000.0, 8000.0, 12000.0),
    (250.0, 4000.0, 10000.0, 14000.0),
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BandExpanderParams {
    pub threshold_db: Option<f32>,
    pub ratio: Option<f32>,
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    pub knee_db: Option<f32>,
    pub range_db: Option<f32>,
    pub hysteresis_db: Option<f32>,
    pub hold_ms: Option<f32>,
    pub solo: bool,
    pub bypass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultibandExpanderPluginParams {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_frequencies: Vec<f32>,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub range_db: f32,
    pub hysteresis_db: f32,
    pub hold_ms: f32,
    pub link_channels: bool,
    pub mix: f32,
    pub bands: Vec<BandExpanderParams>,
}

pub struct MultibandExpanderData {
    pub attenuation_db: Vec<Vec<f32>>,
    pub is_open: Vec<bool>,
    pub band_levels_db: Vec<f32>,
    pub crossover_frequencies: Vec<f32>,
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Open,
    Hold,
    Closing,
}

struct BandExpander {
    envelope: Vec<f32>,
    gate_state: Vec<GateState>,
    hold_counter: Vec<usize>,
    attack_coeff: f32,
    release_coeff: f32,
}

impl BandExpander {
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
    }
}

pub struct MultibandExpanderPlugin {
    channels: usize,
    sample_rate: u32,
    num_bands: usize,
    crossover_preset: i32,
    crossover_frequencies: Vec<f32>,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    range_db: f32,
    hysteresis_db: f32,
    hold_ms: f32,
    link_channels: bool,
    mix: f32,
    band_params: Vec<BandExpanderParams>,
    crossover_points: Vec<CrossoverPoint>,
    band_expanders: Vec<BandExpander>,
    band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>,
    dry_buffer: Vec<f32>,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    xover_smoothers: Vec<LogSmoother>,
}

impl MultibandExpanderPlugin {
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, Default::default())
    }
    pub fn with_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        let nb = params.num_bands.clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);
        let sr = 44100;
        let mut xfs = params.crossover_frequencies.clone();
        while xfs.len() < 4 {
            xfs.push(1000.0);
        }
        let mut bexps = Vec::with_capacity(nb);
        for _ in 0..nb {
            bexps.push(BandExpander {
                envelope: vec![0.0; channels],
                gate_state: vec![GateState::Open; channels],
                hold_counter: vec![0; channels],
                attack_coeff: 0.0,
                release_coeff: 0.0,
            });
        }

        let mut band_params = params.bands;
        while band_params.len() < nb {
            band_params.push(BandExpanderParams::default());
        }

        let mut p = Self {
            channels,
            sample_rate: sr,
            num_bands: nb,
            crossover_preset: params.crossover_preset,
            crossover_frequencies: xfs.clone(),
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            range_db: params.range_db,
            hysteresis_db: params.hysteresis_db,
            hold_ms: params.hold_ms,
            link_channels: params.link_channels,
            mix: params.mix,
            band_params,
            crossover_points: Vec::new(),
            band_expanders: bexps,
            band_buffers: Vec::new(),
            band_levels_db: vec![0.0; nb],
            dry_buffer: Vec::new(),
            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sr),
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            xover_smoothers: xfs.iter().map(|&f| LogSmoother::new(f, 50.0, sr)).collect(),
        };
        p.build_crossovers();
        p.update_coefficients();
        p
    }
    pub fn from_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
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
        for (i, b) in self.band_expanders.iter_mut().enumerate() {
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

    fn calculate_expansion_attenuation(
        idb: f32,
        th: f32,
        ratio: f32,
        knee: f32,
        range: f32,
    ) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);
        let atten = if knee < 0.1 {
            if idb >= th { 0.0 } else { (th - idb) * slope }
        } else if idb > th + knee / 2.0 {
            0.0
        } else if idb < th - knee / 2.0 {
            (th - idb) * slope
        } else {
            let b = th + knee / 2.0 - idb;
            let kf = b / knee;
            kf * kf * (knee / 2.0) * slope
        };
        atten.min(range)
    }
}

impl InPlacePlugin for MultibandExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Multiband Expander", "1.1.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        let mut params = vec![
            Parameter::new_int("num_bands", "Bands", self.num_bands as i32, NUM_BANDS_MIN as i32, NUM_BANDS_MAX as i32)
                .with_group("General")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_group("General")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("mix", "Mix", self.mix, MIX_MIN, MIX_MAX)
                .with_group("General")
                .with_importance(ParameterImportance::Useful),
        ];

        // Crossover frequencies
        for i in 0..(self.num_bands - 1) {
            let name = format!("crossover_freq_{}", i + 1);
            let label = format!("X-Over {}", i + 1);
            params.push(Parameter::new_float(
                &name,
                &label,
                self.crossover_frequencies[i],
                20.0,
                20000.0,
            ).with_group("Crossover"));
        }

        // Global dynamics (defaults for bands)
        params.extend(vec![
            Parameter::new_float("threshold", "Threshold", self.threshold_db, THRESHOLD_MIN, THRESHOLD_MAX)
                .with_group("Global Dynamics"),
            Parameter::new_float("ratio", "Ratio", self.ratio, RATIO_MIN, RATIO_MAX)
                .with_group("Global Dynamics"),
            Parameter::new_float("attack", "Attack", self.attack_ms, ATTACK_MIN, ATTACK_MAX)
                .with_group("Global Dynamics"),
            Parameter::new_float("release", "Release", self.release_ms, RELEASE_MIN, RELEASE_MAX)
                .with_group("Global Dynamics"),
        ]);

        // Per-band dynamics
        for i in 0..self.num_bands {
            let group = format!("Band {}", i + 1);
            let bp = &self.band_params[i];
            
            params.push(Parameter::new_float(&format!("band_{}_threshold", i), "Threshold", bp.threshold_db.unwrap_or(self.threshold_db), THRESHOLD_MIN, THRESHOLD_MAX).with_group(&group));
            params.push(Parameter::new_float(&format!("band_{}_ratio", i), "Ratio", bp.ratio.unwrap_or(self.ratio), RATIO_MIN, RATIO_MAX).with_group(&group));
            params.push(Parameter::new_float(&format!("band_{}_attack", i), "Attack", bp.attack_ms.unwrap_or(self.attack_ms), ATTACK_MIN, ATTACK_MAX).with_group(&group));
            params.push(Parameter::new_float(&format!("band_{}_release", i), "Release", bp.release_ms.unwrap_or(self.release_ms), RELEASE_MIN, RELEASE_MAX).with_group(&group));
            params.push(Parameter::new_bool(&format!("band_{}_solo", i), "Solo", bp.solo).with_group(&group));
            params.push(Parameter::new_bool(&format!("band_{}_bypass", i), "Bypass", bp.bypass).with_group(&group));
        }

        params
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = &id.0;
        
        if name == "num_bands" {
            let nb = value.as_int().ok_or("val")? as usize;
            if nb != self.num_bands {
                self.num_bands = nb.clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);
                // Re-init happens in initialize() or build_crossovers()
                self.build_crossovers();
                // Ensure band_params has enough entries
                while self.band_params.len() < self.num_bands {
                    self.band_params.push(BandExpanderParams::default());
                }
            }
        } else if name == "link_channels" {
            self.link_channels = value.as_bool().ok_or("val")?;
        } else if name == "mix" {
            self.mix = value.as_float().ok_or("val")?.clamp(0.0, 1.0);
            self.mix_smoother.set_target(self.mix);
        } else if name.starts_with("crossover_freq_") {
            let idx = name.replace("crossover_freq_", "").parse::<usize>().map(|i| i - 1).unwrap_or(0);
            if idx < self.xover_smoothers.len() {
                let f = value.as_float().ok_or("val")?;
                self.crossover_frequencies[idx] = f;
                self.xover_smoothers[idx].set_target(f);
            }
        } else if name == "threshold" {
            self.threshold_db = value.as_float().ok_or("val")?;
            self.threshold_smoother.set_target(self.threshold_db);
        } else if name == "ratio" {
            self.ratio = value.as_float().ok_or("val")?.max(1.0);
        } else if name == "attack" {
            self.attack_ms = value.as_float().ok_or("val")?;
            self.update_coefficients();
        } else if name == "release" {
            self.release_ms = value.as_float().ok_or("val")?;
            self.update_coefficients();
        } else if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &mut self.band_params[b_idx];
                    match field {
                        "threshold" => bp.threshold_db = Some(value.as_float().ok_or("val")?),
                        "ratio" => bp.ratio = Some(value.as_float().ok_or("val")?),
                        "attack" => {
                            bp.attack_ms = Some(value.as_float().ok_or("val")?);
                            self.update_coefficients();
                        },
                        "release" => {
                            bp.release_ms = Some(value.as_float().ok_or("val")?);
                            self.update_coefficients();
                        },
                        "solo" => bp.solo = value.as_bool().ok_or("val")?,
                        "bypass" => bp.bypass = value.as_bool().ok_or("val")?,
                        _ => return Err(format!("Unknown band field: {}", field)),
                    }
                }
            }
        } else {
            // Support legacy names or other fields
            match name.as_str() {
                "knee" => self.knee_db = value.as_float().ok_or("val")?,
                "range" => self.range_db = value.as_float().ok_or("val")?,
                "hysteresis" => self.hysteresis_db = value.as_float().ok_or("val")?,
                "hold" => self.hold_ms = value.as_float().ok_or("val")?,
                _ => return Err(format!("Unknown parameter: {}", id)),
            }
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = &id.0;
        if name == "num_bands" { Some(ParameterValue::Int(self.num_bands as i32)) }
        else if name == "link_channels" { Some(ParameterValue::Bool(self.link_channels)) }
        else if name == "mix" { Some(ParameterValue::Float(self.mix)) }
        else if name == "threshold" { Some(ParameterValue::Float(self.threshold_db)) }
        else if name == "ratio" { Some(ParameterValue::Float(self.ratio)) }
        else if name == "attack" { Some(ParameterValue::Float(self.attack_ms)) }
        else if name == "release" { Some(ParameterValue::Float(self.release_ms)) }
        else if name.starts_with("crossover_freq_") {
            let idx = name.replace("crossover_freq_", "").parse::<usize>().map(|i| i - 1).unwrap_or(0);
            self.crossover_frequencies.get(idx).map(|&f| ParameterValue::Float(f))
        } else if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &self.band_params[b_idx];
                    match field {
                        "threshold" => Some(ParameterValue::Float(bp.threshold_db.unwrap_or(self.threshold_db))),
                        "ratio" => Some(ParameterValue::Float(bp.ratio.unwrap_or(self.ratio))),
                        "attack" => Some(ParameterValue::Float(bp.attack_ms.unwrap_or(self.attack_ms))),
                        "release" => Some(ParameterValue::Float(bp.release_ms.unwrap_or(self.release_ms))),
                        "solo" => Some(ParameterValue::Bool(bp.solo)),
                        "bypass" => Some(ParameterValue::Bool(bp.bypass)),
                        _ => None,
                    }
                } else { None }
            } else { None }
        } else {
            match name.as_str() {
                "knee" => Some(ParameterValue::Float(self.knee_db)),
                "range" => Some(ParameterValue::Float(self.range_db)),
                "hysteresis" => Some(ParameterValue::Float(self.hysteresis_db)),
                "hold" => Some(ParameterValue::Float(self.hold_ms)),
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
        let max_frames = 4096; // Standard max block size
        let stride = max_frames * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(max_frames * self.channels, 0.0);
        
        Ok(())
    }
    fn reset(&mut self) {
        for b in &mut self.band_expanders {
            b.reset();
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
        
        // Ensure buffers are large enough (usually a no-op due to initialize)
        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        if self.band_buffers.len() < self.num_bands * stride {
            self.band_buffers.resize(self.num_bands * stride, 0.0);
        }
        
        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // 1. Update crossovers
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

        // 2. Perform Crossover Splitting
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
            if let Some(p) = self.band_params.get(b) {
                if p.solo { any_solo = true; break; }
            }
        }

        // 3. Dynamic Processing per Band
        for b in 0..self.num_bands {
            let bp = self.band_params.get(b);
            let is_bypassed = bp.map(|p| p.bypass).unwrap_or(false);
            let is_muted = any_solo && !bp.map(|p| p.solo).unwrap_or(false);
            
            if is_muted {
                let off = b * stride;
                self.band_buffers[off..off + stride].fill(0.0);
                self.band_levels_db[b] = -100.0;
                continue;
            }
            
            if is_bypassed {
                // Keep band signal as is, but still track levels
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
            let rg = bp.and_then(|p| p.range_db).unwrap_or(self.range_db);
            let hys = bp.and_then(|p| p.hysteresis_db).unwrap_or(self.hysteresis_db);
            let hs = (bp.and_then(|p| p.hold_ms).unwrap_or(self.hold_ms) * 0.001 * self.sample_rate as f32) as usize;
            
            let bexp = &mut self.band_expanders[b];
            let off = b * stride;
            let mut band_max_abs = 0.0f32;

            for frame in 0..nf {
                let mut det = 0.0f32;
                if self.link_channels {
                    for ch in 0..self.channels {
                        det = det.max(self.band_buffers[off + frame * self.channels + ch].abs());
                    }
                }
                
                let idb = if self.link_channels {
                    20.0 * fast_log10(det.max(1e-10))
                } else {
                    0.0 // Computed per-channel below
                };

                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let sample_abs = self.band_buffers[idx].abs();
                    band_max_abs = band_max_abs.max(sample_abs);
                    
                    let db = if self.link_channels { idb } else { 20.0 * fast_log10(sample_abs.max(1e-10)) };

                    let target = match bexp.gate_state[ch] {
                        GateState::Open => {
                            if db < th {
                                bexp.gate_state[ch] = GateState::Hold;
                                bexp.hold_counter[ch] = hs;
                                0.0
                            } else { 0.0 }
                        }
                        GateState::Hold => {
                            if db >= th {
                                bexp.gate_state[ch] = GateState::Open;
                                0.0
                            } else if bexp.hold_counter[ch] > 0 {
                                bexp.hold_counter[ch] -= 1;
                                0.0
                            } else if db < th - hys {
                                bexp.gate_state[ch] = GateState::Closing;
                                Self::calculate_expansion_attenuation(db, th, rat, kn, rg)
                            } else { 0.0 }
                        }
                        GateState::Closing => {
                            if db >= th {
                                bexp.gate_state[ch] = GateState::Open;
                                0.0
                            } else {
                                Self::calculate_expansion_attenuation(db, th, rat, kn, rg)
                            }
                        }
                    };
                    
                    let c = if target > bexp.envelope[ch] {
                        bexp.release_coeff
                    } else {
                        bexp.attack_coeff
                    };
                    bexp.envelope[ch] = target + c * (bexp.envelope[ch] - target);
                    self.band_buffers[idx] *= fast_pow10(-bexp.envelope[ch] / 20.0);
                }
            }
            self.band_levels_db[b] = 20.0 * fast_log10(band_max_abs.max(1e-10));
        }

        // 4. Recombination
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
        
        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(Arc::new(MultibandExpanderData {
            attenuation_db: self
                .band_expanders
                .iter()
                .map(|b| b.envelope.clone())
                .collect(),
            is_open: self
                .band_expanders
                .iter()
                .map(|b| b.gate_state.iter().any(|&s| s != GateState::Closing))
                .collect(),
            band_levels_db: self.band_levels_db.clone(),
            crossover_frequencies: self.crossover_frequencies.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mb_exp_basic() {
        let mut p = MultibandExpanderPlugin::new(1);
        p.initialize(48000).unwrap();
        let mut b = vec![0.1; 1000];
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
}
