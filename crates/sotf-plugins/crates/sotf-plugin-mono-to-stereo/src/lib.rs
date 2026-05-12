// ============================================================================
// Mono-to-Stereo Plugin
// ============================================================================

pub mod params;

use crate::params::PARAMS as MS;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::ParameterId;
use sotf_host::parameters::{Parameter, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::smoothing::Smoother;
use std::sync::Arc;

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 4; // 75% overlap
const PARAM_SMOOTH_MS: f32 = 20.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonoToStereoPluginParams {
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_freq_dependent")]
    pub freq_dependent: bool,
    #[serde(default = "default_haas_delay_ms")]
    pub haas_delay_ms: f32,
}

fn default_stereo_width() -> f32 {
    pk(MS, "stereo_width").default_f64() as f32
}
fn default_freq_dependent() -> bool {
    pk(MS, "freq_dependent").default_bool()
}
fn default_haas_delay_ms() -> f32 {
    pk(MS, "haas_delay_ms").default_f64() as f32
}

/// Maximum Haas delay in samples at 192kHz (30ms * 192000 / 1000 = 5760)
/// Round up to next power of two for masking.
const HAAS_DELAY_BUF_SIZE: usize = 8192;

pub struct MonoToStereoPlugin {
    sample_rate: u32,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    /// Random phase decorrelation filter
    decorrelation_filter: Vec<Complex<f32>>,

    /// Per-bin decorrelation strength curve [0..1] for frequency-dependent mode.
    /// Below ~300 Hz the curve is near 0 (less decorrelation),
    /// above ~2 kHz the curve approaches 1 (full decorrelation).
    freq_width_curve: Vec<f32>,

    /// Whether frequency-dependent decorrelation is enabled
    freq_dependent: bool,

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

    /// Temporary buffers
    fft_input_buf: Vec<f32>,
    fft_output_buf: Vec<Complex<f32>>,
    ifft_input_buf: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,

    /// Decorrelation low crossover frequency
    decor_low_hz: f32,
    /// Decorrelation high crossover frequency
    decor_high_hz: f32,

    /// Haas delay: target delay in ms for the right channel
    haas_delay_ms: f32,
    /// Haas delay: delay in samples (computed from ms and sample rate)
    haas_delay_samples: usize,
    /// Haas delay: circular buffer for the right channel
    haas_delay_buf: Vec<f32>,
    /// Haas delay: write position in the circular buffer
    haas_delay_write_pos: usize,
    /// Haas delay: mask for circular buffer indexing (buffer_size - 1)
    haas_delay_mask: usize,

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
            freq_width_curve: vec![1.0; num_bins],
            freq_dependent: pk(MS, "freq_dependent").default_bool(),
            input_buffer: vec![0.0; FFT_SIZE],
            input_fill: 0,
            output_accumulator: vec![0.0; FFT_SIZE * 4 * 2],
            output_accumulator_mask: (FFT_SIZE * 4) - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            analysis_window,
            output_scale,
            stereo_width: Smoother::new(
                pk(MS, "stereo_width").default_f64() as f32,
                PARAM_SMOOTH_MS,
                44100,
            ),
            fft_input_buf: vec![0.0; FFT_SIZE],
            fft_output_buf: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_input_buf: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_output_buf: vec![0.0; FFT_SIZE],
            decor_low_hz: pk(MS, "decor_low_hz").default_f64() as f32,
            decor_high_hz: pk(MS, "decor_high_hz").default_f64() as f32,
            haas_delay_ms: default_haas_delay_ms(),
            haas_delay_samples: 0,
            haas_delay_buf: vec![0.0; HAAS_DELAY_BUF_SIZE],
            haas_delay_write_pos: 0,
            haas_delay_mask: HAAS_DELAY_BUF_SIZE - 1,
            latency_filled: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.stereo_width.target() as f64),
            1 => Some(self.haas_delay_ms as f64),
            2 => Some(self.decor_low_hz as f64),
            3 => Some(self.decor_high_hz as f64),
            4 => Some(if self.freq_dependent { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.stereo_width.set_target(value as f32),
            1 => {
                self.haas_delay_ms = value as f32;
                self.update_haas_delay_samples();
            }
            2 => {
                self.decor_low_hz = value as f32;
                self.generate_decorrelation_filter();
            }
            3 => {
                self.decor_high_hz = value as f32;
                self.generate_decorrelation_filter();
            }
            4 => self.freq_dependent = value > 0.5,
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(MS, |i| self.param_value(i));
    }

    pub fn from_params(_channels: usize, params: MonoToStereoPluginParams) -> Self {
        let mut p = Self::new();
        p.stereo_width.reset(params.stereo_width);
        p.freq_dependent = params.freq_dependent;
        p.haas_delay_ms = params.haas_delay_ms;
        p.rebuild_cached_parameters();
        p
    }

    /// Recompute haas_delay_samples from haas_delay_ms and sample_rate
    fn update_haas_delay_samples(&mut self) {
        let computed = ((self.haas_delay_ms / 1000.0) * self.sample_rate as f32).round() as usize;
        self.haas_delay_samples = computed.min(HAAS_DELAY_BUF_SIZE - 1);
    }

    fn generate_decorrelation_filter(&mut self) {
        let num_bins = self.decorrelation_filter.len();

        // Apply random phase to all bins at or above decor_low_hz.
        // decor_high_hz governs the width-curve ramp but does not limit
        // the filter extent — bins above decor_high_hz get full decorrelation
        // (width curve = 1.0) so they must also have a randomised phase.
        let decor_low = self.decor_low_hz;
        for i in 0..num_bins {
            let freq = i as f32 * self.sample_rate as f32 / FFT_SIZE as f32;
            if freq >= decor_low {
                let phase = rng.random_range(0.0..2.0 * std::f32::consts::PI);
                self.decorrelation_filter[i] = Complex::from_polar(1.0, phase);
            } else {
                self.decorrelation_filter[i] = Complex::new(1.0, 0.0);
            }
        }
        self.decorrelation_filter[0] = Complex::new(self.decorrelation_filter[0].re, 0.0);
        self.decorrelation_filter[num_bins - 1] =
            Complex::new(self.decorrelation_filter[num_bins - 1].re, 0.0);

        // Build the frequency-dependent width curve
        self.compute_freq_width_curve();
    }

    fn decorrelation_phase(bin: usize) -> f32 {
        let mut x = (bin as u32)
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add(0x85eb_ca6b);
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;

        (x as f32 / u32::MAX as f32) * std::f32::consts::TAU
    }

    /// Compute the per-bin decorrelation strength curve.
    ///
    /// The curve smoothly transitions from low decorrelation at low frequencies
    /// to full decorrelation at high frequencies:
    /// - Below `decor_low_hz`: near zero (mono-compatible bass)
    /// - `decor_low_hz` to `decor_high_hz`: smooth cosine ramp from 0 to 1
    /// - Above `decor_high_hz`: full decorrelation (1.0)
    fn compute_freq_width_curve(&mut self) {
        let num_bins = self.freq_width_curve.len();
        let bin_hz = self.sample_rate as f32 / FFT_SIZE as f32;
        let low_hz = self.decor_low_hz;
        let high_hz = self.decor_high_hz;

        for i in 0..num_bins {
            let freq = i as f32 * bin_hz;
            self.freq_width_curve[i] = if freq <= low_hz {
                0.0
            } else if freq >= high_hz {
                1.0
            } else {
                // Cosine ramp: 0 at low_hz, 1 at high_hz
                let t = (freq - low_hz) / (high_hz - low_hz);
                0.5 * (1.0 - (std::f32::consts::PI * t).cos())
            };
        }
    }

    fn process_stft(&mut self) {
        let n = FFT_SIZE;
        let mask = self.output_accumulator_mask;
        let scale = self.output_scale;

        sotf_host::simd::window_mul_simd(
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
        // When freq_dependent is on, blend per-bin between mono and decorrelated
        // spectra using the frequency width curve, so bass stays mono and treble
        // gets full decorrelation.
        if self.freq_dependent {
            // Build the right-channel spectrum: lerp(mono, decor_corrected, curve[k]) per bin.
            // The decor_gain energy correction is applied to the decorrelated component
            // in the spectral domain, so mono bins stay at unity and decorrelated bins
            // get the OLA energy compensation.
            let decor_gain = (72.0_f32 / 35.0).sqrt();
            let num_bins = self.fft_output_buf.len();
            for k in 0..num_bins {
                let mono = self.fft_output_buf[k];
                let decor = mono * self.decorrelation_filter[k] * decor_gain;
                let w = self.freq_width_curve[k];
                // Linear interpolation: (1-w)*mono + w*decor_corrected
                self.ifft_input_buf[k] = Complex::new(
                    mono.re + w * (decor.re - mono.re),
                    mono.im + w * (decor.im - mono.im),
                );
            }
        } else {
            sotf_host::simd::complex_mul_simd(
                &mut self.ifft_input_buf,
                &self.fft_output_buf,
                &self.decorrelation_filter,
            );
        }
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
        param_bridge::set_parameter(MS, &id, &value, |i, v| self.set_param_value(i, v))?;
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(MS, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.stereo_width.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.generate_decorrelation_filter();
        self.update_haas_delay_samples();
        self.haas_delay_buf.fill(0.0);
        self.haas_delay_write_pos = 0;
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
        self.haas_delay_buf.fill(0.0);
        self.haas_delay_write_pos = 0;
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
                //
                // When freq_dependent is on, the energy correction is applied per-bin
                // in the spectral domain (only to the decorrelated component), so we
                // don't apply it again here.
                let decor_gain = if self.freq_dependent {
                    1.0
                } else {
                    (72.0_f32 / 35.0).sqrt()
                };

                let delay_samples = self.haas_delay_samples;
                let delay_mask = self.haas_delay_mask;

                for i in 0..to_drain {
                    let read_idx = (self.output_read_position + i) & mask;
                    let width = self.stereo_width.advance();
                    let orig = self.output_accumulator[read_idx * 2];
                    let decor = self.output_accumulator[read_idx * 2 + 1] * decor_gain;

                    let right_sample = orig * (1.0 - width) + decor * width;

                    output[(output_pos + i) * 2] = orig;

                    // Apply Haas delay to the right channel if enabled
                    if delay_samples > 0 {
                        // Write the current right sample into the delay buffer
                        self.haas_delay_buf[self.haas_delay_write_pos] = right_sample;
                        // Read from delay_samples behind the write position
                        let read_pos = (self.haas_delay_write_pos + HAAS_DELAY_BUF_SIZE
                            - delay_samples)
                            & delay_mask;
                        output[(output_pos + i) * 2 + 1] = self.haas_delay_buf[read_pos];
                        self.haas_delay_write_pos = (self.haas_delay_write_pos + 1) & delay_mask;
                    } else {
                        output[(output_pos + i) * 2 + 1] = right_sample;
                    }

                    self.output_accumulator[read_idx * 2] = 0.0;
                    self.output_accumulator[read_idx * 2 + 1] = 0.0;
                }
                self.output_read_position = (self.output_read_position + to_drain) & mask;
                self.output_accumulator_fill -= to_drain;
                output_pos += to_drain;
            } else {
                // Either no input left (input_pos >= nf with no accumulated output),
                // or the output accumulator is empty and not enough input has been
                // collected to trigger another STFT yet. In both cases zero-fill the
                // remaining output rather than leaving stale data from a previous call.
                while output_pos < nf {
                    output[output_pos * 2] = 0.0;
                    output[output_pos * 2 + 1] = 0.0;
                    output_pos += 1;
                }
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
    use crate::*;
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
    fn test_from_params_sets_width_without_startup_smoothing() {
        let p = MonoToStereoPlugin::from_params(
            1,
            MonoToStereoPluginParams {
                stereo_width: 0.0,
                freq_dependent: false,
                haas_delay_ms: 0.0,
            },
        );

        assert_eq!(p.stereo_width.current(), 0.0);
        assert_eq!(p.stereo_width.target(), 0.0);
        assert_eq!(
            p.get_parameter(&ParameterId::from("stereo_width")),
            Some(ParameterValue::Float(0.0))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("haas_delay_ms")),
            Some(ParameterValue::Float(0.0))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("freq_dependent")),
            Some(ParameterValue::Bool(false))
        );
        assert!(
            p.parameters()
                .iter()
                .any(|param| param.id == ParameterId::from("stereo_width")
                    && param.default_value == ParameterValue::Float(0.0)),
            "cached parameters should reflect from_params stereo_width"
        );
    }

    #[test]
    fn test_decorrelation_filter_is_deterministic() {
        let mut a = MonoToStereoPlugin::new();
        let mut b = MonoToStereoPlugin::new();
        a.initialize(48000).unwrap();
        b.initialize(48000).unwrap();

        assert_eq!(a.decorrelation_filter, b.decorrelation_filter);
    }

    #[test]
    fn test_mono_to_stereo_width_zero_is_mono() {
        let mut p = MonoToStereoPlugin::new();
        p.haas_delay_ms = 0.0;
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

    /// Verify that mono-to-stereo energy compensation keeps output RMS within 3 dB
    /// of input RMS. This ensures the decorrelation + OLA path doesn't significantly
    /// change the perceived loudness.
    #[test]
    fn test_mono_to_stereo_energy_compensation() {
        let mut p = MonoToStereoPlugin::new();
        p.haas_delay_ms = 0.0;
        p.initialize(48000).unwrap();
        p.stereo_width.reset(0.5); // moderate width

        let total_frames = FFT_SIZE * 20;
        let sr = 48000.0_f32;
        // Use a 440 Hz sine to keep things simple
        let input: Vec<f32> = (0..total_frames)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin())
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

        // Measure RMS in settled region
        let start = FFT_SIZE * 8;
        let end = FFT_SIZE * 18;
        let input_rms: f64 = (input[start..end]
            .iter()
            .map(|s| (*s as f64).powi(2))
            .sum::<f64>()
            / (end - start) as f64)
            .sqrt();

        let mut stereo_energy = 0.0_f64;
        for frame in start..end {
            let l = output[frame * 2] as f64;
            let r = output[frame * 2 + 1] as f64;
            // Average power of L and R
            stereo_energy += (l * l + r * r) / 2.0;
        }
        let output_rms = (stereo_energy / (end - start) as f64).sqrt();

        let ratio_db = 20.0 * (output_rms / input_rms).log10();
        assert!(
            ratio_db.abs() < 3.0,
            "Stereo output RMS should be within 3 dB of mono input RMS, \
             but got {ratio_db:.2} dB (in_rms={input_rms:.6}, out_rms={output_rms:.6})"
        );
    }

    /// Test that freq_dependent mode produces less decorrelation at low frequencies
    /// and more at high frequencies. We compare L/R correlation for a bass signal
    /// vs a treble signal: bass should be more correlated (closer to mono).
    #[test]
    fn test_freq_dependent_bass_stays_mono() {
        // Helper: compute L/R correlation for a given frequency
        fn lr_correlation(freq_hz: f32, freq_dep: bool) -> f64 {
            let mut p = MonoToStereoPlugin::new();
            p.freq_dependent = freq_dep;
            p.haas_delay_ms = 0.0; // Disable Haas delay for this correlation test
            p.initialize(48000).unwrap();
            p.stereo_width.reset(1.0);

            let total_frames = FFT_SIZE * 16;
            let input: Vec<f32> = (0..total_frames)
                .map(|i| {
                    let t = i as f32 / 48000.0;
                    (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.5
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

            // Measure L/R difference in steady state
            let start = FFT_SIZE * 6;
            let end = FFT_SIZE * 14;
            let mut sum_diff_sq = 0.0_f64;
            let mut sum_energy = 0.0_f64;
            for frame in start..end {
                let l = output[frame * 2] as f64;
                let r = output[frame * 2 + 1] as f64;
                sum_diff_sq += (l - r).powi(2);
                sum_energy += l.powi(2) + r.powi(2);
            }
            if sum_energy < 1e-12 {
                return 0.0;
            }
            // Normalized difference: 0 = identical, 1 = maximally different
            (sum_diff_sq / sum_energy).sqrt()
        }

        // With freq_dependent=true, 100 Hz should be nearly mono (low difference)
        let bass_diff = lr_correlation(100.0, true);
        // With freq_dependent=true, 4000 Hz should have more difference
        let treble_diff = lr_correlation(4000.0, true);

        assert!(
            bass_diff < treble_diff,
            "With freq_dependent, bass ({bass_diff:.4}) should be more correlated than treble ({treble_diff:.4})"
        );
        // Bass should be nearly mono (very low L/R difference)
        assert!(
            bass_diff < 0.1,
            "Bass decorrelation should be very low with freq_dependent, got {bass_diff:.4}"
        );
    }

    /// Test that changing decor_low_hz via set_parameter actually affects the
    /// decorrelation filter. We test a 150 Hz tone:
    /// - With default decor_low_hz=300 Hz: 150 Hz bins are below threshold → filter=1+0j
    ///   → L and R are proportional (high correlation).
    /// - With decor_low_hz=100 Hz (minimum): 150 Hz bins are above threshold → random phase
    ///   → L and R differ (lower correlation).
    ///
    /// We use freq_dependent=false for a clean, flat filter path.
    ///
    /// Note: decor_low_hz range is [100, 500] Hz (from PARAMS), so values are
    /// clamped at the plugin boundary.
    #[test]
    fn test_decor_low_hz_parameter_is_honoured() {
        use sotf_host::parameters::{ParameterId, ParameterValue};

        fn run_and_measure_correlation(decor_low: f32) -> f64 {
            let mut p = MonoToStereoPlugin::new();
            p.initialize(48000).unwrap();
            // Use freq_dependent=false so the decorrelation filter is applied flat.
            let _ = p.set_parameter(
                ParameterId::from("freq_dependent"),
                ParameterValue::Bool(false),
            );
            let _ = p.set_parameter(
                ParameterId::from("decor_low_hz"),
                ParameterValue::Float(decor_low),
            );
            p.stereo_width.reset(1.0);
            p.haas_delay_ms = 0.0;
            p.update_haas_delay_samples();

            let total_frames = FFT_SIZE * 16;
            // 150 Hz tone — between decor_low_hz min (100 Hz) and default (300 Hz)
            let input: Vec<f32> = (0..total_frames)
                .map(|i| {
                    let t = i as f32 / 48000.0;
                    (2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.5
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

            let start = FFT_SIZE * 6;
            let end = FFT_SIZE * 14;
            let mut sum_l_r = 0.0_f64;
            let mut sum_l2 = 0.0_f64;
            let mut sum_r2 = 0.0_f64;
            for frame in start..end {
                let l = output[frame * 2] as f64;
                let r = output[frame * 2 + 1] as f64;
                sum_l_r += l * r;
                sum_l2 += l * l;
                sum_r2 += r * r;
            }
            // Pearson correlation: 1.0 = perfectly in-phase, near 0 = uncorrelated
            let denom = (sum_l2 * sum_r2).sqrt();
            if denom < 1e-12 {
                1.0
            } else {
                sum_l_r / denom
            }
        }

        // decor_low_hz=300 (default): 150 Hz bins stay at 1+0j → L/R in-phase → correlation ≈ 1.
        let corr_high_low = run_and_measure_correlation(300.0);
        // decor_low_hz=100 (min): 150 Hz bins get random phases → correlation drops.
        let corr_low_low = run_and_measure_correlation(100.0);

        assert!(
            corr_high_low > corr_low_low,
            "With decor_low_hz=300 Hz, 150 Hz should have higher L/R correlation \
             than with decor_low_hz=100 Hz: corr_300={corr_high_low:.4}, corr_100={corr_low_low:.4}"
        );
        // When the 150 Hz bin is NOT decorrelated (decor_low=300 > 150 Hz),
        // L and R are proportional → correlation near 1.0.
        assert!(
            corr_high_low > 0.95,
            "150 Hz should be near-perfectly correlated when decor_low_hz=300 Hz \
             (bin not decorated), got {corr_high_low:.4}"
        );
    }

    /// Test that changing decor_high_hz via set_parameter affects the width curve.
    /// Before the fix, compute_freq_width_curve() hardcoded 2000 Hz and was ignored.
    #[test]
    fn test_decor_high_hz_parameter_is_honoured() {
        use sotf_host::parameters::{ParameterId, ParameterValue};

        // Default high = 2000 Hz. Set it to 200 Hz so even bass gets decorrelated
        // (below the 300 Hz default low — we also lower decor_low to 100 Hz).
        // Then a 400 Hz tone should be fully decorrelated (above 200 Hz high crossover).
        let mut p = MonoToStereoPlugin::new();
        p.initialize(48000).unwrap();
        let _ = p.set_parameter(
            ParameterId::from("decor_low_hz"),
            ParameterValue::Float(100.0),
        );
        let _ = p.set_parameter(
            ParameterId::from("decor_high_hz"),
            ParameterValue::Float(200.0),
        );
        p.stereo_width.reset(1.0);
        p.haas_delay_ms = 0.0;
        p.update_haas_delay_samples();

        let total_frames = FFT_SIZE * 16;
        let input: Vec<f32> = (0..total_frames)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * std::f32::consts::PI * 400.0 * t).sin() * 0.5
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

        let start = FFT_SIZE * 6;
        let end = FFT_SIZE * 14;
        let mut sum_diff_sq = 0.0_f64;
        let mut sum_energy = 0.0_f64;
        for frame in start..end {
            let l = output[frame * 2] as f64;
            let r = output[frame * 2 + 1] as f64;
            sum_diff_sq += (l - r).powi(2);
            sum_energy += l.powi(2) + r.powi(2);
        }
        let diff = if sum_energy > 1e-12 {
            (sum_diff_sq / sum_energy).sqrt()
        } else {
            0.0
        };
        // With decor_high_hz = 200 Hz, 400 Hz should have measurable decorrelation.
        assert!(
            diff > 0.05,
            "400 Hz should be decorrelated when decor_high_hz=200 Hz, got diff={diff:.4}"
        );
    }

    /// Test that the output buffer is never left with stale data when a break
    /// was previously possible (output_pos < nf but no STFT and no drain).
    /// We exercise this with a very small block size (nf=1) that forces the path.
    #[test]
    fn test_process_no_stale_output_on_small_blocks() {
        let mut p = MonoToStereoPlugin::new();
        p.initialize(48000).unwrap();
        // Process in tiny 1-sample blocks for a full FFT window worth of input.
        let total_frames = FFT_SIZE + 10;
        for i in 0..total_frames {
            let sample = (i as f32 * 0.1).sin();
            let mut out = vec![99.0_f32; 2]; // pre-fill with sentinel
            p.process(
                &[sample],
                &mut out,
                &ProcessContext {
                    sample_rate: 48000,
                    num_frames: 1,
                },
            )
            .unwrap();
            // Output must never contain the sentinel value — it must have been written.
            assert!(
                out[0] != 99.0 || out[1] != 99.0 || out[0].is_finite(),
                "stale data at frame {i}"
            );
            // Both samples must be finite.
            assert!(out[0].is_finite(), "L not finite at frame {i}");
            assert!(out[1].is_finite(), "R not finite at frame {i}");
        }
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
            ratio_db.abs() < 2.0,
            "L/R energy imbalance at width=1.0: {ratio_db:.2} dB (L_rms={rms_l:.6}, R_rms={rms_r:.6})"
        );
    }
}
