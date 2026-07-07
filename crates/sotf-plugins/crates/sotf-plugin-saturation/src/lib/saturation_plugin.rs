use super::default::default_drive;
use super::default::default_exciter_freq;
use super::default::default_mix;
use super::default::default_output_gain;
use super::default::default_tone;
use super::misc::DEFAULT_BUF_SIZE;
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
use sotf_host::oversampling::Oversampler;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
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
    pub(super) oversampler: Option<Oversampler>,
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
    pub(super) dry_buf: Vec<f32>,  // Original signal for mix
    pub(super) low_buf: Vec<f32>,  // Low band (pass-through) for exciter mode
    pub(super) high_buf: Vec<f32>, // High band (saturated) for exciter mode

    pub(super) cached_parameters: Vec<Parameter>,
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

            oversampler: None,
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
            low_buf: vec![0.0; buf_size],
            high_buf: vec![0.0; buf_size],

            cached_parameters: Vec::new(),
        };

        p.rebuild_oversampler();
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
        p.rebuild_oversampler();
        p.rebuild_cached_parameters();
        p
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

    pub(super) fn rebuild_oversampler(&mut self) {
        let factor = match self.oversampling_index {
            1 => 2u32,
            2 => 4u32,
            _ => {
                self.oversampler = None;
                return;
            }
        };
        match Oversampler::new(factor, self.channels) {
            Ok(os) => self.oversampler = Some(os),
            Err(_) => self.oversampler = None,
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_string("mode", "Mode", self.mode_string())
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
            .with_description("Harmonic character (tube mode: even/odd balance)")
            .with_group("Saturation")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "exciter_freq",
                "Exciter Freq",
                self.exciter_freq,
                pk(SAT, "exciter_freq").min_f64() as f32,
                pk(SAT, "exciter_freq").max_f64() as f32,
            )
            .with_description("Crossover frequency for exciter mode")
            .with_group("Exciter")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_string("oversampling", "Oversampling", self.oversampling_string())
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
                .with_group("Quality")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("use_adaa", "ADAA", self.use_adaa)
                .with_group("Quality")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    /// Process exciter mode without oversampling: split -> saturate HF -> recombine
    pub(super) fn process_exciter_direct(
        &mut self,
        buffer: &mut [f32],
        num_frames: usize,
        drive: f32,
    ) {
        let nc = self.channels;
        for frame in 0..num_frames {
            for ch in 0..nc {
                let idx = frame * nc + ch;
                let input = buffer[idx];

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
                self.mode = new_mode;
            } else if id == self.param_drive {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "drive").default_f64() as f32);
                if v.is_finite() {
                    self.drive = v.clamp(1.0, 20.0);
                    self.drive_smoother.set_target(self.drive);
                }
            } else if id == self.param_tone {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "tone").default_f64() as f32);
                if v.is_finite() {
                    self.tone = v.clamp(1.0, 3.0);
                }
            } else if id == self.param_exciter_freq {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "exciter_freq").default_f64() as f32);
                if v.is_finite() {
                    self.exciter_freq = v.clamp(500.0, 10000.0);
                    self.rebuild_crossovers();
                }
            } else if id == self.param_oversampling {
                let new_index = if let Some(s) = value.as_string() {
                    match s {
                        "Off" | "off" => 0,
                        "2x" => 1,
                        "4x" => 2,
                        _ => 0,
                    }
                } else if let Some(v) = value.as_float() {
                    (v as usize).min(2)
                } else {
                    0
                };
                if new_index != self.oversampling_index {
                    self.oversampling_index = new_index;
                    self.rebuild_oversampler();
                }
            } else if id == self.param_output_gain {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "output_gain").default_f64() as f32);
                if v.is_finite() {
                    self.output_gain_db = v.clamp(-12.0, 12.0);
                    self.output_smoother.set_target(self.output_gain_db);
                }
            } else if id == self.param_mix {
                let v = value
                    .as_float()
                    .unwrap_or(pk(SAT, "mix").default_f64() as f32);
                if v.is_finite() {
                    self.mix = v.clamp(0.0, 1.0);
                    self.mix_smoother.set_target(self.mix);
                }
            } else if id == self.param_dynamic_amount {
                let v = value.as_float().unwrap_or(0.0);
                if v.is_finite() {
                    self.dynamic_amount = v.clamp(0.0, 1.0);
                }
            } else if id == self.param_dynamic_attack_ms {
                let v = value.as_float().unwrap_or(5.0);
                if v.is_finite() {
                    self.dynamic_attack_ms = v.clamp(0.1, 100.0);
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
                    self.dc_blocker_enabled = enabled;
                }
            } else if id == self.param_use_adaa {
                if let Some(enabled) = parameter_value_as_legacy_bool(&value) {
                    self.use_adaa = enabled;
                }
            } else {
                return Err(format!("Unknown parameter: {id}"));
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Reinit crossovers
        for xo in &mut self.crossovers {
            xo.reinit(self.exciter_freq, sample_rate as f32, 1);
        }

        // Reinit smoothers
        self.drive_smoother.set_time(10.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        self.output_smoother.set_time(10.0, sample_rate);

        // Rebuild oversampler for new sample rate context
        self.rebuild_oversampler();

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

        // Pre-allocate buffers for the largest block sizes seen in offline and
        // stress hosts without growing on the audio callback path.
        let max_expected_frames = sample_rate as usize + 8192;
        let buf_size = DEFAULT_BUF_SIZE.max(max_expected_frames * self.channels);
        if self.dry_buf.len() < buf_size {
            self.dry_buf.resize(buf_size, 0.0);
            self.low_buf.resize(buf_size, 0.0);
            self.high_buf.resize(buf_size, 0.0);
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Reset crossovers
        for xo in &mut self.crossovers {
            xo.reset();
        }

        // Reset oversampler
        if let Some(ref mut os) = self.oversampler {
            os.reset();
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
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;
        let nc = self.channels;
        let total = nf * nc;

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

        // Capture smoother start values before advancing, so we can ramp per-sample.
        // This eliminates zipper noise when drive/mix/output_gain are automated.
        let drive_start = self.drive_smoother.current();
        let drive_end = self.drive_smoother.next_n(nf);
        let drive_step = if nf > 1 {
            (drive_end - drive_start) / nf as f32
        } else {
            0.0
        };

        let mix_start = self.mix_smoother.current();
        let mix_end = self.mix_smoother.next_n(nf);
        let mix_step = if nf > 1 {
            (mix_end - mix_start) / nf as f32
        } else {
            0.0
        };

        let gain_start = self.output_smoother.current();
        let gain_end = self.output_smoother.next_n(nf);
        let gain_step = if nf > 1 {
            (gain_end - gain_start) / nf as f32
        } else {
            0.0
        };

        // Block-constant values used for code paths that cannot do per-sample smoothing
        // (oversampler inner closure captures drive by value).
        let drive_block = drive_end;

        let mode = self.mode;
        let tone = self.tone;
        let dyn_amount = self.dynamic_amount;

        if mode == SaturationMode::Exciter {
            // Exciter mode: split signal, saturate HF only, recombine
            if let Some(ref mut os) = self.oversampler {
                // Strategy: split at 1x rate, oversample+saturate HF band, recombine.

                // Step 1: split at 1x rate
                for frame in 0..nf {
                    for ch in 0..nc {
                        let idx = frame * nc + ch;
                        let input = buffer[idx];
                        let (low, high) = self.crossovers[ch].process(input, 0);
                        self.low_buf[idx] = low;
                        self.high_buf[idx] = high;
                    }
                }

                // Step 2: put HF into buffer, oversample and saturate
                buffer[..total].copy_from_slice(&self.high_buf[..total]);
                // Use block-constant drive for oversampled path (closure capture).
                // The drive ramp is applied in the final per-frame mix loop below.
                let frames_written = os
                    .process(buffer, nf, |planar, os_frames| {
                        for ch_buf in planar.iter_mut().take(nc) {
                            for sample in ch_buf.iter_mut().take(os_frames) {
                                *sample = soft_clip(*sample, drive_block);
                            }
                        }
                    })
                    .unwrap_or(nf);

                // Only recombine for frames the oversampler actually wrote.
                // Frames beyond frames_written are already zero (pre-zeroed by oversampler).
                let valid = frames_written * nc;
                for (out, &low) in buffer[..valid].iter_mut().zip(self.low_buf[..valid].iter()) {
                    *out += low;
                }
                // Remaining frames: pass through the low band only
                for (out, &low) in buffer[valid..total]
                    .iter_mut()
                    .zip(self.low_buf[valid..total].iter())
                {
                    *out = low;
                }
            } else {
                // No oversampling: direct exciter processing with block-constant drive
                self.process_exciter_direct(buffer, nf, drive_block);
            }
        } else if let Some(ref mut os) = self.oversampler {
            // Oversampled processing for non-exciter modes.
            // Use block-constant drive; per-sample ramp is applied in the final loop.
            let frames_written = os
                .process(buffer, nf, |planar, os_frames| {
                    for ch_buf in planar.iter_mut().take(nc) {
                        for sample in ch_buf.iter_mut().take(os_frames) {
                            *sample = saturate(*sample, mode, drive_block, tone);
                        }
                    }
                })
                .unwrap_or(nf);

            // Zero out tail that oversampler did not write (latency fill period)
            let valid = frames_written * nc;
            for s in buffer[valid..total].iter_mut() {
                *s = 0.0;
            }
        } else if self.use_adaa && mode != SaturationMode::Exciter {
            // ADAA processing (anti-aliased, no oversampling).
            // Tube ADAA: adaa_softclip is built for f(x)=x/(1+|x|), i.e. tone=1.
            // When tone != 1.0, the ADAA nonlinearity no longer matches the direct
            // tube() path. Fall back to direct tube() for Tube mode to keep the
            // harmonic character consistent regardless of the ADAA flag.
            // Per-channel state avoids corruption in interleaved processing.
            for frame in 0..nf {
                let frame_drive = drive_start + frame as f32 * drive_step;
                let frame_tanh_drive = frame_drive.tanh();
                for ch in 0..nc {
                    let idx = frame * nc + ch;
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
                let frame_drive = drive_start + frame as f32 * drive_step;
                for ch in 0..nc {
                    let idx = frame * nc + ch;
                    buffer[idx] = saturate(buffer[idx], mode, frame_drive, tone);
                }
            }
        }

        // Dynamic saturation: modulate drive before the nonlinearity by re-applying
        // with an envelope-scaled drive boost. The envelope follows the dry input so
        // that drive tracks input level, adding dynamic harmonic generation rather than
        // post-distortion amplitude pumping.
        // Max dynamic drive is clamped to 20.0 to prevent blow-up on loud passages.
        const MAX_DYNAMIC_DRIVE: f32 = 20.0;
        if dyn_amount > 0.001 {
            for frame in 0..nf {
                let frame_drive = drive_start + frame as f32 * drive_step;
                for ch in 0..nc {
                    let idx = frame * nc + ch;
                    let dry_abs = self.dry_buf[idx].abs();
                    let env = self.envelope_followers[ch].process(dry_abs);
                    // Compute a drive-modulated re-saturation of the dry signal
                    let dynamic_drive =
                        (frame_drive * (1.0 + env * dyn_amount)).min(MAX_DYNAMIC_DRIVE);
                    buffer[idx] = saturate(self.dry_buf[idx], mode, dynamic_drive, tone);
                }
            }
        }

        // DC blocker on the wet signal (removes saturation-induced DC offset).
        if self.dc_blocker_enabled {
            self.dc_blocker.process_block_interleaved(buffer, nc, nf);
        }

        // Apply per-sample output gain ramp and dry/wet mix
        for frame in 0..nf {
            let frame_gain_db = gain_start + frame as f32 * gain_step;
            let frame_output_linear = fast_pow10(frame_gain_db / 20.0);
            let frame_mix = mix_start + frame as f32 * mix_step;
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
}

fn parameter_value_as_legacy_bool(value: &ParameterValue) -> Option<bool> {
    match value {
        ParameterValue::Bool(enabled) => Some(*enabled),
        ParameterValue::Float(v) if v.is_finite() => Some(*v > 0.5),
        ParameterValue::Int(v) => Some(*v != 0),
        _ => None,
    }
}
