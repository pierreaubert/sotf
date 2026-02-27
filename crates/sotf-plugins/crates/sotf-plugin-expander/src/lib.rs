// ============================================================================
// Expander Plugin
// ============================================================================

use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::{find_by_key as pk, expander::PARAMS as EX};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

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
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
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
            self.release_coeff
        } else {
            self.attack_coeff
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
            let v = value.as_float().unwrap_or(pk(EX, "threshold").default_f64() as f32);
            if v.is_finite() {
                self.threshold_db = v;
                self.threshold_smoother.set_target(self.threshold_db);
            }
        } else if id == self.param_ratio {
            let v = value.as_float().unwrap_or(pk(EX, "ratio").default_f64() as f32);
            if v.is_finite() {
                self.ratio = v.max(1.0);
            }
        } else if id == self.param_attack {
            let v = value.as_float().unwrap_or(pk(EX, "attack").default_f64() as f32);
            if v.is_finite() {
                self.attack_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_release {
            let v = value.as_float().unwrap_or(pk(EX, "release").default_f64() as f32);
            if v.is_finite() {
                self.release_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_range {
            let v = value.as_float().unwrap_or(pk(EX, "range").default_f64() as f32);
            if v.is_finite() {
                self.range_db = v.max(0.0);
            }
        } else if id == self.param_knee {
            let v = value.as_float().unwrap_or(pk(EX, "knee").default_f64() as f32);
            if v.is_finite() {
                self.knee_db = v.max(0.0);
            }
        } else if id == self.param_hysteresis {
            let v = value.as_float().unwrap_or(pk(EX, "hysteresis").default_f64() as f32);
            if v.is_finite() {
                self.hysteresis_db = v.max(0.0);
            }
        } else if id == self.param_hold {
            let v = value.as_float().unwrap_or(pk(EX, "hold").default_f64() as f32);
            if v.is_finite() {
                self.hold_ms = v.max(0.0);
            }
        } else if id == self.param_mix {
            let v = value.as_float().unwrap_or(pk(EX, "mix").default_f64() as f32);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if id == self.param_link_channels {
            self.link_channels = value.as_bool().unwrap_or(pk(EX, "link_channels").default_bool());
        } else if id == self.param_sidechain_hpf_hz {
            let v = value.as_float().unwrap_or(pk(EX, "sidechain_hpf_hz").default_f64() as f32);
            if v.is_finite() {
                self.sidechain_hpf_hz = v.max(0.0);
                self.update_coefficients();
            }
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
        } else if id == &self.param_sidechain_hpf_hz {
            Some(ParameterValue::Float(self.sidechain_hpf_hz))
        } else {
            None
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        Ok(())
    }
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
        self.sidechain_hpf_prev_input.fill(0.0);
        self.sidechain_hpf_prev_output.fill(0.0);
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

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let mut det_level = 0.0f32;
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[idx]);
                    let level = filtered.abs();
                    det_level = det_level.max(level);
                    self.input_levels_db[ch] = 20.0 * fast_log10(level.max(1e-10));
                }

                let input_db = 20.0 * fast_log10(det_level.max(1e-10));
                let atten = self.process_channel(0, input_db, hold_samples, thresh);
                let gain = (1.0 - mix) + mix * fast_pow10(-atten / 20.0);

                for ch in 0..self.channels {
                    buffer[frame * self.channels + ch] *= gain;
                }
            }
        } else {
            for frame in 0..num_frames {
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[idx]);
                    let level = filtered.abs();
                    let input_db = 20.0 * fast_log10(level.max(1e-10));
                    self.input_levels_db[ch] = input_db;

                    let atten = self.process_channel(ch, input_db, hold_samples, thresh);
                    let gain = (1.0 - mix) + mix * fast_pow10(-atten / 20.0);
                    buffer[idx] *= gain;
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
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use sotf_host::*;
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
}
