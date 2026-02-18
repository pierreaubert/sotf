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
const HOP_SIZE: usize = FFT_SIZE / 2;
const PARAM_SMOOTH_MS: f32 = 20.0;

pub struct DownmixPlugin {
    input_ch: usize,
    sample_rate: u32,
    speaker_config: Option<&'static SpeakerConfig>,
    target_coeffs: Vec<DownmixCoeffs>,
    coeff_smoothers: Vec<Smoother>,
    lfe_channels: Vec<usize>,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,
    channel_freq: Vec<Vec<Complex<f32>>>,
    out_freq: [Vec<Complex<f32>>; 2],
    input_ring: Vec<Vec<f32>>,
    input_ring_pos: usize,
    input_fill: usize,
    output_accum: [Vec<f32>; 2],
    output_read_pos: usize,
    output_write_pos: usize,
    window: Vec<f32>,
    fft_input_buf: Vec<f32>,
    fft_output_buf: Vec<Complex<f32>>,
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
        let freq_len = FFT_SIZE / 2 + 1;
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let t = i as f32 / FFT_SIZE as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * t).cos())
            })
            .collect();

        let mut plugin = Self {
            input_ch: input_channels,
            sample_rate: 44100,
            speaker_config: get_speaker_config_by_channels(input_channels),
            target_coeffs: Vec::new(),
            coeff_smoothers: Vec::new(),
            lfe_channels: Vec::new(),
            fft_forward: fft_forward.clone(),
            fft_inverse: fft_inverse.clone(),
            channel_freq: vec![vec![Complex::new(0.0, 0.0); freq_len]; input_channels],
            out_freq: [
                vec![Complex::new(0.0, 0.0); freq_len],
                vec![Complex::new(0.0, 0.0); freq_len],
            ],
            input_ring: vec![vec![0.0; FFT_SIZE]; input_channels],
            input_ring_pos: 0,
            input_fill: 0,
            output_accum: [vec![0.0; FFT_SIZE * 3], vec![0.0; FFT_SIZE * 3]],
            output_read_pos: 0,
            output_write_pos: 0,
            window,
            fft_input_buf: fft_forward.make_input_vec(),
            fft_output_buf: fft_forward.make_output_vec(),
            ifft_input_buf: fft_inverse.make_input_vec(),
            ifft_output_buf: fft_inverse.make_output_vec(),
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
        };
        plugin.compute_coefficients(true);
        plugin
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
        let c_lin = 10.0_f32.powf(self.center_gain_db / 20.0);
        let s_lin = 10.0_f32.powf(self.surround_gain_db / 20.0);
        let h_lin = 10.0_f32.powf(self.height_gain_db / 20.0);
        let l_lin = 10.0_f32.powf(self.lfe_gain_db / 20.0);

        if let Some(config) = self.speaker_config {
            for s in config.speakers {
                if s.is_lfe {
                    self.lfe_channels.push(s.channel);
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: l_lin * 0.5,
                        right_gain: l_lin * 0.5,
                    });
                } else {
                    let cat_gain = if s.elevation.abs() > 10.0 {
                        h_lin
                    } else if s.azimuth.abs() < 15.0 {
                        c_lin
                    } else if s.azimuth.abs() > 60.0 {
                        s_lin
                    } else {
                        1.0
                    };
                    let ang = (s.azimuth.to_radians() + std::f32::consts::FRAC_PI_2) * 0.5;
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: (cat_gain * ang.sin()).max(0.0),
                        right_gain: (cat_gain * ang.cos()).max(0.0),
                    });
                }
            }
        } else {
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: 1.0,
                        right_gain: 1.0,
                    });
                } else {
                    let t = ch as f32 / (self.input_ch - 1).max(1) as f32;
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: 1.0 - t,
                        right_gain: t,
                    });
                }
            }
        }
        let max_sum = new_coeffs
            .iter()
            .map(|c| c.left_gain)
            .sum::<f32>()
            .max(new_coeffs.iter().map(|c| c.right_gain).sum::<f32>());
        if max_sum > 1.0 {
            for c in &mut new_coeffs {
                c.left_gain /= max_sum;
                c.right_gain /= max_sum;
            }
        }
        self.target_coeffs = new_coeffs;
        if self.coeff_smoothers.is_empty() {
            for c in &self.target_coeffs {
                self.coeff_smoothers.push(Smoother::new(
                    c.left_gain,
                    PARAM_SMOOTH_MS,
                    self.sample_rate,
                ));
                self.coeff_smoothers.push(Smoother::new(
                    c.right_gain,
                    PARAM_SMOOTH_MS,
                    self.sample_rate,
                ));
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
                if let Some(l_idx) = self.lfe_channels.iter().position(|&c| c == ch) {
                    if l_idx < self.lfe_lpf.len() {
                        let mut val = s as f64;
                        val = self.lfe_lpf[l_idx][0].process(val);
                        val = self.lfe_lpf[l_idx][1].process(val);
                        s = val as f32;
                    }
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
        let inv_n = 1.0 / n as f32;
        for ch in 0..self.input_ch {
            for i in 0..n {
                self.fft_input_buf[i] =
                    self.input_ring[ch][(self.input_ring_pos + i) % n] * self.window[i];
            }
            self.fft_forward
                .process(&mut self.fft_input_buf, &mut self.fft_output_buf)
                .unwrap();
            self.channel_freq[ch].copy_from_slice(&self.fft_output_buf);
        }
        for bin in 0..(n / 2 + 1) {
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
            let mut sl = Complex::new(0.0, 0.0);
            let mut sr = Complex::new(0.0, 0.0);
            let mut ml = -1.0f32;
            let mut mr = -1.0f32;
            let mut pl = 0.0f32;
            let mut pr = 0.0f32;
            for ch in 0..self.input_ch {
                let gl = self.coeff_smoothers[ch * 2].current();
                let gr = self.coeff_smoothers[ch * 2 + 1].current();
                let wl = self.channel_freq[ch][bin] * gl;
                let wr = self.channel_freq[ch][bin] * gr;
                sl += wl;
                sr += wr;
                if blend > 0.0 {
                    let nml = wl.norm();
                    if nml > ml {
                        ml = nml;
                        pl = wl.arg();
                    }
                    let nmr = wr.norm();
                    if nmr > mr {
                        mr = nmr;
                        pr = wr.arg();
                    }
                }
            }
            if blend < 0.001 {
                self.out_freq[0][bin] = sl;
                self.out_freq[1][bin] = sr;
            } else {
                let mut al = Complex::new(0.0, 0.0);
                let mut ar = Complex::new(0.0, 0.0);
                for ch in 0..self.input_ch {
                    al += Complex::from_polar(
                        (self.channel_freq[ch][bin] * self.coeff_smoothers[ch * 2].current())
                            .norm(),
                        pl,
                    );
                    ar += Complex::from_polar(
                        (self.channel_freq[ch][bin] * self.coeff_smoothers[ch * 2 + 1].current())
                            .norm(),
                        pr,
                    );
                }
                self.out_freq[0][bin] = sl * (1.0 - blend) + al * blend;
                self.out_freq[1][bin] = sr * (1.0 - blend) + ar * blend;
            }
        }
        self.out_freq[0][0].im = 0.0;
        self.out_freq[0][n / 2].im = 0.0;
        self.out_freq[1][0].im = 0.0;
        self.out_freq[1][n / 2].im = 0.0;
        for lr in 0..2 {
            self.ifft_input_buf.copy_from_slice(&self.out_freq[lr]);
            self.fft_inverse
                .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
                .unwrap();
            let write_pos = self.output_write_pos;
            let accum_len = self.output_accum[0].len();
            for i in 0..n {
                self.output_accum[lr][(write_pos + i) % accum_len] +=
                    self.ifft_output_buf[i] * inv_n;
            }
        }
        self.output_write_pos = (self.output_write_pos + HOP_SIZE) % self.output_accum[0].len();
    }
}

impl Plugin for DownmixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Downmix", "1.1.0", "SotF")
    }
    fn input_channels(&self) -> usize {
        self.input_ch
    }
    fn output_channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float(
                "center_gain_db",
                "Center Gain",
                CENTER_GAIN_DB_DEFAULT,
                CENTER_GAIN_DB_MIN,
                CENTER_GAIN_DB_MAX,
            ),
            Parameter::new_float(
                "surround_gain_db",
                "Surround Gain",
                SURROUND_GAIN_DB_DEFAULT,
                SURROUND_GAIN_DB_MIN,
                SURROUND_GAIN_DB_MAX,
            ),
            Parameter::new_float(
                "height_gain_db",
                "Height Gain",
                HEIGHT_GAIN_DB_DEFAULT,
                HEIGHT_GAIN_DB_MIN,
                HEIGHT_GAIN_DB_MAX,
            ),
            Parameter::new_float(
                "lfe_gain_db",
                "LFE Gain",
                LFE_GAIN_DB_DEFAULT,
                LFE_GAIN_DB_MIN,
                LFE_GAIN_DB_MAX,
            ),
            Parameter::new_bool(
                "phase_coherence",
                "Phase Coherence",
                PHASE_COHERENCE_DEFAULT,
            ),
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
        if id == &self.param_center_gain_db {
            Some(ParameterValue::Float(self.center_gain_db))
        } else if id == &self.param_surround_gain_db {
            Some(ParameterValue::Float(self.surround_gain_db))
        } else if id == &self.param_height_gain_db {
            Some(ParameterValue::Float(self.height_gain_db))
        } else if id == &self.param_lfe_gain_db {
            Some(ParameterValue::Float(self.lfe_gain_db))
        } else if id == &self.param_phase_coherence {
            Some(ParameterValue::Bool(self.phase_coherence))
        } else {
            None
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.lfe_lpf = self
            .lfe_channels
            .iter()
            .map(|_| {
                [
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        sample_rate as f64,
                        0.0,
                        0.0,
                    ),
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        sample_rate as f64,
                        0.0,
                        0.0,
                    ),
                ]
            })
            .collect();
        for s in &mut self.coeff_smoothers {
            s.set_time(PARAM_SMOOTH_MS, sample_rate);
        }
        Ok(())
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        if !self.phase_coherence {
            self.process_simple(input, output, num_frames);
            return Ok(num_frames);
        }
        for frame in 0..num_frames {
            for ch in 0..self.input_ch {
                let mut s = input[frame * self.input_ch + ch];
                if let Some(li) = self.lfe_channels.iter().position(|&c| c == ch) {
                    if li < self.lfe_lpf.len() {
                        let mut v = s as f64;
                        v = self.lfe_lpf[li][0].process(v);
                        v = self.lfe_lpf[li][1].process(v);
                        s = v as f32;
                    }
                }
                self.input_ring[ch][self.input_ring_pos] = s;
            }
            self.input_ring_pos = (self.input_ring_pos + 1) % FFT_SIZE;
            self.input_fill += 1;
            if self.input_fill >= HOP_SIZE {
                self.input_fill = 0;
                self.process_fft_block();
            }
            let rp = self.output_read_pos;
            output[frame * 2] = self.output_accum[0][rp];
            output[frame * 2 + 1] = self.output_accum[1][rp];
            self.output_accum[0][rp] = 0.0;
            self.output_accum[1][rp] = 0.0;
            self.output_read_pos = (rp + 1) % self.output_accum[0].len();
            for s in &mut self.coeff_smoothers {
                s.next();
            }
        }
        Ok(num_frames)
    }
    fn reset(&mut self) {
        // Clear ring buffers
        for ring in &mut self.input_ring {
            ring.fill(0.0);
        }
        self.input_ring_pos = 0;
        self.input_fill = 0;

        // Clear output accumulators
        for acc in &mut self.output_accum {
            acc.fill(0.0);
        }
        self.output_read_pos = 0;
        self.output_write_pos = 0;

        // Re-create LFE biquads to clear filter state (state fields are private)
        self.lfe_lpf = self
            .lfe_channels
            .iter()
            .map(|_| {
                [
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        self.sample_rate as f64,
                        0.0,
                        0.0,
                    ),
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        self.sample_rate as f64,
                        0.0,
                        0.0,
                    ),
                ]
            })
            .collect();

        // Reset coefficient smoothers to current targets
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
