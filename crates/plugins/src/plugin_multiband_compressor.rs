// ============================================================================
// Multiband Compressor Plugin
// ============================================================================

use super::param_specs::multiband_compressor::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use super::smoothing::{Smoother, LogSmoother};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0), (100.0, 3000.0, 8000.0, 12000.0), (250.0, 4000.0, 10000.0, 14000.0),
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BandCompressorParams {
    pub threshold_db: Option<f32>, pub ratio: Option<f32>, pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>, pub knee_db: Option<f32>, pub makeup_gain_db: f32,
    pub solo: bool, pub bypass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultibandCompressorPluginParams {
    pub num_bands: usize, pub crossover_preset: i32, pub crossover_frequencies: Vec<f32>,
    pub threshold_db: f32, pub ratio: f32, pub attack_ms: f32, pub release_ms: f32,
    pub knee_db: f32, pub link_channels: bool, pub mix: f32, pub bands: Vec<BandCompressorParams>,
}

pub struct MultibandCompressorData {
    pub gain_reduction_db: Vec<Vec<f32>>, pub band_levels_db: Vec<f32>, pub crossover_frequencies: Vec<f32>,
}

struct CrossoverPoint {
    lowpass: Vec<Vec<Biquad>>, highpass: Vec<Vec<Biquad>>, freq: f32,
}

impl CrossoverPoint {
    fn new(channels: usize, freq: f32, sr: u32) -> Self {
        let q = 1.0 / std::f64::consts::SQRT_2;
        let mut lp = Vec::with_capacity(channels); let mut hp = Vec::with_capacity(channels);
        for _ in 0..channels {
            lp.push(vec![Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr as f64, q, 0.0),
                         Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr as f64, q, 0.0)]);
            hp.push(vec![Biquad::new(BiquadFilterType::Highpass, freq as f64, sr as f64, q, 0.0),
                         Biquad::new(BiquadFilterType::Highpass, freq as f64, sr as f64, q, 0.0)]);
        }
        Self { lowpass: lp, highpass: hp, freq }
    }
    fn process_lowpass(&mut self, ch: usize, mut s: f32) -> f32 {
        for f in &mut self.lowpass[ch] { s = f.process(s as f64) as f32; }
        s
    }
    fn process_highpass(&mut self, ch: usize, mut s: f32) -> f32 {
        for f in &mut self.highpass[ch] { s = f.process(s as f64) as f32; }
        s
    }
    fn reset(&mut self, sr: u32) {
        let q = 1.0 / std::f64::consts::SQRT_2;
        for ch in 0..self.lowpass.len() {
            for f in &mut self.lowpass[ch] { *f = Biquad::new(BiquadFilterType::Lowpass, self.freq as f64, sr as f64, q, 0.0); }
            for f in &mut self.highpass[ch] { *f = Biquad::new(BiquadFilterType::Highpass, self.freq as f64, sr as f64, q, 0.0); }
        }
    }
}

struct BandCompressor { envelope: Vec<f32>, attack_coeff: f32, release_coeff: f32 }

pub struct MultibandCompressorPlugin {
    channels: usize, sample_rate: u32, num_bands: usize, crossover_preset: i32,
    crossover_frequencies: Vec<f32>, threshold_db: f32, ratio: f32, attack_ms: f32,
    release_ms: f32, knee_db: f32, link_channels: bool, mix: f32,
    band_params: Vec<BandCompressorParams>, crossover_points: Vec<CrossoverPoint>,
    band_compressors: Vec<BandCompressor>, band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>, dry_buffer: Vec<f32>,
    threshold_smoother: Smoother, mix_smoother: Smoother,
    xover_smoothers: Vec<LogSmoother>,
}

impl MultibandCompressorPlugin {
    pub fn new(channels: usize) -> Self { Self::with_params(channels, Default::default()) }
    pub fn with_params(channels: usize, params: MultibandCompressorPluginParams) -> Self {
        let nb = params.num_bands.clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);
        let sr = 44100;
        let mut xfs = params.crossover_frequencies.clone();
        while xfs.len() < 4 { xfs.push(1000.0); }
        let mut bcomps = Vec::with_capacity(nb);
        for _ in 0..nb { bcomps.push(BandCompressor { envelope: vec![0.0; channels], attack_coeff: 0.0, release_coeff: 0.0 }); }
        
        let mut p = Self {
            channels, sample_rate: sr, num_bands: nb, crossover_preset: params.crossover_preset,
            crossover_frequencies: xfs.clone(), threshold_db: params.threshold_db, ratio: params.ratio,
            attack_ms: params.attack_ms, release_ms: params.release_ms, knee_db: params.knee_db,
            link_channels: params.link_channels, mix: params.mix, band_params: params.bands,
            crossover_points: Vec::new(), band_compressors: bcomps, band_buffers: Vec::new(),
            band_levels_db: vec![0.0; nb], dry_buffer: Vec::new(),
            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sr),
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            xover_smoothers: xfs.iter().map(|&f| LogSmoother::new(f, 50.0, sr)).collect(),
        };
        p.build_crossovers(); p.update_coefficients(); p
    }
    pub fn from_params(channels: usize, params: MultibandCompressorPluginParams) -> Self { Self::with_params(channels, params) }

    fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        for i in 0..(self.num_bands - 1) {
            let f = self.xover_smoothers[i].target();
            self.crossover_points.push(CrossoverPoint::new(self.channels, f, self.sample_rate));
        }
    }

    fn update_coefficients(&mut self) {
        for (i, b) in self.band_compressors.iter_mut().enumerate() {
            let a = self.band_params.get(i).and_then(|p| p.attack_ms).unwrap_or(self.attack_ms);
            let r = self.band_params.get(i).and_then(|p| p.release_ms).unwrap_or(self.release_ms);
            b.attack_coeff = (-1.0 / (a * 0.001 * self.sample_rate as f32)).exp();
            b.release_coeff = (-1.0 / (r * 0.001 * self.sample_rate as f32)).exp();
        }
    }

    fn calculate_gain_reduction(idb: f32, th: f32, ratio: f32, knee: f32) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);
        if knee < 0.1 { if idb <= th { 0.0 } else { (idb - th) * slope } }
        else if idb < th - knee/2.0 { 0.0 }
        else if idb > th + knee/2.0 { (idb - th) * slope }
        else { let ov = idb - th + knee/2.0; let kf = ov / knee; kf * kf * (knee/2.0) * slope }
    }
}

impl InPlacePlugin for MultibandCompressorPlugin {
    fn info(&self) -> PluginInfo { PluginInfo::new("Multiband Compressor", "1.1.0", "Sotf") }
    fn channels(&self) -> usize { self.channels }
    fn parameters(&self) -> Vec<Parameter> { vec![Parameter::new_float("threshold", "Threshold", -20.0, -60.0, 0.0)] }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id.0 == "threshold" { self.threshold_db = value.as_float().ok_or("val")?; self.threshold_smoother.set_target(self.threshold_db); }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "threshold" { Some(ParameterValue::Float(self.threshold_db)) } else { None }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr; self.build_crossovers(); self.update_coefficients();
        self.threshold_smoother.set_time(20.0, sr); self.mix_smoother.set_time(20.0, sr);
        for s in &mut self.xover_smoothers { *s = LogSmoother::new(s.target(), 50.0, sr); }
        Ok(())
    }
    fn reset(&mut self) { for x in &mut self.crossover_points { x.reset(self.sample_rate); } for b in &mut self.band_compressors { b.envelope.fill(0.0); } }

    fn process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let stride = nf * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(buffer.len(), 0.0);
        self.dry_buffer.copy_from_slice(buffer);

        for i in 0..(self.num_bands - 1) {
            let freq = self.xover_smoothers[i].next();
            if (freq - self.crossover_points[i].freq).abs() > 1.0 {
                self.crossover_points[i].freq = freq;
                let q = 1.0 / std::f64::consts::SQRT_2; let sr = self.sample_rate as f64;
                for ch in 0..self.channels {
                    for f in &mut self.crossover_points[i].lowpass[ch] { *f = Biquad::new(BiquadFilterType::Lowpass, freq as f64, sr, q, 0.0); }
                    for f in &mut self.crossover_points[i].highpass[ch] { *f = Biquad::new(BiquadFilterType::Highpass, freq as f64, sr, q, 0.0); }
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

        let g_th = self.threshold_smoother.next();
        let g_mix = self.mix_smoother.next();

        for b in 0..self.num_bands {
            let bp = self.band_params.get(b);
            let th = bp.and_then(|p| p.threshold_db).unwrap_or(g_th);
            let rat = bp.and_then(|p| p.ratio).unwrap_or(self.ratio);
            let kn = bp.and_then(|p| p.knee_db).unwrap_or(self.knee_db);
            let mk = fast_pow10(bp.map(|p| p.makeup_gain_db).unwrap_or(0.0) / 20.0);
            let bcomp = &mut self.band_compressors[b];
            let off = b * stride;

            for frame in 0..nf {
                let mut det = 0.0f32;
                if self.link_channels {
                    for ch in 0..self.channels { det = det.max(self.band_buffers[off + frame * self.channels + ch].abs()); }
                }
                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let idb = 20.0 * fast_log10((if self.link_channels { det } else { self.band_buffers[idx].abs() }).max(1e-10));
                    let tgr = Self::calculate_gain_reduction(idb, th, rat, kn);
                    let c = if tgr > bcomp.envelope[ch] { bcomp.attack_coeff } else { bcomp.release_coeff };
                    bcomp.envelope[ch] = tgr + c * (bcomp.envelope[ch] - tgr);
                    self.band_buffers[idx] *= fast_pow10(-bcomp.envelope[ch] / 20.0) * mk;
                }
            }
        }

        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut s = 0.0f32;
                for b in 0..self.num_bands { s += self.band_buffers[b * stride + idx]; }
                buffer[idx] = self.dry_buffer[idx] * (1.0 - g_mix) + s * g_mix;
            }
        }
        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(Arc::new(MultibandCompressorData {
            gain_reduction_db: self.band_compressors.iter().map(|b| b.envelope.clone()).collect(),
            band_levels_db: self.band_levels_db.clone(),
            crossover_frequencies: self.crossover_frequencies.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mb_comp_basic() {
        let mut p = MultibandCompressorPlugin::new(1); p.initialize(48000).unwrap();
        let mut b = vec![0.5; 1000];
        p.process_in_place(&mut b, &ProcessContext { sample_rate: 48000, num_frames: 1000 }).unwrap();
        assert!(b[999].is_finite());
    }
}
