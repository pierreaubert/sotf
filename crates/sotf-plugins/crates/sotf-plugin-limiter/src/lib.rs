// ============================================================================
// Limiter Plugin
// ============================================================================

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::{find_by_key as pk, limiter::PARAMS as LM};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

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
    pk(LM, "threshold").default_f64() as f32
}
fn default_release_ms() -> f32 {
    pk(LM, "release").default_f64() as f32
}
fn default_lookahead_ms() -> f32 {
    pk(LM, "lookahead").default_f64() as f32
}
fn default_soft() -> bool {
    pk(LM, "soft").default_bool()
}
fn default_mix() -> f32 {
    pk(LM, "mix").default_f64() as f32
}

/// Data exposed by the limiter for UI monitoring
#[derive(Debug, Clone, Default)]
pub struct LimiterData {
    /// Current gain reduction in dB (positive value, e.g., 6.0 means -6dB gain)
    pub gain_reduction_db: f32,
    /// Peak input level in dB
    pub peak_db: f32,
    /// Whether the limiter is actively limiting
    pub is_limiting: bool,
}

const CACHE_UPDATE_THROTTLE: usize = 10;

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
    cache: RealTimeCache<LimiterData>,
    cache_update_counter: usize,
    monitoring_peak_db: f32,
    monitoring_gr_db: f32,
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
            cache: RealTimeCache::new(LimiterData::default()),
            cache_update_counter: 0,
            monitoring_peak_db: -100.0,
            monitoring_gr_db: 0.0,
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
                pk(LM, "threshold").min_f64() as f32,
                pk(LM, "threshold").max_f64() as f32,
            )
            .with_description("Ceiling level (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(LM, "release").min_f64() as f32,
                pk(LM, "release").max_f64() as f32,
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
                pk(LM, "lookahead").default_f64() as f32,
                pk(LM, "lookahead").max_f64() as f32,
            )
            .with_description("Lookahead time for peak detection (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(LM, "mix").min_f64() as f32,
                pk(LM, "mix").max_f64() as f32,
            )
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
            let val = value
                .as_float()
                .unwrap_or(pk(LM, "threshold").default_f64() as f32);
            if val.is_finite() {
                self.threshold_db = val;
                self.threshold_smoother
                    .set_target(fast_pow10(self.threshold_db / 20.0));
            }
        } else if id == self.param_release {
            let val = value
                .as_float()
                .unwrap_or(pk(LM, "release").default_f64() as f32);
            if val.is_finite() {
                self.release_ms = val.max(1.0);
                self.update_coefficients();
            }
        } else if id == self.param_lookahead {
            let val = value
                .as_float()
                .unwrap_or(pk(LM, "lookahead").default_f64() as f32);
            if val.is_finite() {
                self.lookahead_ms = val.max(0.0);
                self.update_coefficients();
            }
        } else if id == self.param_soft {
            self.soft = value.as_bool().unwrap_or(pk(LM, "soft").default_bool());
        } else if id == self.param_mix {
            let val = value
                .as_float()
                .unwrap_or(pk(LM, "mix").default_f64() as f32);
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
        let mut max_peak = 0.0f32;

        for frame in 0..num_frames {
            let mut frame_peak = 0.0f32;
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                frame_peak = frame_peak.max(buffer[idx].abs());
            }

            max_peak = max_peak.max(frame_peak);

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

        // Smoothers already advanced at the start of this block via .advance().
        // Do not call .next_n() again — that would double-advance, making
        // threshold transitions ~500x faster than intended.

        // Update monitoring cache
        self.monitoring_peak_db = 20.0 * fast_log10(max_peak.max(1e-10));
        self.monitoring_gr_db = self.envelope;

        self.cache_update_counter += 1;
        if self.cache_update_counter >= CACHE_UPDATE_THROTTLE {
            self.cache_update_counter = 0;
            self.cache.update(|d| {
                d.gain_reduction_db = self.monitoring_gr_db;
                d.peak_db = self.monitoring_peak_db;
                d.is_limiting = self.monitoring_gr_db > 0.01;
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_len
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
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

    /// Regression: threshold smoother was advanced twice per block (once via
    /// .advance(), then again via .next_n(num_frames)), making transitions
    /// ~500x faster than intended. This test verifies smooth threshold changes.
    #[test]
    fn test_threshold_transition_is_smooth() {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
        p.initialize(48000).unwrap();

        // Feed loud signal to establish steady-state
        let mut b = vec![1.0f32; 4800];
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 4800,
        };
        p.process_in_place(&mut b, &ctx).unwrap();
        let output_before = b[4799];

        // Now change threshold from -6 dB to -20 dB
        p.set_parameter(
            ParameterId::from("threshold"),
            ParameterValue::Float(-20.0),
        )
        .unwrap();

        // Process one small block (≈1ms = 48 samples)
        // With proper 5ms smoothing, the threshold should NOT have fully
        // transitioned after just 1ms.
        let mut b2 = vec![1.0f32; 48];
        p.process_in_place(
            &mut b2,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 48,
            },
        )
        .unwrap();
        let output_after_1ms = b2[47];

        // The new threshold (-20 dB ≈ 0.1) is much lower than old (-6 dB ≈ 0.5).
        // After only 1ms of a 5ms transition, the output should still be
        // closer to the old threshold than the new one.
        let old_thresh_lin = fast_pow10(-6.0 / 20.0); // ≈ 0.50
        let new_thresh_lin = fast_pow10(-20.0 / 20.0); // ≈ 0.10
        let midpoint = (old_thresh_lin + new_thresh_lin) / 2.0;

        assert!(
            output_after_1ms > midpoint,
            "After 1ms of a 5ms threshold transition, output {output_after_1ms:.4} should be above \
             midpoint {midpoint:.4} (old={old_thresh_lin:.4}, new={new_thresh_lin:.4}). \
             Smoother may be double-advancing."
        );
    }

    /// Verify the limiter actually limits output below threshold.
    #[test]
    fn test_limiter_clamps_output() {
        let mut p = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();

        // Feed loud stereo signal (well above -6 dB threshold)
        let mut b = vec![0.0f32; 2048 * 2];
        for frame in 0..2048 {
            let val = 0.9 * (frame as f32 * 0.1).sin(); // ~-1 dBFS sine
            b[frame * 2] = val;
            b[frame * 2 + 1] = val;
        }
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 2048,
        };
        p.process_in_place(&mut b, &ctx).unwrap();

        // After lookahead fills (≈5ms = 240 samples), all output should be
        // below threshold. Allow a small overshoot margin.
        let thresh_lin = fast_pow10(-6.0 / 20.0);
        for frame in 500..2048 {
            for ch in 0..2 {
                let s = b[frame * 2 + ch].abs();
                assert!(
                    s <= thresh_lin * 1.1,
                    "frame {frame} ch {ch}: {s:.4} exceeds threshold {thresh_lin:.4}"
                );
            }
        }
    }
}
