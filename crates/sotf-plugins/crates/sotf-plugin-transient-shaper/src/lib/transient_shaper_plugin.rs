use super::consts::EPSILON;
use super::consts::FAST_ATTACK_MS;
use super::consts::FAST_RELEASE_MS;
use super::consts::SLOW_ATTACK_MS;
use super::consts::SLOW_RELEASE_MS;
use super::misc::one_pole;
use super::misc::time_to_coeff;
use super::types::TransientShaperData;
use super::types::TransientShaperPluginParams;
use crate::params::PARAMS as TS;
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterImportance, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

pub struct TransientShaperPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Parameters
    pub(super) attack_amount: f32,  // -1.0 to 1.0 (from -100% to +100%)
    pub(super) sustain_amount: f32, // -1.0 to 1.0
    pub(super) sensitivity_db: f32,
    pub(super) output_gain_db: f32,
    pub(super) mix: f32,

    // Envelope state (per channel)
    pub(super) fast_env: Vec<f32>,
    pub(super) slow_env: Vec<f32>,

    // Coefficients
    pub(super) fast_attack_coeff: f32,
    pub(super) fast_release_coeff: f32,
    pub(super) slow_attack_coeff: f32,
    pub(super) slow_release_coeff: f32,

    // Smoothers
    pub(super) attack_smoother: Smoother,
    pub(super) sustain_smoother: Smoother,
    pub(super) sensitivity_smoother: Smoother,
    pub(super) output_gain_smoother: Smoother,
    pub(super) mix_smoother: Smoother,

    // Monitoring
    pub(super) cache: RealTimeCache<TransientShaperData>,
    pub(super) cache_samples: usize,
    pub(super) monitor_peak_transient: f32,
    pub(super) monitor_peak_sustain: f32,
    pub(super) monitor_extreme_gain: f32,

    pub(super) cached_parameters: Vec<Parameter>,
}

impl TransientShaperPlugin {
    pub fn new(channels: usize) -> Self {
        let sr = 44100;
        let mut p = Self {
            channels,
            sample_rate: sr,
            attack_amount: 0.0,
            sustain_amount: 0.0,
            sensitivity_db: 0.0,
            output_gain_db: 0.0,
            mix: 1.0,
            fast_env: vec![0.0; channels],
            slow_env: vec![0.0; channels],
            fast_attack_coeff: time_to_coeff(FAST_ATTACK_MS, sr),
            fast_release_coeff: time_to_coeff(FAST_RELEASE_MS, sr),
            slow_attack_coeff: time_to_coeff(SLOW_ATTACK_MS, sr),
            slow_release_coeff: time_to_coeff(SLOW_RELEASE_MS, sr),
            attack_smoother: Smoother::new(0.0, 10.0, sr),
            sustain_smoother: Smoother::new(0.0, 10.0, sr),
            sensitivity_smoother: Smoother::new(Self::sensitivity_threshold(0.0), 10.0, sr),
            output_gain_smoother: Smoother::new(Self::db_to_linear(0.0), 10.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            cache: RealTimeCache::new(TransientShaperData::default()),
            cache_samples: 0,
            monitor_peak_transient: 0.0,
            monitor_peak_sustain: 0.0,
            monitor_extreme_gain: 1.0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: TransientShaperPluginParams) -> Self {
        let mut p = Self::new(channels);
        p.attack_amount = (params.attack / 100.0).clamp(-1.0, 1.0);
        p.sustain_amount = (params.sustain / 100.0).clamp(-1.0, 1.0);
        p.sensitivity_db = params.sensitivity_db.clamp(-12.0, 12.0);
        p.output_gain_db = params.output_gain_db.clamp(-12.0, 12.0);
        p.mix = params.mix.clamp(0.0, 1.0);
        p.attack_smoother.reset(p.attack_amount);
        p.sustain_smoother.reset(p.sustain_amount);
        p.sensitivity_smoother
            .reset(Self::sensitivity_threshold(p.sensitivity_db));
        p.output_gain_smoother
            .reset(Self::db_to_linear(p.output_gain_db));
        p.mix_smoother.reset(p.mix);
        p.rebuild_cached_parameters();
        p
    }

    pub fn try_from_params(
        channels: usize,
        params: TransientShaperPluginParams,
    ) -> PluginResult<Self> {
        if channels == 0 {
            return Err("Transient Shaper requires at least one channel".to_string());
        }
        let ranges = [
            ("attack", params.attack, -100.0, 100.0),
            ("sustain", params.sustain, -100.0, 100.0),
            ("sensitivity_db", params.sensitivity_db, -12.0, 12.0),
            ("output_gain_db", params.output_gain_db, -12.0, 12.0),
            ("mix", params.mix, 0.0, 1.0),
        ];
        for (name, value, min, max) in ranges {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "Invalid Transient Shaper {name}: expected finite value in {min}..={max}, got {value}"
                ));
            }
        }
        Ok(Self::from_params(channels, params))
    }

    #[inline]
    pub(super) fn attack_component(fast: f32, slow: f32) -> f32 {
        (fast - slow).max(0.0)
    }

    pub(super) fn update_coefficients(&mut self) {
        self.fast_attack_coeff = time_to_coeff(FAST_ATTACK_MS, self.sample_rate);
        self.fast_release_coeff = time_to_coeff(FAST_RELEASE_MS, self.sample_rate);
        self.slow_attack_coeff = time_to_coeff(SLOW_ATTACK_MS, self.sample_rate);
        self.slow_release_coeff = time_to_coeff(SLOW_RELEASE_MS, self.sample_rate);
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_amount * 100.0,
                pk(TS, "attack").min_f64() as f32,
                pk(TS, "attack").max_f64() as f32,
            )
            .with_description("Transient emphasis (-100% to +100%)")
            .with_group("Shape")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "sustain",
                "Sustain",
                self.sustain_amount * 100.0,
                pk(TS, "sustain").min_f64() as f32,
                pk(TS, "sustain").max_f64() as f32,
            )
            .with_description("Sustain emphasis (-100% to +100%)")
            .with_group("Shape")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "sensitivity",
                "Sensitivity",
                self.sensitivity_db,
                pk(TS, "sensitivity").min_f64() as f32,
                pk(TS, "sensitivity").max_f64() as f32,
            )
            .with_description("Detection sensitivity offset (dB)")
            .with_group("Detection")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "output_gain",
                "Output",
                self.output_gain_db,
                pk(TS, "output_gain").min_f64() as f32,
                pk(TS, "output_gain").max_f64() as f32,
            )
            .with_description("Output gain compensation (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(TS, "mix").min_f64() as f32,
                pk(TS, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = shaped)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
        ];
    }

    fn update_cached_parameter(&mut self, id: &str, value: &ParameterValue) {
        if let Some(parameter) = self
            .cached_parameters
            .iter_mut()
            .find(|parameter| parameter.id.as_str() == id)
        {
            parameter.default_value = value.clone();
        }
    }

    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    #[inline]
    fn sensitivity_threshold(db: f32) -> f32 {
        Self::db_to_linear(db) * 1.0e-3
    }

    #[inline]
    fn protected_peak(peak: f32) -> f32 {
        if peak <= 1.0 {
            peak
        } else {
            let excess = peak - 1.0;
            1.0 + excess / (1.0 + excess)
        }
    }
}

impl ParametricInPlacePlugin for TransientShaperPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("TransientShaper", env!("CARGO_PKG_VERSION"), "SotF")
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
        for param in &self.cached_parameters {
            values.insert(param.id.clone(), param.default_value.clone());
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            match id.as_str() {
                "attack" => {
                    if let Some(v) = value.as_float()
                        && v.is_finite()
                    {
                        self.attack_amount = (v / 100.0).clamp(-1.0, 1.0);
                        self.attack_smoother.set_target(self.attack_amount);
                    }
                }
                "sustain" => {
                    if let Some(v) = value.as_float()
                        && v.is_finite()
                    {
                        self.sustain_amount = (v / 100.0).clamp(-1.0, 1.0);
                        self.sustain_smoother.set_target(self.sustain_amount);
                    }
                }
                "sensitivity" => {
                    if let Some(v) = value.as_float()
                        && v.is_finite()
                    {
                        self.sensitivity_db = v.clamp(-12.0, 12.0);
                        self.sensitivity_smoother
                            .set_target(Self::sensitivity_threshold(self.sensitivity_db));
                    }
                }
                "output_gain" => {
                    if let Some(v) = value.as_float()
                        && v.is_finite()
                    {
                        self.output_gain_db = v.clamp(-12.0, 12.0);
                        self.output_gain_smoother
                            .set_target(Self::db_to_linear(self.output_gain_db));
                    }
                }
                "mix" => {
                    if let Some(v) = value.as_float()
                        && v.is_finite()
                    {
                        self.mix = v.clamp(0.0, 1.0);
                        self.mix_smoother.set_target(self.mix);
                    }
                }
                _ => return Err(format!("Unknown parameter: {}", id)),
            }
            self.update_cached_parameter(id.as_str(), &value);
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Transient Shaper sample rate must be greater than zero".to_string());
        }
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.attack_smoother.set_time(10.0, sample_rate);
        self.sustain_smoother.set_time(10.0, sample_rate);
        self.sensitivity_smoother.set_time(10.0, sample_rate);
        self.output_gain_smoother.set_time(10.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        Ok(())
    }

    fn reset(&mut self) {
        self.fast_env.fill(0.0);
        self.slow_env.fill(0.0);
        // Reset smoothers to their current targets so a transport-loop restart
        // doesn't inherit a mid-ramp state from before the reset.
        self.attack_smoother.reset(self.attack_amount);
        self.sustain_smoother.reset(self.sustain_amount);
        self.sensitivity_smoother
            .reset(Self::sensitivity_threshold(self.sensitivity_db));
        self.output_gain_smoother
            .reset(Self::db_to_linear(self.output_gain_db));
        self.mix_smoother.reset(self.mix);
        self.cache_samples = 0;
        self.monitor_peak_transient = 0.0;
        self.monitor_peak_sustain = 0.0;
        self.monitor_extreme_gain = 1.0;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let ch = self.channels;
        let sample_len = num_frames
            .checked_mul(ch)
            .ok_or_else(|| "Transient Shaper block sample count overflow".to_string())?;
        if buffer.len() != sample_len {
            return Err(format!(
                "Transient Shaper buffer size mismatch: expected {sample_len} samples, got {}",
                buffer.len()
            ));
        }

        // Hoist env slice references to help the compiler promote them into
        // registers and avoid repeated &mut self borrows inside the inner loop.
        let fast_env = &mut self.fast_env;
        let slow_env = &mut self.slow_env;

        for frame in 0..num_frames {
            let attack_amt = self.attack_smoother.advance();
            let sustain_amt = self.sustain_smoother.advance();
            let threshold_lin = self.sensitivity_smoother.advance();
            let output_gain_lin = self.output_gain_smoother.advance();
            let current_mix = self.mix_smoother.advance();

            let mut linked_fast = 0.0_f32;
            let mut linked_slow = 0.0_f32;
            for c in 0..ch {
                let idx = frame * ch + c;
                let abs_input = buffer[idx].abs();

                // Fast envelope (tracks transients)
                fast_env[c] = one_pole(
                    fast_env[c],
                    abs_input,
                    self.fast_attack_coeff,
                    self.fast_release_coeff,
                );

                // Slow envelope (tracks sustain/body)
                slow_env[c] = one_pole(
                    slow_env[c],
                    abs_input,
                    self.slow_attack_coeff,
                    self.slow_release_coeff,
                );
                linked_fast = linked_fast.max(fast_env[c]);
                linked_slow = linked_slow.max(slow_env[c]);
            }

            // One linked detector gain preserves inter-channel ratios and image.
            let transient = Self::attack_component(linked_fast, linked_slow);
            let gain = if linked_slow > threshold_lin {
                let transient_ratio = (transient / linked_slow.max(EPSILON)).clamp(0.0, 1.0);
                let sustain_ratio = (linked_slow / linked_fast.max(EPSILON)).clamp(0.0, 1.0);
                (1.0 + attack_amt * transient_ratio + sustain_amt * sustain_ratio).clamp(0.25, 4.0)
            } else {
                1.0
            };
            let mixed_gain = (1.0 + current_mix * (gain - 1.0)) * output_gain_lin;
            let frame_peak = (0..ch)
                .map(|c| buffer[frame * ch + c].abs() * mixed_gain)
                .fold(0.0_f32, f32::max);
            let safety_gain = if mixed_gain > 1.0 && frame_peak > 1.0 {
                Self::protected_peak(frame_peak) / frame_peak
            } else {
                1.0
            };
            for c in 0..ch {
                buffer[frame * ch + c] *= mixed_gain * safety_gain;
            }

            self.monitor_peak_transient = self.monitor_peak_transient.max(transient);
            self.monitor_peak_sustain = self.monitor_peak_sustain.max(linked_slow);
            if (gain - 1.0).abs() > (self.monitor_extreme_gain - 1.0).abs() {
                self.monitor_extreme_gain = gain;
            }
            self.cache_samples += 1;
            let cache_interval = (self.sample_rate as usize / 30).max(1);
            if self.cache_samples >= cache_interval {
                let peak_transient = self.monitor_peak_transient;
                let peak_sustain = self.monitor_peak_sustain;
                let extreme_gain = self.monitor_extreme_gain;
                self.cache.update(|data| {
                    data.transient_level = peak_transient;
                    data.sustain_level = peak_sustain;
                    data.gain = extreme_gain;
                });
                self.cache_samples -= cache_interval;
                self.monitor_peak_transient = 0.0;
                self.monitor_peak_sustain = 0.0;
                self.monitor_extreme_gain = 1.0;
            }
        }

        // Flush envelope states to prevent CPU denormal penalty during silence.
        for c in 0..ch {
            if fast_env[c].abs() < 1e-30 {
                fast_env[c] = 0.0;
            }
            if slow_env[c].abs() < 1e-30 {
                slow_env[c] = 0.0;
            }
        }

        flush_denormals_inplace(&mut buffer[..sample_len]);
        Ok(num_frames)
    }
}
