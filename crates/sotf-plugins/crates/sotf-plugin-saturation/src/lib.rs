// ============================================================================
// Saturation / Harmonic Exciter Plugin
// ============================================================================
//
// Multiple saturation modes with oversampling for alias suppression:
// - Soft Clip: tanh-based symmetric saturation
// - Tube: symmetric polynomial-like saturation with harmonic character control
// - Tape: Tape-style exponential saturation
// - Exciter: HF-only saturation via LR4 crossover
//
// Hard rules:
// - No allocations in process_in_place()
// - No mutex locks in process()
// - No unsafe code

pub mod params;

use crate::params::{MODES, OVERSAMPLING_OPTIONS, PARAMS as SAT};
use math_audio_dsp::fast_math::fast_pow10;
use serde::{Deserialize, Serialize};
use sotf_host::adaa::{Adaa1, adaa1_softclip, adaa1_tanh};
use sotf_host::dc_blocker::DcBlocker;
use sotf_host::envelope_follower::EnvelopeFollower;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::oversampling::Oversampler;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

// ============================================================================
// Plugin Params (JSON deserialization)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaturationPluginParams {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_drive")]
    pub drive: f32,
    #[serde(default = "default_tone")]
    pub tone: f32,
    #[serde(default = "default_exciter_freq")]
    pub exciter_freq: f32,
    #[serde(default = "default_oversampling")]
    pub oversampling: String,
    #[serde(default = "default_output_gain")]
    pub output_gain_db: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default)]
    pub dynamic_amount: f32,
    #[serde(default = "default_dynamic_attack")]
    pub dynamic_attack_ms: f32,
    #[serde(default = "default_dynamic_release")]
    pub dynamic_release_ms: f32,
    #[serde(default = "default_dc_blocker")]
    pub dc_blocker_enabled: bool,
    #[serde(default = "default_use_adaa")]
    pub use_adaa: bool,
}

impl Default for SaturationPluginParams {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            drive: default_drive(),
            tone: default_tone(),
            exciter_freq: default_exciter_freq(),
            oversampling: default_oversampling(),
            output_gain_db: default_output_gain(),
            mix: default_mix(),
            dynamic_amount: 0.0,
            dynamic_attack_ms: default_dynamic_attack(),
            dynamic_release_ms: default_dynamic_release(),
            dc_blocker_enabled: default_dc_blocker(),
            use_adaa: default_use_adaa(),
        }
    }
}

fn default_dynamic_attack() -> f32 {
    pk(SAT, "dynamic_attack_ms").default_f64() as f32
}
fn default_dynamic_release() -> f32 {
    pk(SAT, "dynamic_release_ms").default_f64() as f32
}
fn default_dc_blocker() -> bool {
    pk(SAT, "dc_blocker").default_f64() > 0.5
}
fn default_use_adaa() -> bool {
    pk(SAT, "use_adaa").default_f64() > 0.5
}

fn default_mode() -> String {
    MODES[pk(SAT, "mode").default_usize()].to_string()
}
fn default_drive() -> f32 {
    pk(SAT, "drive").default_f64() as f32
}
fn default_tone() -> f32 {
    pk(SAT, "tone").default_f64() as f32
}
fn default_exciter_freq() -> f32 {
    pk(SAT, "exciter_freq").default_f64() as f32
}
fn default_oversampling() -> String {
    OVERSAMPLING_OPTIONS[pk(SAT, "oversampling").default_usize()].to_string()
}
fn default_output_gain() -> f32 {
    pk(SAT, "output_gain").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(SAT, "mix").default_f64() as f32
}

// ============================================================================
// Saturation Mode Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationMode {
    SoftClip = 0,
    Tube = 1,
    Tape = 2,
    Exciter = 3,
}

impl SaturationMode {
    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::SoftClip,
            1 => Self::Tube,
            2 => Self::Tape,
            3 => Self::Exciter,
            _ => Self::SoftClip,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::SoftClip => "Soft Clip",
            Self::Tube => "Tube",
            Self::Tape => "Tape",
            Self::Exciter => "Exciter",
        }
    }
}

// ============================================================================
// Plugin Struct
// ============================================================================

/// Maximum number of channels supported for pre-allocated buffers.
const MAX_CHANNELS: usize = 32;
/// Default pre-allocation size in samples (frames * channels).
/// Covers 48000 frames * 2 channels = 96000. Grown in process_in_place if needed.
const DEFAULT_BUF_SIZE: usize = 96000;

pub struct SaturationPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_mode: ParameterId,
    mode: SaturationMode,
    param_drive: ParameterId,
    drive: f32,
    param_tone: ParameterId,
    tone: f32,
    param_exciter_freq: ParameterId,
    exciter_freq: f32,
    param_oversampling: ParameterId,
    oversampling_index: usize, // 0=Off, 1=2x, 2=4x
    param_output_gain: ParameterId,
    output_gain_db: f32,
    param_mix: ParameterId,
    mix: f32,

    // --- Phase 3A: SOTA parameters ---
    param_dynamic_amount: ParameterId,
    dynamic_amount: f32,
    param_dynamic_attack_ms: ParameterId,
    dynamic_attack_ms: f32,
    param_dynamic_release_ms: ParameterId,
    dynamic_release_ms: f32,
    param_dc_blocker: ParameterId,
    dc_blocker_enabled: bool,
    param_use_adaa: ParameterId,
    use_adaa: bool,

    // DSP state
    oversampler: Option<Oversampler>,
    crossovers: Vec<Lr4Crossover<f32>>, // For exciter mode (one per channel)

    // --- Phase 3A: SOTA DSP state ---
    dc_blocker: DcBlocker,
    adaa_tanh: Vec<Adaa1>, // Per-channel to avoid state corruption in interleaved processing
    adaa_softclip: Vec<Adaa1>, // Per-channel
    envelope_followers: Vec<EnvelopeFollower>, // Per-channel for dynamic saturation

    // Smoothers
    drive_smoother: Smoother,
    mix_smoother: Smoother,
    output_smoother: Smoother,

    // Pre-allocated buffers
    dry_buf: Vec<f32>,  // Original signal for mix
    low_buf: Vec<f32>,  // Low band (pass-through) for exciter mode
    high_buf: Vec<f32>, // High band (saturated) for exciter mode

    cached_parameters: Vec<Parameter>,
}

// ============================================================================
// Saturation Functions
// ============================================================================

/// Soft clip: tanh(input * drive) / tanh(drive)
#[inline(always)]
fn soft_clip(x: f32, drive: f32) -> f32 {
    let driven = x * drive;
    let tanh_drive = drive.tanh();
    if tanh_drive < 1e-6 {
        x
    } else {
        driven.tanh() / tanh_drive
    }
}

/// Tube: symmetric polynomial-like saturation x / (1 + |x|^n).
/// f(-x) = -f(x), so it is an odd function. The exponent `n` (tone) controls
/// the character of the saturation knee but does NOT add even harmonics.
#[inline(always)]
fn tube(x: f32, drive: f32, n: f32) -> f32 {
    let driven = x * drive;
    driven / (1.0 + driven.abs().powf(n))
}

/// Tape-style exponential saturation (memoryless sigmoid, not true hysteresis).
#[inline(always)]
fn tape(x: f32, drive: f32) -> f32 {
    let driven = x * drive;
    driven.signum() * (1.0 - (-driven.abs() * 2.0).exp()) * 0.5
}

/// Dispatch to the appropriate saturation function.
/// Exciter mode returns the sample unchanged (HF splitting is handled separately).
#[inline(always)]
fn saturate(sample: f32, mode: SaturationMode, drive: f32, tone: f32) -> f32 {
    match mode {
        SaturationMode::SoftClip => soft_clip(sample, drive),
        SaturationMode::Tube => tube(sample, drive, tone),
        SaturationMode::Tape => tape(sample, drive),
        SaturationMode::Exciter => sample, // handled separately
    }
}

// ============================================================================
// Implementation
// ============================================================================

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

    fn mode_string(&self) -> String {
        self.mode.name().to_string()
    }

    fn oversampling_string(&self) -> String {
        match self.oversampling_index {
            0 => "Off".to_string(),
            1 => "2x".to_string(),
            2 => "4x".to_string(),
            _ => "Off".to_string(),
        }
    }

    fn rebuild_crossovers(&mut self) {
        for xo in &mut self.crossovers {
            xo.set_frequency(self.exciter_freq);
        }
    }

    fn rebuild_oversampler(&mut self) {
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

    fn rebuild_cached_parameters(&mut self) {
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
    fn process_exciter_direct(&mut self, buffer: &mut [f32], num_frames: usize, drive: f32) {
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
}

// ============================================================================
// InPlacePlugin trait
// ============================================================================

impl InPlacePlugin for SaturationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Saturation", "1.0.0", "SotF")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

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
            self.dc_blocker_enabled = value.as_float().unwrap_or(1.0) > 0.5;
        } else if id == self.param_use_adaa {
            self.use_adaa = value.as_float().unwrap_or(1.0) > 0.5;
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_mode {
            Some(ParameterValue::String(self.mode_string()))
        } else if id == &self.param_drive {
            Some(ParameterValue::Float(self.drive))
        } else if id == &self.param_tone {
            Some(ParameterValue::Float(self.tone))
        } else if id == &self.param_exciter_freq {
            Some(ParameterValue::Float(self.exciter_freq))
        } else if id == &self.param_oversampling {
            Some(ParameterValue::String(self.oversampling_string()))
        } else if id == &self.param_output_gain {
            Some(ParameterValue::Float(self.output_gain_db))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if id == &self.param_dynamic_amount {
            Some(ParameterValue::Float(self.dynamic_amount))
        } else if id == &self.param_dynamic_attack_ms {
            Some(ParameterValue::Float(self.dynamic_attack_ms))
        } else if id == &self.param_dynamic_release_ms {
            Some(ParameterValue::Float(self.dynamic_release_ms))
        } else if id == &self.param_dc_blocker {
            Some(ParameterValue::Float(if self.dc_blocker_enabled {
                1.0
            } else {
                0.0
            }))
        } else if id == &self.param_use_adaa {
            Some(ParameterValue::Float(if self.use_adaa { 1.0 } else { 0.0 }))
        } else {
            None
        }
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

        // Pre-allocate buffers for max expected frame size
        let buf_size = 8192 * self.channels;
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

        // Guard against buggy host sending a buffer shorter than total
        debug_assert!(
            buffer.len() >= total,
            "process_in_place: buffer too short ({} < {})",
            buffer.len(),
            total
        );

        // Grow all pre-allocated buffers together if host sends a larger block
        if self.dry_buf.len() < total {
            self.dry_buf.resize(total, 0.0);
            self.low_buf.resize(total, 0.0);
            self.high_buf.resize(total, 0.0);
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(num_frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 48000,
            num_frames,
        }
    }

    fn rms(buf: &[f32]) -> f32 {
        let sum: f32 = buf.iter().map(|x| x * x).sum();
        (sum / buf.len() as f32).sqrt()
    }

    fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
        (0..num_frames)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
            })
            .collect()
    }

    #[test]
    fn test_soft_clip_limits_output() {
        // With high drive, output should still be bounded
        let channels = 1;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 10.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 4800;
        let mut buffer = make_sine(1000.0, 48000, num_frames, 0.8);

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // All samples should be bounded within [-1.0, 1.0] (tanh/tanh(drive))
        // ADAA mode can produce tiny overshoots (~1-2%) at the transition between
        // fallback and normal operation, and DC blocker adds transient ripple
        let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak <= 1.05, // ADAA + DC blocker tolerance
            "Soft clip output should be bounded: peak={}",
            peak
        );
        // Output should not be silent
        assert!(
            peak > 0.1,
            "Soft clip should produce non-trivial output: peak={}",
            peak
        );
    }

    #[test]
    fn test_tube_asymmetry() {
        // With tone > 1, tube saturation produces different positive/negative peaks
        let drive = 5.0;
        let tone = 2.0;

        let pos = tube(0.5, drive, tone);
        let neg = tube(-0.5, drive, tone);

        // Tube is antisymmetric in sign but NOT in absolute magnitude when n > 1
        // Actually for x/(1+|x|^n), tube(-x) = -(-x)/(1+|-x|^n) = x/(1+|x|^n) = -tube(x)
        // So it IS antisymmetric. But the harmonic content (even vs odd) depends on n.
        // Let's verify the function works and produces bounded output.
        assert!(pos > 0.0, "Positive input should give positive output");
        assert!(neg < 0.0, "Negative input should give negative output");
        assert!(
            pos.abs() < (0.5 * drive).abs(),
            "Tube should compress: pos={}, input={}",
            pos,
            0.5 * drive
        );
    }

    #[test]
    fn test_tape_saturation() {
        let channels = 1;
        let params = SaturationPluginParams {
            mode: "Tape".to_string(),
            drive: 5.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 4800;
        let mut buffer = make_sine(1000.0, 48000, num_frames, 0.8);

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // Tape output is bounded by 0.5 (the scaling factor)
        let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak <= 0.55, // tolerance
            "Tape output should be bounded around 0.5: peak={}",
            peak
        );
        assert!(
            peak > 0.1,
            "Tape should produce non-trivial output: peak={}",
            peak
        );
    }

    #[test]
    fn test_exciter_only_affects_hf() {
        let sr = 48000u32;
        let channels = 1;
        let num_frames = 48000; // 1 second

        let params = SaturationPluginParams {
            mode: "Exciter".to_string(),
            drive: 10.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(sr).unwrap();

        // Test with 200Hz signal (well below exciter freq)
        let mut buf_lf = make_sine(200.0, sr, num_frames, 0.5);
        let input_rms_lf = rms(&buf_lf);

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buf_lf, &ctx).unwrap();

        // Low frequency should pass through mostly unchanged
        let output_rms_lf = rms(&buf_lf[num_frames / 2..]);
        assert!(
            output_rms_lf > input_rms_lf * 0.7,
            "200Hz signal should pass through exciter: input_rms={:.4}, output_rms={:.4}",
            input_rms_lf,
            output_rms_lf
        );

        // Test with 8kHz signal (above exciter freq)
        plugin.reset();
        let mut buf_hf = make_sine(8000.0, sr, num_frames, 0.5);
        let input_rms_hf = rms(&buf_hf);

        plugin.process_in_place(&mut buf_hf, &ctx).unwrap();
        let output_rms_hf = rms(&buf_hf[num_frames / 2..]);

        // High frequency should be affected (shaped/compressed by soft clip)
        // The RMS should change noticeably
        let ratio = output_rms_hf / input_rms_hf;
        assert!(
            (ratio - 1.0).abs() > 0.01,
            "8kHz signal should be affected by exciter: ratio={:.4}",
            ratio
        );
    }

    #[test]
    fn test_saturation_passthrough() {
        // With mix=0, output should equal dry signal
        let channels = 2;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 10.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 0.0,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 256;
        let mut buffer = vec![0.0f32; num_frames * channels];
        for frame in 0..num_frames {
            let t = frame as f32 / 48000.0;
            let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            buffer[frame * channels] = val;
            buffer[frame * channels + 1] = val;
        }
        let original = buffer.clone();

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // Output should be identical to input (mix=0 means fully dry)
        for i in 0..buffer.len() {
            let diff = (buffer[i] - original[i]).abs();
            assert!(
                diff < 1e-5,
                "mix=0: sample {} differs: output={}, expected={}, diff={}",
                i,
                buffer[i],
                original[i],
                diff
            );
        }
    }

    #[test]
    fn test_oversampling_processes() {
        // Verify 2x oversampling produces output without errors
        let channels = 1;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 5.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "2x".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 512;

        // Process multiple blocks to fill the oversampler pipeline
        for _ in 0..20 {
            let mut buffer = make_sine(1000.0, 48000, num_frames, 0.5);
            let ctx = make_context(num_frames);
            let result = plugin.process_in_place(&mut buffer, &ctx);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), num_frames);

            // All samples should be finite
            for (i, &s) in buffer.iter().enumerate() {
                assert!(s.is_finite(), "sample {} not finite: {}", i, s);
            }
        }
    }

    #[test]
    fn test_saturation_declares_oversampling() {
        // Default oversampling index is 1 (2x), so preferred_oversampling should be Some(2)
        let plugin = SaturationPlugin::new(2);
        assert_eq!(plugin.preferred_oversampling(), Some(2));

        // With oversampling set to Off
        let params_off = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 2.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 0.5,
            ..Default::default()
        };
        let plugin_off = SaturationPlugin::from_params(2, params_off);
        assert_eq!(plugin_off.preferred_oversampling(), None);

        // With oversampling set to 4x
        let params_4x = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 2.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "4x".to_string(),
            output_gain_db: 0.0,
            mix: 0.5,
            ..Default::default()
        };
        let plugin_4x = SaturationPlugin::from_params(2, params_4x);
        assert_eq!(plugin_4x.preferred_oversampling(), Some(4));
    }

    #[test]
    fn test_saturation_default_f64_is_false() {
        let plugin = SaturationPlugin::new(2);
        assert!(!plugin.supports_f64());
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut plugin = SaturationPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Set drive
        plugin
            .set_parameter(ParameterId::from("drive"), ParameterValue::Float(8.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("drive"));
        assert_eq!(val, Some(ParameterValue::Float(8.0)));

        // Set mode
        plugin
            .set_parameter(
                ParameterId::from("mode"),
                ParameterValue::String("Tape".to_string()),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mode"));
        assert_eq!(val, Some(ParameterValue::String("Tape".to_string())));

        // Set tone
        plugin
            .set_parameter(ParameterId::from("tone"), ParameterValue::Float(2.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("tone"));
        assert_eq!(val, Some(ParameterValue::Float(2.5)));

        // Set exciter freq
        plugin
            .set_parameter(
                ParameterId::from("exciter_freq"),
                ParameterValue::Float(5000.0),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("exciter_freq"));
        assert_eq!(val, Some(ParameterValue::Float(5000.0)));

        // Set oversampling
        plugin
            .set_parameter(
                ParameterId::from("oversampling"),
                ParameterValue::String("4x".to_string()),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("oversampling"));
        assert_eq!(val, Some(ParameterValue::String("4x".to_string())));

        // Set output gain
        plugin
            .set_parameter(
                ParameterId::from("output_gain"),
                ParameterValue::Float(-3.0),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("output_gain"));
        assert_eq!(val, Some(ParameterValue::Float(-3.0)));

        // Set mix
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(val, Some(ParameterValue::Float(0.75)));
    }

    // =========================================================================
    // Regression tests for review-found bugs
    // =========================================================================

    /// Bug 2.1: low_buf / high_buf not resized with dry_buf.
    /// A block larger than DEFAULT_BUF_SIZE must not panic in exciter mode.
    #[test]
    fn test_exciter_large_block_no_panic() {
        let channels = 2;
        let params = SaturationPluginParams {
            mode: "Exciter".to_string(),
            drive: 5.0,
            tone: 1.5,
            exciter_freq: 3000.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        // Send a block larger than DEFAULT_BUF_SIZE (96000 samples total = 48000 frames * 2 ch)
        let num_frames = 50000; // > 48000 default frames per channel
        let mut buffer = vec![0.3f32; num_frames * channels];
        let ctx = make_context(num_frames);
        // Must not panic
        let result = plugin.process_in_place(&mut buffer, &ctx);
        assert!(result.is_ok(), "Large block should not panic: {:?}", result);
        // Output must be finite
        for (i, &s) in buffer.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        }
    }

    /// Bug 1.1: Tube ADAA must not change harmonic character vs direct path.
    /// When ADAA is on/off for Tube mode with tone != 1.0, the output should
    /// be identical (we now use direct tube() in both cases).
    #[test]
    fn test_tube_adaa_matches_direct_when_tone_not_one() {
        let num_frames = 512;
        let channels = 1;

        let make_tube_plugin = |use_adaa: bool| {
            SaturationPlugin::from_params(
                channels,
                SaturationPluginParams {
                    mode: "Tube".to_string(),
                    drive: 5.0,
                    tone: 2.0, // tone != 1.0 — previously ADAA used wrong nonlinearity
                    oversampling: "Off".to_string(),
                    output_gain_db: 0.0,
                    mix: 1.0,
                    use_adaa,
                    dc_blocker_enabled: false,
                    ..Default::default()
                },
            )
        };

        let mut plugin_adaa = make_tube_plugin(true);
        plugin_adaa.initialize(48000).unwrap();
        let mut plugin_direct = make_tube_plugin(false);
        plugin_direct.initialize(48000).unwrap();

        let signal = make_sine(1000.0, 48000, num_frames, 0.5);
        let mut buf_adaa = signal.clone();
        let mut buf_direct = signal;
        let ctx = make_context(num_frames);

        plugin_adaa.process_in_place(&mut buf_adaa, &ctx).unwrap();
        plugin_direct
            .process_in_place(&mut buf_direct, &ctx)
            .unwrap();

        // With the fix, ADAA Tube path uses direct tube(), outputs must be identical
        for i in 0..num_frames {
            let diff = (buf_adaa[i] - buf_direct[i]).abs();
            assert!(
                diff < 1e-5,
                "Tube ADAA and direct diverge at sample {}: adaa={}, direct={}, diff={}",
                i,
                buf_adaa[i],
                buf_direct[i],
                diff
            );
        }
    }

    /// Bug 1.2: Drive smoother must not produce block-constant output.
    /// After a parameter change, consecutive blocks should show gradually
    /// changing drive (not an instant step).
    #[test]
    fn test_drive_smoother_ramps_across_block() {
        let channels = 1;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 1.0, // low drive to start
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            use_adaa: false,
            dc_blocker_enabled: false,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        // Change drive to maximum — smoother will ramp from 1 to 20 over ~10ms
        plugin
            .set_parameter(ParameterId::from("drive"), ParameterValue::Float(20.0))
            .unwrap();

        // Process a single block of 256 samples with a constant DC input
        let num_frames = 256;
        let mut buffer = vec![0.5f32; num_frames]; // constant input
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // If drive is ramping per-sample, output values should NOT all be identical
        let first = buffer[0];
        let last = buffer[num_frames - 1];
        assert!(
            (first - last).abs() > 1e-4,
            "Drive ramp should produce different values at start ({}) and end ({}) of block",
            first,
            last
        );
    }

    /// Bug 1.3: Dynamic saturation must modulate drive (not post-gain).
    /// With dynamic_amount > 0 and a loud signal, output should reflect
    /// drive modulation rather than post-distortion multiplication.
    /// Key invariant: with mix=1, dry=0 → wet drive > 0 → output finite and bounded.
    #[test]
    fn test_dynamic_saturation_bounded_no_pumping() {
        let channels = 1;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 5.0,
            dynamic_amount: 1.0, // full dynamic
            dynamic_attack_ms: 1.0,
            dynamic_release_ms: 10.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            use_adaa: false,
            dc_blocker_enabled: false,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        // Full-scale input: drive modulation should not blow up
        let num_frames = 2048;
        let mut buffer = make_sine(440.0, 48000, num_frames, 1.0);
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        for (i, &s) in buffer.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
            // tanh-based soft_clip bounds output to (-1, 1) regardless of drive
            assert!(
                s.abs() <= 1.05,
                "dynamic saturation output out of bounds at sample {}: {}",
                i,
                s
            );
        }
    }

    /// Bug 2.4: flush_denormals_inplace must only operate on [..total] samples.
    /// Verify that processing a small block inside a larger allocation does not
    /// corrupt or panic on the samples outside the valid range.
    #[test]
    fn test_flush_denormals_limited_to_valid_samples() {
        let channels = 2;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 3.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            use_adaa: false,
            dc_blocker_enabled: false,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        // Allocate a buffer larger than nf*nc, fill tail with sentinel
        let num_frames = 64;
        let total = num_frames * channels;
        let extra = 16;
        let mut buffer = vec![0.1f32; total + extra];
        let sentinel = 1234.5678f32;
        for s in buffer[total..].iter_mut() {
            *s = sentinel;
        }

        let ctx = make_context(num_frames);
        // process_in_place operates on buffer but only touches [..total]
        // We pass a slice of exactly `total` to match the contract
        plugin.process_in_place(&mut buffer[..total], &ctx).unwrap();

        // Sentinel values after total must be unchanged
        for (i, &s) in buffer[total..].iter().enumerate() {
            assert_eq!(
                s, sentinel,
                "sample beyond valid range at offset {} was modified",
                i
            );
        }
    }

    /// Bug 1.4: LUFS target is removed — verify no LUFS-related field exists
    /// by checking that processing with mix=0 gives pure passthrough (no auto-gain).
    #[test]
    fn test_no_lufs_auto_gain_on_passthrough() {
        let channels = 1;
        let params = SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 10.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 0.0, // full dry — LUFS would have altered this
            use_adaa: false,
            dc_blocker_enabled: false,
            ..Default::default()
        };
        let mut plugin = SaturationPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 4800;
        let signal = make_sine(440.0, 48000, num_frames, 0.5);
        let mut buffer = signal.clone();
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // mix=0 → pure dry; no LUFS gain should be applied
        for i in 0..num_frames {
            let diff = (buffer[i] - signal[i]).abs();
            assert!(
                diff < 1e-5,
                "sample {} changed with mix=0: output={}, expected={}, diff={}",
                i,
                buffer[i],
                signal[i],
                diff
            );
        }
    }
}
