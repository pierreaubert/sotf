// ============================================================================
// Mono-to-Stereo Plugin
// ============================================================================
//
// Converts 1-channel mono to 2-channel stereo using three complementary
// techniques:
//
// 1. Frequency-dependent decorrelation (FFT-based velvet noise all-pass on R)
// 2. Haas delay (short delay on R channel via circular buffer)
// 3. Complementary EQ (opposite-gain peak filters on L/R)
// 4. Width blend (stereo_width 0=mono, 1=full effect)
//
// Signal flow:
//   Mono → [copy to L, R] → R: velvet decorrelation (FFT) → R: Haas delay
//        → L/R: complementary EQ → width blend → stereo out

use super::param_specs::mono_to_stereo::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

fn default_stereo_width() -> f32 {
    STEREO_WIDTH_DEFAULT
}
fn default_haas_delay_ms() -> f32 {
    HAAS_DELAY_MS_DEFAULT
}
fn default_enable_comp_eq() -> bool {
    ENABLE_COMP_EQ_DEFAULT
}
fn default_comp_eq_depth_db() -> f32 {
    COMP_EQ_DEPTH_DB_DEFAULT
}
fn default_decor_low_hz() -> f32 {
    DECOR_LOW_HZ_DEFAULT
}
fn default_decor_high_hz() -> f32 {
    DECOR_HIGH_HZ_DEFAULT
}

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
    #[serde(default = "default_decor_low_hz")]
    pub decor_low_hz: f32,
    #[serde(default = "default_decor_high_hz")]
    pub decor_high_hz: f32,
}

// ============================================================================
// Plugin
// ============================================================================

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = FFT_SIZE / 2;

pub struct MonoToStereoPlugin {
    sample_rate: u32,

    // FFT
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    // Decorrelation filter (applied to R channel only)
    decorrelation_filter: Vec<Complex<f32>>,

    // FFT overlap-add state for R channel decorrelation
    input_ring: Vec<f32>,
    input_ring_pos: usize,
    // Output overlap-add accumulators: [L, R]
    output_accum: [Vec<f32>; 2],
    output_read_pos: usize,
    output_write_pos: usize,
    // Hann window
    window: Vec<f32>,
    // Scratch buffers for FFT
    fft_input_buf: Vec<f32>,
    fft_output_buf: Vec<Complex<f32>>,
    ifft_input_buf: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,
    // How many input samples accumulated since last FFT
    input_fill: usize,
    // Samples of latency output so far (for initial latency fill)
    samples_output: usize,

    // Haas delay circular buffer (for R channel, time-domain)
    haas_buffer: Vec<f32>,
    haas_write_pos: usize,
    haas_delay_samples: usize,

    // Complementary EQ filters (L and R have opposite gains)
    // 3 bands: ~800Hz, ~2.5kHz, ~6kHz
    eq_left: [Biquad; 3],
    eq_right: [Biquad; 3],

    // Parameters
    stereo_width: Smoother,
    haas_delay_ms: f32,
    enable_comp_eq: bool,
    comp_eq_depth_db: f32,
    decor_low_hz: f32,
    decor_high_hz: f32,

    // Parameter IDs
    param_stereo_width: ParameterId,
    param_haas_delay_ms: ParameterId,
    param_enable_comp_eq: ParameterId,
    param_comp_eq_depth_db: ParameterId,
    param_decor_low_hz: ParameterId,
    param_decor_high_hz: ParameterId,
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

        let freq_len = FFT_SIZE / 2 + 1;

        // Build Hann window
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                let t = i as f32 / FFT_SIZE as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * t).cos())
            })
            .collect();

        // Default EQ filters (will be rebuilt on initialize)
        let default_sample_rate = 44100.0;
        let eq_left = Self::build_comp_eq_filters(
            default_sample_rate,
            COMP_EQ_DEPTH_DB_DEFAULT,
            false,
        );
        let eq_right = Self::build_comp_eq_filters(
            default_sample_rate,
            COMP_EQ_DEPTH_DB_DEFAULT,
            true,
        );

        Self {
            sample_rate: 44100,

            fft_forward: fft_forward.clone(),
            fft_inverse: fft_inverse.clone(),

            decorrelation_filter: vec![Complex::new(1.0, 0.0); freq_len],

            input_ring: vec![0.0; FFT_SIZE],
            input_ring_pos: 0,
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
            input_fill: 0,
            samples_output: 0,

            haas_buffer: vec![0.0; 44100], // 1 second max
            haas_write_pos: 0,
            haas_delay_samples: (HAAS_DELAY_MS_DEFAULT * 44.1) as usize,

            eq_left,
            eq_right,

            stereo_width: Smoother::new(STEREO_WIDTH_DEFAULT, 20.0, 44100),
            haas_delay_ms: HAAS_DELAY_MS_DEFAULT,
            enable_comp_eq: ENABLE_COMP_EQ_DEFAULT,
            comp_eq_depth_db: COMP_EQ_DEPTH_DB_DEFAULT,
            decor_low_hz: DECOR_LOW_HZ_DEFAULT,
            decor_high_hz: DECOR_HIGH_HZ_DEFAULT,

            param_stereo_width: ParameterId::from("stereo_width"),
            param_haas_delay_ms: ParameterId::from("haas_delay_ms"),
            param_enable_comp_eq: ParameterId::from("enable_comp_eq"),
            param_comp_eq_depth_db: ParameterId::from("comp_eq_depth_db"),
            param_decor_low_hz: ParameterId::from("decor_low_hz"),
            param_decor_high_hz: ParameterId::from("decor_high_hz"),
        }
    }

    pub fn from_params(params: MonoToStereoPluginParams) -> Self {
        let mut plugin = Self::new();
        plugin.stereo_width.reset(params.stereo_width);
        plugin.haas_delay_ms = params.haas_delay_ms;
        plugin.enable_comp_eq = params.enable_comp_eq;
        plugin.comp_eq_depth_db = params.comp_eq_depth_db;
        plugin.decor_low_hz = params.decor_low_hz;
        plugin.decor_high_hz = params.decor_high_hz;
        plugin
    }

    /// Build complementary EQ filters.
    /// `invert` = true → right channel (opposite gains)
    fn build_comp_eq_filters(sample_rate: f64, depth_db: f32, invert: bool) -> [Biquad; 3] {
        let sign = if invert { -1.0 } else { 1.0 };
        let d = depth_db as f64;
        [
            // ~800Hz: positive on L, negative on R
            Biquad::new(BiquadFilterType::Peak, 800.0, sample_rate, 1.5, sign * d),
            // ~2.5kHz: negative on L, positive on R
            Biquad::new(BiquadFilterType::Peak, 2500.0, sample_rate, 1.5, -sign * d),
            // ~6kHz: positive on L (half depth), negative on R
            Biquad::new(
                BiquadFilterType::Peak,
                6000.0,
                sample_rate,
                1.5,
                sign * d * 0.5,
            ),
        ]
    }

    /// Generate velvet noise decorrelation filter for R channel
    fn generate_decorrelation_filter(&mut self) {
        let sr = self.sample_rate as f32;
        let duration_ms = 30.0_f32;
        let seq_len = ((duration_ms / 1000.0) * sr) as usize;
        let seq_len = seq_len.clamp(128, FFT_SIZE / 2);

        let pulses_per_sec = 2000.0_f32;
        let grid_size = (sr / pulses_per_sec).max(1.0) as usize;

        // LCG for determinism (different seed from upmixer to get independent filter)
        let mut rng_seed = 98765u64;
        let mut rand_u32 = || {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng_seed >> 32) as u32
        };
        let mut rand_f32 = || rand_u32() as f32 / u32::MAX as f32;

        let mut time_buf = vec![0.0f32; FFT_SIZE];

        // Generate velvet noise
        let mut cursor = (rand_f32() * grid_size as f32) as usize;
        while cursor < seq_len {
            let offset = (rand_f32() * grid_size as f32) as usize;
            let pos = (cursor + offset).min(FFT_SIZE - 1);
            let val = if rand_f32() > 0.5 { 1.0 } else { -1.0 };
            time_buf[pos] = val;
            cursor += grid_size;
        }

        // Fade-out window to avoid truncation artifacts
        let fade_len = seq_len / 4;
        if fade_len > 0 {
            let fade_start = seq_len.saturating_sub(fade_len);
            for (i, sample) in time_buf.iter_mut().enumerate().take(seq_len).skip(fade_start) {
                let t = (i - fade_start) as f32 / fade_len as f32;
                let fade = 0.5 * (1.0 + (std::f32::consts::PI * t).cos());
                *sample *= fade;
            }
        }

        // FFT
        let mut input_fft = self.fft_forward.make_input_vec();
        input_fft.copy_from_slice(&time_buf);
        let mut output_fft = self.fft_forward.make_output_vec();
        self.fft_forward
            .process(&mut input_fft, &mut output_fft)
            .unwrap();

        // Normalize to all-pass (magnitude=1, preserve phase)
        for val in output_fft.iter_mut() {
            let norm = val.norm();
            if norm > 1e-9 {
                *val /= norm;
            } else {
                *val = Complex::new(1.0, 0.0);
            }
        }

        // DC and Nyquist: no phase shift
        output_fft[0] = Complex::new(1.0, 0.0);
        let last = output_fft.len() - 1;
        output_fft[last] = Complex::new(1.0, 0.0);

        // Apply frequency-dependent blending: identity below decor_low_hz,
        // full decorrelation above decor_high_hz
        let freq_per_bin = sr / FFT_SIZE as f32;
        for (i, val) in output_fft.iter_mut().enumerate() {
            let freq = i as f32 * freq_per_bin;
            let blend = if freq <= self.decor_low_hz {
                0.0
            } else if freq >= self.decor_high_hz {
                1.0
            } else {
                (freq - self.decor_low_hz) / (self.decor_high_hz - self.decor_low_hz)
            };
            // Interpolate between identity and decorrelation filter
            let identity = Complex::new(1.0, 0.0);
            *val = identity * (1.0 - blend) + *val * blend;
            // Re-normalize to unit magnitude
            let norm = val.norm();
            if norm > 1e-9 {
                *val /= norm;
            }
        }

        // Store
        for (i, val) in output_fft.iter().enumerate() {
            if i < self.decorrelation_filter.len() {
                self.decorrelation_filter[i] = *val;
            }
        }
    }

    /// Process one FFT block: applies decorrelation to R channel
    fn process_fft_block(&mut self) {
        let n = FFT_SIZE;
        let inv_n = 1.0 / n as f32;

        // Window the input and copy to L output (passthrough) and R (to be decorrelated)
        for i in 0..n {
            let ring_idx = (self.input_ring_pos + i) % n;
            let windowed = self.input_ring[ring_idx] * self.window[i];
            self.fft_input_buf[i] = windowed;
        }

        // L channel: just windowed input (overlap-add passthrough)
        // Add windowed input directly to L accumulator
        let write_pos = self.output_write_pos;
        let accum_len = self.output_accum[0].len();
        for i in 0..n {
            let idx = (write_pos + i) % accum_len;
            self.output_accum[0][idx] += self.fft_input_buf[i];
        }

        // R channel: FFT → decorrelate → IFFT
        self.fft_forward
            .process(&mut self.fft_input_buf, &mut self.fft_output_buf)
            .unwrap();

        // Apply decorrelation filter
        for i in 0..self.fft_output_buf.len().min(self.decorrelation_filter.len()) {
            self.fft_output_buf[i] *= self.decorrelation_filter[i];
        }

        // Copy to IFFT input
        for (i, val) in self.fft_output_buf.iter().enumerate() {
            if i < self.ifft_input_buf.len() {
                self.ifft_input_buf[i] = *val;
            }
        }

        // IFFT
        self.fft_inverse
            .process(&mut self.ifft_input_buf, &mut self.ifft_output_buf)
            .unwrap();

        // Add windowed IFFT output to R accumulator (with normalization)
        for i in 0..n {
            let idx = (write_pos + i) % accum_len;
            self.output_accum[1][idx] += self.ifft_output_buf[i] * inv_n;
        }

        // Advance write position by hop
        self.output_write_pos = (write_pos + HOP_SIZE) % accum_len;
    }
}

impl Plugin for MonoToStereoPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Mono to Stereo", "1.0.0", "SotF")
            .with_description("Converts mono to stereo using decorrelation, Haas delay, and complementary EQ")
    }

    fn input_channels(&self) -> usize {
        1
    }

    fn output_channels(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float(
                "stereo_width",
                "Stereo Width",
                STEREO_WIDTH_DEFAULT,
                STEREO_WIDTH_MIN,
                STEREO_WIDTH_MAX,
            )
            .with_description("Width of stereo effect (0=mono, 1=full)")
            .with_group("General")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "haas_delay_ms",
                "Haas Delay (ms)",
                HAAS_DELAY_MS_DEFAULT,
                HAAS_DELAY_MS_MIN,
                HAAS_DELAY_MS_MAX,
            )
            .with_description("Delay on right channel for Haas effect")
            .with_group("Haas")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("enable_comp_eq", "Complementary EQ", ENABLE_COMP_EQ_DEFAULT)
                .with_description("Enable complementary EQ for frequency-dependent panning")
                .with_group("EQ")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "comp_eq_depth_db",
                "EQ Depth (dB)",
                COMP_EQ_DEPTH_DB_DEFAULT,
                COMP_EQ_DEPTH_DB_MIN,
                COMP_EQ_DEPTH_DB_MAX,
            )
            .with_description("Depth of complementary EQ in dB")
            .with_group("EQ")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "decor_low_hz",
                "Decor Low (Hz)",
                DECOR_LOW_HZ_DEFAULT,
                DECOR_LOW_HZ_MIN,
                DECOR_LOW_HZ_MAX,
            )
            .with_description("Below this frequency, decorrelation is zero (preserves mono compatibility)")
            .with_group("Decorrelation")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "decor_high_hz",
                "Decor High (Hz)",
                DECOR_HIGH_HZ_DEFAULT,
                DECOR_HIGH_HZ_MIN,
                DECOR_HIGH_HZ_MAX,
            )
            .with_description("Above this frequency, decorrelation is full")
            .with_group("Decorrelation")
            .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_stereo_width {
            if let Some(v) = value.as_float() {
                self.stereo_width.set_target(v);
                return Ok(());
            }
            return Err("stereo_width must be float".to_string());
        } else if id == self.param_haas_delay_ms {
            if let Some(v) = value.as_float() {
                self.haas_delay_ms = v;
                self.haas_delay_samples =
                    (v * 0.001 * self.sample_rate as f32) as usize;
                return Ok(());
            }
            return Err("haas_delay_ms must be float".to_string());
        } else if id == self.param_enable_comp_eq {
            if let Some(v) = value.as_bool() {
                self.enable_comp_eq = v;
                return Ok(());
            }
            return Err("enable_comp_eq must be bool".to_string());
        } else if id == self.param_comp_eq_depth_db {
            if let Some(v) = value.as_float() {
                self.comp_eq_depth_db = v;
                // Rebuild EQ filters
                let sr = self.sample_rate as f64;
                self.eq_left = Self::build_comp_eq_filters(sr, v, false);
                self.eq_right = Self::build_comp_eq_filters(sr, v, true);
                return Ok(());
            }
            return Err("comp_eq_depth_db must be float".to_string());
        } else if id == self.param_decor_low_hz {
            if let Some(v) = value.as_float() {
                self.decor_low_hz = v;
                self.generate_decorrelation_filter();
                return Ok(());
            }
            return Err("decor_low_hz must be float".to_string());
        } else if id == self.param_decor_high_hz {
            if let Some(v) = value.as_float() {
                self.decor_high_hz = v;
                self.generate_decorrelation_filter();
                return Ok(());
            }
            return Err("decor_high_hz must be float".to_string());
        }

        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_stereo_width {
            Some(ParameterValue::Float(self.stereo_width.target()))
        } else if id == &self.param_haas_delay_ms {
            Some(ParameterValue::Float(self.haas_delay_ms))
        } else if id == &self.param_enable_comp_eq {
            Some(ParameterValue::Bool(self.enable_comp_eq))
        } else if id == &self.param_comp_eq_depth_db {
            Some(ParameterValue::Float(self.comp_eq_depth_db))
        } else if id == &self.param_decor_low_hz {
            Some(ParameterValue::Float(self.decor_low_hz))
        } else if id == &self.param_decor_high_hz {
            Some(ParameterValue::Float(self.decor_high_hz))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Recalculate Haas delay
        self.haas_delay_samples =
            (self.haas_delay_ms * 0.001 * sample_rate as f32) as usize;
        // Resize Haas buffer (max 5ms at any sample rate, plus margin)
        let max_delay_samples = (HAAS_DELAY_MS_MAX * 0.001 * sample_rate as f32) as usize + 1;
        self.haas_buffer = vec![0.0; max_delay_samples];
        self.haas_write_pos = 0;

        // Rebuild complementary EQ
        let sr = sample_rate as f64;
        self.eq_left = Self::build_comp_eq_filters(sr, self.comp_eq_depth_db, false);
        self.eq_right = Self::build_comp_eq_filters(sr, self.comp_eq_depth_db, true);

        // Generate decorrelation filter
        self.generate_decorrelation_filter();

        // Initialize smoother
        self.stereo_width.set_time(20.0, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        self.input_ring.fill(0.0);
        self.input_ring_pos = 0;
        self.output_accum[0].fill(0.0);
        self.output_accum[1].fill(0.0);
        self.output_read_pos = 0;
        self.output_write_pos = 0;
        self.input_fill = 0;
        self.samples_output = 0;
        self.haas_buffer.fill(0.0);
        self.haas_write_pos = 0;

        // Reset EQ filter states
        let sr = self.sample_rate as f64;
        self.eq_left = Self::build_comp_eq_filters(sr, self.comp_eq_depth_db, false);
        self.eq_right = Self::build_comp_eq_filters(sr, self.comp_eq_depth_db, true);
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;

        // Validate
        if input.len() != num_frames {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                num_frames,
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

        let accum_len = self.output_accum[0].len();

        for frame in 0..num_frames {
            let mono_sample = input[frame];

            // Feed into input ring buffer
            self.input_ring[self.input_ring_pos] = mono_sample;
            self.input_ring_pos = (self.input_ring_pos + 1) % FFT_SIZE;
            self.input_fill += 1;

            // When we've accumulated a hop's worth, process an FFT block
            if self.input_fill >= HOP_SIZE {
                self.input_fill = 0;
                self.process_fft_block();
            }

            // Read from output accumulator
            let read_pos = self.output_read_pos;
            let mut left = self.output_accum[0][read_pos];
            let mut right = self.output_accum[1][read_pos];

            // Clear the read position for next overlap-add cycle
            self.output_accum[0][read_pos] = 0.0;
            self.output_accum[1][read_pos] = 0.0;
            self.output_read_pos = (read_pos + 1) % accum_len;

            // Apply Haas delay to R channel
            if self.haas_delay_samples > 0 && !self.haas_buffer.is_empty() {
                let buf_len = self.haas_buffer.len();
                // Write current R sample
                self.haas_buffer[self.haas_write_pos] = right;
                // Read delayed sample
                let delay = self.haas_delay_samples.min(buf_len - 1);
                let read_idx = (self.haas_write_pos + buf_len - delay) % buf_len;
                right = self.haas_buffer[read_idx];
                self.haas_write_pos = (self.haas_write_pos + 1) % buf_len;
            }

            // Apply complementary EQ
            if self.enable_comp_eq && self.comp_eq_depth_db > 0.001 {
                let l64 = left as f64;
                let r64 = right as f64;
                let mut l_eq = l64;
                let mut r_eq = r64;
                for filter in &mut self.eq_left {
                    l_eq = filter.process(l_eq);
                }
                for filter in &mut self.eq_right {
                    r_eq = filter.process(r_eq);
                }
                left = l_eq as f32;
                right = r_eq as f32;
            }

            // Apply width blend: 0=mono (L=R=original), 1=full stereo effect
            let width = self.stereo_width.next();
            let blended_left = mono_sample * (1.0 - width) + left * width;
            let blended_right = mono_sample * (1.0 - width) + right * width;

            output[frame * 2] = blended_left;
            output[frame * 2 + 1] = blended_right;
        }

        self.samples_output += num_frames;
        Ok(num_frames)
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
        let mut plugin = MonoToStereoPlugin::new();
        plugin.initialize(44100).unwrap();

        let num_frames = 4096;
        let input: Vec<f32> = (0..num_frames)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), num_frames);

        // After initial latency, output should have non-zero samples
        let has_nonzero = output.iter().any(|&s| s.abs() > 1e-10);
        assert!(has_nonzero, "Output should contain non-zero samples");
    }

    #[test]
    fn test_mono_to_stereo_width_zero_is_mono() {
        let mut plugin = MonoToStereoPlugin::new();
        plugin.stereo_width.reset(0.0);
        plugin.initialize(44100).unwrap();

        // Process enough to fill latency
        let num_frames = 8192;
        let input: Vec<f32> = (0..num_frames)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // With width=0, L should equal R (both equal to original mono)
        // Check samples after latency period
        let start = FFT_SIZE * 2; // skip latency (in output samples, *2 for stereo)
        for i in (start..output.len()).step_by(2) {
            let l = output[i];
            let r = output[i + 1];
            assert!(
                (l - r).abs() < 1e-4,
                "At width=0, L and R should be identical. Diff={} at sample {}",
                (l - r).abs(),
                i / 2
            );
        }
    }

    #[test]
    fn test_mono_to_stereo_parameters() {
        let mut plugin = MonoToStereoPlugin::new();

        // Test set/get for stereo_width
        plugin
            .set_parameter(
                ParameterId::from("stereo_width"),
                ParameterValue::Float(0.8),
            )
            .unwrap();
        assert_eq!(
            plugin
                .get_parameter(&ParameterId::from("stereo_width"))
                .unwrap()
                .as_float(),
            Some(0.8)
        );

        // Test set/get for enable_comp_eq
        plugin
            .set_parameter(
                ParameterId::from("enable_comp_eq"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        assert_eq!(
            plugin
                .get_parameter(&ParameterId::from("enable_comp_eq"))
                .unwrap()
                .as_bool(),
            Some(false)
        );

        // Test unknown parameter
        let result = plugin.set_parameter(
            ParameterId::from("unknown"),
            ParameterValue::Float(1.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_mono_to_stereo_from_params() {
        let params = MonoToStereoPluginParams {
            stereo_width: 0.7,
            haas_delay_ms: 2.0,
            enable_comp_eq: false,
            comp_eq_depth_db: 0.5,
            decor_low_hz: 200.0,
            decor_high_hz: 3000.0,
        };

        let plugin = MonoToStereoPlugin::from_params(params);
        assert_eq!(plugin.stereo_width.target(), 0.7);
        assert_eq!(plugin.haas_delay_ms, 2.0);
        assert!(!plugin.enable_comp_eq);
    }

    #[test]
    fn test_plugin_info() {
        let plugin = MonoToStereoPlugin::new();
        assert_eq!(plugin.input_channels(), 1);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.latency_samples(), FFT_SIZE);
    }
}
