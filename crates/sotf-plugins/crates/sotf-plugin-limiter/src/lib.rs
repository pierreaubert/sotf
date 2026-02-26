// ============================================================================
// Limiter Plugin
// ============================================================================

use sotf_host::param_specs::limiter::*;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimiterPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
    #[serde(default = "default_soft")]
    pub soft: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_threshold_db() -> f32 {
    THRESHOLD_DEFAULT
}
fn default_release_ms() -> f32 {
    RELEASE_DEFAULT
}
fn default_lookahead_ms() -> f32 {
    LOOKAHEAD_DEFAULT
}
fn default_soft() -> bool {
    SOFT_DEFAULT
}
fn default_mix() -> f32 {
    MIX_DEFAULT
}

pub struct LimiterPlugin {
    channels: usize,
    sample_rate: u32,
    param_threshold: ParameterId,
    threshold_db: f32,
    param_release: ParameterId,
    release_ms: f32,
    param_lookahead: ParameterId,
    lookahead_ms: f32,
    param_soft: ParameterId,
    soft: bool,
    param_mix: ParameterId,
    mix: f32,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    envelope: f32,
    release_coeff: f32,
    lookahead_buffer: Vec<f32>,
    lookahead_pos: usize,
    lookahead_len: usize,
    cached_parameters: Vec<Parameter>,
}

impl LimiterPlugin {
    pub fn new(
        channels: usize,
        threshold_db: f32,
        release_ms: f32,
        lookahead_ms: f32,
        soft: bool,
    ) -> Self {
        let sr = 44100;
        let lookahead_len = ((lookahead_ms * 0.001 * sr as f32) as usize).max(1);
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_threshold: ParameterId::from("threshold"),
            threshold_db,
            param_release: ParameterId::from("release"),
            release_ms,
            param_lookahead: ParameterId::from("lookahead"),
            lookahead_ms,
            param_soft: ParameterId::from("soft"),
            soft,
            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            threshold_smoother: Smoother::new(fast_pow10(threshold_db / 20.0), 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            envelope: 0.0,
            release_coeff: 0.0,
            lookahead_buffer: vec![0.0; lookahead_len * channels],
            lookahead_pos: 0,
            lookahead_len,
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
            .with_description("Ceiling level (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
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
            Parameter::new_bool("soft", "Soft", self.soft)
                .with_description("Use soft clipping instead of hard limiting")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "lookahead",
                "Lookahead",
                self.lookahead_ms,
                LOOKAHEAD_DEFAULT,
                LOOKAHEAD_MAX,
            )
            .with_description("Lookahead time for peak detection (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float("mix", "Mix", self.mix, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = limited)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    pub fn from_params(channels: usize, params: LimiterPluginParams) -> Self {
        let mut p = Self::new(
            channels,
            params.threshold_db,
            params.release_ms,
            params.lookahead_ms,
            params.soft,
        );
        p.mix = params.mix.clamp(0.0, 1.0);
        p
    }

    fn update_coefficients(&mut self) {
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();
        let new_len = ((self.lookahead_ms * 0.001 * self.sample_rate as f32) as usize).max(1);
        if new_len != self.lookahead_len {
            self.lookahead_len = new_len;
            self.lookahead_buffer.resize(new_len * self.channels, 0.0);
            self.lookahead_pos = 0;
        }
    }
}

impl InPlacePlugin for LimiterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Limiter", "1.1.0", "SotF")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Validate against parameter definitions
        let params = self.parameters();
        if let Some(param) = params.iter().find(|p| p.id == id) {
            param.validate(&value)?;
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }

        if id == self.param_threshold {
            let val = value.as_float().unwrap_or(THRESHOLD_DEFAULT);
            if val.is_finite() {
                self.threshold_db = val;
                self.threshold_smoother
                    .set_target(fast_pow10(self.threshold_db / 20.0));
            }
        } else if id == self.param_release {
            let val = value.as_float().unwrap_or(RELEASE_DEFAULT);
            if val.is_finite() {
                self.release_ms = val.max(1.0);
                self.update_coefficients();
            }
        } else if id == self.param_lookahead {
            let val = value.as_float().unwrap_or(LOOKAHEAD_DEFAULT);
            if val.is_finite() {
                self.lookahead_ms = val.max(0.0);
                self.update_coefficients();
            }
        } else if id == self.param_soft {
            self.soft = value.as_bool().unwrap_or(SOFT_DEFAULT);
        } else if id == self.param_mix {
            let val = value.as_float().unwrap_or(MIX_DEFAULT);
            if val.is_finite() {
                self.mix = val.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_soft {
            Some(ParameterValue::Bool(self.soft))
        } else if id == &self.param_lookahead {
            Some(ParameterValue::Float(self.lookahead_ms))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
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
        self.envelope = 0.0;
        self.lookahead_buffer.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let thresh = self.threshold_smoother.advance();
        let mix = self.mix_smoother.advance();

        for frame in 0..num_frames {
            let mut frame_peak = 0.0f32;
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                frame_peak = frame_peak.max(buffer[idx].abs());
            }

            // Predictive peak from input
            let target_gr = if frame_peak > thresh {
                20.0 * fast_log10(frame_peak / thresh)
            } else {
                0.0
            };

            // Instant attack, smoothed release
            if target_gr > self.envelope {
                self.envelope = target_gr;
            } else {
                self.envelope = target_gr + self.release_coeff * (self.envelope - target_gr);
            }

            let gain = fast_pow10(-self.envelope / 20.0);

            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let input_sample = buffer[idx];

                let buf_idx = self.lookahead_pos * self.channels + ch;
                let delayed = self.lookahead_buffer[buf_idx];
                self.lookahead_buffer[buf_idx] = input_sample;

                let wet = if self.soft {
                    // Soft knee using algebraic curve above 0.9*threshold
                    // Curve: y = limit_start + overshoot / sqrt(1 + (overshoot/limit_width)^2)
                    let signal = delayed * gain;
                    let abs_s = signal.abs();
                    let soft_start = thresh * 0.9;
                    if abs_s > soft_start {
                        let overshoot = abs_s - soft_start;
                        let limit_width = thresh * 0.1;
                        let limited = soft_start
                            + overshoot / (1.0 + (overshoot / limit_width).powi(2)).sqrt();
                        limited * signal.signum()
                    } else {
                        signal
                    }
                } else {
                    (delayed * gain).clamp(-thresh, thresh)
                };

                buffer[idx] = (1.0 - mix) * delayed + mix * wet;
            }
            self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;
        }

        self.threshold_smoother.next_n(num_frames);
        self.mix_smoother.next_n(num_frames);

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_len
    }
}

#[cfg(test)]
mod tests {
    use sotf_host::*;
    use crate::*;
    #[test]
    fn test_limiter_basic() {
        let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();
        let mut b = vec![2.0; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        let thresh_lin = fast_pow10(-1.0 / 20.0);
        for &s in &b[500..] {
            assert!(s.abs() <= thresh_lin * 1.05);
        }
    }
}
