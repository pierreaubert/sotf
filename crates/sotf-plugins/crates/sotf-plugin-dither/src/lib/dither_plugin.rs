use super::default::default_bit_depth;
use super::default::default_dither_type;
use super::default::default_noise_shaping;
use super::misc::BIT_DEPTHS;
use super::misc::NOISE_SHAPING_COEFFS;
use super::misc::random_f32;
use super::types::DitherPluginParams;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
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

    // DSP state (per-channel, pre-allocated)
    pub(super) error_history: Vec<[f32; 3]>,
    pub(super) rng_state: Vec<u64>,
    pub(super) prev_random: Vec<f32>,

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
            error_history: vec![[0.0; 3]; channels],
            rng_state: Self::init_rng_states(channels),
            prev_random: vec![0.0; channels],
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
            error_history: vec![[0.0; 3]; channels],
            rng_state: Self::init_rng_states(channels),
            prev_random: vec![0.0; channels],
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
            .map(|ch| 0xDEAD_BEEF_CAFE_0001_u64.wrapping_add(ch as u64 * 0x9E37_79B9_7F4A_7C15))
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
    pub(super) fn noise_shaping_feedback(history: &[f32; 3]) -> f32 {
        NOISE_SHAPING_COEFFS[0] * history[0]
            + NOISE_SHAPING_COEFFS[1] * history[1]
            + NOISE_SHAPING_COEFFS[2] * history[2]
    }

    /// Push a new error into the history ring (most recent at index 0).
    #[inline(always)]
    pub(super) fn push_error(history: &mut [f32; 3], error: f32) {
        history[2] = history[1];
        history[1] = history[0];
        history[0] = error;
    }

    /// Generate TPDF dither with one random sample:
    /// TPDF[n] = R[n] - R[n-1], where both uniforms are in [-0.5, 0.5].
    #[inline(always)]
    pub(super) fn next_tpdf(&mut self, ch: usize) -> f32 {
        let r = random_f32(&mut self.rng_state[ch]);
        let tpdf = r - self.prev_random[ch];
        self.prev_random[ch] = r;
        tpdf
    }
}

impl InPlacePlugin for DitherPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Dither", "1.0.0", "Sotf")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        if id == self.param_bit_depth
            && let Some(v) = val.as_int()
        {
            let idx = (v as usize).min(BIT_DEPTHS.len() - 1);
            self.bit_depth_index = idx;
            self.update_scales();
            self.rebuild_cached_parameters();
            return Ok(());
        }

        if id == self.param_noise_shaping
            && let Some(v) = val.as_bool()
        {
            self.noise_shaping_enabled = v;
            self.rebuild_cached_parameters();
            return Ok(());
        }

        if id == self.param_dither_type
            && let Some(v) = val.as_int()
        {
            let idx = (v as usize).min(2);
            self.dither_type_index = idx;
            self.rebuild_cached_parameters();
            return Ok(());
        }

        Err(format!("Invalid or unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bit_depth {
            Some(ParameterValue::Int(self.bit_depth_index as i32))
        } else if id == &self.param_noise_shaping {
            Some(ParameterValue::Bool(self.noise_shaping_enabled))
        } else if id == &self.param_dither_type {
            Some(ParameterValue::Int(self.dither_type_index as i32))
        } else {
            None
        }
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        // Re-allocate state for current channel count (in case it changed)
        if self.error_history.len() != self.channels {
            self.error_history.resize(self.channels, [0.0; 3]);
        }
        if self.rng_state.len() != self.channels {
            self.rng_state = Self::init_rng_states(self.channels);
        }
        if self.prev_random.len() != self.channels {
            self.prev_random = vec![0.0; self.channels];
        }
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        for prev in &mut self.prev_random {
            *prev = 0.0;
        }
        for h in &mut self.error_history {
            *h = [0.0; 3];
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
        let dither_type = self.dither_type_index;
        let noise_shaping = self.noise_shaping_enabled;

        for frame in 0..nf {
            let base = frame * ch;
            for c in 0..ch {
                let idx = base + c;
                let input = buffer[idx];

                // Noise shaping feedback
                let shaped = if noise_shaping {
                    input - Self::noise_shaping_feedback(&self.error_history[c])
                } else {
                    input
                };

                let dithered = match dither_type {
                    // TPDF dither: RPDF[n] - RPDF[n-1] -> one RNG call / sample.
                    0 => shaped + self.next_tpdf(c) * inv_scale,
                    _ => shaped,
                };

                let quantized = match dither_type {
                    // index 0: TPDF
                    // index 1: no dither, rounded quantization ("None (round)")
                    1 => (dithered * scale).round() * inv_scale,
                    // index 2: no dither, truncated quantization ("Truncate")
                    2 => (dithered * scale).trunc() * inv_scale,
                    // fallback for malformed values (e.g. serialized legacy state)
                    _ => (dithered * scale).round() * inv_scale,
                };

                // Compute quantization error and store for noise shaping
                if noise_shaping {
                    // Feedback only the quantization residual of the shaped sample.
                    // Do not include explicit dither in the feedback path, which would
                    // otherwise dominate the shaper at high bit depths.
                    let error = quantized - shaped;
                    Self::push_error(&mut self.error_history[c], error);
                }

                buffer[idx] = quantized;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}
