// ============================================================================
// Phase-Coherent Downmix Plugin
// ============================================================================

use super::param_specs::downmix::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};

use super::smoothing::Smoother;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

fn default_center_gain_db() -> f32 {
    CENTER_GAIN_DB_DEFAULT
}
fn default_surround_gain_db() -> f32 {
    SURROUND_GAIN_DB_DEFAULT
}
fn default_height_gain_db() -> f32 {
    HEIGHT_GAIN_DB_DEFAULT
}
fn default_lfe_gain_db() -> f32 {
    LFE_GAIN_DB_DEFAULT
}
fn default_phase_coherence() -> bool {
    PHASE_COHERENCE_DEFAULT
}
fn default_phase_blend_low_hz() -> f32 {
    PHASE_BLEND_LOW_HZ_DEFAULT
}
fn default_phase_blend_high_hz() -> f32 {
    PHASE_BLEND_HIGH_HZ_DEFAULT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownmixPluginParams {
    pub input_channels: usize,
    #[serde(default = "default_center_gain_db")]
    pub center_gain_db: f32,
    #[serde(default = "default_surround_gain_db")]
    pub surround_gain_db: f32,
    #[serde(default = "default_height_gain_db")]
    pub height_gain_db: f32,
    #[serde(default = "default_lfe_gain_db")]
    pub lfe_gain_db: f32,
    #[serde(default = "default_phase_coherence")]
    pub phase_coherence: bool,
    #[serde(default = "default_phase_blend_low_hz")]
    pub phase_blend_low_hz: f32,
    #[serde(default = "default_phase_blend_high_hz")]
    pub phase_blend_high_hz: f32,
}

#[derive(Clone, Copy)]
struct DownmixCoeffs {
    left_gain: f32,
    right_gain: f32,
}

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 4;
const PARAM_SMOOTH_MS: f32 = 20.0;

pub struct DownmixPlugin {
    input_ch: usize,
    sample_rate: u32,
    speaker_config: Option<&'static SpeakerConfig>,
    target_coeffs: Vec<DownmixCoeffs>,
    coeff_smoothers: Vec<Smoother>,
    lfe_channels: Vec<usize>,
    lfe_lpf_idx: Vec<Option<usize>>,
    
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,
    
    analysis_window: Vec<f32>,
    output_scale: f32,
    
    /// Flat input buffer: [channel * FFT_SIZE + sample]
    input_buffer: Vec<f32>,
    input_fill: usize,
    
    output_accumulator: Vec<f32>,
    output_accumulator_mask: usize,
    output_accumulator_fill: usize,
    next_add_position: usize,
    output_read_position: usize,

    /// Flat FFT output: [channel * num_bins + bin]
    fft_output: Vec<Complex<f32>>,
    out_freq_l: Vec<Complex<f32>>,
    out_freq_r: Vec<Complex<f32>>,
    
    fft_input_buf: Vec<f32>,
    ifft_input_buf: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,
    
    lfe_lpf: Vec<[Biquad; 2]>,
    
    center_gain_db: f32,
    surround_gain_db: f32,
    height_gain_db: f32,
    lfe_gain_db: f32,
    phase_coherence: bool,
    phase_blend_low_hz: f32,
    phase_blend_high_hz: f32,
    
    param_center_gain_db: ParameterId,
    param_surround_gain_db: ParameterId,
    param_height_gain_db: ParameterId,
    param_lfe_gain_db: ParameterId,
    param_phase_coherence: ParameterId,
}

impl DownmixPlugin {
    pub fn new(input_channels: usize) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);
        let num_bins = FFT_SIZE / 2 + 1;
        
        let analysis_window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let x = i as f32 / FFT_SIZE as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
            })
            .collect();

        // 75% overlap analysis-only scaling: sum(w) = 2.0 * N
        let output_scale = 1.0 / (FFT_SIZE as f32 * 2.0);

        Self {
            input_ch: input_channels,
            sample_rate: 44100,
            speaker_config: get_speaker_config_by_channels(input_channels),
            target_coeffs: Vec::new(),
            coeff_smoothers: Vec::new(),
            lfe_channels: Vec::new(),
            lfe_lpf_idx: Vec::new(),
            fft_forward,
            fft_inverse,
            analysis_window,
            output_scale,
            input_buffer: vec![0.0; FFT_SIZE * input_channels],
            input_fill: 0,
            output_accumulator: vec![0.0; FFT_SIZE * 4 * 2], // 4*N frames, stereo
            output_accumulator_mask: (FFT_SIZE * 4) - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            fft_output: vec![Complex::new(0.0, 0.0); num_bins * input_channels],
            out_freq_l: vec![Complex::new(0.0, 0.0); num_bins],
            out_freq_r: vec![Complex::new(0.0, 0.0); num_bins],
            fft_input_buf: vec![0.0; FFT_SIZE],
            ifft_input_buf: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_output_buf: vec![0.0; FFT_SIZE],
            lfe_lpf: Vec::new(),
            center_gain_db: CENTER_GAIN_DB_DEFAULT,
            surround_gain_db: SURROUND_GAIN_DB_DEFAULT,
            height_gain_db: HEIGHT_GAIN_DB_DEFAULT,
            lfe_gain_db: LFE_GAIN_DB_DEFAULT,
            phase_coherence: PHASE_COHERENCE_DEFAULT,
            phase_blend_low_hz: PHASE_BLEND_LOW_HZ_DEFAULT,
            phase_blend_high_hz: PHASE_BLEND_HIGH_HZ_DEFAULT,
            param_center_gain_db: ParameterId::from("center_gain_db"),
            param_surround_gain_db: ParameterId::from("surround_gain_db"),
            param_height_gain_db: ParameterId::from("height_gain_db"),
            param_lfe_gain_db: ParameterId::from("lfe_gain_db"),
            param_phase_coherence: ParameterId::from("phase_coherence"),
        }
    }

    pub fn from_params(params: DownmixPluginParams) -> Self {
        let mut plugin = Self::new(params.input_channels);
        plugin.center_gain_db = params.center_gain_db;
        plugin.surround_gain_db = params.surround_gain_db;
        plugin.height_gain_db = params.height_gain_db;
        plugin.lfe_gain_db = params.lfe_gain_db;
        plugin.phase_coherence = params.phase_coherence;
        plugin.phase_blend_low_hz = params.phase_blend_low_hz;
        plugin.phase_blend_high_hz = params.phase_blend_high_hz;
        plugin.compute_coefficients(true);
        plugin
    }

    fn compute_coefficients(&mut self, reset: bool) {
        let mut new_coeffs = Vec::with_capacity(self.input_ch);
        self.lfe_channels.clear();
        
        // Linear gains from dB
        let c_lin = 10.0_f32.powf(self.center_gain_db / 20.0);
        let s_lin = 10.0_f32.powf(self.surround_gain_db / 20.0);
        let h_lin = 10.0_f32.powf(self.height_gain_db / 20.0);
        let l_lin = 10.0_f32.powf(self.lfe_gain_db / 20.0);

        if let Some(config) = self.speaker_config {
            for s in config.speakers {
                if s.is_lfe {
                    self.lfe_channels.push(s.channel);
                    // LFE usually goes to both L and R at -3dB relative to lfe_gain_db
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: l_lin * 0.707,
                        right_gain: l_lin * 0.707,
                    });
                } else {
                    let azimuth = s.azimuth.abs();
                    let elevation = s.elevation.abs();
                    
                    if elevation > 10.0 {
                        // Height channels
                        let ang = (s.azimuth.to_radians() + std::f32::consts::FRAC_PI_2) * 0.5;
                        new_coeffs.push(DownmixCoeffs {
                            left_gain: h_lin * ang.sin(),
                            right_gain: h_lin * ang.cos(),
                        });
                    } else if azimuth < 1.0 {
                        // Center channel
                        new_coeffs.push(DownmixCoeffs {
                            left_gain: c_lin * 0.707,
                            right_gain: c_lin * 0.707,
                        });
                    } else if azimuth < 45.0 {
                        // Front L/R
                        if s.azimuth > 0.0 {
                            // Left side (e.g. +30°)
                            new_coeffs.push(DownmixCoeffs { left_gain: 1.0, right_gain: 0.0 });
                        } else {
                            // Right side (e.g. -30°)
                            new_coeffs.push(DownmixCoeffs { left_gain: 0.0, right_gain: 1.0 });
                        }
                    } else {
                        // Surround channels (Side/Rear)
                        // Map azimuth to stereo width: 45..180 -> Left, -180..-45 -> Right
                        let (lg, rg) = if s.azimuth > 1.0 {
                            // Left side
                            let pan = ((s.azimuth.abs() - 10.0) / 80.0).clamp(0.0, 1.0);
                            (s_lin * pan, s_lin * (1.0 - pan) * 0.5)
                        } else if s.azimuth < -1.0 {
                            // Right side
                            let pan = ((s.azimuth.abs() - 10.0) / 80.0).clamp(0.0, 1.0);
                            (s_lin * (1.0 - pan) * 0.5, s_lin * pan)
                        } else {
                            (s_lin * 0.707, s_lin * 0.707)
                        };
                        new_coeffs.push(DownmixCoeffs { left_gain: lg, right_gain: rg });
                    }
                }
            }
        } else {
            // Generic downmix: linear panned based on channel index
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    new_coeffs.push(DownmixCoeffs { left_gain: 0.707, right_gain: 0.707 });
                } else {
                    let t = ch as f32 / (self.input_ch - 1).max(1) as f32;
                    new_coeffs.push(DownmixCoeffs { left_gain: 1.0 - t, right_gain: t });
                }
            }
        }

        // Normalization: Only normalize if the sum of gains exceeds a safe threshold (e.g., 2.0)
        // This prevents massive attenuation while still protecting against extreme clipping.
        let max_sum = new_coeffs.iter().map(|c| c.left_gain).sum::<f32>()
            .max(new_coeffs.iter().map(|c| c.right_gain).sum::<f32>());
        
        let norm_threshold = 2.0;
        if max_sum > norm_threshold {
            let scale = norm_threshold / max_sum;
            for c in &mut new_coeffs {
                c.left_gain *= scale;
                c.right_gain *= scale;
            }
        }

        self.lfe_lpf_idx.clear();
        self.lfe_lpf_idx.resize(self.input_ch, None);
        for (slot, &ch) in self.lfe_channels.iter().enumerate() {
            if ch < self.input_ch {
                self.lfe_lpf_idx[ch] = Some(slot);
            }
        }

        self.target_coeffs = new_coeffs;
        if self.coeff_smoothers.is_empty() {
            for c in &self.target_coeffs {
                self.coeff_smoothers.push(Smoother::new(c.left_gain, PARAM_SMOOTH_MS, self.sample_rate));
                self.coeff_smoothers.push(Smoother::new(c.right_gain, PARAM_SMOOTH_MS, self.sample_rate));
            }
        } else {
            for (i, c) in self.target_coeffs.iter().enumerate() {
                if reset {
                    self.coeff_smoothers[i * 2].reset(c.left_gain);
                    self.coeff_smoothers[i * 2 + 1].reset(c.right_gain);
                } else {
                    self.coeff_smoothers[i * 2].set_target(c.left_gain);
                    self.coeff_smoothers[i * 2 + 1].set_target(c.right_gain);
                }
            }
        }
    }

    fn process_simple(&mut self, input: &[f32], output: &mut [f32], num_frames: usize) {
        for frame in 0..num_frames {
            let mut l = 0.0;
            let mut r = 0.0;
            for ch in 0..self.input_ch {
                let mut s = input[frame * self.input_ch + ch];
                // O(1) lookup: no linear scan over lfe_channels.
                if let Some(l_idx) = self.lfe_lpf_idx[ch].filter(|&i| i < self.lfe_lpf.len()) {
                    let mut val = s as f64;
                    val = self.lfe_lpf[l_idx][0].process(val);
                    val = self.lfe_lpf[l_idx][1].process(val);
                    s = val as f32;
                }
                l += s * self.coeff_smoothers[ch * 2].next();
                r += s * self.coeff_smoothers[ch * 2 + 1].next();
            }
            output[frame * 2] = l;
            output[frame * 2 + 1] = r;
        }
    }

    fn process_fft_block(&mut self) {
        let n = FFT_SIZE;
        let scale = self.output_scale;
        let mask = self.output_accumulator_mask;
        let num_bins = n / 2 + 1;

        for ch in 0..self.input_ch {
            let ch_offset = ch * n;
            let fft_offset = ch * num_bins;
            // Window and FFT each channel
            for i in 0..n {
                self.fft_input_buf[i] = self.input_buffer[ch_offset + i] * self.analysis_window[i];
            }
            self.fft_forward
                .process(&mut self.fft_input_buf, &mut self.fft_output[fft_offset..fft_offset + num_bins])
                .unwrap();
        }

        // Initialize output frequencies
        self.out_freq_l.fill(Complex::new(0.0, 0.0));
        self.out_freq_r.fill(Complex::new(0.0, 0.0));

        // Power sum (standard downmix) in frequency domain
        for ch in 0..self.input_ch {
            let gl = self.coeff_smoothers[ch * 2].current();
            let gr = self.coeff_smoothers[ch * 2 + 1].current();
            let fft_offset = ch * num_bins;
            let channel_fft = &self.fft_output[fft_offset..fft_offset + num_bins];
            
            for i in 0..num_bins {
                self.out_freq_l[i] += channel_fft[i] * gl;
                self.out_freq_r[i] += channel_fft[i] * gr;
            }
        }

        // Apply Phase Coherence if enabled
        if self.phase_coherence {
            // Per-bin phase alignment logic
            for bin in 0..num_bins {
                let freq = bin as f32 * self.sample_rate as f32 / n as f32;
                let blend = if freq <= self.phase_blend_low_hz {
                    0.0
                } else if freq >= self.phase_blend_high_hz {
                    1.0
                } else {
                    let t = (freq - self.phase_blend_low_hz)
                        / (self.phase_blend_high_hz - self.phase_blend_low_hz);
                    t * t * (3.0 - 2.0 * t)
                };

                if blend > 0.001 {
                    let mut max_mag_l = -1.0f32;
                    let mut max_mag_r = -1.0f32;
                    let mut dominant_phase_l = 0.0f32;
                    let mut dominant_phase_r = 0.0f32;
                    
                    let mut mag_sum_l = 0.0f32;
                    let mut mag_sum_r = 0.0f32;

                    for ch in 0..self.input_ch {
                        let gl = self.coeff_smoothers[ch * 2].current();
                        let gr = self.coeff_smoothers[ch * 2 + 1].current();
                        let val = self.fft_output[ch * num_bins + bin];
                        
                        let mag_l = val.norm() * gl;
                        let mag_r = val.norm() * gr;
                        
                        mag_sum_l += mag_l;
                        mag_sum_r += mag_r;

                        if mag_l > max_mag_l {
                            max_mag_l = mag_l;
                            dominant_phase_l = (val * gl).arg();
                        }
                        if mag_r > max_mag_r {
                            max_mag_r = mag_r;
                            dominant_phase_r = (val * gr).arg();
                        }
                    }

                    let aligned_l = Complex::from_polar(mag_sum_l, dominant_phase_l);
                    let aligned_r = Complex::from_polar(mag_sum_r, dominant_phase_r);
                    
                    self.out_freq_l[bin] = self.out_freq_l[bin] * (1.0 - blend) + aligned_l * blend;
                    self.out_freq_r[bin] = self.out_freq_r[bin] * (1.0 - blend) + aligned_r * blend;
                }
            }
        }

        // IFFT and Accumulate
        self.out_freq_l[0].im = 0.0;
        self.out_freq_l[num_bins - 1].im = 0.0;
        self.ifft_input_buf.copy_from_slice(&self.out_freq_l);
        self.fft_inverse.process(&mut self.ifft_input_buf, &mut self.ifft_output_buf).unwrap();
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2] += self.ifft_output_buf[i] * scale;
        }

        self.out_freq_r[0].im = 0.0;
        self.out_freq_r[num_bins - 1].im = 0.0;
        self.ifft_input_buf.copy_from_slice(&self.out_freq_r);
        self.fft_inverse.process(&mut self.ifft_input_buf, &mut self.ifft_output_buf).unwrap();
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2 + 1] += self.ifft_output_buf[i] * scale;
        }

        self.next_add_position = (self.next_add_position + HOP_SIZE) & mask;
        self.output_accumulator_fill += HOP_SIZE;
    }
}

impl Plugin for DownmixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Downmix", "2.0.0", "SotF").with_description("Phase-coherent surround-to-stereo downmixer")
    }
    fn input_channels(&self) -> usize {
        self.input_ch
    }
    fn output_channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("center_gain_db", "Center Gain", self.center_gain_db, CENTER_GAIN_DB_MIN, CENTER_GAIN_DB_MAX),
            Parameter::new_float("surround_gain_db", "Surround Gain", self.surround_gain_db, SURROUND_GAIN_DB_MIN, SURROUND_GAIN_DB_MAX),
            Parameter::new_float("height_gain_db", "Height Gain", self.height_gain_db, HEIGHT_GAIN_DB_MIN, HEIGHT_GAIN_DB_MAX),
            Parameter::new_float("lfe_gain_db", "LFE Gain", self.lfe_gain_db, LFE_GAIN_DB_MIN, LFE_GAIN_DB_MAX),
            Parameter::new_bool("phase_coherence", "Phase Coherence", self.phase_coherence),
        ]
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_center_gain_db {
            self.center_gain_db = value.as_float().ok_or("val")?;
        } else if id == self.param_surround_gain_db {
            self.surround_gain_db = value.as_float().ok_or("val")?;
        } else if id == self.param_height_gain_db {
            self.height_gain_db = value.as_float().ok_or("val")?;
        } else if id == self.param_lfe_gain_db {
            self.lfe_gain_db = value.as_float().ok_or("val")?;
        } else if id == self.param_phase_coherence {
            self.phase_coherence = value.as_bool().ok_or("val")?;
        } else {
            return Err("unknown".into());
        }
        self.compute_coefficients(false);
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_center_gain_db { Some(ParameterValue::Float(self.center_gain_db)) }
        else if id == &self.param_surround_gain_db { Some(ParameterValue::Float(self.surround_gain_db)) }
        else if id == &self.param_height_gain_db { Some(ParameterValue::Float(self.height_gain_db)) }
        else if id == &self.param_lfe_gain_db { Some(ParameterValue::Float(self.lfe_gain_db)) }
        else if id == &self.param_phase_coherence { Some(ParameterValue::Bool(self.phase_coherence)) }
        else { None }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.lfe_lpf = self.lfe_channels.iter().map(|_| {
            [
                Biquad::new(BiquadFilterType::Lowpass, 120.0, sample_rate as f64, 0.707, 0.0),
                Biquad::new(BiquadFilterType::Lowpass, 120.0, sample_rate as f64, 0.707, 0.0),
            ]
        }).collect();
        for s in &mut self.coeff_smoothers { s.set_time(PARAM_SMOOTH_MS, sample_rate); }
        self.compute_coefficients(true);
        Ok(())
    }
    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String> {
        let num_frames = context.num_frames;
        if !self.phase_coherence {
            self.process_simple(input, output, num_frames);
            return Ok(num_frames);
        }

        let mut input_pos = 0;
        let mut output_pos = 0;
        let mask = self.output_accumulator_mask;
        let n = FFT_SIZE;

        while output_pos < num_frames {
            // Step 1: Fill input buffer
            if input_pos < num_frames {
                let to_copy = (n - self.input_fill).min(num_frames - input_pos);
                if to_copy > 0 {
                    for ch in 0..self.input_ch {
                        let ch_offset = ch * n;
                        let src = &input[input_pos * self.input_ch + ch..];
                        if let Some(li) = self.lfe_lpf_idx[ch].filter(|&i| i < self.lfe_lpf.len()) {
                            for i in 0..to_copy {
                                let mut v = src[i * self.input_ch] as f64;
                                v = self.lfe_lpf[li][0].process(v);
                                v = self.lfe_lpf[li][1].process(v);
                                self.input_buffer[ch_offset + self.input_fill + i] = v as f32;
                            }
                        } else {
                            for i in 0..to_copy {
                                self.input_buffer[ch_offset + self.input_fill + i] = src[i * self.input_ch];
                            }
                        }
                    }
                    self.input_fill += to_copy;
                    input_pos += to_copy;
                }
            }

            // Step 2: Process STFT frames
            while self.input_fill >= n {
                self.process_fft_block();
                let overlap = n - HOP_SIZE;
                for ch in 0..self.input_ch {
                    let ch_offset = ch * n;
                    self.input_buffer[ch_offset..ch_offset + n].copy_within(HOP_SIZE..n, 0);
                }
                self.input_fill = overlap;
            }

            // Step 3: Drain output accumulator
            let to_drain = self.output_accumulator_fill.min(num_frames - output_pos);
            if to_drain > 0 {
                for i in 0..to_drain {
                    let read_idx = (self.output_read_position + i) & mask;
                    output[(output_pos + i) * 2] = self.output_accumulator[read_idx * 2];
                    output[(output_pos + i) * 2 + 1] = self.output_accumulator[read_idx * 2 + 1];
                    self.output_accumulator[read_idx * 2] = 0.0;
                    self.output_accumulator[read_idx * 2 + 1] = 0.0;
                }
                self.output_read_position = (self.output_read_position + to_drain) & mask;
                self.output_accumulator_fill -= to_drain;
                output_pos += to_drain;
            } else {
                if input_pos >= num_frames {
                    let rem = num_frames - output_pos;
                    for i in 0..rem {
                        output[(output_pos + i) * 2] = 0.0;
                        output[(output_pos + i) * 2 + 1] = 0.0;
                    }
                    output_pos = num_frames;
                } else {
                    break;
                }
            }
        }

        for s in &mut self.coeff_smoothers {
            s.next_n(num_frames);
        }

        Ok(num_frames)
    }
    fn reset(&mut self) {
        self.input_buffer.fill(0.0);
        self.input_fill = 0;
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.lfe_lpf = self.lfe_channels.iter().map(|_| {
            [
                Biquad::new(BiquadFilterType::Lowpass, 120.0, self.sample_rate as f64, 0.707, 0.0),
                Biquad::new(BiquadFilterType::Lowpass, 120.0, self.sample_rate as f64, 0.707, 0.0),
            ]
        }).collect();
        for (i, c) in self.target_coeffs.iter().enumerate() {
            self.coeff_smoothers[i * 2].reset(c.left_gain);
            self.coeff_smoothers[i * 2 + 1].reset(c.right_gain);
        }
    }
    fn latency_samples(&self) -> usize {
        if self.phase_coherence { FFT_SIZE } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_downmix_basic() {
        let mut p = DownmixPlugin::new(2);
        p.initialize(44100).unwrap();
        p.phase_coherence = false;
        let mut i = vec![0.0; 2048];
        let mut o = vec![0.0; 2048];
        for k in 0..1024 {
            i[k * 2] = (k as f32 * 0.01).sin();
            i[k * 2 + 1] = (k as f32 * 0.02).sin();
        }
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 44100,
                num_frames: 1024,
            },
        )
        .unwrap();
        assert!(o.iter().any(|&s| s.abs() > 1e-5));
    }
    #[test]
    fn test_downmix_51() {
        let mut p = DownmixPlugin::new(6);
        p.phase_coherence = false;
        p.initialize(44100).unwrap();
        let mut i = vec![0.0; 600];
        let mut o = vec![0.0; 200];
        for k in 0..100 {
            i[k * 6 + 2] = 1.0;
        }
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 44100,
                num_frames: 100,
            },
        )
        .unwrap();
        assert!(o[0].abs() > 0.01);
    }
}
