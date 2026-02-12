// ============================================================================
// Mono-to-Stereo Plugin
// ============================================================================

use super::param_specs::mono_to_stereo::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use super::simd::{window_mul_simd};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 2;
const PARAM_SMOOTH_MS: f32 = 20.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonoToStereoPluginParams {
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_haas_delay_ms")]
    pub haas_delay_ms: f32,
    #[serde(default = "default_enable_comp_eq")]
    pub enable_comp_eq: bool,
    #[serde(default = "default_comp_eq_depth_db")]
    pub comp_eq_depth_db: f32,
}

fn default_stereo_width() -> f32 { STEREO_WIDTH_DEFAULT }
fn default_haas_delay_ms() -> f32 { HAAS_DELAY_MS_DEFAULT }
fn default_enable_comp_eq() -> bool { ENABLE_COMP_EQ_DEFAULT }
fn default_comp_eq_depth_db() -> f32 { COMP_EQ_DEPTH_DB_DEFAULT }

pub struct MonoToStereoPlugin {
    sample_rate: u32,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,
    decorrelation_filter: Vec<Complex<f32>>,
    input_ring: Vec<f32>,
    input_ring_pos: usize,
    output_accum: [Vec<f32>; 2],
    output_read_pos: usize,
    output_write_pos: usize,
    window: Vec<f32>,
    fft_input_buf: Vec<f32>,
    fft_output_buf: Vec<Complex<f32>>,
    ifft_input_buf: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,
    input_fill: usize,
    haas_buffer: Vec<f32>,
    haas_write_pos: usize,
    stereo_width: Smoother,
    haas_delay_samples: Smoother,
    comp_eq_depth: Smoother,
    enable_comp_eq: bool,
    decor_low_hz: f32,
    decor_high_hz: f32,
    param_stereo_width: ParameterId,
    param_haas_delay_ms: ParameterId,
    param_comp_eq_depth_db: ParameterId,
    ring_scratch: Vec<f32>,
}

impl MonoToStereoPlugin {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);
        let freq_len = FFT_SIZE / 2 + 1;
        let window: Vec<f32> = (0..FFT_SIZE).map(|i| {
            let t = i as f32 / FFT_SIZE as f32;
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * t).cos())
        }).collect();

        Self {
            sample_rate: 44100,
            fft_forward: fft_forward.clone(),
            fft_inverse: fft_inverse.clone(),
            decorrelation_filter: vec![Complex::new(1.0, 0.0); freq_len],
            input_ring: vec![0.0; FFT_SIZE],
            input_ring_pos: 0,
            output_accum: [vec![0.0; FFT_SIZE * 3], vec![0.0; FFT_SIZE * 3]],
            output_read_pos: 0,
            output_write_pos: 0,
            window,
            fft_input_buf: fft_forward.make_input_vec(),
            fft_output_buf: fft_forward.make_output_vec(),
            ifft_input_buf: fft_inverse.make_input_vec(),
            ifft_output_buf: fft_inverse.make_output_vec(),
            input_fill: 0,
            haas_buffer: vec![0.0; 48000 * 2], // 2 seconds
            haas_write_pos: 0,
            stereo_width: Smoother::new(STEREO_WIDTH_DEFAULT, PARAM_SMOOTH_MS, 44100),
            haas_delay_samples: Smoother::new(HAAS_DELAY_MS_DEFAULT * 44.1, PARAM_SMOOTH_MS, 44100),
            comp_eq_depth: Smoother::new(COMP_EQ_DEPTH_DB_DEFAULT, PARAM_SMOOTH_MS, 44100),
            enable_comp_eq: ENABLE_COMP_EQ_DEFAULT,
            decor_low_hz: DECOR_LOW_HZ_DEFAULT,
            decor_high_hz: DECOR_HIGH_HZ_DEFAULT,
            param_stereo_width: ParameterId::from("stereo_width"),
            param_haas_delay_ms: ParameterId::from("haas_delay_ms"),
            param_comp_eq_depth_db: ParameterId::from("comp_eq_depth_db"),
            ring_scratch: vec![0.0; FFT_SIZE],
        }
    }

    pub fn from_params(_channels: usize, params: MonoToStereoPluginParams) -> Self {
        let mut p = Self::new();
        p.stereo_width.set_target(params.stereo_width);
        p.haas_delay_samples.set_target(params.haas_delay_ms * 44.1);
        p.enable_comp_eq = params.enable_comp_eq;
        p.comp_eq_depth.set_target(params.comp_eq_depth_db);
        p
    }

    fn generate_decorrelation_filter(&mut self) {
        let freq_len = FFT_SIZE / 2 + 1;
        let mut rng = rand::thread_rng();
        use rand::Rng;

        for i in 0..freq_len {
            let freq = i as f32 * self.sample_rate as f32 / FFT_SIZE as f32;
            if freq >= self.decor_low_hz && freq <= self.decor_high_hz {
                let phase = rng.gen_range(0.0..2.0 * std::f32::consts::PI);
                self.decorrelation_filter[i] = Complex::from_polar(1.0, phase);
            } else {
                self.decorrelation_filter[i] = Complex::new(1.0, 0.0);
            }
        }
        // Normalize for DC and Nyquist
        self.decorrelation_filter[0] = Complex::new(self.decorrelation_filter[0].re, 0.0);
        self.decorrelation_filter[freq_len - 1] = Complex::new(self.decorrelation_filter[freq_len - 1].re, 0.0);
    }
}

impl Plugin for MonoToStereoPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("MonoToStereo", "1.1.0", "Sotf")
    }

    fn input_channels(&self) -> usize { 1 }
    fn output_channels(&self) -> usize { 2 }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("stereo_width", "Stereo Width", STEREO_WIDTH_DEFAULT, 0.0, 1.0),
            Parameter::new_float("haas_delay_ms", "Haas Delay", HAAS_DELAY_MS_DEFAULT, 0.0, 50.0),
            Parameter::new_float("comp_eq_depth_db", "Comp EQ Depth", COMP_EQ_DEPTH_DB_DEFAULT, 0.0, 12.0),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_stereo_width {
            self.stereo_width.set_target(value.as_float().ok_or("Invalid value")?);
        } else if id == self.param_haas_delay_ms {
            let ms = value.as_float().ok_or("Invalid value")?;
            self.haas_delay_samples.set_target(ms * self.sample_rate as f32 / 1000.0);
        } else if id == self.param_comp_eq_depth_db {
            self.comp_eq_depth.set_target(value.as_float().ok_or("Invalid value")?);
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_stereo_width {
            Some(ParameterValue::Float(self.stereo_width.target()))
        } else if id == &self.param_haas_delay_ms {
            Some(ParameterValue::Float(self.haas_delay_samples.target() * 1000.0 / self.sample_rate as f32))
        } else if id == &self.param_comp_eq_depth_db {
            Some(ParameterValue::Float(self.comp_eq_depth.target()))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.stereo_width.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.haas_delay_samples.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.comp_eq_depth.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.haas_buffer.resize(sample_rate as usize * 2, 0.0);
        self.generate_decorrelation_filter();
        Ok(())
    }

    fn reset(&mut self) {
        self.input_ring.fill(0.0);
        self.input_ring_pos = 0;
        self.input_fill = 0;
        for buf in &mut self.output_accum { buf.fill(0.0); }
        self.output_read_pos = 0;
        self.output_write_pos = 0;
        self.haas_buffer.fill(0.0);
        self.haas_write_pos = 0;
    }

    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String> {
        let nf = context.num_frames;
        if input.is_empty() { return Ok(0); }

        for frame in 0..nf {
            self.input_ring[self.input_ring_pos] = input[frame];
            self.haas_buffer[self.haas_write_pos] = input[frame];
            
            self.input_ring_pos = (self.input_ring_pos + 1) % FFT_SIZE;
            self.haas_write_pos = (self.haas_write_pos + 1) % self.haas_buffer.len();
            self.input_fill += 1;

            if self.input_fill >= HOP_SIZE {
                self.process_stft();
                self.input_fill -= HOP_SIZE;
            }

            let haas_delay = self.haas_delay_samples.next();
            let width = self.stereo_width.next();
            let _comp_eq = self.comp_eq_depth.next();

            let delay_idx = (self.haas_write_pos + self.haas_buffer.len() - haas_delay as usize - 1) % self.haas_buffer.len();
            let haas_out = self.haas_buffer[delay_idx];

            let decor_l = self.output_accum[0][self.output_read_pos];
            let decor_r = self.output_accum[1][self.output_read_pos];
            
            self.output_accum[0][self.output_read_pos] = 0.0;
            self.output_accum[1][self.output_read_pos] = 0.0;
            self.output_read_pos = (self.output_read_pos + 1) % (FFT_SIZE * 3);

            let mono = input[frame];
            let side_l = (decor_l - mono) * width;
            let side_r = (decor_r - haas_out) * width;

            output[frame * 2] = mono + side_l;
            output[frame * 2 + 1] = mono + side_r;
        }

        Ok(nf)
    }
}

impl MonoToStereoPlugin {
    fn process_stft(&mut self) {
        let mut idx = (self.input_ring_pos + FFT_SIZE - HOP_SIZE) % FFT_SIZE;
        for i in 0..FFT_SIZE {
            self.fft_input_buf[i] = self.input_ring[idx];
            idx = (idx + 1) % FFT_SIZE;
        }
        super::simd::window_mul_simd_inplace(&mut self.fft_input_buf, &self.window);
        self.fft_forward.process(&mut self.fft_input_buf, &mut self.fft_output_buf).unwrap();

        for i in 0..self.fft_output_buf.len() {
            self.ifft_input_buf[i] = self.fft_output_buf[i] * self.decorrelation_filter[i];
        }
        self.fft_inverse.process(&mut self.ifft_input_buf, &mut self.ifft_output_buf).unwrap();
        let scale = 1.0 / FFT_SIZE as f32;
        for i in 0..FFT_SIZE {
            let val = self.ifft_output_buf[i] * scale * self.window[i];
            let pos = (self.output_write_pos + i) % (FFT_SIZE * 3);
            self.output_accum[0][pos] += val;
            self.output_accum[1][pos] += val;
        }
        self.output_write_pos = (self.output_write_pos + HOP_SIZE) % (FFT_SIZE * 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mono_to_stereo_basic() {
        let mut p = MonoToStereoPlugin::new(); p.initialize(48000).unwrap();
        let i = vec![0.5; 1024]; let mut o = vec![0.0; 2048];
        p.process(&i, &mut o, &ProcessContext { sample_rate: 48000, num_frames: 1024 }).unwrap();
        assert!(o[2047].is_finite());
    }
}
