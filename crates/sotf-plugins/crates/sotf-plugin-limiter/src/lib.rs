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
use sotf_host::{DualRelease, TruePeakDetector};
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
    #[serde(default = "default_true_peak")]
    pub true_peak: bool,
    #[serde(default = "default_dual_release")]
    pub dual_release: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default)]
    pub feed_forward: bool,
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
fn default_true_peak() -> bool {
    pk(LM, "true_peak").default_bool()
}
fn default_dual_release() -> bool {
    pk(LM, "dual_release").default_bool()
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
    /// Per-channel inter-sample true peak in dBTP (empty when true_peak is disabled)
    pub isp_dbtp: Vec<f32>,
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
    param_true_peak: ParameterId,
    true_peak: bool,
    param_dual_release: ParameterId,
    dual_release: bool,
    param_mix: ParameterId,
    mix: f32,
    param_feed_forward: ParameterId,
    feed_forward: bool,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    envelope: f32,
    release_coeff: f32,
    lookahead_buffer: Vec<f32>,
    lookahead_pos: usize,
    lookahead_len: usize,
    true_peak_detectors: Vec<TruePeakDetector>,
    dual_release_env: DualRelease,
    cached_parameters: Vec<Parameter>,
    cache: RealTimeCache<LimiterData>,
    cache_update_counter: usize,
    monitoring_peak_db: f32,
    monitoring_gr_db: f32,
    /// Per-channel ISP (inter-sample true peak) in linear, tracked across blocks
    monitoring_isp_linear: Vec<f32>,
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
            param_true_peak: ParameterId::from("true_peak"),
            true_peak: false,
            param_dual_release: ParameterId::from("dual_release"),
            dual_release: false,
            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            param_feed_forward: ParameterId::from("feed_forward"),
            feed_forward: false,
            threshold_smoother: Smoother::new(fast_pow10(threshold_db / 20.0), 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            envelope: 0.0,
            release_coeff: 0.0,
            lookahead_buffer: vec![0.0; lookahead_len * channels],
            lookahead_pos: 0,
            lookahead_len,
            true_peak_detectors: (0..channels).map(|_| TruePeakDetector::new()).collect(),
            dual_release_env: DualRelease::new(release_ms, release_ms * 5.0, sr),
            cached_parameters: Vec::new(),
            cache: RealTimeCache::new(LimiterData::default()),
            cache_update_counter: 0,
            monitoring_peak_db: -100.0,
            monitoring_gr_db: 0.0,
            monitoring_isp_linear: vec![0.0; channels],
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
            Parameter::new_bool("true_peak", "True Peak", self.true_peak)
                .with_description("Use 4x oversampled true peak detection")
                .with_group("Detection")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("dual_release", "Dual Release", self.dual_release)
                .with_description("Program-dependent fast/slow release")
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
            Parameter::new_bool("feed_forward", "Feed Forward", self.feed_forward)
                .with_description("Scan lookahead buffer for anticipatory gain reduction")
                .with_group("Detection")
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
        p.true_peak = params.true_peak;
        p.dual_release = params.dual_release;
        p.mix = params.mix.clamp(0.0, 1.0);
        p.feed_forward = params.feed_forward;
        p.rebuild_cached_parameters();
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
        self.dual_release_env.set_times(
            self.release_ms,
            self.release_ms * 5.0,
            self.sample_rate,
        );
    }
}

impl InPlacePlugin for LimiterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Limiter", "1.2.0", "SotF")
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
        } else if id == self.param_true_peak {
            self.true_peak = value
                .as_bool()
                .unwrap_or(pk(LM, "true_peak").default_bool());
        } else if id == self.param_dual_release {
            self.dual_release = value
                .as_bool()
                .unwrap_or(pk(LM, "dual_release").default_bool());
        } else if id == self.param_mix {
            let val = value
                .as_float()
                .unwrap_or(pk(LM, "mix").default_f64() as f32);
            if val.is_finite() {
                self.mix = val.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        } else if id == self.param_feed_forward {
            self.feed_forward = value.as_bool().unwrap_or(false);
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
        } else if id == &self.param_true_peak {
            Some(ParameterValue::Bool(self.true_peak))
        } else if id == &self.param_dual_release {
            Some(ParameterValue::Bool(self.dual_release))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if id == &self.param_feed_forward {
            Some(ParameterValue::Bool(self.feed_forward))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        // Resize true peak detectors if channel count changed
        self.true_peak_detectors
            .resize_with(self.channels, TruePeakDetector::new);
        self.dual_release_env = DualRelease::new(
            self.release_ms,
            self.release_ms * 5.0,
            sample_rate,
        );
        Ok(())
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.lookahead_buffer.fill(0.0);
        for det in &mut self.true_peak_detectors {
            det.reset();
        }
        self.dual_release_env.reset();
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
        let use_true_peak = self.true_peak;
        let use_dual_release = self.dual_release;
        let use_feed_forward = self.feed_forward && self.lookahead_len > 1;

        // Reset per-block ISP tracking
        if use_true_peak {
            self.monitoring_isp_linear
                .resize(self.channels, 0.0);
            self.monitoring_isp_linear.fill(0.0);
        }

        for frame in 0..num_frames {
            let mut frame_peak = 0.0f32;
            if use_true_peak {
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let tp = self.true_peak_detectors[ch].process_linear(buffer[idx]);
                    frame_peak = frame_peak.max(tp);
                    // Track per-channel ISP
                    if tp > self.monitoring_isp_linear[ch] {
                        self.monitoring_isp_linear[ch] = tp;
                    }
                }
            } else {
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    frame_peak = frame_peak.max(buffer[idx].abs());
                }
            }

            max_peak = max_peak.max(frame_peak);

            // Feed-forward: scan the entire lookahead buffer for the maximum
            // upcoming peak, then use that to compute gain reduction.
            // This anticipates loud transients before they arrive at the output.
            let effective_peak = if use_feed_forward {
                let mut la_peak = frame_peak;
                for pos in 0..self.lookahead_len {
                    let base = pos * self.channels;
                    for ch in 0..self.channels {
                        la_peak = la_peak.max(self.lookahead_buffer[base + ch].abs());
                    }
                }
                la_peak
            } else {
                frame_peak
            };

            // Predictive peak from input (or lookahead scan)
            let target_gr = if effective_peak > thresh {
                20.0 * fast_log10(effective_peak / thresh)
            } else {
                0.0
            };

            // Instant attack, smoothed release
            if target_gr > self.envelope {
                self.envelope = target_gr;
            } else {
                let rc = if use_dual_release {
                    self.dual_release_env.process(self.envelope)
                } else {
                    self.release_coeff
                };
                self.envelope = target_gr + rc * (self.envelope - target_gr);
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
                if use_true_peak {
                    d.isp_dbtp.resize(self.channels, -120.0);
                    for (ch, &lin) in self.monitoring_isp_linear.iter().enumerate() {
                        d.isp_dbtp[ch] = if lin < 1e-12 {
                            -120.0
                        } else {
                            20.0 * lin.log10()
                        };
                    }
                } else {
                    d.isp_dbtp.clear();
                }
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        if self.lookahead_ms > 0.0 {
            self.lookahead_len
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
        let _output_before = b[4799];

        // Now change threshold from -6 dB to -20 dB
        p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-20.0))
            .unwrap();

        // Process one small block (=1ms = 48 samples)
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

        // The new threshold (-20 dB = 0.1) is much lower than old (-6 dB = 0.5).
        // After only 1ms of a 5ms transition, the output should still be
        // closer to the old threshold than the new one.
        let old_thresh_lin = fast_pow10(-6.0 / 20.0); // = 0.50
        let new_thresh_lin = fast_pow10(-20.0 / 20.0); // = 0.10
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

        // After lookahead fills (=5ms = 240 samples), all output should be
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

    /// Test that true peak detection catches inter-sample peaks.
    #[test]
    fn test_true_peak_detection() {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p.true_peak = true;
        p.rebuild_cached_parameters();
        p.initialize(48000).unwrap();

        // Create a signal with inter-sample peaks: alternating +0.8/-0.8
        // at Nyquist causes overshoots between samples
        let frames = 2048;
        let mut b = vec![0.0f32; frames];
        for i in 0..frames {
            b[i] = if i % 2 == 0 { 0.8 } else { -0.8 };
        }
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: frames,
        };
        p.process_in_place(&mut b, &ctx).unwrap();

        // With true peak, the limiter should detect the inter-sample overshoot
        // and apply more gain reduction than sample-peak would.
        // Verify the output is still limited.
        let thresh_lin = fast_pow10(-6.0 / 20.0);
        for &s in &b[500..] {
            assert!(
                s.abs() <= thresh_lin * 1.15,
                "true peak: sample {s:.4} exceeds threshold {thresh_lin:.4}"
            );
        }
    }

    /// Test that true peak parameter can be set via set_parameter.
    #[test]
    fn test_true_peak_parameter() {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();
        assert!(!p.true_peak);

        p.set_parameter(
            ParameterId::from("true_peak"),
            ParameterValue::Bool(true),
        )
        .unwrap();
        assert!(p.true_peak);

        let val = p.get_parameter(&ParameterId::from("true_peak"));
        assert_eq!(val, Some(ParameterValue::Bool(true)));
    }

    /// Test that dual release parameter can be set via set_parameter.
    #[test]
    fn test_dual_release_parameter() {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();
        assert!(!p.dual_release);

        p.set_parameter(
            ParameterId::from("dual_release"),
            ParameterValue::Bool(true),
        )
        .unwrap();
        assert!(p.dual_release);

        let val = p.get_parameter(&ParameterId::from("dual_release"));
        assert_eq!(val, Some(ParameterValue::Bool(true)));
    }

    /// Test that dual release still limits correctly.
    #[test]
    fn test_dual_release_limits() {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p.dual_release = true;
        p.rebuild_cached_parameters();
        p.initialize(48000).unwrap();

        let frames = 4096;
        let mut b = vec![0.0f32; frames];
        for i in 0..frames {
            b[i] = 0.9 * (i as f32 * 0.1).sin();
        }
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: frames,
        };
        p.process_in_place(&mut b, &ctx).unwrap();

        let thresh_lin = fast_pow10(-6.0 / 20.0);
        for &s in &b[500..] {
            assert!(
                s.abs() <= thresh_lin * 1.1,
                "dual release: sample {s:.4} exceeds threshold {thresh_lin:.4}"
            );
        }
    }

    /// Test from_params wires true_peak and dual_release correctly.
    #[test]
    fn test_from_params_new_fields() {
        let params = LimiterPluginParams {
            threshold_db: -3.0,
            release_ms: 100.0,
            lookahead_ms: 10.0,
            soft: true,
            true_peak: true,
            dual_release: true,
            mix: 0.8,
            feed_forward: true,
        };
        let p = LimiterPlugin::from_params(2, params);
        assert!(p.true_peak);
        assert!(p.dual_release);
        assert!(p.feed_forward);
        assert_eq!(p.mix, 0.8);

        let tp_val = p.get_parameter(&ParameterId::from("true_peak"));
        assert_eq!(tp_val, Some(ParameterValue::Bool(true)));
        let dr_val = p.get_parameter(&ParameterId::from("dual_release"));
        assert_eq!(dr_val, Some(ParameterValue::Bool(true)));
    }

    /// Ceiling and mix parameters: with mix=0.0, output should be the dry
    /// (delayed) signal unchanged. With mix=1.0, output should be limited.
    #[test]
    fn test_limiter_mix_parameter() {
        let sr = 48000u32;

        // Create limiter with mix=0 set via parameter after init (so smoother starts at 0)
        let mut p_dry = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p_dry.initialize(sr).unwrap();
        p_dry
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
            .unwrap();

        let mut p_wet = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p_wet.initialize(sr).unwrap();

        // Process a warmup block to let the mix smoother converge to 0
        let warmup = 4800; // 100ms
        let mut warmup_buf_dry = vec![0.0f32; warmup];
        let mut warmup_buf_wet = vec![0.0f32; warmup];
        let warmup_ctx = ProcessContext {
            sample_rate: sr,
            num_frames: warmup,
        };
        p_dry
            .process_in_place(&mut warmup_buf_dry, &warmup_ctx)
            .unwrap();
        p_wet
            .process_in_place(&mut warmup_buf_wet, &warmup_ctx)
            .unwrap();

        let num_frames = 4096;
        let make_signal = || {
            let mut buf = vec![0.0f32; num_frames];
            for i in 0..num_frames {
                buf[i] = 0.9 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin();
            }
            buf
        };
        let mut buf_dry = make_signal();
        let mut buf_wet = make_signal();
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        p_dry.process_in_place(&mut buf_dry, &ctx).unwrap();
        p_wet.process_in_place(&mut buf_wet, &ctx).unwrap();

        let thresh_lin = fast_pow10(-6.0 / 20.0);

        // mix=0 (dry): after lookahead fills, peaks should exceed threshold (no limiting)
        let dry_peak: f32 = buf_dry[500..].iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        assert!(
            dry_peak > thresh_lin,
            "mix=0 (dry) should pass through unaltered, peak={dry_peak:.4} > threshold={thresh_lin:.4}"
        );

        // mix=1 (wet): after lookahead fills, peaks should be below threshold
        let wet_peak: f32 = buf_wet[500..].iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        assert!(
            wet_peak <= thresh_lin * 1.1,
            "mix=1 (wet) should be limited, peak={wet_peak:.4} > threshold={thresh_lin:.4}"
        );
    }

    /// With threshold=-6dB, a 0dBFS signal should not exceed the threshold
    /// in the output (after lookahead fills).
    #[test]
    fn test_limiter_ceiling_enforcement() {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();

        let num_frames = 4096;
        let mut buf = vec![1.0f32; num_frames]; // 0 dBFS
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        p.process_in_place(&mut buf, &ctx).unwrap();

        let thresh_lin = fast_pow10(-6.0 / 20.0);
        // After lookahead settles (~240 samples at 5ms), output should not exceed threshold
        for (i, &s) in buf[500..].iter().enumerate() {
            assert!(
                s.abs() <= thresh_lin * 1.05,
                "Frame {}: sample {s:.4} exceeds ceiling {thresh_lin:.4}",
                i + 500
            );
        }
    }

    /// Verify ISP (inter-sample true peak) meter is exposed through LimiterData.
    #[test]
    fn test_isp_meter_exposure() {
        let mut p = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
        p.true_peak = true;
        p.rebuild_cached_parameters();
        p.initialize(48000).unwrap();

        // Create a signal with inter-sample peaks on both channels
        let frames = 2048;
        let mut b = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let val = if i % 2 == 0 { 0.8 } else { -0.8 };
            b[i * 2] = val;
            b[i * 2 + 1] = val * 0.5;
        }

        // Process enough blocks to trigger cache update (>= CACHE_UPDATE_THROTTLE)
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: frames,
        };
        for _ in 0..CACHE_UPDATE_THROTTLE + 1 {
            p.process_in_place(&mut b, &ctx).unwrap();
        }

        let data = p.cache.load();
        let data = data.as_ref();
        assert_eq!(data.isp_dbtp.len(), 2, "ISP should have 2 channels");
        // Both channels should show non-trivial ISP values
        assert!(
            data.isp_dbtp[0] > -20.0,
            "ch0 ISP {} dBTP should be above -20",
            data.isp_dbtp[0]
        );
        assert!(
            data.isp_dbtp[1] > -20.0,
            "ch1 ISP {} dBTP should be above -20",
            data.isp_dbtp[1]
        );
        // Channel 0 (full scale) should have higher ISP than channel 1 (half scale)
        assert!(
            data.isp_dbtp[0] > data.isp_dbtp[1],
            "ch0 ISP {} should exceed ch1 ISP {}",
            data.isp_dbtp[0],
            data.isp_dbtp[1]
        );
    }

    /// Verify ISP meter is empty when true_peak is disabled.
    #[test]
    fn test_isp_meter_empty_without_true_peak() {
        let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();
        assert!(!p.true_peak);

        let frames = 512;
        let mut b = vec![0.5f32; frames];
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: frames,
        };
        for _ in 0..CACHE_UPDATE_THROTTLE + 1 {
            p.process_in_place(&mut b, &ctx).unwrap();
        }

        let data = p.cache.load();
        assert!(
            data.isp_dbtp.is_empty(),
            "ISP should be empty when true_peak is disabled"
        );
    }

    /// Test that reset clears true peak detectors and dual release state.
    #[test]
    fn test_reset_clears_new_state() {
        let mut p = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
        p.true_peak = true;
        p.dual_release = true;
        p.initialize(48000).unwrap();

        // Process some audio to build up state
        let mut b = vec![0.9f32; 2 * 1024];
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };
        p.process_in_place(&mut b, &ctx).unwrap();

        p.reset();

        // After reset, detectors should be zeroed
        for det in &p.true_peak_detectors {
            assert_eq!(det.peak_linear(), 0.0);
        }
        assert_eq!(p.envelope, 0.0);
    }
}
