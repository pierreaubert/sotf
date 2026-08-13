use super::default::default_drive;
use super::default::default_exciter_freq;
use super::default::default_mix;
use super::default::default_output_gain;
use super::default::default_tone;
use super::misc::DEFAULT_BUF_SIZE;
use super::misc::MAX_BLOCK_FRAMES;
use super::misc::MAX_CHANNELS;
use super::misc::soft_clip;
use super::misc::tape;
use super::misc::tube;
use super::saturation_mode::SaturationMode;
use super::saturation_mode::saturate;
use super::saturation_plugin_params::SaturationPluginParams;
use crate::params::PARAMS as SAT;
use math_audio_dsp::fast_math::fast_pow10;
use sotf_host::adaa::{Adaa1, adaa1_softclip, adaa1_tanh};
use sotf_host::dc_blocker::DcBlocker;
use sotf_host::envelope_follower::EnvelopeFollower;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::param_specs::UpdateMode;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

pub struct SaturationPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Parameters
    pub(super) param_mode: ParameterId,
    pub(super) mode: SaturationMode,
    pub(super) param_drive: ParameterId,
    pub(super) drive: f32,
    pub(super) param_tone: ParameterId,
    pub(super) tone: f32,
    pub(super) param_exciter_freq: ParameterId,
    pub(super) exciter_freq: f32,
    pub(super) param_oversampling: ParameterId,
    pub(super) oversampling_index: usize, // 0=Off, 1=2x, 2=4x
    pub(super) param_output_gain: ParameterId,
    pub(super) output_gain_db: f32,
    pub(super) param_mix: ParameterId,
    pub(super) mix: f32,

    // --- Phase 3A: SOTA parameters ---
    pub(super) param_dynamic_amount: ParameterId,
    pub(super) dynamic_amount: f32,
    pub(super) param_dynamic_attack_ms: ParameterId,
    pub(super) dynamic_attack_ms: f32,
    pub(super) param_dynamic_release_ms: ParameterId,
    pub(super) dynamic_release_ms: f32,
    pub(super) param_dc_blocker: ParameterId,
    pub(super) dc_blocker_enabled: bool,
    pub(super) param_use_adaa: ParameterId,
    pub(super) use_adaa: bool,

    // DSP state
    pub(super) crossovers: Vec<Lr4Crossover<f32>>, // For exciter mode (one per channel)

    // --- Phase 3A: SOTA DSP state ---
    pub(super) dc_blocker: DcBlocker,
    pub(super) adaa_tanh: Vec<Adaa1>, // Per-channel to avoid state corruption in interleaved processing
    pub(super) adaa_softclip: Vec<Adaa1>, // Per-channel
    pub(super) envelope_followers: Vec<EnvelopeFollower>, // Per-channel for dynamic saturation

    // Smoothers
    pub(super) drive_smoother: Smoother,
    pub(super) mix_smoother: Smoother,
    pub(super) output_smoother: Smoother,

    // Pre-allocated buffers
    pub(super) dry_buf: Vec<f32>, // Original signal for mix

    pub(super) cached_parameters: Vec<Parameter>,
    pub(super) initialized: bool,
}

impl SaturationPlugin {
    pub fn new(channels: usize) -> Self {
        let sr = 44100u32;
        let drive = default_drive();
        let mix = default_mix();
        let output_gain = default_output_gain();
        let exciter_freq = default_exciter_freq();
        let os_index = pk(SAT, "oversampling").default_usize();

        let buf_size = DEFAULT_BUF_SIZE.max(4096 * channels.min(MAX_CHANNELS));

        let dynamic_attack = pk(SAT, "dynamic_attack_ms").default_f64() as f32;
        let dynamic_release = pk(SAT, "dynamic_release_ms").default_f64() as f32;

        let mut p = Self {
            channels,
            sample_rate: sr,

            param_mode: ParameterId::from("mode"),
            mode: SaturationMode::from_index(pk(SAT, "mode").default_usize()),
            param_drive: ParameterId::from("drive"),
            drive,
            param_tone: ParameterId::from("tone"),
            tone: default_tone(),
            param_exciter_freq: ParameterId::from("exciter_freq"),
            exciter_freq,
            param_oversampling: ParameterId::from("oversampling"),
            oversampling_index: os_index,
            param_output_gain: ParameterId::from("output_gain"),
            output_gain_db: output_gain,
            param_mix: ParameterId::from("mix"),
            mix,

            // Phase 3A: SOTA parameters
            param_dynamic_amount: ParameterId::from("dynamic_amount"),
            dynamic_amount: pk(SAT, "dynamic_amount").default_f64() as f32,
            param_dynamic_attack_ms: ParameterId::from("dynamic_attack_ms"),
            dynamic_attack_ms: dynamic_attack,
            param_dynamic_release_ms: ParameterId::from("dynamic_release_ms"),
            dynamic_release_ms: dynamic_release,
            param_dc_blocker: ParameterId::from("dc_blocker"),
            dc_blocker_enabled: pk(SAT, "dc_blocker").default_f64() > 0.5,
            param_use_adaa: ParameterId::from("use_adaa"),
            use_adaa: pk(SAT, "use_adaa").default_f64() > 0.5,

            crossovers: (0..channels)
                .map(|_| Lr4Crossover::new(exciter_freq, sr as f32, 1))
                .collect(),

            // Phase 3A: SOTA DSP state
            dc_blocker: DcBlocker::new_default(channels, sr),
            adaa_tanh: (0..channels).map(|_| adaa1_tanh()).collect(),
            adaa_softclip: (0..channels).map(|_| adaa1_softclip()).collect(),
            envelope_followers: (0..channels)
                .map(|_| EnvelopeFollower::new(dynamic_attack, dynamic_release, sr))
                .collect(),

            drive_smoother: Smoother::new(drive, 10.0, sr),
            mix_smoother: Smoother::new(mix, 5.0, sr),
            output_smoother: Smoother::new(output_gain, 10.0, sr),

            dry_buf: vec![0.0; buf_size],

            cached_parameters: Vec::new(),
            initialized: false,
        };

        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: SaturationPluginParams) -> Self {
        let mut p = Self::new(channels);

        // Mode
        p.mode = match params.mode.as_str() {
            "Soft Clip" | "soft_clip" => SaturationMode::SoftClip,
            "Tube" | "tube" => SaturationMode::Tube,
            "Tape" | "tape" => SaturationMode::Tape,
            "Exciter" | "exciter" => SaturationMode::Exciter,
            _ => SaturationMode::SoftClip,
        };

        p.drive = params.drive.clamp(1.0, 20.0);
        p.tone = params.tone.clamp(1.0, 3.0);
        p.exciter_freq = params.exciter_freq.clamp(500.0, 10000.0);

        // Oversampling
        p.oversampling_index = match params.oversampling.as_str() {
            "Off" | "off" | "0" => 0,
            "2x" | "2" => 1,
            "4x" | "4" => 2,
            _ => 1,
        };

        p.output_gain_db = params.output_gain_db.clamp(-12.0, 12.0);
        p.mix = params.mix.clamp(0.0, 1.0);

        // Phase 3A params
        p.dynamic_amount = params.dynamic_amount.clamp(0.0, 1.0);
        p.dynamic_attack_ms = params.dynamic_attack_ms.clamp(0.1, 100.0);
        p.dynamic_release_ms = params.dynamic_release_ms.clamp(1.0, 500.0);
        p.dc_blocker_enabled = params.dc_blocker_enabled;
        p.use_adaa = params.use_adaa;

        // Re-create smoothers at the actual parameter values so they start settled
        let sr = p.sample_rate;
        p.drive_smoother = Smoother::new(p.drive, 10.0, sr);
        p.mix_smoother = Smoother::new(p.mix, 5.0, sr);
        p.output_smoother = Smoother::new(p.output_gain_db, 10.0, sr);

        p.rebuild_crossovers();
        p.rebuild_cached_parameters();
        p
    }

    /// Construct a plugin from external configuration without silently repairing
    /// malformed presets. The legacy `from_params` remains for trusted callers.
    pub fn try_from_params(channels: usize, params: SaturationPluginParams) -> PluginResult<Self> {
        if channels == 0 || channels > MAX_CHANNELS {
            return Err(format!(
                "Saturation channel count must be in 1..={MAX_CHANNELS}, got {channels}"
            ));
        }
        if !matches!(
            params.mode.as_str(),
            "Soft Clip" | "soft_clip" | "Tube" | "tube" | "Tape" | "tape" | "Exciter" | "exciter"
        ) {
            return Err(format!("Unknown saturation mode: {}", params.mode));
        }
        if !matches!(
            params.oversampling.as_str(),
            "Off" | "off" | "0" | "2x" | "2" | "4x" | "4"
        ) {
            return Err(format!(
                "Unknown saturation oversampling factor: {}",
                params.oversampling
            ));
        }
        let ranged = [
            ("drive", params.drive, 1.0, 20.0),
            ("tone", params.tone, 1.0, 3.0),
            ("exciter_freq", params.exciter_freq, 500.0, 10_000.0),
            ("output_gain_db", params.output_gain_db, -12.0, 12.0),
            ("mix", params.mix, 0.0, 1.0),
            ("dynamic_amount", params.dynamic_amount, 0.0, 1.0),
            ("dynamic_attack_ms", params.dynamic_attack_ms, 0.1, 100.0),
            ("dynamic_release_ms", params.dynamic_release_ms, 1.0, 500.0),
        ];
        for (name, value, min, max) in ranged {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "Invalid saturation {name}: expected finite value in {min}..={max}, got {value}"
                ));
            }
        }
        Ok(Self::from_params(channels, params))
    }

    pub(super) fn mode_string(&self) -> String {
        self.mode.name().to_string()
    }

    pub(super) fn oversampling_string(&self) -> String {
        match self.oversampling_index {
            0 => "Off".to_string(),
            1 => "2x".to_string(),
            2 => "4x".to_string(),
            _ => "Off".to_string(),
        }
    }

    pub(super) fn rebuild_crossovers(&mut self) {
        for xo in &mut self.crossovers {
            xo.set_frequency(self.exciter_freq);
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_string("mode", "Mode", self.mode_string())
                .with_update_mode(UpdateMode::Structural)
                .with_description("Saturation algorithm")
                .with_group("Saturation")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "drive",
                "Drive",
                self.drive,
                pk(SAT, "drive").min_f64() as f32,
                pk(SAT, "drive").max_f64() as f32,
            )
            .with_description("Saturation intensity")
            .with_group("Saturation")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "tone",
                "Tone",
                self.tone,
                pk(SAT, "tone").min_f64() as f32,
                pk(SAT, "tone").max_f64() as f32,
            )
            .with_description("Static waveshaper knee/exponent; Tube remains odd-symmetric")
            .with_group("Saturation")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "exciter_freq",
                "Exciter Freq",
                self.exciter_freq,
                pk(SAT, "exciter_freq").min_f64() as f32,
                pk(SAT, "exciter_freq").max_f64() as f32,
            )
            .with_update_mode(UpdateMode::Structural)
            .with_description("Crossover frequency for exciter mode")
            .with_group("Exciter")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_string("oversampling", "Oversampling", self.oversampling_string())
                .with_update_mode(UpdateMode::Structural)
                .with_description("Oversampling factor for alias suppression")
                .with_group("Quality")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "output_gain",
                "Output",
                self.output_gain_db,
                pk(SAT, "output_gain").min_f64() as f32,
                pk(SAT, "output_gain").max_f64() as f32,
            )
            .with_description("Output gain compensation (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(SAT, "mix").min_f64() as f32,
                pk(SAT, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet blend (0 = dry, 1 = processed)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            // Phase 3A: SOTA params
            Parameter::new_float(
                "dynamic_amount",
                "Dynamic",
                self.dynamic_amount,
                pk(SAT, "dynamic_amount").min_f64() as f32,
                pk(SAT, "dynamic_amount").max_f64() as f32,
            )
            .with_group("Dynamic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "dynamic_attack_ms",
                "Dyn Attack",
                self.dynamic_attack_ms,
                pk(SAT, "dynamic_attack_ms").min_f64() as f32,
                pk(SAT, "dynamic_attack_ms").max_f64() as f32,
            )
            .with_group("Dynamic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "dynamic_release_ms",
                "Dyn Release",
                self.dynamic_release_ms,
                pk(SAT, "dynamic_release_ms").min_f64() as f32,
                pk(SAT, "dynamic_release_ms").max_f64() as f32,
            )
            .with_group("Dynamic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("dc_blocker", "DC Block", self.dc_blocker_enabled)
                .with_update_mode(UpdateMode::Structural)
                .with_group("Quality")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("use_adaa", "ADAA", self.use_adaa)
                .with_update_mode(UpdateMode::Structural)
                .with_group("Quality")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    fn update_cached_float(&mut self, id: &str, value: f32) {
        if let Some(parameter) = self
            .cached_parameters
            .iter_mut()
            .find(|parameter| parameter.id.as_str() == id)
        {
            parameter.default_value = ParameterValue::Float(value);
        }
    }

    /// Process exciter mode without oversampling: split -> saturate HF -> recombine
    pub(super) fn process_exciter_direct(&mut self, buffer: &mut [f32], num_frames: usize) {
        let nc = self.channels;
        for frame in 0..num_frames {
            let base_drive = self.drive_smoother.advance();
            for ch in 0..nc {
                let idx = frame * nc + ch;
                let input = buffer[idx];
                let env = self.envelope_followers[ch].process(self.dry_buf[idx].abs());
                let drive = (base_drive * (1.0 + env * self.dynamic_amount)).min(20.0);

                let (low, high) = self.crossovers[ch].process(input, 0);
                let saturated_high = soft_clip(high, drive);
                buffer[idx] = low + saturated_high;
            }
        }
    }

    /// Backward-compatible parameter list accessor.
    pub fn parameters(&self) -> Vec<Parameter> {
        self.parameter_schema()
    }

    /// Backward-compatible single-parameter getter.
    pub fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.current_values().get(id).cloned()
    }

    /// Backward-compatible parameter validation.
    pub fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        if (id == &self.param_dc_blocker || id == &self.param_use_adaa)
            && parameter_value_as_legacy_bool(value).is_some()
        {
            return Ok(());
        }

        if let Some(param) = self.parameters().iter().find(|p| &p.id == id) {
            param.validate(value).map_err(|e| format!("{}: {}", id, e))
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    /// Backward-compatible single-parameter setter.
    pub fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        let mut values = ParameterSet::new();
        values.insert(id, value);
        self.apply_values(values)
    }
}

impl ParametricInPlacePlugin for SaturationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Saturation", "1.0.0", "SotF")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Dynamics
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(
            PluginCostClass::Dynamics,
            None,
            self.latency_samples(),
            false,
        )
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
            self.param_mode.clone(),
            ParameterValue::String(self.mode_string()),
        );
        values.insert(self.param_drive.clone(), ParameterValue::Float(self.drive));
        values.insert(self.param_tone.clone(), ParameterValue::Float(self.tone));
        values.insert(
            self.param_exciter_freq.clone(),
            ParameterValue::Float(self.exciter_freq),
        );
        values.insert(
            self.param_oversampling.clone(),
            ParameterValue::String(self.oversampling_string()),
        );
        values.insert(
            self.param_output_gain.clone(),
            ParameterValue::Float(self.output_gain_db),
        );
        values.insert(self.param_mix.clone(), ParameterValue::Float(self.mix));
        values.insert(
            self.param_dynamic_amount.clone(),
            ParameterValue::Float(self.dynamic_amount),
        );
        values.insert(
            self.param_dynamic_attack_ms.clone(),
            ParameterValue::Float(self.dynamic_attack_ms),
        );
        values.insert(
            self.param_dynamic_release_ms.clone(),
            ParameterValue::Float(self.dynamic_release_ms),
        );
        values.insert(
            self.param_dc_blocker.clone(),
            ParameterValue::Bool(self.dc_blocker_enabled),
        );
        values.insert(
            self.param_use_adaa.clone(),
            ParameterValue::Bool(self.use_adaa),
        );
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        // Validate enum values before mutating any state.  `Parameter` validates
        // the string type but intentionally does not know this plugin's aliases,
        // so silently mapping an unknown value to a different topology here
        // would make bulk updates order-dependent and corrupt preset state.
        for (id, value) in &values {
            let Some(parameter) = self
                .cached_parameters
                .iter()
                .find(|parameter| &parameter.id == id)
            else {
                return Err(format!("Unknown parameter: {id}"));
            };
            let legacy_bool = (id == &self.param_dc_blocker || id == &self.param_use_adaa)
                && parameter_value_as_legacy_bool(value).is_some();
            if !legacy_bool {
                parameter
                    .validate(value)
                    .map_err(|error| format!("{id}: {error}"))?;
            }
            if id == &self.param_mode {
                let Some(mode) = value.as_string() else {
                    return Err("mode must be a string enum".to_string());
                };
                if !matches!(
                    mode,
                    "Soft Clip"
                        | "soft_clip"
                        | "Tube"
                        | "tube"
                        | "Tape"
                        | "tape"
                        | "Exciter"
                        | "exciter"
                ) {
                    return Err(format!("Unknown saturation mode: {mode}"));
                }
            } else if id == &self.param_oversampling {
                let Some(oversampling) = value.as_string() else {
                    return Err("oversampling must be a string enum".to_string());
                };
                if !matches!(oversampling, "Off" | "off" | "0" | "2x" | "2" | "4x" | "4") {
                    return Err(format!(
                        "Unknown saturation oversampling factor: {oversampling}"
                    ));
                }
            }
        }

        if self.initialized {
            for (id, value) in &values {
                let changed = if id == &self.param_mode {
                    value.as_string().is_some_and(|value| {
                        let mode = match value {
                            "Soft Clip" | "soft_clip" => SaturationMode::SoftClip,
                            "Tube" | "tube" => SaturationMode::Tube,
                            "Tape" | "tape" => SaturationMode::Tape,
                            "Exciter" | "exciter" => SaturationMode::Exciter,
                            _ => self.mode,
                        };
                        mode != self.mode
                    })
                } else if id == &self.param_oversampling {
                    value.as_string().is_some_and(|value| {
                        let index = match value {
                            "Off" | "off" | "0" => 0,
                            "2x" | "2" => 1,
                            "4x" | "4" => 2,
                            _ => self.oversampling_index,
                        };
                        index != self.oversampling_index
                    })
                } else if id == &self.param_exciter_freq {
                    value
                        .as_float()
                        .is_some_and(|value| value != self.exciter_freq)
                } else if id == &self.param_use_adaa {
                    parameter_value_as_legacy_bool(value)
                        .is_some_and(|value| value != self.use_adaa)
                } else if id == &self.param_dc_blocker {
                    parameter_value_as_legacy_bool(value)
                        .is_some_and(|value| value != self.dc_blocker_enabled)
                } else {
                    false
                };
                if changed {
                    return Err(format!(
                        "{id} is structural; recreate Saturation so the host can rebuild topology and latency"
                    ));
                }
            }
        }

        let mut structural_changed = false;
        for (id, value) in values {
            if id == self.param_mode {
                let new_mode = if let Some(s) = value.as_string() {
                    match s {
                        "Soft Clip" | "soft_clip" => SaturationMode::SoftClip,
                        "Tube" | "tube" => SaturationMode::Tube,
                        "Tape" | "tape" => SaturationMode::Tape,
                        "Exciter" | "exciter" => SaturationMode::Exciter,
                        _ => SaturationMode::SoftClip,
                    }
                } else if let Some(v) = value.as_float() {
                    SaturationMode::from_index(v as usize)
                } else {
                    SaturationMode::SoftClip
                };
                structural_changed |= new_mode != self.mode;
                self.mode = new_mode;
            } else if id == self.param_drive {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "drive").default_f64() as f32);
                if v.is_finite() {
                    self.drive = v.clamp(1.0, 20.0);
                    self.drive_smoother.set_target(self.drive);
                    self.update_cached_float("drive", self.drive);
                }
            } else if id == self.param_tone {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "tone").default_f64() as f32);
                if v.is_finite() {
                    self.tone = v.clamp(1.0, 3.0);
                    self.update_cached_float("tone", self.tone);
                }
            } else if id == self.param_exciter_freq {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "exciter_freq").default_f64() as f32);
                if v.is_finite() {
                    structural_changed |= v != self.exciter_freq;
                    self.exciter_freq = v.clamp(500.0, 10000.0);
                    self.rebuild_crossovers();
                }
            } else if id == self.param_oversampling {
                let new_index = if let Some(s) = value.as_string() {
                    match s {
                        "Off" | "off" | "0" => 0,
                        "2x" | "2" => 1,
                        "4x" | "4" => 2,
                        _ => 0,
                    }
                } else if let Some(v) = value.as_float() {
                    (v as usize).min(2)
                } else {
                    0
                };
                if new_index != self.oversampling_index {
                    structural_changed = true;
                    self.oversampling_index = new_index;
                }
            } else if id == self.param_output_gain {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "output_gain").default_f64() as f32);
                if v.is_finite() {
                    self.output_gain_db = v.clamp(-12.0, 12.0);
                    self.output_smoother.set_target(self.output_gain_db);
                    self.update_cached_float("output_gain", self.output_gain_db);
                }
            } else if id == self.param_mix {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "mix").default_f64() as f32);
                if v.is_finite() {
                    self.mix = v.clamp(0.0, 1.0);
                    self.mix_smoother.set_target(self.mix);
                    self.update_cached_float("mix", self.mix);
                }
            } else if id == self.param_dynamic_amount {
                let v = value.as_float().unwrap_or(0.0);
                if v.is_finite() {
                    self.dynamic_amount = v.clamp(0.0, 1.0);
                    self.update_cached_float("dynamic_amount", self.dynamic_amount);
                }
            } else if id == self.param_dynamic_attack_ms {
                let v = value.as_float().unwrap_or(5.0);
                if v.is_finite() {
                    self.dynamic_attack_ms = v.clamp(0.1, 100.0);
                    self.update_cached_float("dynamic_attack_ms", self.dynamic_attack_ms);
                    for ef in &mut self.envelope_followers {
                        ef.set_times(
                            self.dynamic_attack_ms,
                            self.dynamic_release_ms,
                            self.sample_rate,
                        );
                    }
                }
            } else if id == self.param_dynamic_release_ms {
                let v = value.as_float().unwrap_or(50.0);
                if v.is_finite() {
                    self.dynamic_release_ms = v.clamp(1.0, 500.0);
                    self.update_cached_float("dynamic_release_ms", self.dynamic_release_ms);
                    for ef in &mut self.envelope_followers {
                        ef.set_times(
                            self.dynamic_attack_ms,
                            self.dynamic_release_ms,
                            self.sample_rate,
                        );
                    }
                }
            } else if id == self.param_dc_blocker {
                if let Some(enabled) = parameter_value_as_legacy_bool(&value) {
                    structural_changed |= enabled != self.dc_blocker_enabled;
                    self.dc_blocker_enabled = enabled;
                }
            } else if id == self.param_use_adaa {
                if let Some(enabled) = parameter_value_as_legacy_bool(&value) {
                    structural_changed |= enabled != self.use_adaa;
                    self.use_adaa = enabled;
                }
            } else {
                return Err(format!("Unknown parameter: {id}"));
            }
        }
        if structural_changed {
            self.rebuild_cached_parameters();
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Saturation sample rate must be greater than zero".to_string());
        }
        self.sample_rate = sample_rate;

        let maximum_exciter = sample_rate as f32 * 0.475;
        if self.exciter_freq > maximum_exciter {
            return Err(format!(
                "Saturation exciter frequency {} Hz must not exceed {maximum_exciter} Hz at {sample_rate} Hz",
                self.exciter_freq
            ));
        }

        // Reinit crossovers
        for xo in &mut self.crossovers {
            xo.reinit(self.exciter_freq, sample_rate as f32, 1);
        }

        // Reinit smoothers
        self.drive_smoother.set_time(10.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        self.output_smoother.set_time(10.0, sample_rate);

        // Reinit SOTA DSP components
        self.dc_blocker.set_sample_rate(sample_rate, 5.0);
        self.dc_blocker.set_channels(self.channels);
        self.adaa_tanh = (0..self.channels).map(|_| adaa1_tanh()).collect();
        self.adaa_softclip = (0..self.channels).map(|_| adaa1_softclip()).collect();
        self.envelope_followers = (0..self.channels)
            .map(|_| {
                EnvelopeFollower::new(self.dynamic_attack_ms, self.dynamic_release_ms, sample_rate)
            })
            .collect();

        // Scratch follows one explicit maximum-block contract rather than sample
        // rate (which is unrelated to callback size and caused tens of MiB waste).
        let buf_size = DEFAULT_BUF_SIZE.max(MAX_BLOCK_FRAMES.saturating_mul(self.channels));
        if self.dry_buf.len() < buf_size {
            self.dry_buf.resize(buf_size, 0.0);
        }

        self.initialized = true;

        Ok(())
    }

    fn reset(&mut self) {
        // Reset crossovers
        for xo in &mut self.crossovers {
            xo.reset();
        }

        // Reset SOTA DSP components
        self.dc_blocker.reset();
        for a in &mut self.adaa_tanh {
            a.reset();
        }
        for a in &mut self.adaa_softclip {
            a.reset();
        }
        for ef in &mut self.envelope_followers {
            ef.reset();
        }
        self.drive_smoother.reset(self.drive);
        self.mix_smoother.reset(self.mix);
        self.output_smoother.reset(self.output_gain_db);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let nc = self.channels;
        if !self.initialized {
            return Err("Saturation must be initialized before processing".to_string());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Saturation context rate {} Hz differs from initialized rate {} Hz",
                context.sample_rate, self.sample_rate
            ));
        }
        let total = nf
            .checked_mul(nc)
            .ok_or_else(|| "Saturation frame/sample count overflow".to_string())?;

        if buffer.len() < total {
            return Err(format!(
                "process_in_place: buffer too short ({} < {})",
                buffer.len(),
                total
            ));
        }

        if self.dry_buf.len() < total {
            return Err(format!(
                "process_in_place: block requires {total} samples, exceeds preallocated scratch capacity {}",
                self.dry_buf.len()
            ));
        }

        // Save dry signal for mix
        self.dry_buf[..total].copy_from_slice(&buffer[..total]);

        let mode = self.mode;
        let tone = self.tone;
        if mode == SaturationMode::Exciter {
            // Host-owned oversampling means this exact per-frame envelope runs
            // at the processing rate supplied by the wrapper and dry/wet remains
            // in the same time domain.
            self.process_exciter_direct(buffer, nf);
        } else if self.use_adaa && mode != SaturationMode::Exciter {
            // ADAA processing (anti-aliased, no oversampling).
            // Tube ADAA: adaa_softclip is built for f(x)=x/(1+|x|), i.e. tone=1.
            // When tone != 1.0, the ADAA nonlinearity no longer matches the direct
            // tube() path. Fall back to direct tube() for Tube mode to keep the
            // harmonic character consistent regardless of the ADAA flag.
            // Per-channel state avoids corruption in interleaved processing.
            for frame in 0..nf {
                let base_drive = self.drive_smoother.advance();
                for ch in 0..nc {
                    let idx = frame * nc + ch;
                    let env = self.envelope_followers[ch].process(self.dry_buf[idx].abs());
                    let frame_drive = (base_drive * (1.0 + env * self.dynamic_amount)).min(20.0);
                    let frame_tanh_drive = frame_drive.tanh();
                    match mode {
                        SaturationMode::SoftClip => {
                            let driven = buffer[idx] * frame_drive;
                            let adaa_out = self.adaa_tanh[ch].process(driven);
                            buffer[idx] = if frame_tanh_drive < 1e-6 {
                                buffer[idx]
                            } else {
                                adaa_out / frame_tanh_drive
                            };
                        }
                        SaturationMode::Tube => {
                            // Use direct tube() so tone is always respected.
                            buffer[idx] = tube(buffer[idx], frame_drive, tone);
                        }
                        SaturationMode::Tape => {
                            buffer[idx] = tape(buffer[idx], frame_drive);
                        }
                        SaturationMode::Exciter => {} // handled above
                    }
                }
            }
        } else {
            // Direct processing (no oversampling, no ADAA) with per-sample drive ramp
            for frame in 0..nf {
                let base_drive = self.drive_smoother.advance();
                for ch in 0..nc {
                    let idx = frame * nc + ch;
                    let env = self.envelope_followers[ch].process(self.dry_buf[idx].abs());
                    let frame_drive = (base_drive * (1.0 + env * self.dynamic_amount)).min(20.0);
                    buffer[idx] = saturate(buffer[idx], mode, frame_drive, tone);
                }
            }
        }

        // DC blocker on the wet signal (removes saturation-induced DC offset).
        if self.dc_blocker_enabled {
            self.dc_blocker.process_block_interleaved(buffer, nc, nf);
        }

        // Apply per-sample output gain ramp and dry/wet mix
        for frame in 0..nf {
            let frame_gain_db = self.output_smoother.advance();
            let frame_output_linear = fast_pow10(frame_gain_db / 20.0);
            let frame_mix = self.mix_smoother.advance();
            for ch in 0..nc {
                let idx = frame * nc + ch;
                let dry = self.dry_buf[idx];
                let wet = buffer[idx] * frame_output_linear;
                buffer[idx] = dry * (1.0 - frame_mix) + wet * frame_mix;
            }
        }

        // Flush denormals only on the samples we actually processed
        flush_denormals_inplace(&mut buffer[..total]);
        Ok(nf)
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        match self.oversampling_index {
            1 => Some(2),
            2 => Some(4),
            _ => None,
        }
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

fn parameter_value_as_legacy_bool(value: &ParameterValue) -> Option<bool> {
    match value {
        ParameterValue::Bool(enabled) => Some(*enabled),
        ParameterValue::Float(v) if v.is_finite() => Some(*v > 0.5),
        ParameterValue::Int(v) => Some(*v != 0),
        _ => None,
    }
}
