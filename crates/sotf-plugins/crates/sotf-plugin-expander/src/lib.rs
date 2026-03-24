// ============================================================================
// Expander Plugin
// ============================================================================

pub mod params;

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::find_by_key as pk;
use crate::params::PARAMS as EX;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::{DetectionMode, LevelDetector, LookaheadBuffer, MeasuredMakeup};
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

const AUTO_MAKEUP_OVERSHOOT_FACTOR: f32 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpanderPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_hysteresis_db")]
    pub hysteresis_db: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
    #[serde(default = "default_auto_makeup")]
    pub auto_makeup: bool,
    #[serde(default)]
    pub lookahead_ms: f32,
    #[serde(default = "default_detection_mode_str")]
    pub detection_mode: String,
    #[serde(default)]
    pub measured_auto_makeup: bool,
}

fn default_threshold_db() -> f32 {
    pk(EX, "threshold").default_f64() as f32
}
fn default_ratio() -> f32 {
    pk(EX, "ratio").default_f64() as f32
}
fn default_attack_ms() -> f32 {
    pk(EX, "attack").default_f64() as f32
}
fn default_release_ms() -> f32 {
    pk(EX, "release").default_f64() as f32
}
fn default_range_db() -> f32 {
    pk(EX, "range").default_f64() as f32
}
fn default_knee_db() -> f32 {
    pk(EX, "knee").default_f64() as f32
}
fn default_hysteresis_db() -> f32 {
    pk(EX, "hysteresis").default_f64() as f32
}
fn default_hold_ms() -> f32 {
    pk(EX, "hold").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(EX, "mix").default_f64() as f32
}
fn default_link_channels() -> bool {
    pk(EX, "link_channels").default_bool()
}
fn default_sidechain_hpf_hz() -> f32 {
    pk(EX, "sidechain_hpf_hz").default_f64() as f32
}
fn default_auto_makeup() -> bool {
    pk(EX, "auto_makeup").default_bool()
}
fn default_detection_mode_str() -> String {
    "peak".to_string()
}

const RMS_WINDOW_MS: f32 = 10.0;
const MEASURED_MAKEUP_SMOOTHING_MS: f32 = 1000.0;
const MAX_LOOKAHEAD_MS: f32 = 20.0;

fn parse_detection_mode(s: &str) -> DetectionMode {
    match s.to_ascii_lowercase().as_str() {
        "rms" => DetectionMode::Rms {
            window_ms: RMS_WINDOW_MS,
        },
        _ => DetectionMode::Peak,
    }
}

impl Default for ExpanderPluginParams {
    fn default() -> Self {
        Self {
            threshold_db: default_threshold_db(),
            ratio: default_ratio(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            range_db: default_range_db(),
            knee_db: default_knee_db(),
            hysteresis_db: default_hysteresis_db(),
            hold_ms: default_hold_ms(),
            mix: default_mix(),
            link_channels: default_link_channels(),
            sidechain_hpf_hz: default_sidechain_hpf_hz(),
            auto_makeup: default_auto_makeup(),
            lookahead_ms: 0.0,
            detection_mode: default_detection_mode_str(),
            measured_auto_makeup: false,
        }
    }
}

/// Data exposed by the expander for monitoring
#[derive(Debug, Clone)]
pub struct ExpanderData {
    pub input_levels_db: Arc<Vec<f32>>,
    pub is_open: bool,
    pub attenuation_db: Arc<Vec<f32>>,
}

impl Default for ExpanderData {
    fn default() -> Self {
        Self {
            input_levels_db: Arc::new(Vec::new()),
            is_open: false,
            attenuation_db: Arc::new(Vec::new()),
        }
    }
}

impl ExpanderData {
    pub fn new(channels: usize) -> Self {
        Self {
            input_levels_db: Arc::new(vec![-120.0; channels]),
            is_open: false,
            attenuation_db: Arc::new(vec![0.0; channels]),
        }
    }

    pub fn update(&mut self, is_open: bool, attenuation: &[f32]) {
        self.is_open = is_open;
        if let Some(mut_att) = Arc::get_mut(&mut self.attenuation_db)
            && mut_att.len() == attenuation.len()
        {
            mut_att.copy_from_slice(attenuation);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Open,
    Hold,
    Closing,
}

pub struct ExpanderPlugin {
    channels: usize,
    sample_rate: u32,
    param_threshold: ParameterId,
    threshold_db: f32,
    param_ratio: ParameterId,
    ratio: f32,
    param_attack: ParameterId,
    attack_ms: f32,
    param_release: ParameterId,
    release_ms: f32,
    param_range: ParameterId,
    range_db: f32,
    param_knee: ParameterId,
    knee_db: f32,
    param_hysteresis: ParameterId,
    hysteresis_db: f32,
    param_hold: ParameterId,
    hold_ms: f32,
    param_mix: ParameterId,
    mix: f32,
    param_link_channels: ParameterId,
    link_channels: bool,
    param_sidechain_hpf_hz: ParameterId,
    sidechain_hpf_hz: f32,
    param_auto_makeup: ParameterId,
    auto_makeup: bool,
    param_lookahead: ParameterId,
    lookahead_ms: f32,
    param_detection_mode: ParameterId,
    detection_mode_str: String,
    param_measured_auto_makeup: ParameterId,
    measured_auto_makeup: bool,
    lookahead_buffers: Vec<LookaheadBuffer>,
    level_detectors: Vec<LevelDetector>,
    measured_makeup: MeasuredMakeup,
    envelope: Vec<f32>,
    gate_state: Vec<GateState>,
    hold_counter: Vec<usize>,
    input_levels_db: Vec<f32>,
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    sidechain_hpf_alpha: f32,
    attack_coeff: f32,
    release_coeff: f32,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    cache: RealTimeCache<ExpanderData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

impl ExpanderPlugin {
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, ExpanderPluginParams::default())
    }
    pub fn with_params(channels: usize, params: ExpanderPluginParams) -> Self {
        let sr = 44100;
        let lookahead_ms = params.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS);
        let detection_mode = parse_detection_mode(&params.detection_mode);
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_threshold: ParameterId::from("threshold"),
            threshold_db: params.threshold_db,
            param_ratio: ParameterId::from("ratio"),
            ratio: params.ratio,
            param_attack: ParameterId::from("attack"),
            attack_ms: params.attack_ms,
            param_release: ParameterId::from("release"),
            release_ms: params.release_ms,
            param_range: ParameterId::from("range"),
            range_db: params.range_db,
            param_knee: ParameterId::from("knee"),
            knee_db: params.knee_db,
            param_hysteresis: ParameterId::from("hysteresis"),
            hysteresis_db: params.hysteresis_db,
            param_hold: ParameterId::from("hold"),
            hold_ms: params.hold_ms,
            param_mix: ParameterId::from("mix"),
            mix: params.mix.clamp(0.0, 1.0),
            param_link_channels: ParameterId::from("link_channels"),
            link_channels: params.link_channels,
            param_sidechain_hpf_hz: ParameterId::from("sidechain_hpf_hz"),
            sidechain_hpf_hz: params.sidechain_hpf_hz.max(0.0),
            param_auto_makeup: ParameterId::from("auto_makeup"),
            auto_makeup: params.auto_makeup,
            param_lookahead: ParameterId::from("lookahead"),
            lookahead_ms,
            param_detection_mode: ParameterId::from("detection_mode"),
            detection_mode_str: params.detection_mode.clone(),
            param_measured_auto_makeup: ParameterId::from("measured_auto_makeup"),
            measured_auto_makeup: params.measured_auto_makeup,
            lookahead_buffers: (0..channels)
                .map(|_| LookaheadBuffer::from_ms(MAX_LOOKAHEAD_MS, sr, 1))
                .collect(),
            level_detectors: (0..channels)
                .map(|_| LevelDetector::new(detection_mode, sr))
                .collect(),
            measured_makeup: MeasuredMakeup::new(MEASURED_MAKEUP_SMOOTHING_MS, sr),
            envelope: vec![0.0; channels],
            gate_state: vec![GateState::Open; channels],
            hold_counter: vec![0; channels],
            input_levels_db: vec![-100.0; channels],
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            sidechain_hpf_alpha: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            threshold_smoother: Smoother::new(params.threshold_db, 5.0, sr),
            mix_smoother: Smoother::new(params.mix, 5.0, sr),
            cache: RealTimeCache::new(ExpanderData::new(channels)),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.update_lookahead_delay();
        p.rebuild_cached_parameters();
        p
    }

    fn update_lookahead_delay(&mut self) {
        for buf in &mut self.lookahead_buffers {
            buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        let det_idx = if self.detection_mode_str == "rms" {
            1.0f32
        } else {
            0.0
        };
        self.cached_parameters = vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(EX, "threshold").min_f64() as f32,
                pk(EX, "threshold").max_f64() as f32,
            )
            .with_description("Level below which expansion starts (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(EX, "ratio").min_f64() as f32,
                pk(EX, "ratio").max_f64() as f32,
            )
            .with_description("Expansion ratio (1:1 to 20:1)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(EX, "attack").min_f64() as f32,
                pk(EX, "attack").max_f64() as f32,
            )
            .with_description("Attack time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(EX, "release").min_f64() as f32,
                pk(EX, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "range",
                "Range",
                self.range_db,
                pk(EX, "range").min_f64() as f32,
                pk(EX, "range").max_f64() as f32,
            )
            .with_description("Maximum attenuation depth (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "knee",
                "Knee",
                self.knee_db,
                pk(EX, "knee").min_f64() as f32,
                pk(EX, "knee").max_f64() as f32,
            )
            .with_description("Soft knee width (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "hysteresis",
                "Hysteresis",
                self.hysteresis_db,
                pk(EX, "hysteresis").min_f64() as f32,
                pk(EX, "hysteresis").max_f64() as f32,
            )
            .with_description("Hysteresis between open and close thresholds (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "hold",
                "Hold",
                self.hold_ms,
                pk(EX, "hold").min_f64() as f32,
                pk(EX, "hold").max_f64() as f32,
            )
            .with_description("Hold time before closing (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(EX, "mix").min_f64() as f32,
                pk(EX, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = expanded)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_description("Use linked sidechain for all channels")
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("auto_makeup", "Auto Makeup", self.auto_makeup)
                .with_description("Automatically compensate for expansion attenuation")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "sidechain_hpf_hz",
                "Sidechain HPF",
                self.sidechain_hpf_hz,
                pk(EX, "sidechain_hpf_hz").min_f64() as f32,
                pk(EX, "sidechain_hpf_hz").max_f64() as f32,
            )
            .with_description("High-pass filter frequency for sidechain (Hz)")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "lookahead",
                "Lookahead",
                self.lookahead_ms,
                0.0,
                MAX_LOOKAHEAD_MS,
            )
            .with_description("Lookahead delay for gain computation (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float("detection_mode", "Detection Mode", det_idx, 0.0, 1.0)
                .with_description("Level detection mode (0=peak, 1=rms)")
                .with_group("Sidechain")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "measured_auto_makeup",
                "Measured Makeup",
                self.measured_auto_makeup,
            )
            .with_description(
                "Use measured gain reduction for auto-makeup instead of heuristic",
            )
            .with_group("Output")
            .with_importance(ParameterImportance::FineTuning),
        ];
    }
    pub fn from_params(channels: usize, params: ExpanderPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    fn calculate_expansion_attenuation(&self, input_db: f32, threshold: f32) -> f32 {
        let knee = self.knee_db.max(0.0);
        let slope = 1.0 - 1.0 / self.ratio.max(1.0);
        let atten = if knee < 0.1 {
            if input_db >= threshold {
                0.0
            } else {
                (threshold - input_db) * slope
            }
        } else if input_db > threshold + knee / 2.0 {
            0.0
        } else if input_db < threshold - knee / 2.0 {
            (threshold - input_db) * slope
        } else {
            let below = threshold + knee / 2.0 - input_db;
            let kf = below / knee;
            kf * kf * (knee / 2.0) * slope
        };
        atten.min(self.range_db.max(0.0))
    }

    fn update_coefficients(&mut self) {
        self.attack_coeff = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate as f32)).exp();
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();
        let fc = self.sidechain_hpf_hz.max(0.0);
        if fc > 0.0 && self.sample_rate > 0 {
            let dt = 1.0 / self.sample_rate as f32;
            let rc = 1.0 / (2.0 * PI * fc);
            self.sidechain_hpf_alpha = rc / (rc + dt);
        } else {
            self.sidechain_hpf_alpha = 0.0;
        }
    }

    #[inline]
    fn apply_sidechain_filter(&mut self, ch: usize, sample: f32) -> f32 {
        if self.sidechain_hpf_alpha <= 0.0 {
            return sample;
        }
        let y = self.sidechain_hpf_alpha
            * (self.sidechain_hpf_prev_output[ch] + sample - self.sidechain_hpf_prev_input[ch]);
        self.sidechain_hpf_prev_input[ch] = sample;
        self.sidechain_hpf_prev_output[ch] = y;
        y
    }

    fn process_channel(
        &mut self,
        ch: usize,
        input_db: f32,
        hold_samples: usize,
        threshold: f32,
    ) -> f32 {
        let open_th = threshold;
        let close_th = threshold - self.hysteresis_db;
        let target = match self.gate_state[ch] {
            GateState::Open => {
                if input_db < open_th {
                    self.gate_state[ch] = GateState::Hold;
                    self.hold_counter[ch] = hold_samples;
                    0.0
                } else {
                    0.0
                }
            }
            GateState::Hold => {
                if input_db >= open_th {
                    self.gate_state[ch] = GateState::Open;
                    self.hold_counter[ch] = 0;
                    0.0
                } else if self.hold_counter[ch] > 0 {
                    self.hold_counter[ch] -= 1;
                    0.0
                } else if input_db < close_th {
                    self.gate_state[ch] = GateState::Closing;
                    self.calculate_expansion_attenuation(input_db, threshold)
                } else {
                    0.0
                }
            }
            GateState::Closing => {
                if input_db >= open_th {
                    self.gate_state[ch] = GateState::Open;
                    0.0
                } else {
                    self.calculate_expansion_attenuation(input_db, threshold)
                }
            }
        };
        let coeff = if target > self.envelope[ch] {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope[ch] = target + coeff * (self.envelope[ch] - target);
        self.envelope[ch]
    }
}

impl InPlacePlugin for ExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Expander", "1.1.0", "SotF")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_threshold {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "threshold").default_f64() as f32);
            if v.is_finite() {
                self.threshold_db = v;
                self.threshold_smoother.set_target(self.threshold_db);
            }
        } else if id == self.param_ratio {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "ratio").default_f64() as f32);
            if v.is_finite() {
                self.ratio = v.max(1.0);
            }
        } else if id == self.param_attack {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "attack").default_f64() as f32);
            if v.is_finite() {
                self.attack_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_release {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "release").default_f64() as f32);
            if v.is_finite() {
                self.release_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_range {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "range").default_f64() as f32);
            if v.is_finite() {
                self.range_db = v.max(0.0);
            }
        } else if id == self.param_knee {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "knee").default_f64() as f32);
            if v.is_finite() {
                self.knee_db = v.max(0.0);
            }
        } else if id == self.param_hysteresis {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "hysteresis").default_f64() as f32);
            if v.is_finite() {
                self.hysteresis_db = v.max(0.0);
            }
        } else if id == self.param_hold {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "hold").default_f64() as f32);
            if v.is_finite() {
                self.hold_ms = v.max(0.0);
            }
        } else if id == self.param_mix {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "mix").default_f64() as f32);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if id == self.param_link_channels {
            self.link_channels = value
                .as_bool()
                .unwrap_or(pk(EX, "link_channels").default_bool());
        } else if id == self.param_auto_makeup {
            self.auto_makeup = value
                .as_bool()
                .unwrap_or(pk(EX, "auto_makeup").default_bool());
        } else if id == self.param_sidechain_hpf_hz {
            let v = value
                .as_float()
                .unwrap_or(pk(EX, "sidechain_hpf_hz").default_f64() as f32);
            if v.is_finite() {
                self.sidechain_hpf_hz = v.max(0.0);
                self.update_coefficients();
            }
        } else if id == self.param_lookahead {
            let v = value.as_float().unwrap_or(0.0);
            if v.is_finite() {
                self.lookahead_ms = v.clamp(0.0, MAX_LOOKAHEAD_MS);
                self.update_lookahead_delay();
            }
        } else if id == self.param_detection_mode {
            // Accept float index: 0=peak, 1=rms
            let idx = value.as_float().unwrap_or(0.0) as usize;
            let mode_str = if idx >= 1 { "rms" } else { "peak" };
            self.detection_mode_str = mode_str.to_string();
            let mode = parse_detection_mode(mode_str);
            for det in &mut self.level_detectors {
                det.set_mode(mode);
            }
        } else if id == self.param_measured_auto_makeup {
            self.measured_auto_makeup = value.as_bool().unwrap_or(false);
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.ratio))
        } else if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_range {
            Some(ParameterValue::Float(self.range_db))
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_hysteresis {
            Some(ParameterValue::Float(self.hysteresis_db))
        } else if id == &self.param_hold {
            Some(ParameterValue::Float(self.hold_ms))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if id == &self.param_link_channels {
            Some(ParameterValue::Bool(self.link_channels))
        } else if id == &self.param_auto_makeup {
            Some(ParameterValue::Bool(self.auto_makeup))
        } else if id == &self.param_sidechain_hpf_hz {
            Some(ParameterValue::Float(self.sidechain_hpf_hz))
        } else if id == &self.param_lookahead {
            Some(ParameterValue::Float(self.lookahead_ms))
        } else if id == &self.param_detection_mode {
            let idx = if self.detection_mode_str == "rms" {
                1.0
            } else {
                0.0
            };
            Some(ParameterValue::Float(idx))
        } else if id == &self.param_measured_auto_makeup {
            Some(ParameterValue::Bool(self.measured_auto_makeup))
        } else {
            None
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        let max_samples =
            (MAX_LOOKAHEAD_MS * 0.001 * sample_rate as f32).round() as usize;
        for buf in &mut self.lookahead_buffers {
            buf.resize(max_samples, 1);
        }
        self.update_lookahead_delay();
        let mode = parse_detection_mode(&self.detection_mode_str);
        self.level_detectors = (0..self.channels)
            .map(|_| LevelDetector::new(mode, sample_rate))
            .collect();
        self.measured_makeup
            .set_smoothing(MEASURED_MAKEUP_SMOOTHING_MS, sample_rate);
        Ok(())
    }
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
        self.sidechain_hpf_prev_input.fill(0.0);
        self.sidechain_hpf_prev_output.fill(0.0);
        for buf in &mut self.lookahead_buffers {
            buf.reset();
        }
        for det in &mut self.level_detectors {
            det.reset();
        }
        self.measured_makeup.reset();
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let hold_samples = (self.hold_ms * 0.001 * self.sample_rate as f32) as usize;

        let thresh = self.threshold_smoother.next_n(num_frames);
        let mix = self.mix_smoother.next_n(num_frames);
        let use_lookahead = self.lookahead_ms > 0.0;
        let use_measured = self.auto_makeup && self.measured_auto_makeup;

        // Heuristic auto-makeup (used when measured is off)
        let heuristic_makeup_gain = if self.auto_makeup && !use_measured {
            let slope = 1.0 - 1.0 / self.ratio.max(1.0);
            let avg_atten = self.range_db.max(0.0) * slope * AUTO_MAKEUP_OVERSHOOT_FACTOR;
            fast_pow10(avg_atten / 20.0)
        } else {
            1.0
        };

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let mut det_level = 0.0f32;
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[idx]);
                    let level = self.level_detectors[ch].process_linear(filtered);
                    det_level = det_level.max(level);
                    self.input_levels_db[ch] = 20.0 * fast_log10(level.max(1e-10));
                }

                let input_db = 20.0 * fast_log10(det_level.max(1e-10));
                let atten = self.process_channel(0, input_db, hold_samples, thresh);

                // Update measured makeup tracker
                if use_measured {
                    self.measured_makeup.update(atten);
                }
                let auto_makeup_gain = if use_measured {
                    self.measured_makeup.makeup_linear()
                } else {
                    heuristic_makeup_gain
                };

                let gain = (1.0 - mix) + mix * fast_pow10(-atten / 20.0) * auto_makeup_gain;

                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    if use_lookahead {
                        let delayed = self.lookahead_buffers[ch].push(buffer[idx]);
                        buffer[idx] = delayed * gain;
                    } else {
                        buffer[idx] *= gain;
                    }
                }
            }
        } else {
            for frame in 0..num_frames {
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[idx]);
                    let level = self.level_detectors[ch].process_linear(filtered);
                    let input_db = 20.0 * fast_log10(level.max(1e-10));
                    self.input_levels_db[ch] = input_db;

                    let atten = self.process_channel(ch, input_db, hold_samples, thresh);

                    if use_measured {
                        self.measured_makeup.update(atten);
                    }
                    let auto_makeup_gain = if use_measured {
                        self.measured_makeup.makeup_linear()
                    } else {
                        heuristic_makeup_gain
                    };

                    let gain = (1.0 - mix) + mix * fast_pow10(-atten / 20.0) * auto_makeup_gain;
                    if use_lookahead {
                        let delayed = self.lookahead_buffers[ch].push(buffer[idx]);
                        buffer[idx] = delayed * gain;
                    } else {
                        buffer[idx] *= gain;
                    }
                }
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            let is_open = self
                .gate_state
                .iter()
                .any(|&s| s == GateState::Open || s == GateState::Hold);
            self.cache.update(|d| {
                d.update(is_open, &self.envelope);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        if self.lookahead_ms > 0.0 {
            (self.lookahead_ms * 0.001 * self.sample_rate as f32).round() as usize
        } else {
            0
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn test_expander_basic() {
        let mut p = ExpanderPlugin::new(1);
        p.initialize(48000).unwrap();
        let mut b = vec![0.1; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999] < 0.1);
    }

    #[test]
    fn test_auto_makeup_with_zero_mix_is_unity() {
        // With mix=0, the expander has no effect on the signal.
        // Auto-makeup should not add gain when there's no expansion.
        let params = ExpanderPluginParams {
            mix: 0.0,
            auto_makeup: true,
            ratio: 4.0,
            range_db: 40.0,
            ..Default::default()
        };
        let mut p = ExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();
        let input_val = 0.5f32;
        let mut b = vec![input_val; 480];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 480,
            },
        )
        .unwrap();
        // With mix=0, output should equal input regardless of auto_makeup
        let last = b[479];
        assert!(
            (last - input_val).abs() < 1e-5,
            "mix=0 + auto_makeup should be unity, got {last} (expected {input_val})"
        );
    }

    #[test]
    fn test_auto_makeup_boosts_with_full_mix() {
        // With mix=1 and auto_makeup, the output should be boosted relative to
        // expansion-only (no auto_makeup) to compensate for attenuation.
        let base = ExpanderPluginParams {
            mix: 1.0,
            auto_makeup: false,
            threshold_db: -10.0,
            ratio: 4.0,
            range_db: 40.0,
            ..Default::default()
        };
        let mut p_no_am = ExpanderPlugin::with_params(1, base.clone());
        p_no_am.initialize(48000).unwrap();
        let mut p_am = ExpanderPlugin::with_params(
            1,
            ExpanderPluginParams {
                auto_makeup: true,
                ..base
            },
        );
        p_am.initialize(48000).unwrap();

        let input_val = 0.01f32; // quiet signal, below threshold
        let mut b_no = vec![input_val; 4800];
        let mut b_am = vec![input_val; 4800];
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 4800,
        };
        p_no_am.process_in_place(&mut b_no, &ctx).unwrap();
        p_am.process_in_place(&mut b_am, &ctx).unwrap();

        // Auto-makeup version should be louder than non-auto-makeup
        let last_no = b_no[4799].abs();
        let last_am = b_am[4799].abs();
        assert!(
            last_am > last_no,
            "auto_makeup should boost output: {last_am} > {last_no}"
        );
    }

    /// Sidechain HPF at 0 Hz should be effectively disabled: a low-frequency
    /// signal should still trigger expansion normally.
    #[test]
    fn test_sidechain_hpf_zero_hz_passes_low_freq() {
        let params = ExpanderPluginParams {
            sidechain_hpf_hz: 0.0,
            threshold_db: -20.0,
            ratio: 4.0,
            range_db: 40.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut p = ExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Generate a 50 Hz sine at -10 dBFS (above -20 threshold)
        let num_frames = 4800;
        let amplitude = 10.0_f32.powf(-10.0 / 20.0); // ~0.316
        let mut buf = vec![0.0f32; num_frames];
        for (i, sample) in buf.iter_mut().enumerate() {
            *sample = amplitude * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin();
        }

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        p.process_in_place(&mut buf, &ctx).unwrap();

        // With HPF=0 (disabled), the 50 Hz signal should be detected and the
        // gate should be open (signal above threshold = no expansion).
        // Output should be close to input amplitude.
        let last_rms: f32 = buf[4000..].iter().map(|x| x * x).sum::<f32>()
            / (num_frames - 4000) as f32;
        let last_rms = last_rms.sqrt();
        let expected_rms = amplitude / 2.0_f32.sqrt(); // RMS of sine
        assert!(
            last_rms > expected_rms * 0.5,
            "With HPF=0, low-freq signal above threshold should pass through. \
             RMS={last_rms:.4}, expected ~{expected_rms:.4}"
        );
    }

    /// Regression: attack/release coefficients were swapped.
    /// With fast attack (1ms) and slow release (200ms), the gate should close
    /// quickly when signal drops below threshold. If coefficients are swapped,
    /// the gate would close slowly (using release time instead).
    #[test]
    fn test_attack_release_not_swapped() {
        let fast_attack = ExpanderPluginParams {
            attack_ms: 1.0,
            release_ms: 200.0,
            threshold_db: -20.0,
            ratio: 10.0,
            range_db: 60.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut p = ExpanderPlugin::with_params(1, fast_attack);
        p.initialize(48000).unwrap();

        // First, feed loud signal to open the gate
        let mut loud = vec![0.5f32; 4800]; // -6 dBFS, well above -20 threshold
        p.process_in_place(
            &mut loud,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 4800,
            },
        )
        .unwrap();

        // Now feed quiet signal — gate should close within a few ms (fast attack)
        let quiet_val = 0.001f32; // -60 dBFS, well below threshold
        let mut quiet = vec![quiet_val; 480]; // 10ms of quiet
        p.process_in_place(
            &mut quiet,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 480,
            },
        )
        .unwrap();

        // After 10ms with 1ms attack, the gate should be mostly closed.
        // The signal should be significantly attenuated (not just passing through).
        let last_sample = quiet[479].abs();
        assert!(
            last_sample < quiet_val * 0.5,
            "Gate should close fast with 1ms attack, but output {last_sample} is still near input {quiet_val}. \
             Attack/release coefficients may be swapped."
        );
    }
}
