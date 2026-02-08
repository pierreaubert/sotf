// ============================================================================
// Limiter Plugin
// ============================================================================
//
// Brickwall limiter that prevents audio from exceeding a specified threshold.
// Uses lookahead for transparent limiting with minimal distortion.
//
// Parameters:
// - threshold: Maximum output level (dB)
// - release: Time to return to unity gain (ms)
// - lookahead: Lookahead time for predictive limiting (ms)
// - soft: Enable soft limiting with saturation curve (more musical)
// - mix: Dry/wet mix between unprocessed and limited signal (0.0 = dry, 1.0 = limited)

use super::param_specs::limiter::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ============================================================================
// Configuration
// ============================================================================

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

/// Configuration parameters for LimiterPlugin
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

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Brickwall limiter with lookahead
pub struct LimiterPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
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

    // Smoothed parameters for click-free parameter changes
    threshold_smoother: Smoother,
    mix_smoother: Smoother,

    // State
    envelope: f32,                   // Current gain reduction envelope
    release_coeff: f32,              // Release coefficient
    attack_coeff: f32,               // Attack coefficient (for fast attack instead of instant)
    lookahead_buffer: VecDeque<f32>, // Circular buffer for lookahead (interleaved)
    lookahead_samples: usize,        // Lookahead buffer size in samples
}

impl LimiterPlugin {
    /// Create a new limiter plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `threshold_db` - Maximum output level in dB (default: -0.1)
    /// * `release_ms` - Release time in milliseconds (default: 50.0)
    /// * `lookahead_ms` - Lookahead time in milliseconds (default: 5.0)
    /// * `soft` - Enable soft limiting with saturation curve (default: false)
    pub fn new(
        channels: usize,
        threshold_db: f32,
        release_ms: f32,
        lookahead_ms: f32,
        soft: bool,
    ) -> Self {
        Self {
            channels,
            sample_rate: 44100, // Updated in initialize()

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

            // Smoothed parameters (5ms for fast but smooth response)
            threshold_smoother: Smoother::new(10.0_f32.powf(threshold_db / 20.0), 5.0, 44100),
            mix_smoother: Smoother::new(1.0, 5.0, 44100),

            envelope: 0.0,
            release_coeff: 0.0,
            attack_coeff: 0.9, // Fast attack (0.9 = ~1ms attack time)
            lookahead_buffer: VecDeque::new(),
            lookahead_samples: 0,
        }
    }

    /// Create a new limiter plugin from configuration parameters
    pub fn from_params(channels: usize, params: LimiterPluginParams) -> Self {
        let mut plugin = Self::new(
            channels,
            params.threshold_db,
            params.release_ms.max(1.0),
            params.lookahead_ms.max(0.0),
            params.soft,
        );

        plugin.mix = params.mix.clamp(0.0, 1.0);

        plugin
    }

    /// Calculate time coefficient for envelope follower
    fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
        }
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);

        // Update lookahead buffer size
        let new_lookahead_samples =
            ((self.lookahead_ms * 0.001 * self.sample_rate as f32) as usize).max(1) * self.channels;

        if new_lookahead_samples != self.lookahead_samples {
            self.lookahead_samples = new_lookahead_samples;
            self.lookahead_buffer.clear();
            // Pre-fill with zeros
            self.lookahead_buffer.resize(self.lookahead_samples, 0.0);
        }
    }

    /// Apply soft limiting using hyperbolic tangent saturation curve
    /// This provides a more musical limiting with smooth transition into saturation
    fn apply_soft_limit(&self, sample: f32, threshold_linear: f32) -> f32 {
        if self.soft {
            // Use tanh for smooth saturation curve.
            // Normalize by threshold, apply gentle drive, then scale back.
            let normalized = sample / threshold_linear;
            let driven = normalized * 0.75;
            threshold_linear * driven.tanh()
        } else {
            // Hard limiting (clamp to threshold)
            sample.clamp(-threshold_linear, threshold_linear)
        }
    }
}

impl InPlacePlugin for LimiterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Limiter", "1.0.0", "SotF")
            .with_description("Brickwall limiter with lookahead for transparent peak control")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                THRESHOLD_DEFAULT,
                THRESHOLD_MIN,
                THRESHOLD_MAX,
            )
            .with_description("Maximum output level (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                RELEASE_DEFAULT,
                RELEASE_MIN,
                RELEASE_MAX,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "lookahead",
                "Lookahead",
                LOOKAHEAD_DEFAULT,
                LOOKAHEAD_MIN,
                LOOKAHEAD_MAX,
            )
            .with_description("Lookahead time for predictive limiting (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("soft", "Soft", SOFT_DEFAULT)
                .with_description("Enable soft limiting with saturation curve (more musical)")
                .with_group("Character")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = limited)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_threshold {
            self.threshold_db = value.as_float().ok_or("Invalid threshold value")?;
            // Convert dB to linear and set target for smoother
            let linear = 10.0_f32.powf(self.threshold_db / 20.0);
            self.threshold_smoother.set_target(linear);
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release value")?.max(1.0);
            self.update_coefficients();
        } else if id == self.param_lookahead {
            self.lookahead_ms = value.as_float().ok_or("Invalid lookahead value")?.max(0.0);
            self.update_coefficients();
        } else if id == self.param_soft {
            self.soft = value.as_bool().ok_or("Invalid soft value")?;
        } else if id == self.param_mix {
            self.mix = value.as_float().ok_or("Invalid mix value")?.clamp(0.0, 1.0);
            self.mix_smoother.set_target(self.mix);
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_lookahead {
            Some(ParameterValue::Float(self.lookahead_ms))
        } else if id == &self.param_soft {
            Some(ParameterValue::Bool(self.soft))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();

        // Update smoother times for the new sample rate
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.lookahead_buffer.clear();
        self.lookahead_buffer.resize(self.lookahead_samples, 0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let num_frames = context.num_frames;

        for frame in 0..num_frames {
            // Update smoothers per sample for smooth parameter transitions
            let _ = self.threshold_smoother.next();
            let _ = self.mix_smoother.next();
            let threshold_linear = self.threshold_smoother.current();
            let smoothed_mix = self.mix_smoother.current();
            let dry_mix = 1.0 - smoothed_mix;
            let wet_mix = smoothed_mix;

            // Process all channels for this frame
            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;
                let input_sample = buffer[sample_idx];

                // Push to lookahead buffer
                self.lookahead_buffer.push_back(input_sample);
            }

            // Find peak in lookahead buffer to predict future peaks
            let mut lookahead_peak = 0.0_f32;
            for sample in self.lookahead_buffer.iter() {
                lookahead_peak = lookahead_peak.max(sample.abs());
            }

            // Calculate required gain reduction based on lookahead peak
            let target_gain = if lookahead_peak > threshold_linear {
                threshold_linear / lookahead_peak
            } else {
                1.0
            };

            // Convert to dB for envelope
            let target_gr_db = if target_gain < 1.0 {
                20.0 * (1.0 / target_gain).log10()
            } else {
                0.0
            };

            // Envelope follower (fast attack, smooth release) instead of instant attack
            // This prevents clicks while still providing effective limiting
            if target_gr_db > self.envelope {
                // Fast attack (0.9 coeff = ~1ms attack time at 44.1kHz)
                self.envelope = target_gr_db + self.attack_coeff * (self.envelope - target_gr_db);
            } else {
                // Smooth release
                self.envelope = target_gr_db + self.release_coeff * (self.envelope - target_gr_db);
            }

            // Calculate gain and ensure it doesn't exceed 1.0
            let gain = (10.0_f32.powf(-self.envelope / 20.0)).min(1.0);

            // Process all channels for this frame (apply gain to delayed samples)
            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;

                // Get delayed sample from lookahead buffer
                if let Some(delayed_sample) = self.lookahead_buffer.pop_front() {
                    // Apply gain to delayed sample
                    let limited_sample = delayed_sample * gain;

                    // Apply soft/hard limiting to obtain wet signal
                    let wet = self.apply_soft_limit(limited_sample, threshold_linear);
                    let dry = delayed_sample;

                    // Dry/wet mix
                    buffer[sample_idx] = dry_mix * dry + wet_mix * wet;

                    // Flush denormals to prevent CPU performance spikes and audio crackle
                    if buffer[sample_idx].abs() < 1e-30 && buffer[sample_idx] != 0.0 {
                        buffer[sample_idx] = 0.0;
                    }
                } else {
                    buffer[sample_idx] = 0.0;
                }
            }
        }

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_samples / self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limiter_creation() {
        let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);
        assert_eq!(limiter.channels(), 2);
        assert_eq!(limiter.threshold_db, -0.1);
    }

    #[test]
    fn test_limiter_prevents_clipping() {
        let mut limiter = LimiterPlugin::new(1, 0.0, 50.0, 0.0, false); // No lookahead for simple test
        limiter.initialize(48000).unwrap();

        // Test with a signal that would clip
        let context = ProcessContext {
            num_frames: 10,
            sample_rate: 48000,
        };
        let mut buffer = vec![1.5; 10]; // Signal exceeds 1.0

        limiter.process_in_place(&mut buffer, &context).unwrap();

        // All samples should be <= 1.0
        for sample in &buffer {
            assert!(sample.abs() <= 1.0, "Sample {} exceeds 1.0", sample);
        }
    }

    #[test]
    fn test_limiter_additional_parameters_defaults() {
        let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);

        assert_eq!(limiter.mix, 1.0);
    }

    #[test]
    fn test_limiter_mix_parameter_set_get() {
        let mut limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);
        limiter.initialize(48000).unwrap();

        limiter
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();

        let mix = limiter.get_parameter(&ParameterId::from("mix"));
        assert_eq!(mix, Some(ParameterValue::Float(0.5)));
    }

    #[test]
    fn test_limiter_soft_vs_hard_behaviour() {
        let mut hard = LimiterPlugin::new(1, 0.0, 50.0, 0.0, false);
        let mut soft = LimiterPlugin::new(1, 0.0, 50.0, 0.0, true);
        hard.initialize(48000).unwrap();
        soft.initialize(48000).unwrap();

        let context = ProcessContext {
            num_frames: 1,
            sample_rate: 48000,
        };

        let mut hard_buf = vec![2.0_f32];
        let mut soft_buf = vec![2.0_f32];

        hard.process_in_place(&mut hard_buf, &context).unwrap();
        soft.process_in_place(&mut soft_buf, &context).unwrap();

        assert!(hard_buf[0].abs() <= 1.0);
        assert!(soft_buf[0].abs() <= 1.0);
        // Soft limiter should not produce a larger peak than the hard limiter
        // for the same hot input, ensuring a safe, rounded response.
        assert!(soft_buf[0].abs() <= hard_buf[0].abs());
    }

    #[test]
    fn test_limiter_various_sample_rates() {
        for &sample_rate in &[22050, 44100, 48000, 96000, 192000] {
            let mut limiter = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
            limiter.initialize(sample_rate).unwrap();

            let num_frames = 512;
            let mut buffer: Vec<f32> = (0..num_frames * 2)
                .map(|i| {
                    let t = i as f32 / (sample_rate as f32 * 2.0);
                    (t * 1000.0 * 2.0 * std::f32::consts::PI).sin() * 2.0
                })
                .collect();

            let context = ProcessContext {
                sample_rate,
                num_frames,
            };

            limiter.process_in_place(&mut buffer, &context).unwrap();

            for s in &buffer {
                assert!(s.is_finite(), "Non-finite value at sample rate {}", sample_rate);
            }
        }
    }

    #[test]
    fn test_limiter_from_params() {
        let params = LimiterPluginParams {
            threshold_db: -3.0,
            release_ms: 100.0,
            lookahead_ms: 10.0,
            soft: true,
            mix: 0.8,
        };
        let plugin = LimiterPlugin::from_params(2, params);
        assert_eq!(plugin.threshold_db, -3.0);
        assert_eq!(plugin.release_ms, 100.0);
        assert_eq!(plugin.lookahead_ms, 10.0);
        assert!(plugin.soft);
        assert_eq!(plugin.mix, 0.8);
    }

    #[test]
    fn test_limiter_time_to_coeff() {
        let coeff = LimiterPlugin::time_to_coeff(0.0, 48000);
        assert_eq!(coeff, 0.0);

        let coeff = LimiterPlugin::time_to_coeff(50.0, 48000);
        assert!(coeff > 0.0 && coeff < 1.0);

        let coeff_short = LimiterPlugin::time_to_coeff(10.0, 48000);
        let coeff_long = LimiterPlugin::time_to_coeff(100.0, 48000);
        assert!(coeff_long > coeff_short);
    }

    #[test]
    fn test_limiter_lookahead_latency() {
        let mut limiter = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
        limiter.initialize(48000).unwrap();

        let latency = limiter.latency_samples();
        let expected = ((5.0 * 0.001 * 48000.0) as usize).max(1);
        assert_eq!(latency, expected);
    }

    #[test]
    fn test_limiter_reset() {
        let mut limiter = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
        limiter.initialize(48000).unwrap();

        let num_frames = 256;
        let mut buffer = vec![2.0_f32; num_frames];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        limiter.process_in_place(&mut buffer, &context).unwrap();

        limiter.reset();
        assert_eq!(limiter.envelope, 0.0);
    }

    mod proptest_limiter {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Property: limiter output should never exceed threshold (in linear domain)
            /// after sufficient processing to settle the envelope.
            #[test]
            fn output_never_exceeds_threshold(
                threshold_db in -20.0f32..0.0,
                input_amplitude in 0.01f32..4.0,
            ) {
                let threshold_linear = 10.0f32.powf(threshold_db / 20.0);
                let mut limiter = LimiterPlugin::new(1, threshold_db, 50.0, 0.0, false);
                limiter.initialize(48000).unwrap();

                // Process multiple blocks for envelope to settle
                let num_frames = 4096;
                let mut buffer = vec![input_amplitude; num_frames];
                let context = ProcessContext {
                    sample_rate: 48000,
                    num_frames,
                };
                limiter.process_in_place(&mut buffer, &context).unwrap();

                // Check last 512 samples (after envelope has settled)
                for &sample in &buffer[num_frames - 512..] {
                    prop_assert!(
                        sample.abs() <= threshold_linear * 1.01, // 1% tolerance for numerical precision
                        "sample {:.6} exceeded threshold {:.6} (threshold_db={:.1}, input={:.2})",
                        sample.abs(), threshold_linear, threshold_db, input_amplitude
                    );
                }
            }

            /// Property: limiter should not produce NaN or Inf
            #[test]
            fn no_nan_or_inf(
                threshold_db in -20.0f32..0.0,
                release_ms in 10.0f32..500.0,
                input_amplitude in 0.01f32..4.0,
            ) {
                let mut limiter = LimiterPlugin::new(1, threshold_db, release_ms, 0.0, false);
                limiter.initialize(48000).unwrap();

                let num_frames = 1024;
                let mut buffer = vec![input_amplitude; num_frames];
                let context = ProcessContext { sample_rate: 48000, num_frames };
                limiter.process_in_place(&mut buffer, &context).unwrap();

                for &sample in &buffer {
                    prop_assert!(!sample.is_nan(), "NaN in output");
                    prop_assert!(!sample.is_infinite(), "Inf in output");
                }
            }
        }
    }
}
