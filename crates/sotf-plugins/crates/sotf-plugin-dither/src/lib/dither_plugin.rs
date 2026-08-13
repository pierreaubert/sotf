use super::default::default_bit_depth;
use super::default::default_dither_type;
use super::default::default_noise_shaping;
use super::misc::BIT_DEPTHS;
use super::misc::NOISE_SHAPING_COEFFS;
use super::misc::random_f32;
use super::types::DitherPluginParams;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};

pub struct DitherPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Parameters
    pub(super) bit_depth_index: usize,
    pub(super) noise_shaping_enabled: bool,
    pub(super) dither_type_index: usize,

    // Pre-computed from parameters
    pub(super) scale: f32,
    pub(super) inv_scale: f32,
    /// Error-history tap delays, expressed in samples at the active rate.
    /// The published F-weighted shaper is referenced to 44.1 kHz; scaling
    /// these delays preserves its absolute-frequency response at higher rates.
    pub(super) noise_shaping_delays_samples: [f32; 3],

    // DSP state (per-channel, pre-allocated)
    pub(super) error_history: Vec<Vec<f32>>,
    pub(super) error_history_heads: Vec<usize>,
    pub(super) rng_state: Vec<u64>,

    // Parameter IDs
    pub(super) param_bit_depth: ParameterId,
    pub(super) param_noise_shaping: ParameterId,
    pub(super) param_dither_type: ParameterId,

    pub(super) cached_parameters: Vec<Parameter>,
}

impl DitherPlugin {
    pub fn new(channels: usize) -> Self {
        let bit_depth_index = default_bit_depth();
        let noise_shaping = default_noise_shaping();
        let dither_type = default_dither_type();

        let bits = BIT_DEPTHS[bit_depth_index];
        let scale = 2.0_f32.powi(bits - 1);

        let mut p = Self {
            channels,
            sample_rate: 48000,
            bit_depth_index,
            noise_shaping_enabled: noise_shaping,
            dither_type_index: dither_type,
            scale,
            inv_scale: 1.0 / scale,
            noise_shaping_delays_samples: [1.0, 2.0, 3.0],
            error_history: vec![vec![0.0; 4]; channels],
            error_history_heads: vec![3; channels],
            rng_state: Self::init_rng_states(channels),
            param_bit_depth: ParameterId::from("bit_depth"),
            param_noise_shaping: ParameterId::from("noise_shaping"),
            param_dither_type: ParameterId::from("dither_type"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: DitherPluginParams) -> Self {
        let bit_depth_index = params.bit_depth.min(BIT_DEPTHS.len() - 1);
        let bits = BIT_DEPTHS[bit_depth_index];
        let scale = 2.0_f32.powi(bits - 1);

        let mut p = Self {
            channels,
            sample_rate: 48000,
            bit_depth_index,
            noise_shaping_enabled: params.noise_shaping,
            dither_type_index: params.dither_type.min(2),
            scale,
            inv_scale: 1.0 / scale,
            noise_shaping_delays_samples: [1.0, 2.0, 3.0],
            error_history: vec![vec![0.0; 4]; channels],
            error_history_heads: vec![3; channels],
            rng_state: Self::init_rng_states(channels),
            param_bit_depth: ParameterId::from("bit_depth"),
            param_noise_shaping: ParameterId::from("noise_shaping"),
            param_dither_type: ParameterId::from("dither_type"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub(super) fn init_rng_states(channels: usize) -> Vec<u64> {
        // Seed each channel with a different non-zero value
        (0..channels)
            .map(|ch| {
                0xDEAD_BEEF_CAFE_0001_u64
                    .wrapping_add((ch as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            })
            .collect()
    }

    pub(super) fn update_scales(&mut self) {
        let bits = BIT_DEPTHS[self.bit_depth_index];
        self.scale = 2.0_f32.powi(bits - 1);
        self.inv_scale = 1.0 / self.scale;
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int(
                "bit_depth",
                "Bit Depth",
                self.bit_depth_index as i32,
                0,
                (BIT_DEPTHS.len() - 1) as i32,
            ),
            Parameter::new_bool("noise_shaping", "Noise Shaping", self.noise_shaping_enabled),
            Parameter::new_int(
                "dither_type",
                "Dither Type",
                self.dither_type_index as i32,
                0,
                2,
            ),
        ];
    }

    /// Compute noise shaping feedback from the error history for one channel.
    #[inline(always)]
    pub(super) fn noise_shaping_feedback(
        history: &[f32],
        head: usize,
        delays_samples: &[f32; 3],
    ) -> f32 {
        NOISE_SHAPING_COEFFS
            .iter()
            .zip(delays_samples)
            .map(|(coefficient, delay_samples)| {
                // history[0] is e[n-1]. Linear interpolation between adjacent
                // past errors realizes sample-rate-scaled (fractional) taps
                // without allocation or coefficient redesign in the callback.
                let history_position = (delay_samples - 1.0).max(0.0);
                let lower = history_position.floor() as usize;
                let fraction = history_position - lower as f32;
                let upper = (lower + 1).min(history.len() - 1);
                let lower_index = (head + history.len() - lower) % history.len();
                let upper_index = (head + history.len() - upper) % history.len();
                coefficient
                    * (history[lower_index] * (1.0 - fraction) + history[upper_index] * fraction)
            })
            .sum()
    }

    /// Push a new error into the history ring (most recent at index 0).
    #[inline(always)]
    pub(super) fn push_error(history: &mut [f32], head: &mut usize, error: f32) {
        *head = (*head + 1) % history.len();
        history[*head] = error;
    }

    fn configure_noise_shaping_rate(&mut self, sample_rate: u32) -> PluginResult<()> {
        const REFERENCE_SAMPLE_RATE: f32 = 44_100.0;
        const MAX_SUPPORTED_SAMPLE_RATE: u32 = 768_000;
        if sample_rate == 0 || sample_rate > MAX_SUPPORTED_SAMPLE_RATE {
            return Err(format!(
                "Dither sample rate must be in 1..={MAX_SUPPORTED_SAMPLE_RATE}, got {sample_rate}"
            ));
        }

        // Below the reference rate, retain the normalized published response:
        // its ultrasonic destination band is no longer representable. At and
        // above 44.1 kHz, scale tap time so the weighting stays anchored in Hz.
        let rate_scale = (sample_rate as f32 / REFERENCE_SAMPLE_RATE).max(1.0);
        self.noise_shaping_delays_samples = [rate_scale, 2.0 * rate_scale, 3.0 * rate_scale];
        let history_len = self.noise_shaping_delays_samples[2].ceil() as usize + 1;
        self.error_history = vec![vec![0.0; history_len]; self.channels];
        self.error_history_heads = vec![history_len - 1; self.channels];
        Ok(())
    }

    /// Generate independent TPDF dither as the difference of two independent
    /// uniforms in [-0.5, 0.5]. No random component is reused across samples.
    #[inline(always)]
    pub(super) fn next_tpdf(&mut self, ch: usize) -> f32 {
        let a = random_f32(&mut self.rng_state[ch]);
        let b = random_f32(&mut self.rng_state[ch]);
        a - b
    }
}

impl ParametricInPlacePlugin for DitherPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Dither", env!("CARGO_PKG_VERSION"), "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Dynamics
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Dynamics, None, 0, false)
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        values.insert(
            ParameterId::from("bit_depth"),
            ParameterValue::Int(self.bit_depth_index as i32),
        );
        values.insert(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(self.noise_shaping_enabled),
        );
        values.insert(
            ParameterId::from("dither_type"),
            ParameterValue::Int(self.dither_type_index as i32),
        );
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, val) in values {
            if id == self.param_bit_depth {
                if let Some(v) = val.as_int() {
                    let idx = v.clamp(0, (BIT_DEPTHS.len() - 1) as i32) as usize;
                    self.bit_depth_index = idx;
                    self.update_scales();
                } else {
                    return Err("bit_depth must be an int".to_string());
                }
            } else if id == self.param_noise_shaping {
                if let Some(v) = val.as_bool() {
                    self.noise_shaping_enabled = v;
                } else {
                    return Err("noise_shaping must be a bool".to_string());
                }
            } else if id == self.param_dither_type {
                if let Some(v) = val.as_int() {
                    let idx = v.clamp(0, 2) as usize;
                    self.dither_type_index = idx;
                } else {
                    return Err("dither_type must be an int".to_string());
                }
            } else {
                return Err(format!("Invalid or unknown parameter: {}", id));
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    /// Preserve original DitherPlugin behavior: clamp out-of-range ints instead of
    /// rejecting them at the schema-validation layer.
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let mut values = ParameterSet::new();
        values.insert(id, value);
        self.apply_values(values)
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.configure_noise_shaping_rate(sr)?;
        self.sample_rate = sr;
        if self.rng_state.len() != self.channels {
            self.rng_state = Self::init_rng_states(self.channels);
        }
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.rng_state = Self::init_rng_states(self.channels);
        for h in &mut self.error_history {
            h.fill(0.0);
        }
        for (head, history) in self.error_history_heads.iter_mut().zip(&self.error_history) {
            *head = history.len() - 1;
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let ch = self.channels;
        let scale = self.scale;
        let inv_scale = self.inv_scale;
        // Signed PCM uses one more negative code than positive code.  Keep
        // the integer bounds explicit so +1.0 (and noise-shaping overshoot)
        // cannot produce an unrepresentable code before conversion back to
        // normalized float.
        let min_code = -(scale as i32);
        let max_code = scale as i32 - 1;
        let dither_type = self.dither_type_index;
        let noise_shaping = self.noise_shaping_enabled;

        for frame in 0..nf {
            let base = frame * ch;
            for c in 0..ch {
                let idx = base + c;
                let input = buffer[idx];

                // Noise shaping feedback
                let shaped = if noise_shaping {
                    input
                        - Self::noise_shaping_feedback(
                            &self.error_history[c],
                            self.error_history_heads[c],
                            &self.noise_shaping_delays_samples,
                        )
                } else {
                    input
                };

                let dithered = match dither_type {
                    // TPDF dither: difference of two independent uniforms.
                    0 => shaped + self.next_tpdf(c) * inv_scale,
                    _ => shaped,
                };

                let quantized_code = match dither_type {
                    // index 0: TPDF
                    // index 1: no dither, rounded quantization ("None (round)")
                    1 => (dithered * scale).round() as i32,
                    // index 2: no dither, truncated quantization ("Truncate")
                    2 => (dithered * scale).trunc() as i32,
                    // fallback for malformed values (e.g. serialized legacy state)
                    _ => (dithered * scale).round() as i32,
                };
                let quantized = quantized_code.clamp(min_code, max_code) as f32 * inv_scale;

                // Compute quantization error and store for noise shaping
                if noise_shaping {
                    // Feedback only quantizer error relative to its actual input.
                    // Subtracting `dithered` excludes explicit dither from the state.
                    let error = quantized - dithered;
                    Self::push_error(
                        &mut self.error_history[c],
                        &mut self.error_history_heads[c],
                        error,
                    );
                }

                buffer[idx] = quantized;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}
