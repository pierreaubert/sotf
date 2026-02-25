// ============================================================================
// Mono-to-Stereo Plugin
// ============================================================================

use super::param_specs::mono_to_stereo::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 4; // 75% overlap
const PARAM_SMOOTH_MS: f32 = 20.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonoToStereoPluginParams {
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_comp_eq_depth_db")]
    pub comp_eq_depth_db: f32,
}

fn default_stereo_width() -> f32 {
    STEREO_WIDTH_DEFAULT
}
fn default_comp_eq_depth_db() -> f32 {
    COMP_EQ_DEPTH_DB_DEFAULT
}

pub struct MonoToStereoPlugin {
    sample_rate: u32,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    /// Random phase decorrelation filter
    decorrelation_filter: Vec<Complex<f32>>,

    /// Flat input buffer
    input_buffer: Vec<f32>,
    input_fill: usize,

    /// Interleaved output ring buffer [L0, R0, L1, R1, ...]
    output_accumulator: Vec<f32>,
    output_accumulator_mask: usize,
    output_accumulator_fill: usize,
    next_add_position: usize,
    output_read_position: usize,

    analysis_window: Vec<f32>,
    output_scale: f32,

    /// Smoothers
    stereo_width: Smoother,
    comp_eq_depth: Smoother,

    /// Temporary buffers
    fft_input_buf: Vec<f32>,
    fft_output_buf: Vec<Complex<f32>>,
    ifft_input_buf: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,

    param_stereo_width: ParameterId,
    param_comp_eq_depth_db: ParameterId,
    latency_filled: usize,
    cached_parameters: Vec<Parameter>,
}

impl Default for MonoToStereoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl MonoToStereoPlugin {
    pub fn new() -> Self {
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

        // 75% overlap dual-window scaling: Sum(w^2) = 1.5
        let output_scale = 1.0 / (FFT_SIZE as f32 * 1.5);

        let mut p = Self {
            sample_rate: 44100,
            fft_forward,
            fft_inverse,
            decorrelation_filter: vec![Complex::new(1.0, 0.0); num_bins],
            input_buffer: vec![0.0; FFT_SIZE],
            input_fill: 0,
            output_accumulator: vec![0.0; FFT_SIZE * 4 * 2],
            output_accumulator_mask: (FFT_SIZE * 4) - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            analysis_window,
            output_scale,
            stereo_width: Smoother::new(STEREO_WIDTH_DEFAULT, PARAM_SMOOTH_MS, 44100),
            comp_eq_depth: Smoother::new(COMP_EQ_DEPTH_DB_DEFAULT, PARAM_SMOOTH_MS, 44100),
            fft_input_buf: vec![0.0; FFT_SIZE],
            fft_output_buf: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_input_buf: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_output_buf: vec![0.0; FFT_SIZE],
            param_stereo_width: ParameterId::from("stereo_width"),
            param_comp_eq_depth_db: ParameterId::from("comp_eq_depth_db"),
            latency_filled: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "stereo_width",
                "Stereo Width",
                self.stereo_width.target(),
                0.0,
                1.0,
            ),
            Parameter::new_float(
                "comp_eq_depth_db",
                "Comp EQ Depth",
                self.comp_eq_depth.target(),
                0.0,
                12.0,
            ),
        ];
    }

    pub fn from_params(_channels: usize, params: MonoToStereoPluginParams) -> Self {
        let mut p = Self::new();
        p.stereo_width.set_target(params.stereo_width);
        p.comp_eq_depth.set_target(params.comp_eq_depth_db);
        p
    }

    fn generate_decorrelation_filter(&mut self) {
        let mut rng = rand::rng();
        use rand::Rng;
        let num_bins = self.decorrelation_filter.len();

        for i in 0..num_bins {
            let freq = i as f32 * self.sample_rate as f32 / FFT_SIZE as f32;
            if (300.0..=15000.0).contains(&freq) {
                let phase = rng.random_range(0.0..2.0 * std::f32::consts::PI);
                if i == 100 {
                    // println!("DEBUG: bin 100 phase={}", phase);
                }
                self.decorrelation_filter[i] = Complex::from_polar(1.0, phase);
            } else {
                self.decorrelation_filter[i] = Complex::new(1.0, 0.0);
            }
        }
        self.decorrelation_filter[0] = Complex::new(self.decorrelation_filter[0].re, 0.0);
        self.decorrelation_filter[num_bins - 1] =
            Complex::new(self.decorrelation_filter[num_bins - 1].re, 0.0);
    }

    fn process_stft(&mut self) {
        let n = FFT_SIZE;
        let mask = self.output_accumulator_mask;
        let scale = self.output_scale;

        super::simd::window_mul_simd(
            &mut self.fft_input_buf,
            &self.input_buffer,
            &self.analysis_window,
        );
        self.fft_forward
            .process(&mut self.fft_input_buf, &mut self.fft_output_buf)
            .unwrap();

        // Left channel: latent mono
        self.ifft_input_buf.copy_from_slice(&self.fft_output_buf);
        self.fft_inverse
            .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
            .unwrap();
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            let s = self.ifft_output_buf[i] * self.analysis_window[i] * scale;
            self.output_accumulator[idx * 2] += s;
        }

        // Right channel: decorrelated
        super::simd::complex_mul_simd(
            &mut self.ifft_input_buf,
            &self.fft_output_buf,
            &self.decorrelation_filter,
        );
        self.fft_inverse
            .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
            .unwrap();
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            let s = self.ifft_output_buf[i] * self.analysis_window[i] * scale;
            self.output_accumulator[idx * 2 + 1] += s;
        }

        self.next_add_position = (self.next_add_position + HOP_SIZE) & mask;
        self.output_accumulator_fill += HOP_SIZE;
        self.latency_filled += HOP_SIZE;
    }
}

impl Plugin for MonoToStereoPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("MonoToStereo", "2.0.0", "Sotf")
    }

    fn input_channels(&self) -> usize {
        1
    }
    fn output_channels(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_stereo_width {
            let v = value.as_float().unwrap_or(STEREO_WIDTH_DEFAULT);
            if v.is_finite() {
                self.stereo_width.set_target(v);
            }
        } else if id == self.param_comp_eq_depth_db {
            let v = value.as_float().unwrap_or(COMP_EQ_DEPTH_DB_DEFAULT);
            if v.is_finite() {
                self.comp_eq_depth.set_target(v);
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_stereo_width {
            Some(ParameterValue::Float(self.stereo_width.target()))
        } else if id == &self.param_comp_eq_depth_db {
            Some(ParameterValue::Float(self.comp_eq_depth.target()))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.stereo_width.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.comp_eq_depth.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.generate_decorrelation_filter();
        Ok(())
    }

    fn reset(&mut self) {
        self.input_buffer.fill(0.0);
        self.input_fill = 0;
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;
        self.stereo_width.reset(self.stereo_width.target());
        self.comp_eq_depth.reset(self.comp_eq_depth.target());
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;
        let mut input_pos = 0;
        let mut output_pos = 0;
        let mask = self.output_accumulator_mask;

        while output_pos < nf {
            if input_pos < nf {
                let to_copy = (FFT_SIZE - self.input_fill).min(nf - input_pos);
                self.input_buffer[self.input_fill..self.input_fill + to_copy]
                    .copy_from_slice(&input[input_pos..input_pos + to_copy]);
                self.input_fill += to_copy;
                input_pos += to_copy;
            }

            while self.input_fill >= FFT_SIZE {
                self.process_stft();
                self.input_buffer.copy_within(HOP_SIZE..FFT_SIZE, 0);
                self.input_fill = FFT_SIZE - HOP_SIZE;
            }

            let to_drain = self.output_accumulator_fill.min(nf - output_pos);
            if to_drain > 0 {
                // Dual-window OLA energy correction for decorrelated signal.
                // Random-phase decorrelation spreads energy uniformly in time,
                // so the synthesis window attenuates it more than the coherent path.
                // Factor = COLA / sqrt(sum(w^4)/hop) = 1.5 / sqrt(35/32) ≈ 1.434
                // for Hann window with 75% overlap.
                let decor_gain = (72.0_f32 / 35.0).sqrt();

                for i in 0..to_drain {
                    let read_idx = (self.output_read_position + i) & mask;
                    let width = self.stereo_width.advance();
                    let orig = self.output_accumulator[read_idx * 2];
                    let decor = self.output_accumulator[read_idx * 2 + 1] * decor_gain;

                    output[(output_pos + i) * 2] = orig;
                    output[(output_pos + i) * 2 + 1] = orig * (1.0 - width) + decor * width;

                    self.output_accumulator[read_idx * 2] = 0.0;
                    self.output_accumulator[read_idx * 2 + 1] = 0.0;
                }
                self.output_read_position = (self.output_read_position + to_drain) & mask;
                self.output_accumulator_fill -= to_drain;
                output_pos += to_drain;
            } else if input_pos >= nf {
                while output_pos < nf {
                    output[output_pos * 2] = 0.0;
                    output[output_pos * 2 + 1] = 0.0;
                    output_pos += 1;
                }
            } else {
                break;
            }
        }
        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        FFT_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mono_to_stereo_basic() {
        let mut p = MonoToStereoPlugin::new();
        p.initialize(48000).unwrap();
        let i = vec![0.5; 1024];
        let mut o = vec![0.0; 2048];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1024,
            },
        )
        .unwrap();
        assert!(o[2047].is_finite());
    }

    #[test]
    fn test_mono_to_stereo_width_zero_is_mono() {
        let mut p = MonoToStereoPlugin::new();
        p.initialize(48000).unwrap();
        p.stereo_width.reset(0.0);
        let total_frames = FFT_SIZE * 10;
        let input: Vec<f32> = (0..total_frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![0.0; total_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: total_frames,
            },
        )
        .unwrap();
        for frame in (FFT_SIZE * 5)..(FFT_SIZE * 6) {
            let l = output[frame * 2];
            let r = output[frame * 2 + 1];
            assert!(
                (l - r).abs() < 1e-5,
                "L/R differ at frame {frame}: L={l}, R={r}"
            );
        }
    }

    #[test]
    fn test_mono_to_stereo_width_one_differs() {
        let mut p = MonoToStereoPlugin::new();
        p.initialize(48000).unwrap();
        p.stereo_width.reset(1.0);
        let total_frames = FFT_SIZE * 10;
        let input: Vec<f32> = (0..total_frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![0.0; total_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: total_frames,
            },
        )
        .unwrap();
        let mut any_differ = false;
        let mut non_zero = false;
        for frame in (FFT_SIZE * 5)..(FFT_SIZE * 6) {
            let l = output[frame * 2];
            let r = output[frame * 2 + 1];
            if l.abs() > 1e-4 || r.abs() > 1e-4 {
                non_zero = true;
            }
            if (l - r).abs() > 1e-3 {
                any_differ = true;
                break;
            }
        }
        assert!(
            non_zero,
            "Output should not be zero in the middle of the stream"
        );
        assert!(any_differ, "L and R should differ at width=1.0");
    }

    /// Test L/R energy balance at width=1.0 using broadband noise.
    /// A single sine is unstable because the random-phase decorrelation filter
    /// shifts a tonal signal by a random amount, causing large OLA variance.
    /// Broadband content averages across many bins and gives a stable ratio.
    #[test]
    fn test_mono_to_stereo_lr_energy_balance() {
        let mut p = MonoToStereoPlugin::new();
        p.initialize(48000).unwrap();
        p.stereo_width.reset(1.0);
        let total_frames = FFT_SIZE * 32;
        // Sum of many sines for broadband coverage (300–15000 Hz decorrelation band)
        let input: Vec<f32> = (0..total_frames)
            .map(|i| {
                let t = i as f32 / 48000.0;
                let mut s = 0.0_f32;
                let mut freq = 200.0;
                while freq < 16000.0 {
                    s += (2.0 * std::f32::consts::PI * freq * t).sin();
                    freq *= 1.07; // ~40 frequencies, roughly 1/3 octave spacing
                }
                s * 0.02 // scale to avoid clipping
            })
            .collect();
        let mut output = vec![0.0; total_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: total_frames,
            },
        )
        .unwrap();

        // Skip warmup, measure steady-state RMS
        let start = FFT_SIZE * 10;
        let end = FFT_SIZE * 28;
        let mut rms_l = 0.0_f64;
        let mut rms_r = 0.0_f64;
        for frame in start..end {
            rms_l += (output[frame * 2] as f64).powi(2);
            rms_r += (output[frame * 2 + 1] as f64).powi(2);
        }
        let n = (end - start) as f64;
        rms_l = (rms_l / n).sqrt();
        rms_r = (rms_r / n).sqrt();
        let ratio_db = 20.0 * (rms_r / rms_l).log10();
        assert!(
            ratio_db.abs() < 1.0,
            "L/R energy imbalance at width=1.0: {ratio_db:.2} dB (L_rms={rms_l:.6}, R_rms={rms_r:.6})"
        );
    }
}
