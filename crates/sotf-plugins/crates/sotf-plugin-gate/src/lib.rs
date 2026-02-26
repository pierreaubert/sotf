// ============================================================================
// Gate Plugin
// ============================================================================

use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::gate::*;
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
}

fn default_threshold_db() -> f32 {
    THRESHOLD_DEFAULT
}
fn default_ratio() -> f32 {
    RATIO_DEFAULT
}
fn default_attack_ms() -> f32 {
    ATTACK_DEFAULT
}
fn default_hold_ms() -> f32 {
    HOLD_DEFAULT
}
fn default_release_ms() -> f32 {
    RELEASE_DEFAULT
}
fn default_mix() -> f32 {
    MIX_DEFAULT
}
fn default_link_channels() -> bool {
    LINK_CHANNELS_DEFAULT
}
fn default_sidechain_hpf_hz() -> f32 {
    SIDECHAIN_HPF_HZ_DEFAULT
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
                THRESHOLD_MIN,
                THRESHOLD_MAX,
            )
            .with_description("Level below which gating starts (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("ratio", "Ratio", self.ratio, RATIO_MIN, RATIO_MAX)
                .with_description("Gate ratio (1:1 to 100:1)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("attack", "Attack", self.attack_ms, ATTACK_MIN, ATTACK_MAX)
                .with_description("Attack time (ms)")
                .with_group("Timing")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("hold", "Hold", self.hold_ms, HOLD_MIN, HOLD_MAX)
                .with_description("Hold time before closing (ms)")
                .with_group("Timing")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                RELEASE_MIN,
                RELEASE_MAX,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("mix", "Mix", self.mix, MIX_MIN, MIX_MAX)
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
                SIDECHAIN_HPF_HZ_MIN,
                SIDECHAIN_HPF_HZ_MAX,
            )
            .with_description("High-pass filter frequency for sidechain (Hz)")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::FineTuning),
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
        p
    }

    fn calculate_gate_attenuation(&self, input_db: f32, threshold: f32) -> f32 {
        if input_db >= threshold {
            0.0
        } else {
            (threshold - input_db) * (1.0 - 1.0 / self.ratio.max(1.0))
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
            let v = value.as_float().unwrap_or(THRESHOLD_DEFAULT);
            if v.is_finite() {
                self.threshold_db = v;
                self.threshold_smoother.set_target(self.threshold_db);
            }
        } else if id == self.param_ratio {
            let v = value.as_float().unwrap_or(RATIO_DEFAULT);
            if v.is_finite() {
                self.ratio = v.max(1.0);
            }
        } else if id == self.param_attack {
            let v = value.as_float().unwrap_or(ATTACK_DEFAULT);
            if v.is_finite() {
                self.attack_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_hold {
            let v = value.as_float().unwrap_or(HOLD_DEFAULT);
            if v.is_finite() {
                self.hold_ms = v;
            }
        } else if id == self.param_release {
            let v = value.as_float().unwrap_or(RELEASE_DEFAULT);
            if v.is_finite() {
                self.release_ms = v;
                self.update_coefficients();
            }
        } else if id == self.param_mix {
            let v = value.as_float().unwrap_or(MIX_DEFAULT);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if id == self.param_link_channels {
            self.link_channels = value.as_bool().unwrap_or(LINK_CHANNELS_DEFAULT);
        } else if id == self.param_sidechain_hpf_hz {
            let v = value.as_float().unwrap_or(SIDECHAIN_HPF_HZ_DEFAULT);
            if v.is_finite() {
                self.sidechain_hpf_hz = v.max(0.0);
                self.update_coefficients();
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
        let hs = (self.hold_ms * 0.001 * self.sample_rate as f32) as usize;

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

                // Detection logic (channel 0 is used as master for linked)
                let target = if idb >= thresh {
                    self.hold_counter[0] = hs;
                    0.0
                } else if self.hold_counter[0] > 0 {
                    self.hold_counter[0] -= 1;
                    0.0
                } else {
                    atten_target
                };

                let coeff = if target > self.envelope[0] {
                    self.release_coeff
                } else {
                    self.attack_coeff
                };
                self.envelope[0] = target + coeff * (self.envelope[0] - target);
                let gain = (1.0 - mix) + mix * fast_pow10(-self.envelope[0] / 20.0);

                for ch in 0..self.channels {
                    buffer[frame * self.channels + ch] *= gain;
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

                    let target = if idb >= thresh {
                        self.hold_counter[ch] = hs;
                        0.0
                    } else if self.hold_counter[ch] > 0 {
                        self.hold_counter[ch] -= 1;
                        0.0
                    } else {
                        atten_target
                    };

                    let coeff = if target > self.envelope[ch] {
                        self.release_coeff
                    } else {
                        self.attack_coeff
                    };
                    self.envelope[ch] = target + coeff * (self.envelope[ch] - target);
                    let gain = (1.0 - mix) + mix * fast_pow10(-self.envelope[ch] / 20.0);
                    buffer[idx] *= gain;
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
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use sotf_host::*;
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
}
