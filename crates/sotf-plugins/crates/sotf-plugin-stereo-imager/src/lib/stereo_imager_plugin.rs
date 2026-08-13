use super::misc::SMOOTHING_MS;
use super::stereo_imager_plugin_params::StereoImagerPluginParams;
use crate::params::PARAMS as SI;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::Parameter;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

const CROSSOVER_CONTROL_INTERVAL: usize = 16;

pub struct StereoImagerPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Parameters
    pub(super) width: f32,
    pub(super) low_mid_freq: f32,
    pub(super) mid_high_freq: f32,
    pub(super) low_width: f32,
    pub(super) mid_width: f32,
    pub(super) high_width: f32,
    pub(super) mono_bass: bool,
    pub(super) mix: f32,

    // Crossovers: each crossover handles 2 channels (one for mid signal, one for side signal)
    pub(super) crossover_low: Lr4Crossover<f32>,
    pub(super) crossover_high: Lr4Crossover<f32>,

    // Smoothers for click-free parameter changes
    pub(super) width_smoother: Smoother,
    pub(super) low_mid_freq_smoother: Smoother,
    pub(super) mid_high_freq_smoother: Smoother,
    pub(super) low_width_smoother: Smoother,
    pub(super) mid_width_smoother: Smoother,
    pub(super) high_width_smoother: Smoother,
    pub(super) mono_bass_smoother: Smoother,
    pub(super) mix_smoother: Smoother,

    pub(super) crossover_control_phase: usize,
    #[cfg(test)]
    pub(super) crossover_update_count: usize,

    pub(super) cached_parameters: Vec<Parameter>,
}

impl StereoImagerPlugin {
    pub fn try_new(channels: usize, params: StereoImagerPluginParams) -> PluginResult<Self> {
        if channels != 2 {
            return Err(format!(
                "Stereo Imager requires exactly 2 channels, got {channels}"
            ));
        }
        let values = [
            ("width", params.width, 0.0, 2.0),
            ("low_mid_freq", params.low_mid_freq, 20.0, 1000.0),
            ("mid_high_freq", params.mid_high_freq, 1000.0, 10000.0),
            ("low_width", params.low_width, 0.0, 2.0),
            ("mid_width", params.mid_width, 0.0, 2.0),
            ("high_width", params.high_width, 0.0, 2.0),
            ("mix", params.mix, 0.0, 1.0),
        ];
        for (name, value, min, max) in values {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "{name} must be finite and in [{min}, {max}], got {value}"
                ));
            }
        }
        if params.low_mid_freq >= params.mid_high_freq {
            return Err("low_mid_freq must be lower than mid_high_freq".into());
        }
        Ok(Self::new_validated(channels, params))
    }

    pub fn new(channels: usize, params: StereoImagerPluginParams) -> Self {
        Self::try_new(channels, params).expect("invalid Stereo Imager parameters")
    }

    fn new_validated(channels: usize, params: StereoImagerPluginParams) -> Self {
        let sr = 48000;
        let mut plugin = Self {
            channels,
            sample_rate: sr,

            width: params.width,
            low_mid_freq: params.low_mid_freq,
            mid_high_freq: params.mid_high_freq,
            low_width: params.low_width,
            mid_width: params.mid_width,
            high_width: params.high_width,
            mono_bass: params.mono_bass,
            mix: params.mix,

            // 2 channels: channel 0 = mid, channel 1 = side
            crossover_low: Lr4Crossover::new(params.low_mid_freq, sr as f32, 2),
            crossover_high: Lr4Crossover::new(params.mid_high_freq, sr as f32, 2),

            width_smoother: Smoother::new(params.width, SMOOTHING_MS, sr),
            low_mid_freq_smoother: Smoother::new(params.low_mid_freq, SMOOTHING_MS, sr),
            mid_high_freq_smoother: Smoother::new(params.mid_high_freq, SMOOTHING_MS, sr),
            low_width_smoother: Smoother::new(params.low_width, SMOOTHING_MS, sr),
            mid_width_smoother: Smoother::new(params.mid_width, SMOOTHING_MS, sr),
            high_width_smoother: Smoother::new(params.high_width, SMOOTHING_MS, sr),
            mono_bass_smoother: Smoother::new(
                if params.mono_bass { 0.0 } else { 1.0 },
                SMOOTHING_MS,
                sr,
            ),
            mix_smoother: Smoother::new(params.mix, SMOOTHING_MS, sr),

            crossover_control_phase: 0,
            #[cfg(test)]
            crossover_update_count: 0,

            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    pub fn from_params(channels: usize, params: StereoImagerPluginParams) -> Self {
        Self::new(channels, params)
    }

    pub fn try_from_params(
        channels: usize,
        params: StereoImagerPluginParams,
    ) -> PluginResult<Self> {
        Self::try_new(channels, params)
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "width",
                "Width",
                self.width,
                pk(SI, "width").min_f64() as f32,
                pk(SI, "width").max_f64() as f32,
            ),
            Parameter::new_float(
                "low_mid_freq",
                "Low-Mid Freq",
                self.low_mid_freq,
                pk(SI, "low_mid_freq").min_f64() as f32,
                pk(SI, "low_mid_freq").max_f64() as f32,
            ),
            Parameter::new_float(
                "mid_high_freq",
                "Mid-High Freq",
                self.mid_high_freq,
                pk(SI, "mid_high_freq").min_f64() as f32,
                pk(SI, "mid_high_freq").max_f64() as f32,
            ),
            Parameter::new_float(
                "low_width",
                "Low Width",
                self.low_width,
                pk(SI, "low_width").min_f64() as f32,
                pk(SI, "low_width").max_f64() as f32,
            ),
            Parameter::new_float(
                "mid_width",
                "Mid Width",
                self.mid_width,
                pk(SI, "mid_width").min_f64() as f32,
                pk(SI, "mid_width").max_f64() as f32,
            ),
            Parameter::new_float(
                "high_width",
                "High Width",
                self.high_width,
                pk(SI, "high_width").min_f64() as f32,
                pk(SI, "high_width").max_f64() as f32,
            ),
            Parameter::new_bool("mono_bass", "Mono Bass", self.mono_bass),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(SI, "mix").min_f64() as f32,
                pk(SI, "mix").max_f64() as f32,
            ),
        ];
    }
}

impl ParametricInPlacePlugin for StereoImagerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("StereoImager", env!("CARGO_PKG_VERSION"), "SotF")
            .with_description("Multi-band M/S stereo width control")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Iir
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::linear_transform(PluginCostClass::Iir, None, 0, true, true, false)
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        for param in &self.cached_parameters {
            values.insert(param.id.clone(), param.default_value.clone());
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            match id.as_str() {
                "width" => {
                    if let Some(v) = value.as_float() {
                        self.width = v;
                        self.width_smoother.set_target(v);
                    }
                }
                "low_mid_freq" => {
                    if let Some(v) = value.as_float() {
                        if v >= self.mid_high_freq {
                            return Err("low_mid_freq must be lower than mid_high_freq".into());
                        }
                        self.low_mid_freq = v;
                        self.low_mid_freq_smoother.set_target(v);
                    }
                }
                "mid_high_freq" => {
                    if let Some(v) = value.as_float() {
                        if v <= self.low_mid_freq {
                            return Err("mid_high_freq must be higher than low_mid_freq".into());
                        }
                        self.mid_high_freq = v;
                        self.mid_high_freq_smoother.set_target(v);
                    }
                }
                "low_width" => {
                    if let Some(v) = value.as_float() {
                        self.low_width = v;
                        self.low_width_smoother.set_target(v);
                    }
                }
                "mid_width" => {
                    if let Some(v) = value.as_float() {
                        self.mid_width = v;
                        self.mid_width_smoother.set_target(v);
                    }
                }
                "high_width" => {
                    if let Some(v) = value.as_float() {
                        self.high_width = v;
                        self.high_width_smoother.set_target(v);
                    }
                }
                "mono_bass" => {
                    if let Some(v) = value.as_bool() {
                        self.mono_bass = v;
                        self.mono_bass_smoother
                            .set_target(if v { 0.0 } else { 1.0 });
                    }
                }
                "mix" => {
                    if let Some(v) = value.as_float() {
                        self.mix = v;
                        self.mix_smoother.set_target(v);
                    }
                }
                _ => return Err(format!("Unknown parameter: {}", id)),
            }
            // Keep the cached parameter list in sync with the live state.
            if let Some(p) = self.cached_parameters.iter_mut().find(|p| p.id == id) {
                p.default_value = value;
            }
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("sample rate must be greater than zero".into());
        }
        if self.mid_high_freq >= sample_rate as f32 * 0.5 {
            return Err(format!(
                "mid_high_freq {} must be below Nyquist {}",
                self.mid_high_freq,
                sample_rate as f32 * 0.5
            ));
        }
        self.sample_rate = sample_rate;

        // Reinitialize crossovers at the correct sample rate
        self.crossover_low
            .reinit(self.low_mid_freq, sample_rate as f32, 2);
        self.crossover_high
            .reinit(self.mid_high_freq, sample_rate as f32, 2);

        // Reset smoothers at the new sample rate
        self.width_smoother = Smoother::new(self.width, SMOOTHING_MS, sample_rate);
        self.low_mid_freq_smoother = Smoother::new(self.low_mid_freq, SMOOTHING_MS, sample_rate);
        self.mid_high_freq_smoother = Smoother::new(self.mid_high_freq, SMOOTHING_MS, sample_rate);
        self.low_width_smoother = Smoother::new(self.low_width, SMOOTHING_MS, sample_rate);
        self.mid_width_smoother = Smoother::new(self.mid_width, SMOOTHING_MS, sample_rate);
        self.high_width_smoother = Smoother::new(self.high_width, SMOOTHING_MS, sample_rate);
        self.mono_bass_smoother = Smoother::new(
            if self.mono_bass { 0.0 } else { 1.0 },
            SMOOTHING_MS,
            sample_rate,
        );
        self.mix_smoother = Smoother::new(self.mix, SMOOTHING_MS, sample_rate);
        self.crossover_control_phase = 0;
        #[cfg(test)]
        {
            self.crossover_update_count = 0;
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.crossover_low.reset();
        self.crossover_high.reset();
        // Snap all smoothers to their current target values so a reset
        // during a parameter transition does not resume the ramp.
        self.width_smoother.reset(self.width);
        self.low_mid_freq_smoother.reset(self.low_mid_freq);
        self.mid_high_freq_smoother.reset(self.mid_high_freq);
        self.low_width_smoother.reset(self.low_width);
        self.mid_width_smoother.reset(self.mid_width);
        self.high_width_smoother.reset(self.high_width);
        self.mono_bass_smoother
            .reset(if self.mono_bass { 0.0 } else { 1.0 });
        self.mix_smoother.reset(self.mix);
        self.crossover_control_phase = 0;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();

        let nf = context.num_frames;
        let required = nf
            .checked_mul(2)
            .ok_or_else(|| "Stereo Imager frame count overflow".to_string())?;
        if buffer.len() != required {
            return Err(format!(
                "Stereo Imager buffer size mismatch: expected {required}, got {}",
                buffer.len()
            ));
        }
        let unity = |smoother: &Smoother| {
            (smoother.target() - 1.0).abs() <= f32::EPSILON
                && (smoother.current() - 1.0).abs() <= f32::EPSILON
        };
        if self.mono_bass_smoother.target() >= 1.0 - f32::EPSILON
            && self.mono_bass_smoother.current() >= 1.0 - f32::EPSILON
            && unity(&self.width_smoother)
            && unity(&self.low_width_smoother)
            && unity(&self.mid_width_smoother)
            && unity(&self.high_width_smoother)
            && (self.low_mid_freq_smoother.target() - self.low_mid_freq_smoother.current()).abs()
                <= f32::EPSILON
            && (self.mid_high_freq_smoother.target() - self.mid_high_freq_smoother.current()).abs()
                <= f32::EPSILON
        {
            return Ok(nf);
        }

        // Fully dry path: no need to allocate/copy dry buffers or run any DSP.
        let mix_is_zero = self.mix_smoother.target() <= f32::EPSILON
            && self.mix_smoother.current() <= f32::EPSILON;
        if mix_is_zero {
            return Ok(nf);
        }

        // The current input sample remains in local registers until the wet sample is
        // produced, so intermediate mix values need no callback-sized scratch buffer.
        let need_dry = !(self.mix_smoother.target() >= 1.0 - f32::EPSILON
            && self.mix_smoother.current() >= 1.0 - f32::EPSILON);

        for frame in 0..nf {
            let idx = frame * 2;
            let l = buffer[idx];
            let r = buffer[idx + 1];

            // M/S encode
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5;

            // Advance targets at audio rate, but redesign biquad coefficients at a
            // bounded control rate. This retains time-based smoothing without doing
            // trigonometric filter design for every sample.
            let low_mid_freq = self.low_mid_freq_smoother.advance();
            let mid_high_freq = self.mid_high_freq_smoother.advance();
            if self.crossover_control_phase == 0 {
                if (self.crossover_low.frequency() - low_mid_freq).abs() > f32::EPSILON {
                    self.crossover_low.set_frequency(low_mid_freq);
                    #[cfg(test)]
                    {
                        self.crossover_update_count += 1;
                    }
                }
                if (self.crossover_high.frequency() - mid_high_freq).abs() > f32::EPSILON {
                    self.crossover_high.set_frequency(mid_high_freq);
                    #[cfg(test)]
                    {
                        self.crossover_update_count += 1;
                    }
                }
            }
            self.crossover_control_phase =
                (self.crossover_control_phase + 1) % CROSSOVER_CONTROL_INTERVAL;

            // Split mid and side into bands via cascaded crossovers.
            // crossover_low: channel 0 = mid signal, channel 1 = side signal
            let (side_low, side_rest) = self.crossover_low.process(side, 1);
            // crossover_high: channel 0 = mid rest, channel 1 = side rest
            let (side_mid, side_high) = self.crossover_high.process(side_rest, 1);

            // Advance smoothers (per-sample)
            let gw = self.width_smoother.advance();
            let lw = self.low_width_smoother.advance();
            let mw = self.mid_width_smoother.advance();
            let hw = self.high_width_smoother.advance();
            let low_side_enable = self.mono_bass_smoother.advance();

            // Apply per-band width scaling to side signal.
            // Apply only the width correction to the untouched M/S reference.
            // At neutral widths every correction is exactly zero, avoiding the
            // phase rotation and dry/wet comb filtering of crossover reconstruction.
            let total_mid = mid;
            let total_side = side * gw
                + side_low * gw * (lw * low_side_enable - 1.0)
                + side_mid * gw * (mw - 1.0)
                + side_high * gw * (hw - 1.0);

            // M/S decode
            let wet_l = total_mid + total_side;
            let wet_r = total_mid - total_side;

            // Dry/wet mix
            let m = self.mix_smoother.advance();
            if need_dry {
                buffer[idx] = l + (wet_l - l) * m;
                buffer[idx + 1] = r + (wet_r - r) * m;
            } else {
                buffer[idx] = wet_l;
                buffer[idx + 1] = wet_r;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}
