// ============================================================================
// Gate Plugin
// ============================================================================

pub mod params;

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::find_by_key as pk;
use crate::params::PARAMS as GT;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::LookaheadBuffer;
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

const MAX_LOOKAHEAD_MS: f32 = 20.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
    /// Maximum attenuation in dB (0 = unlimited). Caps how much the gate attenuates.
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    /// Hysteresis in dB. Close threshold = threshold - hysteresis.
    #[serde(default)]
    pub hysteresis_db: f32,
    /// Soft knee width in dB (0 = hard knee).
    #[serde(default)]
    pub knee_db: f32,
    /// Lookahead delay in ms (0 = off, max 20ms). Delays audio so gain is computed from non-delayed signal.
    #[serde(default)]
    pub lookahead_ms: f32,
}

fn default_threshold_db() -> f32 {
    pk(GT, "threshold").default_f64() as f32
}
fn default_ratio() -> f32 {
    pk(GT, "ratio").default_f64() as f32
}
fn default_attack_ms() -> f32 {
    pk(GT, "attack").default_f64() as f32
}
fn default_hold_ms() -> f32 {
    pk(GT, "hold").default_f64() as f32
}
fn default_release_ms() -> f32 {
    pk(GT, "release").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(GT, "mix").default_f64() as f32
}
fn default_link_channels() -> bool {
    pk(GT, "link_channels").default_bool()
}
fn default_sidechain_hpf_hz() -> f32 {
    pk(GT, "sidechain_hpf_hz").default_f64() as f32
}
fn default_range_db() -> f32 {
    80.0
}

#[derive(Debug, Clone)]
pub struct GateData {
    pub input_levels_db: Arc<Vec<f32>>,
    pub is_open: bool,
    pub attenuation_db: Arc<Vec<f32>>,
}

impl Default for GateData {
    fn default() -> Self {
        Self {
            input_levels_db: Arc::new(Vec::new()),
            is_open: false,
            attenuation_db: Arc::new(Vec::new()),
        }
    }
}

impl GateData {
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

pub struct GatePlugin {
    channels: usize,
    sample_rate: u32,
    param_threshold: ParameterId,
    threshold_db: f32,
    param_ratio: ParameterId,
    ratio: f32,
    param_attack: ParameterId,
    attack_ms: f32,
    param_hold: ParameterId,
    hold_ms: f32,
    param_release: ParameterId,
    release_ms: f32,
    param_mix: ParameterId,
    mix: f32,
    param_link_channels: ParameterId,
    link_channels: bool,
    param_sidechain_hpf_hz: ParameterId,
    sidechain_hpf_hz: f32,
    param_range_db: ParameterId,
    range_db: f32,
    param_hysteresis_db: ParameterId,
    hysteresis_db: f32,
    param_knee_db: ParameterId,
    knee_db: f32,
    param_lookahead: ParameterId,
    lookahead_ms: f32,
    lookahead_buffers: Vec<LookaheadBuffer>,
    /// Gate state per channel for hysteresis
    gate_open: Vec<bool>,
    hold_counter: Vec<usize>,
    attack_coeff: f32,
    release_coeff: f32,
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    sidechain_hpf_alpha: f32,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    /// Gain reduction envelope in dB (positive value)
    envelope: Vec<f32>,
    /// Instantaneous input levels in dB for monitoring
    monitoring_levels: Vec<f32>,
    cache: RealTimeCache<GateData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

impl GatePlugin {
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Self {
        let sr = 44100;
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_threshold: ParameterId::from("threshold"),
            threshold_db,
            param_ratio: ParameterId::from("ratio"),
            ratio,
            param_attack: ParameterId::from("attack"),
            attack_ms,
            param_hold: ParameterId::from("hold"),
            hold_ms,
            param_release: ParameterId::from("release"),
            release_ms,
            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            param_link_channels: ParameterId::from("link_channels"),
            link_channels: true,
            param_sidechain_hpf_hz: ParameterId::from("sidechain_hpf_hz"),
            sidechain_hpf_hz: 0.0,
            param_range_db: ParameterId::from("range_db"),
            range_db: default_range_db(),
            param_hysteresis_db: ParameterId::from("hysteresis_db"),
            hysteresis_db: 0.0,
            param_knee_db: ParameterId::from("knee_db"),
            knee_db: 0.0,
            param_lookahead: ParameterId::from("lookahead"),
            lookahead_ms: 0.0,
            lookahead_buffers: (0..channels)
                .map(|_| LookaheadBuffer::from_ms(MAX_LOOKAHEAD_MS, sr, 1))
                .collect(),
            gate_open: vec![false; channels],
            envelope: vec![0.0; channels],
            monitoring_levels: vec![-120.0; channels],
            hold_counter: vec![0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            sidechain_hpf_alpha: 0.0,
            threshold_smoother: Smoother::new(threshold_db, 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            cache: RealTimeCache::new(GateData::new(channels)),
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
                pk(GT, "threshold").min_f64() as f32,
                pk(GT, "threshold").max_f64() as f32,
            )
            .with_description("Level below which gating starts (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(GT, "ratio").min_f64() as f32,
                pk(GT, "ratio").max_f64() as f32,
            )
            .with_description("Gate ratio (1:1 to 100:1)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(GT, "attack").min_f64() as f32,
                pk(GT, "attack").max_f64() as f32,
            )
            .with_description("Attack time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "hold",
                "Hold",
                self.hold_ms,
                pk(GT, "hold").min_f64() as f32,
                pk(GT, "hold").max_f64() as f32,
            )
            .with_description("Hold time before closing (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(GT, "release").min_f64() as f32,
                pk(GT, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(GT, "mix").min_f64() as f32,
                pk(GT, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = gated)")
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
                pk(GT, "sidechain_hpf_hz").min_f64() as f32,
                pk(GT, "sidechain_hpf_hz").max_f64() as f32,
            )
            .with_description("High-pass filter frequency for sidechain (Hz)")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("range_db", "Range", self.range_db, 0.0, 120.0)
                .with_description("Maximum attenuation in dB (0 = unlimited)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("hysteresis_db", "Hysteresis", self.hysteresis_db, 0.0, 20.0)
                .with_description("Hysteresis between open/close thresholds (dB)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("knee_db", "Knee", self.knee_db, 0.0, 24.0)
                .with_description("Soft knee width (dB)")
                .with_group("Dynamics")
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
        ];
    }

    pub fn from_params(channels: usize, params: GatePluginParams) -> Self {
        let mut p = Self::new(
            channels,
            params.threshold_db,
            params.ratio,
            params.attack_ms,
            params.hold_ms,
            params.release_ms,
        );
        p.mix = params.mix.clamp(0.0, 1.0);
        p.link_channels = params.link_channels;
        p.sidechain_hpf_hz = params.sidechain_hpf_hz.max(0.0);
        p.range_db = params.range_db.max(0.0);
        p.hysteresis_db = params.hysteresis_db.max(0.0);
        p.knee_db = params.knee_db.max(0.0);
        p.lookahead_ms = params.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS);
        p.update_lookahead_delay();
        p
    }

    fn update_lookahead_delay(&mut self) {
        for buf in &mut self.lookahead_buffers {
            buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
        }
    }

    fn calculate_gate_attenuation(&self, input_db: f32, threshold: f32) -> f32 {
        let knee = self.knee_db.max(0.0);
        let slope = 1.0 - 1.0 / self.ratio.max(1.0);

        let atten = if knee < 0.1 {
            // Hard knee
            if input_db >= threshold {
                0.0
            } else {
                (threshold - input_db) * slope
            }
        } else if input_db > threshold + knee / 2.0 {
            // Above knee zone — no attenuation
            0.0
        } else if input_db < threshold - knee / 2.0 {
            // Below knee zone — full gate
            (threshold - input_db) * slope
        } else {
            // Within knee zone — quadratic transition (ported from expander)
            let below = threshold + knee / 2.0 - input_db;
            let kf = below / knee;
            kf * kf * (knee / 2.0) * slope
        };

        // Cap attenuation at range_db
        atten.min(self.range_db.max(0.0))
    }

    /// Check if the gate should be open for a given channel, using hysteresis.
    fn should_gate_open(&self, input_db: f32, threshold: f32, ch: usize) -> bool {
        if self.hysteresis_db <= 0.0 {
            return input_db >= threshold;
        }
        let close_threshold = threshold - self.hysteresis_db;
        if self.gate_open[ch] {
            // Gate is open — only close if below close threshold
            input_db >= close_threshold
        } else {
            // Gate is closed — only open if above open threshold
            input_db >= threshold
        }
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
}

impl InPlacePlugin for GatePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gate", "1.1.0", "SotF")
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
                .unwrap_or(pk(GT, "threshold").default_f64() as f32);
            if v.is_finite() {
                self.threshold_db = v;
                self.threshold_smoother.set_target(self.threshold_db);
            }
        } else if id == self.param_ratio {
            let v = value
                .as_float()
                .unwrap_or(pk(GT, "ratio").default_f64() as f32);
            if v.is_finite() {
                self.ratio = v.max(1.0);
            }
        } else if id == self.param_attack {
            let v = value
                .as_float()
                .unwrap_or(pk(GT, "attack").default_f64() as f32);
            if v.is_finite() {
                self.attack_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_hold {
            let v = value
                .as_float()
                .unwrap_or(pk(GT, "hold").default_f64() as f32);
            if v.is_finite() {
                self.hold_ms = v;
            }
        } else if id == self.param_release {
            let v = value
                .as_float()
                .unwrap_or(pk(GT, "release").default_f64() as f32);
            if v.is_finite() {
                self.release_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_mix {
            let v = value
                .as_float()
                .unwrap_or(pk(GT, "mix").default_f64() as f32);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if id == self.param_link_channels {
            self.link_channels = value
                .as_bool()
                .unwrap_or(pk(GT, "link_channels").default_bool());
        } else if id == self.param_sidechain_hpf_hz {
            let v = value
                .as_float()
                .unwrap_or(pk(GT, "sidechain_hpf_hz").default_f64() as f32);
            if v.is_finite() {
                self.sidechain_hpf_hz = v.max(0.0);
                self.update_coefficients();
            }
        } else if id == self.param_range_db {
            if let Some(v) = value.as_float() {
                if v.is_finite() {
                    self.range_db = v.max(0.0);
                }
            }
        } else if id == self.param_hysteresis_db {
            if let Some(v) = value.as_float() {
                if v.is_finite() {
                    self.hysteresis_db = v.max(0.0);
                }
            }
        } else if id == self.param_knee_db {
            if let Some(v) = value.as_float() {
                if v.is_finite() {
                    self.knee_db = v.max(0.0);
                }
            }
        } else if id == self.param_lookahead {
            let v = value.as_float().unwrap_or(0.0);
            if v.is_finite() {
                self.lookahead_ms = v.clamp(0.0, MAX_LOOKAHEAD_MS);
                self.update_lookahead_delay();
            }
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
        } else if id == &self.param_hold {
            Some(ParameterValue::Float(self.hold_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if id == &self.param_link_channels {
            Some(ParameterValue::Bool(self.link_channels))
        } else if id == &self.param_sidechain_hpf_hz {
            Some(ParameterValue::Float(self.sidechain_hpf_hz))
        } else if id == &self.param_range_db {
            Some(ParameterValue::Float(self.range_db))
        } else if id == &self.param_hysteresis_db {
            Some(ParameterValue::Float(self.hysteresis_db))
        } else if id == &self.param_knee_db {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_lookahead {
            Some(ParameterValue::Float(self.lookahead_ms))
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
        Ok(())
    }
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.hold_counter.fill(0);
        self.gate_open.fill(false);
        self.sidechain_hpf_prev_input.fill(0.0);
        self.sidechain_hpf_prev_output.fill(0.0);
        for buf in &mut self.lookahead_buffers {
            buf.reset();
        }
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let hs = (self.hold_ms * 0.001 * self.sample_rate as f32) as usize;
        let use_lookahead = self.lookahead_ms > 0.0;

        // Block-based smoothing: advance once per block
        let thresh = self.threshold_smoother.next_n(num_frames);
        let mix = self.mix_smoother.next_n(num_frames);

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let mut det = 0.0f32;
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[idx]);
                    let level = filtered.abs();
                    det = det.max(level);
                    // Update monitoring
                    self.monitoring_levels[ch] = 20.0 * fast_log10(level.max(1e-10));
                }

                let idb = 20.0 * fast_log10(det.max(1e-10));
                let atten_target = self.calculate_gate_attenuation(idb, thresh);

                // Detection with hysteresis (channel 0 is master for linked)
                let is_open = self.should_gate_open(idb, thresh, 0);
                self.gate_open[0] = is_open;
                let target = if is_open {
                    self.hold_counter[0] = hs;
                    0.0
                } else if self.hold_counter[0] > 0 {
                    self.hold_counter[0] -= 1;
                    0.0
                } else {
                    atten_target
                };

                let coeff = if target > self.envelope[0] {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };
                self.envelope[0] = target + coeff * (self.envelope[0] - target);
                let gain = (1.0 - mix) + mix * fast_pow10(-self.envelope[0] / 20.0);

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
                    let level_abs = filtered.abs();
                    self.monitoring_levels[ch] = 20.0 * fast_log10(level_abs.max(1e-10));
                    let idb = self.monitoring_levels[ch];
                    let atten_target = self.calculate_gate_attenuation(idb, thresh);

                    let is_open = self.should_gate_open(idb, thresh, ch);
                    self.gate_open[ch] = is_open;
                    let target = if is_open {
                        self.hold_counter[ch] = hs;
                        0.0
                    } else if self.hold_counter[ch] > 0 {
                        self.hold_counter[ch] -= 1;
                        0.0
                    } else {
                        atten_target
                    };

                    let coeff = if target > self.envelope[ch] {
                        self.attack_coeff
                    } else {
                        self.release_coeff
                    };
                    self.envelope[ch] = target + coeff * (self.envelope[ch] - target);
                    let gain = (1.0 - mix) + mix * fast_pow10(-self.envelope[ch] / 20.0);
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
            let is_open = self.envelope.iter().any(|&a| a < 0.1);
            if self.link_channels {
                self.monitoring_levels.fill(self.envelope[0]);
            } else {
                self.monitoring_levels.copy_from_slice(&self.envelope);
            }
            self.cache.update(|d| {
                d.update(is_open, &self.monitoring_levels);
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
    fn test_gate_basic() {
        let mut p = GatePlugin::new(1, -20.0, 100.0, 1.0, 10.0, 50.0);
        p.initialize(48000).unwrap();
        let mut b = vec![0.05; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999] < 0.05);
    }

    /// Sidechain HPF at 200 Hz: a 50 Hz signal below threshold should NOT open
    /// the gate (HPF filters out the low-freq detection signal). A 1 kHz signal
    /// at the same level should open it.
    #[test]
    fn test_sidechain_hpf_filters_low_freq_detection() {
        let sr = 48000u32;
        let threshold_db = -20.0;
        // Signal amplitude is above threshold in raw dB but below after HPF
        let amplitude = 10.0_f32.powf(-15.0 / 20.0); // -15 dBFS (above -20 threshold)

        // --- Test 1: 50 Hz signal with HPF=200 Hz. Gate should stay closed. ---
        let mut p_low = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db,
                ratio: 100.0,
                attack_ms: 1.0,
                hold_ms: 0.0,
                release_ms: 10.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 200.0,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p_low.initialize(sr).unwrap();

        let num_frames = 9600; // 200ms
        let mut buf_low = vec![0.0f32; num_frames];
        for i in 0..num_frames {
            buf_low[i] =
                amplitude * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr as f32).sin();
        }

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        p_low.process_in_place(&mut buf_low, &ctx).unwrap();

        // The 50 Hz signal should be significantly attenuated because the HPF
        // at 200 Hz filters out the 50 Hz from the sidechain detection.
        let rms_low: f32 = buf_low[4800..].iter().map(|x| x * x).sum::<f32>()
            / (num_frames - 4800) as f32;
        let rms_low = rms_low.sqrt();

        // --- Test 2: 1 kHz signal with HPF=200 Hz. Gate should open. ---
        let mut p_high = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db,
                ratio: 100.0,
                attack_ms: 1.0,
                hold_ms: 0.0,
                release_ms: 10.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 200.0,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p_high.initialize(sr).unwrap();

        let mut buf_high = vec![0.0f32; num_frames];
        for i in 0..num_frames {
            buf_high[i] =
                amplitude * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin();
        }

        p_high.process_in_place(&mut buf_high, &ctx).unwrap();

        let rms_high: f32 = buf_high[4800..].iter().map(|x| x * x).sum::<f32>()
            / (num_frames - 4800) as f32;
        let rms_high = rms_high.sqrt();

        // 1 kHz should pass through much louder than 50 Hz (gate open vs closed)
        assert!(
            rms_high > rms_low * 2.0,
            "1kHz (RMS={rms_high:.5}) should pass through gate much louder than 50Hz (RMS={rms_low:.5}) \
             when sidechain HPF=200Hz"
        );
    }

    /// Hysteresis test: a signal that oscillates ±2 dB around the threshold should
    /// not cause the gate to "chatter" (rapidly open and close).
    ///
    /// Setup:
    ///   threshold = -20 dB, hysteresis = 4 dB
    ///   → open threshold  = -20 dB
    ///   → close threshold = -24 dB
    ///
    /// The test signal alternates every 100 samples between -18 dBFS and -22 dBFS.
    /// Both levels are between -24 dB and -20 dB when the gate is open, so once
    /// opened the gate should remain open for the entire alternating region.
    ///
    /// Without hysteresis the gate would open on -18 dB and close on -22 dB every
    /// 100-sample segment, producing many transitions.  With hysteresis it should
    /// stay open after the first opening.
    #[test]
    fn test_gate_hysteresis_no_chatter() {
        let sr = 48000u32;
        // Fast attack/release so the envelope reacts within the 100-sample segments
        let mut p = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db: -20.0,
                hysteresis_db: 4.0,
                ratio: 100.0,
                attack_ms: 0.5,
                hold_ms: 0.0,
                release_ms: 1.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 0.0,
                range_db: 80.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p.initialize(sr).unwrap();

        // Build 1-second buffer that alternates every 100 samples between
        // -18 dBFS (above open threshold -20 dB) and -22 dBFS (between open and
        // close thresholds, so gate should stay open once opened).
        let amp_high = 10.0_f32.powf(-18.0 / 20.0); // -18 dBFS
        let amp_low = 10.0_f32.powf(-22.0 / 20.0);  // -22 dBFS  (above close threshold -24 dB)
        let num_frames = sr as usize; // 1 second
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| {
                if (i / 100) % 2 == 0 {
                    amp_high
                } else {
                    amp_low
                }
            })
            .collect();

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        p.process_in_place(&mut buffer, &ctx).unwrap();

        // Count how many times the output crosses a "gate closed" boundary.
        // If the gate chatters, the output will swing between near-zero and amp_low
        // each 100-sample segment.  With hysteresis the output should be consistently
        // passed through after the initial opening.
        //
        // Threshold for "effectively gated": output below 10 % of amp_low.
        let closed_threshold = amp_low * 0.1;

        // Skip the first 500 samples (attack / settling period).
        let steady_state = &buffer[500..];

        // Count sign-changes between "open" and "closed" state.
        let mut transitions = 0usize;
        let mut prev_open = steady_state[0] > closed_threshold;
        for &s in steady_state.iter().skip(1) {
            let cur_open = s > closed_threshold;
            if cur_open != prev_open {
                transitions += 1;
                prev_open = cur_open;
            }
        }

        // With hysteresis the gate should open once and stay open: 0 or at most 1
        // transition (the initial opening) throughout the steady-state region.
        // Without hysteresis we would expect ~2 * (num_frames / 100) ≈ 190 transitions.
        assert!(
            transitions <= 2,
            "Gate with hysteresis=4dB should not chatter on a ±2dB oscillating signal, \
             but observed {transitions} open/closed transitions in steady-state"
        );
    }
}
