use super::consts::FFT_SIZE;
use super::consts::HOP_SIZE;
use super::consts::PARAM_SMOOTH_MS;
use super::types::DownmixCoeffs;
use super::types::DownmixPluginParams;
use crate::params::PARAMS as DM;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::ParameterId;
use sotf_host::parameters::ParameterValue;
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::smoothing::Smoother;
use sotf_host::speaker_config::{
    SpeakerConfig, get_speaker_config, get_speaker_config_by_channels,
};
use std::sync::Arc;

pub struct DownmixPlugin {
    pub(super) input_ch: usize,
    pub(super) sample_rate: u32,
    pub(super) speaker_config: Option<&'static SpeakerConfig>,
    pub(crate) target_coeffs: Vec<DownmixCoeffs>,
    pub(super) coeff_smoothers: Vec<Smoother>,
    pub(super) lfe_channels: Vec<usize>,
    pub(super) lfe_is_channel: Vec<bool>,

    pub(super) fft_forward: Arc<dyn RealToComplex<f32>>,
    pub(super) fft_inverse: Arc<dyn ComplexToReal<f32>>,

    pub(super) analysis_window: Vec<f32>,
    pub(super) output_scale: f32,

    /// Flat input buffer: [channel * FFT_SIZE + sample]
    pub(super) input_buffer: Vec<f32>,
    pub(super) input_fill: usize,

    pub(super) output_accumulator: Vec<f32>,
    pub(super) output_accumulator_mask: usize,
    pub(super) output_accumulator_fill: usize,
    pub(super) next_add_position: usize,
    pub(super) output_read_position: usize,
    /// Samples left on the fixed causal STFT delay before accumulated output is read.
    pub(super) startup_delay_remaining: usize,

    /// Flat FFT output: [channel * num_bins + bin]
    pub(super) fft_output: Vec<Complex<f32>>,
    pub(super) out_freq_l: Vec<Complex<f32>>,
    pub(super) out_freq_r: Vec<Complex<f32>>,

    pub(super) fft_input_buf: Vec<f32>,
    pub(super) ifft_input_buf: Vec<Complex<f32>>,
    pub(super) ifft_output_buf: Vec<f32>,

    pub(super) lfe_lpf: Vec<[Biquad; 2]>,

    pub(super) center_gain_db: f32,
    pub(super) surround_gain_db: f32,
    pub(super) height_gain_db: f32,
    pub(super) lfe_gain_db: f32,
    pub(super) phase_coherence: bool,
    pub(super) phase_blend_low_hz: f32,
    pub(super) phase_blend_high_hz: f32,
    pub(super) itu_mode: bool,
    pub(super) matrix_ltrt: bool,

    pub(super) cached_parameters: Vec<sotf_host::parameters::Parameter>,
}

impl DownmixPlugin {
    const MAX_INPUT_CHANNELS: usize = 32;

    pub fn new(input_channels: usize) -> Self {
        Self::try_new(input_channels).expect("invalid Downmix channel count")
    }

    pub fn try_new(input_channels: usize) -> PluginResult<Self> {
        if !(1..=Self::MAX_INPUT_CHANNELS).contains(&input_channels) {
            return Err(format!(
                "Downmix input channel count must be between 1 and {}, got {input_channels}",
                Self::MAX_INPUT_CHANNELS
            ));
        }
        FFT_SIZE
            .checked_mul(input_channels)
            .ok_or_else(|| "Downmix input buffer size overflow".to_string())?;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);
        let num_bins = FFT_SIZE / 2 + 1;

        // sqrt-Hann window: product of analysis * synthesis = Hann, which satisfies COLA
        // at 50% overlap (hop = N/2). This ensures perfect reconstruction in WOLA.
        // Full Hann (w²) does NOT satisfy COLA at any standard overlap because
        //   w²[i] + w²[i+N/2] = 0.75 + 0.25*cos(4πi/N) ≠ constant.
        // sqrt-Hann satisfies COLA because:
        //   hann[i] + hann[i+N/2] = 1.0 (exactly constant).
        let analysis_window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let x = i as f32 / FFT_SIZE as f32;
                (0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())).sqrt()
            })
            .collect();

        // WOLA scale: sqrt-Hann² at 50% overlap gives COLA constant = 1.0.
        // realfft IFFT is unnormalized (output = N * input), so we divide by N.
        // output_scale = 1/N ensures unity gain reconstruction.
        let output_scale = 1.0 / FFT_SIZE as f32;

        let mut p = Self {
            input_ch: input_channels,
            sample_rate: 44100,
            speaker_config: get_speaker_config_by_channels(input_channels),
            target_coeffs: vec![DownmixCoeffs::default(); input_channels],
            coeff_smoothers: Vec::with_capacity(input_channels * 2),
            lfe_channels: Vec::with_capacity(input_channels),
            lfe_is_channel: vec![false; input_channels],
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
            startup_delay_remaining: FFT_SIZE,
            fft_output: vec![Complex::new(0.0, 0.0); num_bins * input_channels],
            out_freq_l: vec![Complex::new(0.0, 0.0); num_bins],
            out_freq_r: vec![Complex::new(0.0, 0.0); num_bins],
            fft_input_buf: vec![0.0; FFT_SIZE],
            ifft_input_buf: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_output_buf: vec![0.0; FFT_SIZE],
            lfe_lpf: Vec::new(),
            center_gain_db: pk(DM, "center_gain_db").default_f64() as f32,
            surround_gain_db: pk(DM, "surround_gain_db").default_f64() as f32,
            height_gain_db: pk(DM, "height_gain_db").default_f64() as f32,
            lfe_gain_db: pk(DM, "lfe_gain_db").default_f64() as f32,
            phase_coherence: pk(DM, "phase_coherence").default_bool(),
            phase_blend_low_hz: pk(DM, "phase_blend_low_hz").default_f64() as f32,
            phase_blend_high_hz: pk(DM, "phase_blend_high_hz").default_f64() as f32,
            itu_mode: pk(DM, "itu_mode").default_bool(),
            matrix_ltrt: false,
            cached_parameters: Vec::new(),
        };
        p.compute_coefficients(true);
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.center_gain_db as f64),
            1 => Some(self.surround_gain_db as f64),
            2 => Some(self.height_gain_db as f64),
            3 => Some(self.lfe_gain_db as f64),
            4 => Some(if self.phase_coherence { 1.0 } else { 0.0 }),
            5 => Some(self.phase_blend_low_hz as f64),
            6 => Some(self.phase_blend_high_hz as f64),
            7 => Some(if self.itu_mode { 1.0 } else { 0.0 }),
            8 => Some(if self.matrix_ltrt { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.center_gain_db = value as f32,
            1 => self.surround_gain_db = value as f32,
            2 => self.height_gain_db = value as f32,
            3 => self.lfe_gain_db = value as f32,
            4 => self.phase_coherence = value > 0.5,
            5 => self.phase_blend_low_hz = value as f32,
            6 => self.phase_blend_high_hz = value as f32,
            7 => self.itu_mode = value > 0.5,
            8 => self.matrix_ltrt = value > 0.5,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(DM, |i| self.param_value(i));
    }

    pub fn from_params(params: DownmixPluginParams) -> Self {
        Self::try_from_params(params).expect("invalid Downmix parameters")
    }

    pub fn try_from_params(params: DownmixPluginParams) -> PluginResult<Self> {
        let values = [
            ("center_gain_db", params.center_gain_db, -12.0, 0.0),
            ("surround_gain_db", params.surround_gain_db, -12.0, 0.0),
            ("height_gain_db", params.height_gain_db, -60.0, 0.0),
            ("lfe_gain_db", params.lfe_gain_db, -60.0, 0.0),
            (
                "phase_blend_low_hz",
                params.phase_blend_low_hz,
                100.0,
                1000.0,
            ),
            (
                "phase_blend_high_hz",
                params.phase_blend_high_hz,
                1000.0,
                5000.0,
            ),
        ];
        for (name, value, min, max) in values {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "Downmix {name} must be finite and in [{min}, {max}], got {value}"
                ));
            }
        }
        if params.phase_blend_low_hz >= params.phase_blend_high_hz {
            return Err("Downmix phase_blend_low_hz must be below phase_blend_high_hz".into());
        }
        if params.phase_coherence && params.matrix_ltrt {
            return Err(
                "Downmix phase_coherence and matrix_ltrt are mutually exclusive structural modes"
                    .into(),
            );
        }

        let explicit_config = if let Some(layout) = params.input_layout.as_deref() {
            let config = get_speaker_config(layout)
                .ok_or_else(|| format!("Downmix input_layout '{layout}' is not supported"))?;
            if config.speakers.len() != params.input_channels {
                return Err(format!(
                    "Downmix input_layout '{layout}' has {} channels, but input_channels is {}",
                    config.speakers.len(),
                    params.input_channels
                ));
            }
            Some(config)
        } else {
            if matches!(params.input_channels, 8 | 10) {
                return Err(format!(
                    "Downmix input_layout is required for ambiguous {}-channel input",
                    params.input_channels
                ));
            }
            None
        };

        let mut plugin = Self::try_new(params.input_channels)?;
        if let Some(config) = explicit_config {
            plugin.speaker_config = Some(config);
        }
        plugin.center_gain_db = params.center_gain_db;
        plugin.surround_gain_db = params.surround_gain_db;
        plugin.height_gain_db = params.height_gain_db;
        plugin.lfe_gain_db = params.lfe_gain_db;
        plugin.phase_coherence = params.phase_coherence;
        plugin.phase_blend_low_hz = params.phase_blend_low_hz;
        plugin.phase_blend_high_hz = params.phase_blend_high_hz;
        plugin.itu_mode = params.itu_mode;
        plugin.matrix_ltrt = params.matrix_ltrt;
        plugin.compute_coefficients(true);
        plugin.rebuild_cached_parameters();
        plugin.startup_delay_remaining = if plugin.uses_spectral_path() {
            FFT_SIZE
        } else {
            0
        };
        Ok(plugin)
    }

    #[inline]
    fn uses_spectral_path(&self) -> bool {
        self.phase_coherence || self.matrix_ltrt
    }

    fn clear_stream_state(&mut self) {
        self.input_buffer.fill(0.0);
        self.input_fill = 0;
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.startup_delay_remaining = if self.uses_spectral_path() {
            FFT_SIZE
        } else {
            0
        };
    }

    /// Compute ITU-R BS.775 standard coefficients for 5.1 → stereo downmix.
    /// L_out = L + 0.707*C + 0.707*Ls
    /// R_out = R + 0.707*C + 0.707*Rs
    /// LFE is discarded (standard practice for ITU-R BS.775).
    ///
    /// For non-5.1 layouts, the ITU mode extends the same principle:
    /// - Front L/R pass through at unity
    /// - Center at -3 dB (0.707) to both
    /// - All surround channels at -3 dB (0.707) panned L/R by azimuth
    /// - Height channels at -6 dB (0.5) panned L/R by azimuth
    /// - LFE discarded
    pub(super) fn compute_itu_coefficients(&mut self) {
        self.lfe_channels.clear();

        const ITU_ATTEN: f32 = 0.707; // -3 dB

        if let Some(config) = self.speaker_config {
            for s in config.speakers {
                if s.is_lfe {
                    self.lfe_channels.push(s.channel);
                    // ITU-R BS.775: LFE is discarded
                    self.target_coeffs[s.channel] = DownmixCoeffs {
                        left_gain: 0.0,
                        right_gain: 0.0,
                    };
                } else {
                    let azimuth = s.azimuth.abs();
                    let elevation = s.elevation.abs();

                    if elevation > 10.0 {
                        // Height channels: -6 dB panned by azimuth
                        let leftness = s.azimuth.to_radians().sin();
                        let pan_angle = (1.0 - leftness) * std::f32::consts::FRAC_PI_4;
                        self.target_coeffs[s.channel] = DownmixCoeffs {
                            left_gain: 0.5 * pan_angle.cos(),
                            right_gain: 0.5 * pan_angle.sin(),
                        };
                    } else if azimuth < 1.0 {
                        // Center: -3 dB to both L and R
                        self.target_coeffs[s.channel] = DownmixCoeffs {
                            left_gain: ITU_ATTEN,
                            right_gain: ITU_ATTEN,
                        };
                    } else if azimuth < 45.0 {
                        // Front L/R: unity pass-through
                        if s.azimuth > 0.0 {
                            self.target_coeffs[s.channel] = DownmixCoeffs {
                                left_gain: 1.0,
                                right_gain: 0.0,
                            };
                        } else {
                            self.target_coeffs[s.channel] = DownmixCoeffs {
                                left_gain: 0.0,
                                right_gain: 1.0,
                            };
                        }
                    } else {
                        // Surround: -3 dB panned by azimuth
                        if s.azimuth > 0.0 {
                            self.target_coeffs[s.channel] = DownmixCoeffs {
                                left_gain: ITU_ATTEN,
                                right_gain: 0.0,
                            };
                        } else {
                            self.target_coeffs[s.channel] = DownmixCoeffs {
                                left_gain: 0.0,
                                right_gain: ITU_ATTEN,
                            };
                        }
                    }
                }
            }
        } else {
            // Fallback for unknown layouts: linear pan
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    self.target_coeffs[ch] = DownmixCoeffs {
                        left_gain: 0.707,
                        right_gain: 0.707,
                    };
                } else {
                    let t = ch as f32 / (self.input_ch - 1).max(1) as f32;
                    self.target_coeffs[ch] = DownmixCoeffs {
                        left_gain: 1.0 - t,
                        right_gain: t,
                    };
                }
            }
        }
    }

    pub(super) fn compute_coefficients(&mut self, reset: bool) {
        if self.itu_mode {
            self.compute_itu_coefficients();
        } else {
            self.compute_standard_coefficients();
        }

        self.lfe_is_channel.fill(false);
        for &ch in &self.lfe_channels {
            if ch < self.input_ch {
                self.lfe_is_channel[ch] = true;
            }
        }

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

    pub(super) fn advance_coeff_smoothers_by(&mut self, samples: usize) {
        for smoother in &mut self.coeff_smoothers {
            smoother.next_n(samples);
        }
    }

    pub(super) fn compute_standard_coefficients(&mut self) {
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
                    self.target_coeffs[s.channel] = DownmixCoeffs {
                        left_gain: l_lin * 0.707,
                        right_gain: l_lin * 0.707,
                    };
                } else {
                    let azimuth = s.azimuth.abs();
                    let elevation = s.elevation.abs();

                    if elevation > 10.0 {
                        // Height channels: constant-power pan with elevation attenuation.
                        // cos(elevation) reduces contribution of high-elevation speakers
                        // to the horizontal stereo image (per ITU-R BS.2051 intent).
                        let el_factor = s.elevation.to_radians().cos();
                        let effective_gain = h_lin * el_factor;
                        let leftness = s.azimuth.to_radians().sin();
                        let pan_angle = (1.0 - leftness) * std::f32::consts::FRAC_PI_4;
                        self.target_coeffs[s.channel] = DownmixCoeffs {
                            left_gain: (effective_gain * pan_angle.cos()).max(0.0),
                            right_gain: (effective_gain * pan_angle.sin()).max(0.0),
                        };
                    } else if azimuth < 1.0 {
                        // Center channel
                        self.target_coeffs[s.channel] = DownmixCoeffs {
                            left_gain: c_lin * 0.707,
                            right_gain: c_lin * 0.707,
                        };
                    } else if azimuth < 45.0 {
                        // Front L/R
                        if s.azimuth > 0.0 {
                            // Left side (e.g. +30°)
                            self.target_coeffs[s.channel] = DownmixCoeffs {
                                left_gain: 1.0,
                                right_gain: 0.0,
                            };
                        } else {
                            // Right side (e.g. -30°)
                            self.target_coeffs[s.channel] = DownmixCoeffs {
                                left_gain: 0.0,
                                right_gain: 1.0,
                            };
                        }
                    } else {
                        // Surround channels (Side/Rear)
                        // Constant-power pan using sin(azimuth) as leftness.
                        // L² + R² = s_lin² at every angle, no energy loss.
                        let leftness = s.azimuth.to_radians().sin();
                        let pan_angle = (1.0 - leftness) * std::f32::consts::FRAC_PI_4;
                        self.target_coeffs[s.channel] = DownmixCoeffs {
                            left_gain: (s_lin * pan_angle.cos()).max(0.0),
                            right_gain: (s_lin * pan_angle.sin()).max(0.0),
                        };
                    }
                }
            }
        } else {
            // Generic downmix: linear panned based on channel index
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    self.target_coeffs[ch] = DownmixCoeffs {
                        left_gain: 0.707,
                        right_gain: 0.707,
                    };
                } else {
                    let t = ch as f32 / (self.input_ch - 1).max(1) as f32;
                    self.target_coeffs[ch] = DownmixCoeffs {
                        left_gain: 1.0 - t,
                        right_gain: t,
                    };
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn count_surround_channels(&self) -> usize {
        self.speaker_config
            .map(|config| {
                config
                    .speakers
                    .iter()
                    .filter(|speaker| {
                        !speaker.is_lfe
                            && speaker.azimuth.abs() >= 45.0
                            && speaker.elevation.abs() <= 10.0
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Check if a speaker channel index is a surround channel.
    pub(super) fn is_surround_channel(&self, ch: usize) -> Option<usize> {
        if let Some(config) = self.speaker_config {
            let mut surround_idx = 0;
            for s in config.speakers {
                if !s.is_lfe && s.azimuth.abs() >= 45.0 && s.elevation.abs() <= 10.0 {
                    if s.channel == ch {
                        return Some(surround_idx);
                    }
                    surround_idx += 1;
                }
            }
        }
        None
    }

    /// Check if a speaker channel is a center channel (|azimuth| < 1°).
    pub(super) fn is_center_channel(&self, ch: usize) -> bool {
        self.speaker_config
            .and_then(|cfg| cfg.speakers.get(ch))
            .map(|s| !s.is_lfe && s.azimuth.abs() < 1.0 && s.elevation.abs() <= 10.0)
            .unwrap_or(false)
    }

    pub(super) fn process_simple(&mut self, input: &[f32], output: &mut [f32], num_frames: usize) {
        for frame in 0..num_frames {
            let mut l = 0.0;
            let mut r = 0.0;
            for ch in 0..self.input_ch {
                let mut s = input[frame * self.input_ch + ch];
                // O(1) lookup: no linear scan over lfe_channels.
                if self.lfe_is_channel.get(ch).copied().unwrap_or(false) && ch < self.lfe_lpf.len()
                {
                    let mut val = s as f64;
                    val = self.lfe_lpf[ch][0].process(val);
                    val = self.lfe_lpf[ch][1].process(val);
                    s = val as f32;
                }
                l += s * self.coeff_smoothers[ch * 2].advance();
                r += s * self.coeff_smoothers[ch * 2 + 1].advance();
            }
            output[frame * 2] = l;
            output[frame * 2 + 1] = r;
        }
    }

    /// Matrix Lt/Rt stereo encoding (Dolby Surround / Pro Logic).
    ///
    /// Lt = L + 0.707*C - 0.707*j*Ls + 0.707*j*Rs
    /// Rt = R + 0.707*C + 0.707*j*Ls - 0.707*j*Rs
    ///
    /// where j = broadband 90° phase shift approximated by the LtRtAllpass Hilbert pair.
    /// The Hilbert pair uses two complementary 2-stage allpass chains (A and B) whose
    /// phase difference ∠A - ∠B ≈ 90° from ~150 Hz to ~12 kHz. The 90°-shifted signal
    /// is `chain_a_out - chain_b_out`.
    ///
    /// This matches the standard Dolby Surround matrix (Scheiber 1971) where
    /// S = Ls - Rs: Lt contains -j*Ls and +j*Rs; Rt contains +j*Ls and -j*Rs.
    pub(super) fn process_fft_block(&mut self) {
        let n = FFT_SIZE;
        let scale = self.output_scale;
        let mask = self.output_accumulator_mask;
        let num_bins = n / 2 + 1;

        for ch in 0..self.input_ch {
            let ch_offset = ch * n;
            let fft_offset = ch * num_bins;
            // Window and FFT each channel (SIMD optimized windowing)
            sotf_host::simd::window_mul_simd(
                &mut self.fft_input_buf,
                &self.input_buffer[ch_offset..ch_offset + n],
                &self.analysis_window,
            );
            self.fft_forward
                .process(
                    &mut self.fft_input_buf,
                    &mut self.fft_output[fft_offset..fft_offset + num_bins],
                )
                .unwrap();
        }

        // Initialize output frequencies
        self.out_freq_l.fill(Complex::new(0.0, 0.0));
        self.out_freq_r.fill(Complex::new(0.0, 0.0));

        if self.matrix_ltrt {
            // Lt/Rt is encoded in the frequency domain so the surround branch is a
            // true unity-magnitude quadrature rotation. DC and Nyquist cannot carry
            // a real-signal quadrature component and are cleared below.
            const ATTEN: f32 = std::f32::consts::FRAC_1_SQRT_2;
            let quadrature = Complex::new(0.0, 1.0);
            for ch in 0..self.input_ch {
                let channel_fft = &self.fft_output[ch * num_bins..(ch + 1) * num_bins];
                if self.lfe_is_channel.get(ch).copied().unwrap_or(false) {
                    continue;
                }
                if self.is_center_channel(ch) {
                    for (bin, &sample) in channel_fft.iter().enumerate() {
                        self.out_freq_l[bin] += sample * ATTEN;
                        self.out_freq_r[bin] += sample * ATTEN;
                    }
                } else if self.is_surround_channel(ch).is_some() {
                    let is_left = self
                        .speaker_config
                        .and_then(|cfg| cfg.speakers.iter().find(|speaker| speaker.channel == ch))
                        .map(|speaker| speaker.azimuth > 0.0)
                        .unwrap_or(false);
                    let sign = if is_left { -ATTEN } else { ATTEN };
                    for (bin, &sample) in channel_fft.iter().enumerate() {
                        let shifted = sample * quadrature * sign;
                        self.out_freq_l[bin] += shifted;
                        self.out_freq_r[bin] -= shifted;
                    }
                } else {
                    let gl = self.coeff_smoothers[ch * 2].current();
                    let gr = self.coeff_smoothers[ch * 2 + 1].current();
                    for (bin, &sample) in channel_fft.iter().enumerate() {
                        self.out_freq_l[bin] += sample * gl;
                        self.out_freq_r[bin] += sample * gr;
                    }
                }
            }
        } else {
            // Power sum (standard downmix) in frequency domain.
            for ch in 0..self.input_ch {
                let gl = self.coeff_smoothers[ch * 2].current();
                let gr = self.coeff_smoothers[ch * 2 + 1].current();
                let fft_offset = ch * num_bins;
                let channel_fft = &self.fft_output[fft_offset..fft_offset + num_bins];
                for (i, &cf) in channel_fft.iter().enumerate() {
                    self.out_freq_l[i] += cf * gl;
                    self.out_freq_r[i] += cf * gr;
                }
            }

            // Apply Phase Coherence if enabled
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
                    // Energy-weighted phase average: compute the output phase
                    // as the energy-weighted average of all input channels' phases
                    // (instead of using only the dominant/loudest channel's phase).
                    // We accumulate weighted unit-circle vectors (cos/sin of phase)
                    // weighted by energy (magnitude squared * gain squared).
                    let mut phase_vec_l = Complex::new(0.0f32, 0.0f32);
                    let mut phase_vec_r = Complex::new(0.0f32, 0.0f32);
                    let mut energy_sum_l = 0.0f32;
                    let mut energy_sum_r = 0.0f32;

                    for ch in 0..self.input_ch {
                        let gl = self.coeff_smoothers[ch * 2].current();
                        let gr = self.coeff_smoothers[ch * 2 + 1].current();
                        let val = self.fft_output[ch * num_bins + bin];

                        let mag_sq = val.norm_sqr();
                        let mag = if mag_sq > 1e-12 {
                            mag_sq * sotf_host::simd::fast_inv_sqrt(mag_sq)
                        } else {
                            0.0
                        };

                        // Energy weight = magnitude_squared * gain_squared
                        let energy_l = mag_sq * gl * gl;
                        let energy_r = mag_sq * gr * gr;

                        if mag > 1e-12 {
                            // Normalize the complex sample directly. This avoids
                            // atan2/sin/cos in the audio callback.
                            let unit_phase = val * (1.0 / mag);
                            phase_vec_l += unit_phase * energy_l;
                            phase_vec_r += unit_phase * energy_r;
                            energy_sum_l += energy_l;
                            energy_sum_r += energy_r;
                        }
                    }

                    // Phase alignment is magnitude preserving: it may rotate the
                    // ordinary downmix, but never replaces it with a linear sum of
                    // input magnitudes. Confidence tends to zero when the phase
                    // vector is unstable/cancelling, avoiding frame-to-frame flips.
                    for (ordinary, phase_vec, energy_sum) in [
                        (&mut self.out_freq_l[bin], phase_vec_l, energy_sum_l),
                        (&mut self.out_freq_r[bin], phase_vec_r, energy_sum_r),
                    ] {
                        let phase_norm_sq = phase_vec.norm_sqr();
                        if phase_norm_sq > 1e-20 && energy_sum > 1e-20 {
                            let phase_norm = phase_norm_sq.sqrt();
                            let confidence = (phase_norm / energy_sum).clamp(0.0, 1.0);
                            let ordinary_mag = ordinary.norm();
                            let aligned = phase_vec * (ordinary_mag / phase_norm);
                            let effective_blend = blend * confidence;
                            *ordinary =
                                *ordinary * (1.0 - effective_blend) + aligned * effective_blend;
                        }
                    }
                }
            }
        }

        // IFFT and Accumulate with Synthesis Window
        self.out_freq_l[0].im = 0.0;
        self.out_freq_l[num_bins - 1].im = 0.0;
        self.ifft_input_buf.copy_from_slice(&self.out_freq_l);
        self.fft_inverse
            .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
            .unwrap();

        // Use SIMD windowing and scaling
        sotf_host::simd::window_mul_simd_inplace(&mut self.ifft_output_buf, &self.analysis_window);
        sotf_host::simd::scale_add_simd_inplace(&mut self.ifft_output_buf, scale);

        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2] += self.ifft_output_buf[i];
        }

        self.out_freq_r[0].im = 0.0;
        self.out_freq_r[num_bins - 1].im = 0.0;
        self.ifft_input_buf.copy_from_slice(&self.out_freq_r);
        self.fft_inverse
            .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
            .unwrap();

        sotf_host::simd::window_mul_simd_inplace(&mut self.ifft_output_buf, &self.analysis_window);
        sotf_host::simd::scale_add_simd_inplace(&mut self.ifft_output_buf, scale);

        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2 + 1] += self.ifft_output_buf[i];
        }

        self.advance_coeff_smoothers_by(HOP_SIZE);
        self.next_add_position = (self.next_add_position + HOP_SIZE) & mask;
        self.output_accumulator_fill += HOP_SIZE;
    }
}

impl Plugin for DownmixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Downmix", env!("CARGO_PKG_VERSION"), "SotF")
            .with_description("Phase-coherent surround-to-stereo downmixer")
    }
    fn input_channels(&self) -> usize {
        self.input_ch
    }
    fn output_channels(&self) -> usize {
        2
    }
    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::boundary(
            if self.uses_spectral_path() {
                PluginCostClass::Fft
            } else {
                PluginCostClass::Scalar
            },
            self.latency_samples(),
        )
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let mut changed_index = None;
        let old_phase = self.phase_coherence;
        let old_ltrt = self.matrix_ltrt;
        let old_low = self.phase_blend_low_hz;
        let old_high = self.phase_blend_high_hz;
        param_bridge::set_parameter(DM, &id, &value, |i, v| {
            changed_index = Some(i);
            self.set_param_value(i, v)
        })?;
        let index = changed_index.expect("validated Downmix parameter must have an index");
        if matches!(index, 4 | 8)
            && !self.lfe_lpf.is_empty()
            && (old_phase != self.phase_coherence || old_ltrt != self.matrix_ltrt)
        {
            self.phase_coherence = old_phase;
            self.matrix_ltrt = old_ltrt;
            return Err(format!(
                "Downmix structural parameter '{}' requires plugin reconstruction",
                id
            ));
        }
        if self.phase_coherence && self.matrix_ltrt {
            self.phase_coherence = old_phase;
            self.matrix_ltrt = old_ltrt;
            return Err(
                "Downmix phase_coherence and matrix_ltrt are mutually exclusive structural modes"
                    .into(),
            );
        }
        if self.phase_blend_low_hz >= self.phase_blend_high_hz {
            self.phase_blend_low_hz = old_low;
            self.phase_blend_high_hz = old_high;
            return Err("Downmix phase_blend_low_hz must be below phase_blend_high_hz".into());
        }
        if matches!(index, 0..=3 | 7) {
            self.compute_coefficients(false);
        }
        self.cached_parameters[index].default_value = match DM[index].param_type {
            sotf_host::param_specs::ParamType::Bool { .. } => {
                ParameterValue::Bool(self.param_value(index).unwrap_or(0.0) > 0.5)
            }
            _ => ParameterValue::Float(self.param_value(index).unwrap_or(0.0) as f32),
        };
        if old_phase != self.phase_coherence || old_ltrt != self.matrix_ltrt {
            self.clear_stream_state();
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(DM, id, |i| self.param_value(i))
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Downmix sample rate must be greater than zero".into());
        }
        if self.phase_blend_high_hz >= sample_rate as f32 * 0.5 {
            return Err(format!(
                "Downmix phase_blend_high_hz {} must be below Nyquist {}",
                self.phase_blend_high_hz,
                sample_rate as f32 * 0.5
            ));
        }
        self.sample_rate = sample_rate;
        self.lfe_lpf = (0..self.input_ch)
            .map(|_| {
                [
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        sample_rate as f64,
                        0.707,
                        0.0,
                    ),
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        sample_rate as f64,
                        0.707,
                        0.0,
                    ),
                ]
            })
            .collect();
        for s in &mut self.coeff_smoothers {
            s.set_time(PARAM_SMOOTH_MS, sample_rate);
        }
        self.compute_coefficients(true);
        self.clear_stream_state();
        Ok(())
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        let expected_input = num_frames
            .checked_mul(self.input_ch)
            .ok_or_else(|| "Downmix input length overflow".to_string())?;
        let expected_output = num_frames
            .checked_mul(2)
            .ok_or_else(|| "Downmix output length overflow".to_string())?;
        if input.len() != expected_input {
            return Err(format!(
                "Downmix expected {expected_input} input samples for {num_frames} frames and {} channels, got {}",
                self.input_ch,
                input.len()
            ));
        }
        if output.len() != expected_output {
            return Err(format!(
                "Downmix expected {expected_output} output samples for {num_frames} stereo frames, got {}",
                output.len()
            ));
        }
        output.fill(0.0);
        if !self.uses_spectral_path() {
            self.process_simple(input, output, num_frames);
            return Ok(num_frames);
        }

        let mask = self.output_accumulator_mask;
        let n = FFT_SIZE;
        for frame in 0..num_frames {
            for ch in 0..self.input_ch {
                let mut sample = input[frame * self.input_ch + ch];
                if self.lfe_is_channel.get(ch).copied().unwrap_or(false) && ch < self.lfe_lpf.len()
                {
                    let mut value = sample as f64;
                    value = self.lfe_lpf[ch][0].process(value);
                    value = self.lfe_lpf[ch][1].process(value);
                    sample = value as f32;
                }
                self.input_buffer[ch * n + self.input_fill] = sample;
            }
            self.input_fill += 1;

            if self.input_fill == n {
                self.process_fft_block();
                let overlap = n - HOP_SIZE;
                for ch in 0..self.input_ch {
                    let ch_offset = ch * n;
                    self.input_buffer[ch_offset..ch_offset + n].copy_within(HOP_SIZE..n, 0);
                }
                self.input_fill = overlap;
            }

            if self.startup_delay_remaining > 0 {
                self.startup_delay_remaining -= 1;
            } else if self.output_accumulator_fill > 0 {
                let read_idx = self.output_read_position & mask;
                output[frame * 2] = self.output_accumulator[read_idx * 2];
                output[frame * 2 + 1] = self.output_accumulator[read_idx * 2 + 1];
                self.output_accumulator[read_idx * 2] = 0.0;
                self.output_accumulator[read_idx * 2 + 1] = 0.0;
                self.output_read_position = (self.output_read_position + 1) & mask;
                self.output_accumulator_fill -= 1;
            }
        }

        Ok(num_frames)
    }
    fn reset(&mut self) {
        self.clear_stream_state();
        for filters in &mut self.lfe_lpf {
            filters[0].reset();
            filters[1].reset();
        }
        for (i, c) in self.target_coeffs.iter().enumerate() {
            self.coeff_smoothers[i * 2].reset(c.left_gain);
            self.coeff_smoothers[i * 2 + 1].reset(c.right_gain);
        }
    }
    fn latency_samples(&self) -> usize {
        if self.uses_spectral_path() {
            FFT_SIZE
        } else {
            0
        }
    }
}
