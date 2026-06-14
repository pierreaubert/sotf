use super::consts::FFT_SIZE;
use super::consts::HAAS_DELAY_BUF_SIZE;
use super::consts::HOP_SIZE;
use super::consts::PARAM_SMOOTH_MS;
use super::default::default_haas_delay_ms;
use super::types::MonoToStereoPluginParams;
use crate::params::PARAMS as MS;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::ParameterId;
use sotf_host::parameters::{Parameter, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::smoothing::Smoother;
use std::sync::Arc;

pub struct MonoToStereoPlugin {
    pub(super) sample_rate: u32,
    pub(super) fft_forward: Arc<dyn RealToComplex<f32>>,
    pub(super) fft_inverse: Arc<dyn ComplexToReal<f32>>,

    /// Random phase decorrelation filter
    pub(super) decorrelation_filter: Vec<Complex<f32>>,

    /// Per-bin decorrelation strength curve [0..1] for frequency-dependent mode.
    /// Below ~300 Hz the curve is near 0 (less decorrelation),
    /// above ~2 kHz the curve approaches 1 (full decorrelation).
    pub(super) freq_width_curve: Vec<f32>,

    /// Whether frequency-dependent decorrelation is enabled
    pub(super) freq_dependent: bool,

    /// Flat input buffer
    pub(super) input_buffer: Vec<f32>,
    pub(super) input_fill: usize,

    /// Interleaved output ring buffer [L0, R0, L1, R1, ...]
    pub(super) output_accumulator: Vec<f32>,
    pub(super) output_accumulator_mask: usize,
    pub(super) output_accumulator_fill: usize,
    pub(super) next_add_position: usize,
    pub(super) output_read_position: usize,

    pub(super) analysis_window: Vec<f32>,
    pub(super) output_scale: f32,

    /// Smoothers
    pub(super) stereo_width: Smoother,

    /// Temporary buffers
    pub(super) fft_input_buf: Vec<f32>,
    pub(super) fft_output_buf: Vec<Complex<f32>>,
    pub(super) ifft_input_buf: Vec<Complex<f32>>,
    pub(super) ifft_output_buf: Vec<f32>,

    /// Decorrelation low crossover frequency
    pub(super) decor_low_hz: f32,
    /// Decorrelation high crossover frequency
    pub(super) decor_high_hz: f32,

    /// Haas delay: target delay in ms for the right channel
    pub(super) haas_delay_ms: f32,
    /// Haas delay: delay in samples (computed from ms and sample rate)
    pub(super) haas_delay_samples: usize,
    /// Haas delay: circular buffer for the right channel
    pub(super) haas_delay_buf: Vec<f32>,
    /// Haas delay: write position in the circular buffer
    pub(super) haas_delay_write_pos: usize,
    /// Haas delay: mask for circular buffer indexing (buffer_size - 1)
    pub(super) haas_delay_mask: usize,

    pub(super) latency_filled: usize,
    pub(super) cached_parameters: Vec<Parameter>,
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
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
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
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
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

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(MS, |i| self.param_value(i));
    }

    pub fn from_params(_channels: usize, params: MonoToStereoPluginParams) -> Self {
        let mut p = Self::new();
        p.stereo_width.set_target(params.stereo_width);
        p.freq_dependent = params.freq_dependent;
        p.haas_delay_ms = params.haas_delay_ms;
        p
    }

    /// Recompute haas_delay_samples from haas_delay_ms and sample_rate
    pub(super) fn update_haas_delay_samples(&mut self) {
        let computed = ((self.haas_delay_ms / 1000.0) * self.sample_rate as f32).round() as usize;
        self.haas_delay_samples = computed.min(HAAS_DELAY_BUF_SIZE - 1);
    }

    pub(super) fn generate_decorrelation_filter(&mut self) {
        let num_bins = self.decorrelation_filter.len();

        // Apply random phase to all bins at or above decor_low_hz.
        // decor_high_hz governs the width-curve ramp but does not limit
        // the filter extent — bins above decor_high_hz get full decorrelation
        // (width curve = 1.0) so they must also have a randomised phase.
        let decor_low = self.decor_low_hz;
        for i in 0..num_bins {
            let freq = i as f32 * self.sample_rate as f32 / FFT_SIZE as f32;
            if freq >= decor_low {
                let phase = Self::decorrelation_phase(i);
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

    pub(super) fn decorrelation_phase(bin: usize) -> f32 {
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
    pub(super) fn compute_freq_width_curve(&mut self) {
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

    pub(super) fn process_stft(&mut self) -> Result<(), String> {
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
            .map_err(|e| format!("FFT forward failed: {:?}", e))?;

        // Left channel: latent mono
        self.ifft_input_buf.copy_from_slice(&self.fft_output_buf);
        self.fft_inverse
            .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
            .map_err(|e| format!("FFT inverse failed (left): {:?}", e))?;
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
            .map_err(|e| format!("FFT inverse failed (right): {:?}", e))?;
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            let s = self.ifft_output_buf[i] * self.analysis_window[i] * scale;
            self.output_accumulator[idx * 2 + 1] += s;
        }

        self.next_add_position = (self.next_add_position + HOP_SIZE) & mask;
        self.output_accumulator_fill += HOP_SIZE;
        self.latency_filled += HOP_SIZE;
        Ok(())
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
                self.process_stft()?;
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
        // Report only the STFT pipeline latency. The optional Haas delay is a
        // deliberate right-channel widening effect; host latency compensation
        // must not time-align it away.
        FFT_SIZE
    }
}
