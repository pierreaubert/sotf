// ============================================================================
// Phase-Coherent Downmix Plugin
// ============================================================================

use sotf_host::param_specs::{downmix::PARAMS as DM, find_by_key as pk};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};

use math_audio_dsp::fast_math::{fast_atan2, fast_cos, fast_sin};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::smoothing::Smoother;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

fn default_center_gain_db() -> f32 {
    pk(DM, "center_gain_db").default_f64() as f32
}
fn default_surround_gain_db() -> f32 {
    pk(DM, "surround_gain_db").default_f64() as f32
}
fn default_height_gain_db() -> f32 {
    pk(DM, "height_gain_db").default_f64() as f32
}
fn default_lfe_gain_db() -> f32 {
    pk(DM, "lfe_gain_db").default_f64() as f32
}
fn default_phase_coherence() -> bool {
    pk(DM, "phase_coherence").default_bool()
}
fn default_phase_blend_low_hz() -> f32 {
    pk(DM, "phase_blend_low_hz").default_f64() as f32
}
fn default_phase_blend_high_hz() -> f32 {
    pk(DM, "phase_blend_high_hz").default_f64() as f32
}
fn default_itu_mode() -> bool {
    pk(DM, "itu_mode").default_bool()
}
fn default_dolby_ltrt() -> bool {
    false
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
    /// When true, use ITU-R BS.775 standard downmix coefficients for 5.1→stereo
    #[serde(default = "default_itu_mode")]
    pub itu_mode: bool,
    /// When true, use Dolby Pro Logic Lt/Rt encoding for surround channels
    #[serde(default = "default_dolby_ltrt")]
    pub dolby_ltrt: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct DownmixCoeffs {
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
    pub(crate) target_coeffs: Vec<DownmixCoeffs>,
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
    itu_mode: bool,
    dolby_ltrt: bool,

    /// Per-surround-channel first-order allpass filters for ~90° phase shift.
    /// Used only when dolby_ltrt is enabled.
    /// Each surround channel gets one allpass filter.
    ltrt_allpass: Vec<LtRtAllpass>,

    param_center_gain_db: ParameterId,
    param_surround_gain_db: ParameterId,
    param_height_gain_db: ParameterId,
    param_lfe_gain_db: ParameterId,
    param_phase_coherence: ParameterId,
    param_itu_mode: ParameterId,
    param_dolby_ltrt: ParameterId,
    cached_parameters: Vec<Parameter>,
}

/// First-order allpass filter state for 90° phase shift approximation.
///
/// Uses a first-order allpass: y[n] = -a*x[n] + x[n-1] + a*y[n-1]
/// where a = (tan(pi*fc/fs) - 1) / (tan(pi*fc/fs) + 1).
/// At frequencies >> fc, the phase approaches -180°; at fc, it's -90°.
/// For Dolby Lt/Rt, fc is tuned to ~300 Hz so surround content (primarily
/// 300 Hz+) gets approximately 90° shift relative to the direct path.
struct LtRtAllpass {
    coeff_a: f32,
    x_prev: f32,
    y_prev: f32,
}

impl LtRtAllpass {
    fn new(sample_rate: u32) -> Self {
        let fc = 300.0_f32; // Crossover frequency for 90° at ~300 Hz
        let coeff_a = Self::compute_coeff(fc, sample_rate);
        Self {
            coeff_a,
            x_prev: 0.0,
            y_prev: 0.0,
        }
    }

    fn compute_coeff(fc: f32, sample_rate: u32) -> f32 {
        let t = (std::f32::consts::PI * fc / sample_rate as f32).tan();
        (t - 1.0) / (t + 1.0)
    }

    fn update_sample_rate(&mut self, sample_rate: u32) {
        self.coeff_a = Self::compute_coeff(300.0, sample_rate);
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = -self.coeff_a * x + self.x_prev + self.coeff_a * self.y_prev;
        self.x_prev = x;
        self.y_prev = y;
        y
    }

    fn reset(&mut self) {
        self.x_prev = 0.0;
        self.y_prev = 0.0;
    }
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

        // 75% overlap dual-window scaling: Sum(w^2) = (N/H) * (3/8) * N = 1.5 * N
        let output_scale = 1.0 / (FFT_SIZE as f32 * 1.5);

        let mut p = Self {
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
            center_gain_db: pk(DM, "center_gain_db").default_f64() as f32,
            surround_gain_db: pk(DM, "surround_gain_db").default_f64() as f32,
            height_gain_db: pk(DM, "height_gain_db").default_f64() as f32,
            lfe_gain_db: pk(DM, "lfe_gain_db").default_f64() as f32,
            phase_coherence: pk(DM, "phase_coherence").default_bool(),
            phase_blend_low_hz: pk(DM, "phase_blend_low_hz").default_f64() as f32,
            phase_blend_high_hz: pk(DM, "phase_blend_high_hz").default_f64() as f32,
            itu_mode: pk(DM, "itu_mode").default_bool(),
            dolby_ltrt: false,
            ltrt_allpass: Vec::new(),
            param_center_gain_db: ParameterId::from("center_gain_db"),
            param_surround_gain_db: ParameterId::from("surround_gain_db"),
            param_height_gain_db: ParameterId::from("height_gain_db"),
            param_lfe_gain_db: ParameterId::from("lfe_gain_db"),
            param_phase_coherence: ParameterId::from("phase_coherence"),
            param_itu_mode: ParameterId::from("itu_mode"),
            param_dolby_ltrt: ParameterId::from("dolby_ltrt"),
            cached_parameters: Vec::new(),
        };
        p.compute_coefficients(true);
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "center_gain_db",
                "Center Gain",
                self.center_gain_db,
                pk(DM, "center_gain_db").min_f64() as f32,
                pk(DM, "center_gain_db").max_f64() as f32,
            ),
            Parameter::new_float(
                "surround_gain_db",
                "Surround Gain",
                self.surround_gain_db,
                pk(DM, "surround_gain_db").min_f64() as f32,
                pk(DM, "surround_gain_db").max_f64() as f32,
            ),
            Parameter::new_float(
                "height_gain_db",
                "Height Gain",
                self.height_gain_db,
                pk(DM, "height_gain_db").min_f64() as f32,
                pk(DM, "height_gain_db").max_f64() as f32,
            ),
            Parameter::new_float(
                "lfe_gain_db",
                "LFE Gain",
                self.lfe_gain_db,
                pk(DM, "lfe_gain_db").min_f64() as f32,
                pk(DM, "lfe_gain_db").max_f64() as f32,
            ),
            Parameter::new_bool("phase_coherence", "Phase Coherence", self.phase_coherence),
            Parameter::new_bool("itu_mode", "ITU-R BS.775 Mode", self.itu_mode),
            Parameter::new_bool("dolby_ltrt", "Dolby Lt/Rt", self.dolby_ltrt),
        ];
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
        plugin.itu_mode = params.itu_mode;
        plugin.dolby_ltrt = params.dolby_ltrt;
        plugin.compute_coefficients(true);
        plugin.rebuild_cached_parameters();
        plugin
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
    fn compute_itu_coefficients(&mut self) -> Vec<DownmixCoeffs> {
        let mut new_coeffs = Vec::with_capacity(self.input_ch);
        self.lfe_channels.clear();

        const ITU_ATTEN: f32 = 0.707; // -3 dB

        if let Some(config) = self.speaker_config {
            for s in config.speakers {
                if s.is_lfe {
                    self.lfe_channels.push(s.channel);
                    // ITU-R BS.775: LFE is discarded
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: 0.0,
                        right_gain: 0.0,
                    });
                } else {
                    let azimuth = s.azimuth.abs();
                    let elevation = s.elevation.abs();

                    if elevation > 10.0 {
                        // Height channels: -6 dB panned by azimuth
                        let leftness = s.azimuth.to_radians().sin();
                        let pan_angle = (1.0 - leftness) * std::f32::consts::FRAC_PI_4;
                        new_coeffs.push(DownmixCoeffs {
                            left_gain: 0.5 * pan_angle.cos(),
                            right_gain: 0.5 * pan_angle.sin(),
                        });
                    } else if azimuth < 1.0 {
                        // Center: -3 dB to both L and R
                        new_coeffs.push(DownmixCoeffs {
                            left_gain: ITU_ATTEN,
                            right_gain: ITU_ATTEN,
                        });
                    } else if azimuth < 45.0 {
                        // Front L/R: unity pass-through
                        if s.azimuth > 0.0 {
                            new_coeffs.push(DownmixCoeffs {
                                left_gain: 1.0,
                                right_gain: 0.0,
                            });
                        } else {
                            new_coeffs.push(DownmixCoeffs {
                                left_gain: 0.0,
                                right_gain: 1.0,
                            });
                        }
                    } else {
                        // Surround: -3 dB panned by azimuth
                        if s.azimuth > 0.0 {
                            new_coeffs.push(DownmixCoeffs {
                                left_gain: ITU_ATTEN,
                                right_gain: 0.0,
                            });
                        } else {
                            new_coeffs.push(DownmixCoeffs {
                                left_gain: 0.0,
                                right_gain: ITU_ATTEN,
                            });
                        }
                    }
                }
            }
        } else {
            // Fallback for unknown layouts: linear pan
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: 0.707,
                        right_gain: 0.707,
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

        new_coeffs
    }

    fn compute_coefficients(&mut self, reset: bool) {
        let new_coeffs = if self.itu_mode {
            self.compute_itu_coefficients()
        } else {
            self.compute_standard_coefficients()
        };

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

    fn compute_standard_coefficients(&mut self) -> Vec<DownmixCoeffs> {
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
                        // Height channels: constant-power pan using sin(azimuth)
                        // as leftness. This works over the full 360° circle:
                        //   0° → center, ±90° → full L/R, ±180° → center again.
                        // L² + R² = gain² always (no energy loss).
                        let leftness = s.azimuth.to_radians().sin();
                        let pan_angle = (1.0 - leftness) * std::f32::consts::FRAC_PI_4;
                        new_coeffs.push(DownmixCoeffs {
                            left_gain: (h_lin * pan_angle.cos()).max(0.0),
                            right_gain: (h_lin * pan_angle.sin()).max(0.0),
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
                            new_coeffs.push(DownmixCoeffs {
                                left_gain: 1.0,
                                right_gain: 0.0,
                            });
                        } else {
                            // Right side (e.g. -30°)
                            new_coeffs.push(DownmixCoeffs {
                                left_gain: 0.0,
                                right_gain: 1.0,
                            });
                        }
                    } else {
                        // Surround channels (Side/Rear)
                        // Constant-power pan using sin(azimuth) as leftness.
                        // L² + R² = s_lin² at every angle, no energy loss.
                        let leftness = s.azimuth.to_radians().sin();
                        let pan_angle = (1.0 - leftness) * std::f32::consts::FRAC_PI_4;
                        new_coeffs.push(DownmixCoeffs {
                            left_gain: (s_lin * pan_angle.cos()).max(0.0),
                            right_gain: (s_lin * pan_angle.sin()).max(0.0),
                        });
                    }
                }
            }
        } else {
            // Generic downmix: linear panned based on channel index
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    new_coeffs.push(DownmixCoeffs {
                        left_gain: 0.707,
                        right_gain: 0.707,
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

        // Normalization: Only normalize if the sum of absolute gains exceeds a safe threshold.
        // Using abs() ensures negative coefficients don't reduce the perceived sum.
        let max_sum = new_coeffs
            .iter()
            .map(|c| c.left_gain.abs())
            .sum::<f32>()
            .max(new_coeffs.iter().map(|c| c.right_gain.abs()).sum::<f32>());

        let norm_threshold = 2.0;
        if max_sum > norm_threshold {
            let scale = norm_threshold / max_sum;
            for c in &mut new_coeffs {
                c.left_gain *= scale;
                c.right_gain *= scale;
            }
        }

        new_coeffs
    }

    /// Count surround channels (|azimuth| >= 45°, not LFE, not height).
    fn count_surround_channels(&self) -> usize {
        if let Some(config) = self.speaker_config {
            config
                .speakers
                .iter()
                .filter(|s| !s.is_lfe && s.azimuth.abs() >= 45.0 && s.elevation.abs() <= 10.0)
                .count()
        } else {
            0
        }
    }

    /// Check if a speaker channel index is a surround channel.
    fn is_surround_channel(&self, ch: usize) -> Option<usize> {
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
    fn is_center_channel(&self, ch: usize) -> bool {
        self.speaker_config
            .and_then(|cfg| cfg.speakers.get(ch))
            .map(|s| !s.is_lfe && s.azimuth.abs() < 1.0 && s.elevation.abs() <= 10.0)
            .unwrap_or(false)
    }

    fn process_simple(&mut self, input: &[f32], output: &mut [f32], num_frames: usize) {
        if self.dolby_ltrt {
            self.process_dolby_ltrt(input, output, num_frames);
            return;
        }
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
                l += s * self.coeff_smoothers[ch * 2].advance();
                r += s * self.coeff_smoothers[ch * 2 + 1].advance();
            }
            output[frame * 2] = l;
            output[frame * 2 + 1] = r;
        }
    }

    /// Dolby Pro Logic Lt/Rt stereo encoding.
    ///
    /// Lt = L + 0.707*C - 0.707*j*Ls - 0.707*j*Rs
    /// Rt = R + 0.707*C + 0.707*j*Ls + 0.707*j*Rs
    ///
    /// where j = 90° phase shift, approximated by a first-order allpass filter.
    /// For speaker configurations without standard 5.1 layout, we identify
    /// center/surround channels by azimuth.
    fn process_dolby_ltrt(&mut self, input: &[f32], output: &mut [f32], num_frames: usize) {
        const ATTEN: f32 = 0.707; // -3 dB

        for frame in 0..num_frames {
            let mut lt = 0.0f32;
            let mut rt = 0.0f32;

            for ch in 0..self.input_ch {
                let s = input[frame * self.input_ch + ch];

                if self.is_center_channel(ch) {
                    // Center: 0.707 * C to both Lt and Rt
                    lt += ATTEN * s;
                    rt += ATTEN * s;
                } else if let Some(surr_idx) = self.is_surround_channel(ch) {
                    // Surround: apply 90° phase shift via allpass
                    if surr_idx < self.ltrt_allpass.len() {
                        let shifted = self.ltrt_allpass[surr_idx].process(s);
                        // Determine if left-side or right-side surround from speaker config
                        let is_left = self
                            .speaker_config
                            .and_then(|cfg| cfg.speakers.iter().find(|sp| sp.channel == ch))
                            .map(|sp| sp.azimuth > 0.0)
                            .unwrap_or(false);
                        if is_left {
                            // Left surround: -0.707*j*Ls to Lt, +0.707*j*Ls to Rt
                            lt -= ATTEN * shifted;
                            rt += ATTEN * shifted;
                        } else {
                            // Right surround: +0.707*j*Rs to Lt, -0.707*j*Rs to Rt
                            lt += ATTEN * shifted;
                            rt -= ATTEN * shifted;
                        }
                    }
                } else if self.lfe_lpf_idx.get(ch).copied().flatten().is_some() {
                    // LFE: discard in standard Lt/Rt encoding
                } else {
                    // Front L/R: pass through directly using smoother gains
                    lt += s * self.coeff_smoothers[ch * 2].advance();
                    rt += s * self.coeff_smoothers[ch * 2 + 1].advance();
                    continue;
                }
            }

            output[frame * 2] = lt;
            output[frame * 2 + 1] = rt;
        }

        // Advance smoothers for non-front channels that weren't advanced above
        // (smoothers are used only for coefficient interpolation in non-ltrt mode)
    }

    fn process_fft_block(&mut self) {
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

        // Power sum (standard downmix) in frequency domain (SIMD optimized)
        for ch in 0..self.input_ch {
            let gl = self.coeff_smoothers[ch * 2].current();
            let gr = self.coeff_smoothers[ch * 2 + 1].current();
            let fft_offset = ch * num_bins;
            let channel_fft = &self.fft_output[fft_offset..fft_offset + num_bins];

            // Complex multiply-accumulate: out += channel * gain
            // Since gains are real, we can optimize the loop easily.
            for (i, &cf) in channel_fft.iter().enumerate() {
                self.out_freq_l[i] += cf * gl;
                self.out_freq_r[i] += cf * gr;
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
                    // Energy-weighted phase average: compute the output phase
                    // as the energy-weighted average of all input channels' phases
                    // (instead of using only the dominant/loudest channel's phase).
                    // We accumulate weighted unit-circle vectors (cos/sin of phase)
                    // weighted by energy (magnitude squared * gain squared).
                    let mut phase_vec_l_re = 0.0f32;
                    let mut phase_vec_l_im = 0.0f32;
                    let mut phase_vec_r_re = 0.0f32;
                    let mut phase_vec_r_im = 0.0f32;

                    let mut mag_sum_l = 0.0f32;
                    let mut mag_sum_r = 0.0f32;

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

                        let mag_l = mag * gl;
                        let mag_r = mag * gr;

                        mag_sum_l += mag_l;
                        mag_sum_r += mag_r;

                        // Energy weight = magnitude_squared * gain_squared
                        let energy_l = mag_sq * gl * gl;
                        let energy_r = mag_sq * gr * gr;

                        if energy_l > 1e-20 || energy_r > 1e-20 {
                            let phase = fast_atan2(val.im, val.re);
                            let cos_p = fast_cos(phase);
                            let sin_p = fast_sin(phase);

                            phase_vec_l_re += energy_l * cos_p;
                            phase_vec_l_im += energy_l * sin_p;
                            phase_vec_r_re += energy_r * cos_p;
                            phase_vec_r_im += energy_r * sin_p;
                        }
                    }

                    // Compute the energy-weighted average phase from the accumulated vector
                    let avg_phase_l = fast_atan2(phase_vec_l_im, phase_vec_l_re);
                    let avg_phase_r = fast_atan2(phase_vec_r_im, phase_vec_r_re);

                    let aligned_l = Complex::new(
                        mag_sum_l * fast_cos(avg_phase_l),
                        mag_sum_l * fast_sin(avg_phase_l),
                    );
                    let aligned_r = Complex::new(
                        mag_sum_r * fast_cos(avg_phase_r),
                        mag_sum_r * fast_sin(avg_phase_r),
                    );

                    self.out_freq_l[bin] = self.out_freq_l[bin] * (1.0 - blend) + aligned_l * blend;
                    self.out_freq_r[bin] = self.out_freq_r[bin] * (1.0 - blend) + aligned_r * blend;
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

        self.next_add_position = (self.next_add_position + HOP_SIZE) & mask;
        self.output_accumulator_fill += HOP_SIZE;
    }
}

impl Plugin for DownmixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Downmix", "2.0.0", "SotF")
            .with_description("Phase-coherent surround-to-stereo downmixer")
    }
    fn input_channels(&self) -> usize {
        self.input_ch
    }
    fn output_channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_center_gain_db {
            let v = value
                .as_float()
                .unwrap_or(pk(DM, "center_gain_db").default_f64() as f32);
            if v.is_finite() {
                self.center_gain_db = v;
            }
        } else if id == self.param_surround_gain_db {
            let v = value
                .as_float()
                .unwrap_or(pk(DM, "surround_gain_db").default_f64() as f32);
            if v.is_finite() {
                self.surround_gain_db = v;
            }
        } else if id == self.param_height_gain_db {
            let v = value
                .as_float()
                .unwrap_or(pk(DM, "height_gain_db").default_f64() as f32);
            if v.is_finite() {
                self.height_gain_db = v;
            }
        } else if id == self.param_lfe_gain_db {
            let v = value
                .as_float()
                .unwrap_or(pk(DM, "lfe_gain_db").default_f64() as f32);
            if v.is_finite() {
                self.lfe_gain_db = v;
            }
        } else if id == self.param_phase_coherence {
            self.phase_coherence = value
                .as_bool()
                .unwrap_or(pk(DM, "phase_coherence").default_bool());
        } else if id == self.param_itu_mode {
            self.itu_mode = value
                .as_bool()
                .unwrap_or(pk(DM, "itu_mode").default_bool());
        } else if id == self.param_dolby_ltrt {
            self.dolby_ltrt = value.as_bool().unwrap_or(false);
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.compute_coefficients(false);
        self.rebuild_cached_parameters();
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
        } else if id == &self.param_itu_mode {
            Some(ParameterValue::Bool(self.itu_mode))
        } else if id == &self.param_dolby_ltrt {
            Some(ParameterValue::Bool(self.dolby_ltrt))
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
        // Initialize Lt/Rt allpass filters: one per surround channel.
        // Surround channels are those with |azimuth| >= 45° and not LFE.
        let surround_count = self.count_surround_channels();
        if self.ltrt_allpass.len() == surround_count {
            for ap in &mut self.ltrt_allpass {
                ap.update_sample_rate(sample_rate);
            }
        } else {
            self.ltrt_allpass = (0..surround_count)
                .map(|_| LtRtAllpass::new(sample_rate))
                .collect();
        }
        for s in &mut self.coeff_smoothers {
            s.set_time(PARAM_SMOOTH_MS, sample_rate);
        }
        self.compute_coefficients(true);
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
                                self.input_buffer[ch_offset + self.input_fill + i] =
                                    src[i * self.input_ch];
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
                break;
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
        for ap in &mut self.ltrt_allpass {
            ap.reset();
        }
        self.lfe_lpf = self
            .lfe_channels
            .iter()
            .map(|_| {
                [
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        self.sample_rate as f64,
                        0.707,
                        0.0,
                    ),
                    Biquad::new(
                        BiquadFilterType::Lowpass,
                        120.0,
                        self.sample_rate as f64,
                        0.707,
                        0.0,
                    ),
                ]
            })
            .collect();
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
    use crate::*;
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

    /// Helper: create a 5.1.4 downmix plugin with all gains at 0dB, phase_coherence off,
    /// feed DC=1.0 into a single channel, and return the (left, right) output after settling.
    fn probe_514_channel(channel: usize) -> (f32, f32) {
        let input_ch = 10;
        let mut p = DownmixPlugin::from_params(DownmixPluginParams {
            input_channels: input_ch,
            center_gain_db: 0.0,
            surround_gain_db: 0.0,
            height_gain_db: 0.0,
            lfe_gain_db: 0.0,
            phase_coherence: false,
            phase_blend_low_hz: 200.0,
            phase_blend_high_hz: 5000.0,
            itu_mode: false,
            dolby_ltrt: false,
        });
        p.initialize(48000).unwrap();

        let num_frames = 2048;
        let mut input = vec![0.0f32; num_frames * input_ch];
        for k in 0..num_frames {
            input[k * input_ch + channel] = 1.0;
        }
        let mut output = vec![0.0f32; num_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();

        // Return the last frame (after smoother settles)
        let l = output[(num_frames - 1) * 2];
        let r = output[(num_frames - 1) * 2 + 1];
        (l, r)
    }

    /// Bug 1: Rear height channels (TBL at +150°, TBR at -150°) must NOT produce
    /// negative gains. Negative gains cause phase inversion and cancellation.
    #[test]
    fn test_514_rear_height_no_negative_gains() {
        // 5.1.4 layout: ch8=TBL(+150°, 45°), ch9=TBR(-150°, 45°)
        let (l_tbl, r_tbl) = probe_514_channel(8); // TBL
        let (l_tbr, r_tbr) = probe_514_channel(9); // TBR

        assert!(
            r_tbl >= 0.0,
            "TBL right gain must be non-negative, got {r_tbl}"
        );
        assert!(
            l_tbr >= 0.0,
            "TBR left gain must be non-negative, got {l_tbr}"
        );
        // TBL should go primarily to left
        assert!(l_tbl > r_tbl, "TBL should map more to left than right");
        // TBR should go primarily to right
        assert!(r_tbr > l_tbr, "TBR should map more to right than left");
    }

    /// Bug 2: Normalization should use absolute values of gains so that negative
    /// coefficients don't reduce the perceived sum and under-normalize.
    #[test]
    fn test_514_normalization_uses_abs() {
        let input_ch = 10;
        let p = DownmixPlugin::from_params(DownmixPluginParams {
            input_channels: input_ch,
            center_gain_db: 0.0,
            surround_gain_db: 0.0,
            height_gain_db: 0.0,
            lfe_gain_db: 0.0,
            phase_coherence: false,
            phase_blend_low_hz: 200.0,
            phase_blend_high_hz: 5000.0,
            itu_mode: false,
            dolby_ltrt: false,
        });

        // Sum the absolute values of all left gains — should be <= 2.0 after normalization
        let abs_sum_l: f32 = p.target_coeffs.iter().map(|c| c.left_gain.abs()).sum();
        let abs_sum_r: f32 = p.target_coeffs.iter().map(|c| c.right_gain.abs()).sum();
        let max_abs = abs_sum_l.max(abs_sum_r);

        assert!(
            max_abs <= 2.05, // small epsilon for float
            "Absolute gain sum should be <= 2.0 after normalization, got L={abs_sum_l}, R={abs_sum_r}"
        );
    }

    /// Bug 3: Surround panning should preserve constant-power relationships.
    /// All surround speakers at the same gain should have equal L²+R² (before
    /// normalization scales them uniformly). We verify this by checking that all
    /// surround channels have the same power after normalization (within tolerance).
    #[test]
    fn test_surround_panning_energy_preservation() {
        let input_ch = 8; // 7.1 layout
        let p = DownmixPlugin::from_params(DownmixPluginParams {
            input_channels: input_ch,
            center_gain_db: -100.0,
            surround_gain_db: 0.0, // s_lin = 1.0
            height_gain_db: -100.0,
            lfe_gain_db: -100.0,
            phase_coherence: false,
            phase_blend_low_hz: 200.0,
            phase_blend_high_hz: 5000.0,
            itu_mode: false,
            dolby_ltrt: false,
        });

        // In 7.1: ch4=SL(90°), ch5=SR(-90°), ch6=BL(150°), ch7=BR(-150°)
        // All surround speakers should have equal power (constant-power panning).
        let powers: Vec<f32> = [4, 5, 6, 7]
            .iter()
            .map(|&ch| {
                let c = &p.target_coeffs[ch];
                c.left_gain * c.left_gain + c.right_gain * c.right_gain
            })
            .collect();

        let max_power = powers.iter().cloned().fold(0.0f32, f32::max);
        let min_power = powers.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            (max_power - min_power) < 0.01,
            "Surround speakers should have equal power: {:?}",
            powers
        );
        // All should have positive power
        assert!(
            min_power > 0.01,
            "Surround power should be non-trivial: {min_power}"
        );
    }

    /// Verify all speaker configs produce valid coefficients:
    /// - No negative gains
    /// - All height speakers at the same gain have equal power (constant-power)
    /// - All surround speakers at the same gain have equal power
    /// - Left-side speakers go more to left, right-side more to right
    #[test]
    fn test_all_configs_valid_coefficients() {
        use sotf_host::speaker_config::get_speaker_config;

        for config_id in &[
            "2.0", "2.1", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
        ] {
            let config = get_speaker_config(config_id).unwrap();
            let p = DownmixPlugin::from_params(DownmixPluginParams {
                input_channels: config.total_channels,
                center_gain_db: 0.0,
                surround_gain_db: 0.0,
                height_gain_db: 0.0,
                lfe_gain_db: 0.0,
                phase_coherence: false,
                phase_blend_low_hz: 200.0,
                phase_blend_high_hz: 5000.0,
                itu_mode: false,
                dolby_ltrt: false,
            });

            assert_eq!(
                p.target_coeffs.len(),
                config.speakers.len(),
                "{config_id}: coeff count mismatch"
            );

            let mut height_powers = Vec::new();
            let mut surround_powers = Vec::new();

            for (i, spk) in config.speakers.iter().enumerate() {
                let c = &p.target_coeffs[i];

                // No negative gains
                assert!(
                    c.left_gain >= 0.0,
                    "{config_id} {}: left_gain={} is negative",
                    spk.label,
                    c.left_gain
                );
                assert!(
                    c.right_gain >= 0.0,
                    "{config_id} {}: right_gain={} is negative",
                    spk.label,
                    c.right_gain
                );

                // Left-side speakers (azimuth > 1°) should have left_gain >= right_gain
                if !spk.is_lfe && spk.azimuth > 1.0 {
                    assert!(
                        c.left_gain >= c.right_gain,
                        "{config_id} {}: left speaker should favor left (L={}, R={})",
                        spk.label,
                        c.left_gain,
                        c.right_gain
                    );
                }
                // Right-side speakers (azimuth < -1°) should have right_gain >= left_gain
                if !spk.is_lfe && spk.azimuth < -1.0 {
                    assert!(
                        c.right_gain >= c.left_gain,
                        "{config_id} {}: right speaker should favor right (L={}, R={})",
                        spk.label,
                        c.left_gain,
                        c.right_gain
                    );
                }

                let power = c.left_gain * c.left_gain + c.right_gain * c.right_gain;
                if spk.elevation.abs() > 10.0 {
                    height_powers.push(power);
                } else if spk.azimuth.abs() >= 45.0 && !spk.is_lfe {
                    surround_powers.push(power);
                }
            }

            // All height speakers should have equal power (constant-power pan)
            if height_powers.len() > 1 {
                let max_h = height_powers.iter().cloned().fold(0.0f32, f32::max);
                let min_h = height_powers.iter().cloned().fold(f32::MAX, f32::min);
                assert!(
                    (max_h - min_h) < 0.01,
                    "{config_id}: height power variance too large: {:?}",
                    height_powers
                );
            }

            // All surround speakers should have equal power
            if surround_powers.len() > 1 {
                let max_s = surround_powers.iter().cloned().fold(0.0f32, f32::max);
                let min_s = surround_powers.iter().cloned().fold(f32::MAX, f32::min);
                assert!(
                    (max_s - min_s) < 0.01,
                    "{config_id}: surround power variance too large: {:?}",
                    surround_powers
                );
            }
        }
    }
}
