// ============================================================================
// Phase-Coherent Downmix Plugin
// ============================================================================
//
// Converts N-channel surround (5.0, 5.1, 7.1, 7.1.4, etc.) to stereo using
// frequency-domain phase-aware summing.
//
// Algorithm:
// 1. ITU-R BS.775 baseline coefficients per speaker
// 2. Phase-coherent summing: FFT all channels, align phases to dominant
//    contributor per bin (above blend_high_hz), preserve phases below blend_low_hz
// 3. LFE handling with LR4 lowpass at 120Hz
// 4. Channel layout detection via speaker_config

use super::param_specs::downmix::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};
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
    /// Number of input channels
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

// ============================================================================
// Downmix coefficients per speaker
// ============================================================================

/// Which stereo bus a speaker contributes to
struct DownmixCoeffs {
    /// Gain into left output
    left_gain: f32,
    /// Gain into right output
    right_gain: f32,
}

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 2;

// ============================================================================
// Plugin
// ============================================================================

pub struct DownmixPlugin {
    input_ch: usize,
    sample_rate: u32,

    // Speaker config
    speaker_config: Option<&'static SpeakerConfig>,
    /// Per-input-channel downmix coefficients
    coeffs: Vec<DownmixCoeffs>,
    /// Which channels are LFE
    lfe_channels: Vec<usize>,

    // FFT (only used when phase_coherence is enabled)
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    // Per-channel FFT buffers
    channel_freq: Vec<Vec<Complex<f32>>>,
    // Output L/R frequency domain
    out_freq: [Vec<Complex<f32>>; 2],

    // Overlap-add state
    input_ring: Vec<Vec<f32>>,    // [channel][sample]
    input_ring_pos: usize,
    input_fill: usize,
    output_accum: [Vec<f32>; 2],  // [L/R][sample]
    output_read_pos: usize,
    output_write_pos: usize,
    window: Vec<f32>,

    // Scratch buffers
    fft_input_buf: Vec<f32>,
    fft_output_buf: Vec<Complex<f32>>,
    ifft_input_buf: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,

    // LFE lowpass (LR4 = two cascaded 2nd-order Butterworth at 120Hz)
    // One pair per LFE channel, applied to L and R contributions
    lfe_lpf: Vec<[Biquad; 2]>,

    // Parameters
    center_gain_db: f32,
    surround_gain_db: f32,
    height_gain_db: f32,
    lfe_gain_db: f32,
    phase_coherence: bool,
    phase_blend_low_hz: f32,
    phase_blend_high_hz: f32,

    // Parameter IDs
    param_center_gain_db: ParameterId,
    param_surround_gain_db: ParameterId,
    param_height_gain_db: ParameterId,
    param_lfe_gain_db: ParameterId,
    param_phase_coherence: ParameterId,
    param_phase_blend_low_hz: ParameterId,
    param_phase_blend_high_hz: ParameterId,
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
            coeffs: Vec::new(),
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
            output_accum: [
                vec![0.0; FFT_SIZE * 3],
                vec![0.0; FFT_SIZE * 3],
            ],
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
            param_phase_blend_low_hz: ParameterId::from("phase_blend_low_hz"),
            param_phase_blend_high_hz: ParameterId::from("phase_blend_high_hz"),
        };

        plugin.compute_coefficients();
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
        plugin.compute_coefficients();
        plugin
    }

    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    /// Compute downmix coefficients based on speaker positions and gain parameters
    fn compute_coefficients(&mut self) {
        self.coeffs.clear();
        self.lfe_channels.clear();

        let center_lin = Self::db_to_linear(self.center_gain_db);
        let surround_lin = Self::db_to_linear(self.surround_gain_db);
        let height_lin = Self::db_to_linear(self.height_gain_db);
        let lfe_lin = Self::db_to_linear(self.lfe_gain_db);

        if let Some(config) = self.speaker_config {
            for speaker in config.speakers {
                if speaker.is_lfe {
                    self.lfe_channels.push(speaker.channel);
                    // LFE mixed equally into both channels
                    self.coeffs.push(DownmixCoeffs {
                        left_gain: lfe_lin * 0.5,
                        right_gain: lfe_lin * 0.5,
                    });
                    continue;
                }

                let azimuth = speaker.azimuth;
                let elevation = speaker.elevation;

                // Determine category gain
                let category_gain = if elevation.abs() > 10.0 {
                    // Height speaker
                    height_lin
                } else if azimuth.abs() < 15.0 {
                    // Center (azimuth ~0)
                    center_lin
                } else if azimuth.abs() > 90.0 {
                    // Rear/surround
                    surround_lin
                } else if azimuth.abs() > 60.0 {
                    // Side surround
                    surround_lin
                } else {
                    // Front L/R - unity
                    1.0
                };

                // Constant-power panning based on azimuth
                // azimuth: +angle = left, -angle = right
                let pan_rad = azimuth.to_radians();
                // Map: 0° → equal L/R, +90° → full L, -90° → full R
                let angle = (pan_rad + std::f32::consts::FRAC_PI_2) * 0.5;
                let left_gain = category_gain * angle.cos();
                let right_gain = category_gain * angle.sin();

                self.coeffs.push(DownmixCoeffs {
                    left_gain,
                    right_gain,
                });
            }
        } else {
            // Fallback: simple stereo passthrough or equal mix
            for ch in 0..self.input_ch {
                if self.input_ch == 1 {
                    self.coeffs.push(DownmixCoeffs {
                        left_gain: 1.0,
                        right_gain: 1.0,
                    });
                } else {
                    // Assume first half left, second half right
                    let t = ch as f32 / (self.input_ch - 1).max(1) as f32;
                    self.coeffs.push(DownmixCoeffs {
                        left_gain: 1.0 - t,
                        right_gain: t,
                    });
                }
            }
        }
    }

    fn build_lfe_lowpass(sample_rate: f64) -> [Biquad; 2] {
        // LR4 = two cascaded 2nd-order Butterworth lowpass at 120Hz
        [
            Biquad::new(BiquadFilterType::Lowpass, 120.0, sample_rate, 0.0, 0.0),
            Biquad::new(BiquadFilterType::Lowpass, 120.0, sample_rate, 0.0, 0.0),
        ]
    }

    /// Simple (non-FFT) downmix for when phase_coherence is disabled
    fn process_simple(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        num_frames: usize,
    ) {
        for frame in 0..num_frames {
            let mut left = 0.0f32;
            let mut right = 0.0f32;

            for ch in 0..self.input_ch {
                let sample = input[frame * self.input_ch + ch];

                // Apply LFE lowpass if this is an LFE channel
                let filtered = if self.lfe_channels.contains(&ch) {
                    if let Some(lfe_idx) = self.lfe_channels.iter().position(|&c| c == ch) {
                        if lfe_idx < self.lfe_lpf.len() {
                            let mut s = sample as f64;
                            s = self.lfe_lpf[lfe_idx][0].process(s);
                            s = self.lfe_lpf[lfe_idx][1].process(s);
                            s as f32
                        } else {
                            sample
                        }
                    } else {
                        sample
                    }
                } else {
                    sample
                };

                if ch < self.coeffs.len() {
                    left += filtered * self.coeffs[ch].left_gain;
                    right += filtered * self.coeffs[ch].right_gain;
                }
            }

            output[frame * 2] = left;
            output[frame * 2 + 1] = right;
        }
    }

    /// Process one FFT block with phase-coherent summing
    fn process_fft_block(&mut self) {
        let n = FFT_SIZE;
        let inv_n = 1.0 / n as f32;
        let freq_len = n / 2 + 1;
        let freq_per_bin = self.sample_rate as f32 / n as f32;

        // FFT each input channel
        for ch in 0..self.input_ch {
            for i in 0..n {
                let ring_idx = (self.input_ring_pos + i) % n;
                self.fft_input_buf[i] = self.input_ring[ch][ring_idx] * self.window[i];
            }
            self.fft_forward
                .process(&mut self.fft_input_buf, &mut self.fft_output_buf)
                .unwrap();
            self.channel_freq[ch].copy_from_slice(&self.fft_output_buf);
        }

        // Phase-coherent summing per bin
        for bin in 0..freq_len {
            let freq = bin as f32 * freq_per_bin;

            // Compute phase coherence blend factor
            let blend = if freq <= self.phase_blend_low_hz {
                0.0 // Preserve original phases
            } else if freq >= self.phase_blend_high_hz {
                1.0 // Full phase alignment
            } else {
                let t = (freq - self.phase_blend_low_hz)
                    / (self.phase_blend_high_hz - self.phase_blend_low_hz);
                // Smooth step
                t * t * (3.0 - 2.0 * t)
            };

            // Find dominant contributor for L and R
            let mut sum_l = Complex::new(0.0f32, 0.0);
            let mut sum_r = Complex::new(0.0f32, 0.0);
            let mut max_l_mag = 0.0f32;
            let mut max_r_mag = 0.0f32;
            let mut dominant_l_phase = 0.0f32;
            let mut dominant_r_phase = 0.0f32;

            for ch in 0..self.input_ch {
                if ch >= self.coeffs.len() {
                    continue;
                }
                let spec = self.channel_freq[ch][bin];
                let weighted_l = spec * self.coeffs[ch].left_gain;
                let weighted_r = spec * self.coeffs[ch].right_gain;

                let mag_l = weighted_l.norm();
                let mag_r = weighted_r.norm();

                if mag_l > max_l_mag {
                    max_l_mag = mag_l;
                    dominant_l_phase = weighted_l.arg();
                }
                if mag_r > max_r_mag {
                    max_r_mag = mag_r;
                    dominant_r_phase = weighted_r.arg();
                }

                // Sum with original phases (for blend=0 path)
                sum_l += weighted_l;
                sum_r += weighted_r;
            }

            if blend < 0.001 {
                // Pure original-phase sum
                self.out_freq[0][bin] = sum_l;
                self.out_freq[1][bin] = sum_r;
            } else if blend > 0.999 {
                // Full phase-aligned sum
                let mut aligned_l = Complex::new(0.0, 0.0);
                let mut aligned_r = Complex::new(0.0, 0.0);

                for ch in 0..self.input_ch {
                    if ch >= self.coeffs.len() {
                        continue;
                    }
                    let spec = self.channel_freq[ch][bin];
                    let weighted_l = spec * self.coeffs[ch].left_gain;
                    let weighted_r = spec * self.coeffs[ch].right_gain;

                    // Align to dominant phase
                    let mag_l = weighted_l.norm();
                    let mag_r = weighted_r.norm();
                    aligned_l += Complex::from_polar(mag_l, dominant_l_phase);
                    aligned_r += Complex::from_polar(mag_r, dominant_r_phase);
                }

                self.out_freq[0][bin] = aligned_l;
                self.out_freq[1][bin] = aligned_r;
            } else {
                // Blended: interpolate between original and aligned
                let mut aligned_l = Complex::new(0.0, 0.0);
                let mut aligned_r = Complex::new(0.0, 0.0);

                for ch in 0..self.input_ch {
                    if ch >= self.coeffs.len() {
                        continue;
                    }
                    let spec = self.channel_freq[ch][bin];
                    let weighted_l = spec * self.coeffs[ch].left_gain;
                    let weighted_r = spec * self.coeffs[ch].right_gain;

                    let mag_l = weighted_l.norm();
                    let mag_r = weighted_r.norm();
                    aligned_l += Complex::from_polar(mag_l, dominant_l_phase);
                    aligned_r += Complex::from_polar(mag_r, dominant_r_phase);
                }

                self.out_freq[0][bin] = sum_l * (1.0 - blend) + aligned_l * blend;
                self.out_freq[1][bin] = sum_r * (1.0 - blend) + aligned_r * blend;
            }
        }

        // DC and Nyquist bins must be real for realfft IFFT
        let last_bin = freq_len - 1;
        for lr in 0..2 {
            self.out_freq[lr][0] = Complex::new(self.out_freq[lr][0].re, 0.0);
            self.out_freq[lr][last_bin] = Complex::new(self.out_freq[lr][last_bin].re, 0.0);
        }

        // IFFT L and R, overlap-add
        let write_pos = self.output_write_pos;
        let accum_len = self.output_accum[0].len();

        for lr in 0..2 {
            for (i, val) in self.out_freq[lr].iter().enumerate() {
                if i < self.ifft_input_buf.len() {
                    self.ifft_input_buf[i] = *val;
                }
            }

            self.fft_inverse
                .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
                .unwrap();

            for i in 0..FFT_SIZE {
                let idx = (write_pos + i) % accum_len;
                self.output_accum[lr][idx] += self.ifft_output_buf[i] * inv_n;
            }
        }

        self.output_write_pos = (write_pos + HOP_SIZE) % accum_len;
    }
}

impl Plugin for DownmixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Downmix", "1.0.0", "SotF")
            .with_description("Phase-coherent surround to stereo downmix")
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
                "Center Gain (dB)",
                CENTER_GAIN_DB_DEFAULT,
                CENTER_GAIN_DB_MIN,
                CENTER_GAIN_DB_MAX,
            )
            .with_description("Gain for center channel in downmix")
            .with_group("Levels")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "surround_gain_db",
                "Surround Gain (dB)",
                SURROUND_GAIN_DB_DEFAULT,
                SURROUND_GAIN_DB_MIN,
                SURROUND_GAIN_DB_MAX,
            )
            .with_description("Gain for surround channels in downmix")
            .with_group("Levels")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "height_gain_db",
                "Height Gain (dB)",
                HEIGHT_GAIN_DB_DEFAULT,
                HEIGHT_GAIN_DB_MIN,
                HEIGHT_GAIN_DB_MAX,
            )
            .with_description("Gain for height channels in downmix")
            .with_group("Levels")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "lfe_gain_db",
                "LFE Gain (dB)",
                LFE_GAIN_DB_DEFAULT,
                LFE_GAIN_DB_MIN,
                LFE_GAIN_DB_MAX,
            )
            .with_description("Gain for LFE channel in downmix")
            .with_group("Levels")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "phase_coherence",
                "Phase Coherence",
                PHASE_COHERENCE_DEFAULT,
            )
            .with_description("Enable FFT-based phase-coherent summing")
            .with_group("Processing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "phase_blend_low_hz",
                "Blend Low (Hz)",
                PHASE_BLEND_LOW_HZ_DEFAULT,
                PHASE_BLEND_LOW_HZ_MIN,
                PHASE_BLEND_LOW_HZ_MAX,
            )
            .with_description("Below this freq: preserve original phases")
            .with_group("Processing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "phase_blend_high_hz",
                "Blend High (Hz)",
                PHASE_BLEND_HIGH_HZ_DEFAULT,
                PHASE_BLEND_HIGH_HZ_MIN,
                PHASE_BLEND_HIGH_HZ_MAX,
            )
            .with_description("Above this freq: full phase alignment")
            .with_group("Processing")
            .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_center_gain_db {
            if let Some(v) = value.as_float() {
                self.center_gain_db = v;
                self.compute_coefficients();
                return Ok(());
            }
            return Err("center_gain_db must be float".to_string());
        } else if id == self.param_surround_gain_db {
            if let Some(v) = value.as_float() {
                self.surround_gain_db = v;
                self.compute_coefficients();
                return Ok(());
            }
            return Err("surround_gain_db must be float".to_string());
        } else if id == self.param_height_gain_db {
            if let Some(v) = value.as_float() {
                self.height_gain_db = v;
                self.compute_coefficients();
                return Ok(());
            }
            return Err("height_gain_db must be float".to_string());
        } else if id == self.param_lfe_gain_db {
            if let Some(v) = value.as_float() {
                self.lfe_gain_db = v;
                self.compute_coefficients();
                return Ok(());
            }
            return Err("lfe_gain_db must be float".to_string());
        } else if id == self.param_phase_coherence {
            if let Some(v) = value.as_bool() {
                self.phase_coherence = v;
                return Ok(());
            }
            return Err("phase_coherence must be bool".to_string());
        } else if id == self.param_phase_blend_low_hz {
            if let Some(v) = value.as_float() {
                self.phase_blend_low_hz = v;
                return Ok(());
            }
            return Err("phase_blend_low_hz must be float".to_string());
        } else if id == self.param_phase_blend_high_hz {
            if let Some(v) = value.as_float() {
                self.phase_blend_high_hz = v;
                return Ok(());
            }
            return Err("phase_blend_high_hz must be float".to_string());
        }

        Err(format!("Unknown parameter: {}", id))
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
        } else if id == &self.param_phase_blend_low_hz {
            Some(ParameterValue::Float(self.phase_blend_low_hz))
        } else if id == &self.param_phase_blend_high_hz {
            Some(ParameterValue::Float(self.phase_blend_high_hz))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Build LFE lowpass filters
        let sr = sample_rate as f64;
        self.lfe_lpf = self
            .lfe_channels
            .iter()
            .map(|_| Self::build_lfe_lowpass(sr))
            .collect();

        Ok(())
    }

    fn reset(&mut self) {
        for ch_ring in &mut self.input_ring {
            ch_ring.fill(0.0);
        }
        self.input_ring_pos = 0;
        self.input_fill = 0;
        self.output_accum[0].fill(0.0);
        self.output_accum[1].fill(0.0);
        self.output_read_pos = 0;
        self.output_write_pos = 0;

        // Rebuild LFE filters to clear state
        let sr = self.sample_rate as f64;
        self.lfe_lpf = self
            .lfe_channels
            .iter()
            .map(|_| Self::build_lfe_lowpass(sr))
            .collect();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;

        if input.len() != num_frames * self.input_ch {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                num_frames * self.input_ch,
                input.len()
            ));
        }
        if output.len() != num_frames * 2 {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                num_frames * 2,
                output.len()
            ));
        }

        output.fill(0.0);

        // Simple path: no phase coherence
        if !self.phase_coherence {
            self.process_simple(input, output, num_frames);
            return Ok(num_frames);
        }

        // Phase-coherent path: FFT overlap-add
        let accum_len = self.output_accum[0].len();

        for frame in 0..num_frames {
            // Feed all channels into ring buffers
            for ch in 0..self.input_ch {
                let sample = input[frame * self.input_ch + ch];

                // Apply LFE lowpass in time domain before FFT
                let filtered = if let Some(lfe_idx) =
                    self.lfe_channels.iter().position(|&c| c == ch)
                {
                    if lfe_idx < self.lfe_lpf.len() {
                        let mut s = sample as f64;
                        s = self.lfe_lpf[lfe_idx][0].process(s);
                        s = self.lfe_lpf[lfe_idx][1].process(s);
                        s as f32
                    } else {
                        sample
                    }
                } else {
                    sample
                };

                self.input_ring[ch][self.input_ring_pos] = filtered;
            }
            self.input_ring_pos = (self.input_ring_pos + 1) % FFT_SIZE;
            self.input_fill += 1;

            if self.input_fill >= HOP_SIZE {
                self.input_fill = 0;
                self.process_fft_block();
            }

            // Read from output accumulator
            let read_pos = self.output_read_pos;
            let left = self.output_accum[0][read_pos];
            let right = self.output_accum[1][read_pos];
            self.output_accum[0][read_pos] = 0.0;
            self.output_accum[1][read_pos] = 0.0;
            self.output_read_pos = (read_pos + 1) % accum_len;

            output[frame * 2] = left;
            output[frame * 2 + 1] = right;
        }

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        if self.phase_coherence {
            FFT_SIZE
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downmix_basic_stereo_passthrough() {
        // 2ch input → 2ch output should be close to passthrough
        let mut plugin = DownmixPlugin::new(2);
        plugin.initialize(44100).unwrap();
        // Disable phase coherence for simple test
        plugin.phase_coherence = false;

        let num_frames = 1024;
        let mut input = vec![0.0f32; num_frames * 2];
        for i in 0..num_frames {
            input[i * 2] = (i as f32 * 0.01).sin();     // L
            input[i * 2 + 1] = (i as f32 * 0.02).sin(); // R
        }
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Output should have non-zero data
        let has_nonzero = output.iter().any(|&s| s.abs() > 1e-10);
        assert!(has_nonzero);
    }

    #[test]
    fn test_downmix_51_simple() {
        // 6ch (5.1) → 2ch
        let mut plugin = DownmixPlugin::new(6);
        plugin.phase_coherence = false;
        plugin.initialize(44100).unwrap();

        let num_frames = 1024;
        let mut input = vec![0.0f32; num_frames * 6];
        // Put signal only in center channel (ch 2 for 5.1)
        for i in 0..num_frames {
            input[i * 6 + 2] = 1.0; // Center
        }
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Center should appear in both L and R
        for i in 0..num_frames {
            assert!(output[i * 2].abs() > 0.01, "L should have center signal");
            assert!(output[i * 2 + 1].abs() > 0.01, "R should have center signal");
            // L and R should be similar for center content
            assert!(
                (output[i * 2] - output[i * 2 + 1]).abs() < 0.1,
                "Center should be equal in L and R"
            );
        }
    }

    #[test]
    fn test_downmix_parameters() {
        let mut plugin = DownmixPlugin::new(6);

        plugin
            .set_parameter(
                ParameterId::from("center_gain_db"),
                ParameterValue::Float(-6.0),
            )
            .unwrap();
        assert_eq!(
            plugin
                .get_parameter(&ParameterId::from("center_gain_db"))
                .unwrap()
                .as_float(),
            Some(-6.0)
        );

        plugin
            .set_parameter(
                ParameterId::from("phase_coherence"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        assert_eq!(
            plugin
                .get_parameter(&ParameterId::from("phase_coherence"))
                .unwrap()
                .as_bool(),
            Some(false)
        );

        let result = plugin.set_parameter(
            ParameterId::from("unknown"),
            ParameterValue::Float(1.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_downmix_plugin_info() {
        let plugin = DownmixPlugin::new(6);
        assert_eq!(plugin.input_channels(), 6);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_downmix_phase_coherent() {
        let mut plugin = DownmixPlugin::new(6);
        plugin.phase_coherence = true;
        plugin.initialize(44100).unwrap();

        let num_frames = 4096;
        let mut input = vec![0.0f32; num_frames * 6];
        for i in 0..num_frames {
            let s = (i as f32 * 0.01).sin();
            input[i * 6] = s; // FL
            input[i * 6 + 1] = s; // FR
        }
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());
    }
}
