// ============================================================================
// Transient Shaper Plugin — SPL Transient Designer approach
// ============================================================================
//
// Uses a differential envelope detector to separate transient and sustain
// components.  Two one-pole envelope followers (fast and slow) track the
// input level.  The difference between the envelopes is the transient
// component; the slow envelope is the sustain component.
//
// Hard rules:
// - No allocations in process_in_place()
// - No mutex locks in process()
// - No unsafe code

pub mod params;

use crate::params::PARAMS as TS;
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

const EPSILON: f32 = 1e-10;
const CACHE_UPDATE_THROTTLE: usize = 10;

// Envelope time constants (milliseconds)
const FAST_ATTACK_MS: f32 = 0.1;
const FAST_RELEASE_MS: f32 = 5.0;
const SLOW_ATTACK_MS: f32 = 10.0;
const SLOW_RELEASE_MS: f32 = 100.0;

// ============================================================================
// Plugin Params (JSON deserialization)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransientShaperPluginParams {
    /// -100.0 to +100.0 (percent)
    #[serde(default = "default_attack")]
    pub attack: f32,
    /// -100.0 to +100.0 (percent)
    #[serde(default = "default_sustain")]
    pub sustain: f32,
    #[serde(default = "default_sensitivity")]
    pub sensitivity_db: f32,
    #[serde(default = "default_output_gain")]
    pub output_gain_db: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_attack() -> f32 {
    pk(TS, "attack").default_f64() as f32
}
fn default_sustain() -> f32 {
    pk(TS, "sustain").default_f64() as f32
}
fn default_sensitivity() -> f32 {
    pk(TS, "sensitivity").default_f64() as f32
}
fn default_output_gain() -> f32 {
    pk(TS, "output_gain").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(TS, "mix").default_f64() as f32
}

// ============================================================================
// Monitoring Data
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct TransientShaperData {
    /// Peak transient level (positive = transient detected)
    pub transient_level: f32,
    /// Peak sustain level
    pub sustain_level: f32,
    /// Current gain applied (linear)
    pub gain: f32,
}

// ============================================================================
// Plugin Struct
// ============================================================================

pub struct TransientShaperPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_attack: ParameterId,
    attack_amount: f32, // -1.0 to 1.0 (from -100% to +100%)
    param_sustain: ParameterId,
    sustain_amount: f32, // -1.0 to 1.0
    param_sensitivity: ParameterId,
    sensitivity_db: f32,
    param_output_gain: ParameterId,
    output_gain_db: f32,
    param_mix: ParameterId,
    mix: f32,

    // Envelope state (per channel)
    fast_env: Vec<f32>,
    slow_env: Vec<f32>,

    // Coefficients
    fast_attack_coeff: f32,
    fast_release_coeff: f32,
    slow_attack_coeff: f32,
    slow_release_coeff: f32,

    // Smoothers
    attack_smoother: Smoother,
    sustain_smoother: Smoother,
    mix_smoother: Smoother,

    // Monitoring
    cache: RealTimeCache<TransientShaperData>,
    cache_counter: usize,

    cached_parameters: Vec<Parameter>,
}

// ============================================================================
// Coefficient calculation
// ============================================================================

/// Calculate one-pole filter coefficient from time constant in ms and sample rate.
///
/// coeff = 1.0 - exp(-1.0 / (time_ms * 0.001 * sample_rate))
#[inline]
fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
    1.0 - (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
}

/// One-pole envelope follower: tracks `target` with separate attack/release
/// coefficients.
#[inline]
fn one_pole(current: f32, target: f32, attack_coeff: f32, release_coeff: f32) -> f32 {
    let coeff = if target > current {
        attack_coeff
    } else {
        release_coeff
    };
    current + coeff * (target - current)
}

// ============================================================================
// Implementation
// ============================================================================

impl TransientShaperPlugin {
    pub fn new(channels: usize) -> Self {
        let sr = 44100;
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_attack: ParameterId::from("attack"),
            attack_amount: 0.0,
            param_sustain: ParameterId::from("sustain"),
            sustain_amount: 0.0,
            param_sensitivity: ParameterId::from("sensitivity"),
            sensitivity_db: 0.0,
            param_output_gain: ParameterId::from("output_gain"),
            output_gain_db: 0.0,
            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            fast_env: vec![0.0; channels],
            slow_env: vec![0.0; channels],
            fast_attack_coeff: time_to_coeff(FAST_ATTACK_MS, sr),
            fast_release_coeff: time_to_coeff(FAST_RELEASE_MS, sr),
            slow_attack_coeff: time_to_coeff(SLOW_ATTACK_MS, sr),
            slow_release_coeff: time_to_coeff(SLOW_RELEASE_MS, sr),
            attack_smoother: Smoother::new(0.0, 10.0, sr),
            sustain_smoother: Smoother::new(0.0, 10.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            cache: RealTimeCache::new(TransientShaperData::default()),
            cache_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: TransientShaperPluginParams) -> Self {
        let mut p = Self::new(channels);
        p.attack_amount = (params.attack / 100.0).clamp(-1.0, 1.0);
        p.sustain_amount = (params.sustain / 100.0).clamp(-1.0, 1.0);
        p.sensitivity_db = params.sensitivity_db.clamp(-12.0, 12.0);
        p.output_gain_db = params.output_gain_db.clamp(-12.0, 12.0);
        p.mix = params.mix.clamp(0.0, 1.0);
        p.attack_smoother.set_target(p.attack_amount);
        p.sustain_smoother.set_target(p.sustain_amount);
        p.mix_smoother.set_target(p.mix);
        p.rebuild_cached_parameters();
        p
    }

    fn update_coefficients(&mut self) {
        self.fast_attack_coeff = time_to_coeff(FAST_ATTACK_MS, self.sample_rate);
        self.fast_release_coeff = time_to_coeff(FAST_RELEASE_MS, self.sample_rate);
        self.slow_attack_coeff = time_to_coeff(SLOW_ATTACK_MS, self.sample_rate);
        self.slow_release_coeff = time_to_coeff(SLOW_RELEASE_MS, self.sample_rate);
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_amount * 100.0,
                pk(TS, "attack").min_f64() as f32,
                pk(TS, "attack").max_f64() as f32,
            )
            .with_description("Transient emphasis (-100% to +100%)")
            .with_group("Shape")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "sustain",
                "Sustain",
                self.sustain_amount * 100.0,
                pk(TS, "sustain").min_f64() as f32,
                pk(TS, "sustain").max_f64() as f32,
            )
            .with_description("Sustain emphasis (-100% to +100%)")
            .with_group("Shape")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "sensitivity",
                "Sensitivity",
                self.sensitivity_db,
                pk(TS, "sensitivity").min_f64() as f32,
                pk(TS, "sensitivity").max_f64() as f32,
            )
            .with_description("Detection sensitivity offset (dB)")
            .with_group("Detection")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "output_gain",
                "Output",
                self.output_gain_db,
                pk(TS, "output_gain").min_f64() as f32,
                pk(TS, "output_gain").max_f64() as f32,
            )
            .with_description("Output gain compensation (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(TS, "mix").min_f64() as f32,
                pk(TS, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = shaped)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
        ];
    }
}

// ============================================================================
// InPlacePlugin trait
// ============================================================================

impl InPlacePlugin for TransientShaperPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("TransientShaper", "1.0.0", "SotF")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_attack {
            let v = value
                .as_float()
                .unwrap_or(pk(TS, "attack").default_f64() as f32);
            if v.is_finite() {
                self.attack_amount = (v / 100.0).clamp(-1.0, 1.0);
                self.attack_smoother.set_target(self.attack_amount);
            }
        } else if id == self.param_sustain {
            let v = value
                .as_float()
                .unwrap_or(pk(TS, "sustain").default_f64() as f32);
            if v.is_finite() {
                self.sustain_amount = (v / 100.0).clamp(-1.0, 1.0);
                self.sustain_smoother.set_target(self.sustain_amount);
            }
        } else if id == self.param_sensitivity {
            let v = value
                .as_float()
                .unwrap_or(pk(TS, "sensitivity").default_f64() as f32);
            if v.is_finite() {
                self.sensitivity_db = v.clamp(-12.0, 12.0);
            }
        } else if id == self.param_output_gain {
            let v = value
                .as_float()
                .unwrap_or(pk(TS, "output_gain").default_f64() as f32);
            if v.is_finite() {
                self.output_gain_db = v.clamp(-12.0, 12.0);
            }
        } else if id == self.param_mix {
            let v = value
                .as_float()
                .unwrap_or(pk(TS, "mix").default_f64() as f32);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_amount * 100.0))
        } else if id == &self.param_sustain {
            Some(ParameterValue::Float(self.sustain_amount * 100.0))
        } else if id == &self.param_sensitivity {
            Some(ParameterValue::Float(self.sensitivity_db))
        } else if id == &self.param_output_gain {
            Some(ParameterValue::Float(self.output_gain_db))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.attack_smoother.set_time(10.0, sample_rate);
        self.sustain_smoother.set_time(10.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        Ok(())
    }

    fn reset(&mut self) {
        self.fast_env.fill(0.0);
        self.slow_env.fill(0.0);
        self.attack_smoother.reset(self.attack_amount);
        self.sustain_smoother.reset(self.sustain_amount);
        self.mix_smoother.reset(self.mix);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let ch = self.channels;

        // Pre-compute detection threshold from sensitivity (dB)
        // +12 dB = lower threshold (more sensitive), -12 dB = higher threshold (less sensitive)
        let threshold_lin = 1e-3f32 * 10.0f32.powf(-self.sensitivity_db / 20.0);
        // Pre-compute output gain (linear multiplier from dB)
        let output_gain_lin = 10.0f32.powf(self.output_gain_db / 20.0);

        // Monitoring accumulators
        let mut peak_transient: f32 = 0.0;
        let mut peak_sustain: f32 = 0.0;
        let mut last_gain: f32 = 1.0;

        for frame in 0..num_frames {
            let attack_amt = self.attack_smoother.advance();
            let sustain_amt = self.sustain_smoother.advance();
            let current_mix = self.mix_smoother.advance();

            for c in 0..ch {
                let idx = frame * ch + c;
                let input = buffer[idx];
                let abs_input = input.abs();

                // Fast envelope (tracks transients)
                self.fast_env[c] = one_pole(
                    self.fast_env[c],
                    abs_input,
                    self.fast_attack_coeff,
                    self.fast_release_coeff,
                );

                // Slow envelope (tracks sustain/body)
                self.slow_env[c] = one_pole(
                    self.slow_env[c],
                    abs_input,
                    self.slow_attack_coeff,
                    self.slow_release_coeff,
                );

                let fast = self.fast_env[c];
                let slow = self.slow_env[c];

                // Transient = difference between fast and slow envelopes
                let transient = fast - slow;
                // Sustain = slow envelope
                let sustain = slow;

                // Compute gain modulation
                let transient_ratio = (transient / slow.max(EPSILON)).clamp(-1.0, 1.0);
                let sustain_ratio = (sustain / fast.max(EPSILON)).clamp(-1.0, 1.0);

                let gain: f32 = if slow > threshold_lin {
                    (1.0 + attack_amt * transient_ratio + sustain_amt * sustain_ratio).max(0.0)
                } else {
                    1.0
                };

                let wet = input * gain;
                let mixed = input + current_mix * (wet - input);
                buffer[idx] = mixed * output_gain_lin;

                // Update monitoring
                peak_transient = peak_transient.max(transient.abs());
                peak_sustain = peak_sustain.max(sustain);
                last_gain = gain;
            }
        }

        // Update monitoring cache (throttled)
        self.cache_counter += 1;
        if self.cache_counter >= CACHE_UPDATE_THROTTLE {
            self.cache_counter = 0;
            self.cache.update(|d| {
                d.transient_level = peak_transient;
                d.sustain_level = peak_sustain;
                d.gain = last_gain;
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    fn make_context(num_frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 48000,
            num_frames,
        }
    }

    #[test]
    fn test_transient_shaper_passthrough() {
        // With attack=0, sustain=0, output_gain=0, mix=1.0:
        // gain = 1.0 + 0*transient_ratio + 0*sustain_ratio = 1.0
        // output = input * 1.0 * 1.0 = input
        let channels = 2;
        let mut plugin = TransientShaperPlugin::new(channels);
        plugin.initialize(48000).unwrap();

        let num_frames = 256;
        let mut buffer = vec![0.0f32; num_frames * channels];

        // Fill with a sine wave
        for frame in 0..num_frames {
            let t = frame as f32 / 48000.0;
            let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            buffer[frame * channels] = val;
            buffer[frame * channels + 1] = val;
        }
        let original = buffer.clone();

        let ctx = make_context(num_frames);
        let result = plugin.process_in_place(&mut buffer, &ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), num_frames);

        // Output should be close to input (envelopes need time to settle,
        // so we only check the second half)
        let start = num_frames / 2;
        for frame in start..num_frames {
            for c in 0..channels {
                let idx = frame * channels + c;
                let diff = (buffer[idx] - original[idx]).abs();
                assert!(
                    diff < 0.05,
                    "frame={}, ch={}: output={}, expected={}, diff={}",
                    frame,
                    c,
                    buffer[idx],
                    original[idx],
                    diff
                );
            }
        }
    }

    #[test]
    fn test_transient_shaper_enhances_attack() {
        // With attack=+100%, transient peaks should be louder
        let channels = 1;
        let params = TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: 0.0,
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin = TransientShaperPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        // Create a signal with a sharp transient followed by sustained signal
        let num_frames = 4800; // 100ms at 48kHz
        let mut buffer = vec![0.0f32; num_frames * channels];

        // First 10ms: silence (let envelopes settle at zero)
        // Then sharp transient: sudden jump to full scale
        for frame in 480..num_frames {
            buffer[frame] = 0.3; // sustained level
        }
        // Spike the first few samples of the sustained portion
        for frame in 480..490 {
            buffer[frame] = 0.9;
        }
        let original = buffer.clone();

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // The transient spike region should have higher amplitude than original
        let mut max_shaped = 0.0f32;
        let mut max_original = 0.0f32;
        for frame in 480..500 {
            max_shaped = max_shaped.max(buffer[frame].abs());
            max_original = max_original.max(original[frame].abs());
        }
        assert!(
            max_shaped > max_original,
            "Enhanced transient should be louder: shaped={}, original={}",
            max_shaped,
            max_original
        );
    }

    #[test]
    fn test_transient_shaper_reduces_sustain() {
        // With sustain=-100%, sustained portions should be quieter
        let channels = 1;
        let params = TransientShaperPluginParams {
            attack: 0.0,
            sustain: -100.0,
            sensitivity_db: 0.0,
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin = TransientShaperPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        // Create a sustained signal (no transient, just continuous)
        let num_frames = 9600; // 200ms at 48kHz
        let mut buffer = vec![0.0f32; num_frames * channels];
        for frame in 0..num_frames {
            let t = frame as f32 / 48000.0;
            buffer[frame] = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.5;
        }
        let original = buffer.clone();

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // Measure RMS of the last quarter (after envelopes have settled)
        let start = num_frames * 3 / 4;
        let mut rms_original = 0.0f64;
        let mut rms_shaped = 0.0f64;
        for frame in start..num_frames {
            rms_original += (original[frame] as f64).powi(2);
            rms_shaped += (buffer[frame] as f64).powi(2);
        }
        let count = (num_frames - start) as f64;
        rms_original = (rms_original / count).sqrt();
        rms_shaped = (rms_shaped / count).sqrt();

        assert!(
            rms_shaped < rms_original,
            "Reduced sustain should be quieter: shaped_rms={}, original_rms={}",
            rms_shaped,
            rms_original
        );
    }

    #[test]
    fn test_sensitivity_affects_audio_output() {
        let channels = 1;
        let num_frames = 2400; // 50ms at 48kHz

        // Create a step signal with sustained level 0.001
        let mut signal = vec![0.0f32; num_frames * channels];
        for frame in 480..num_frames {
            // 10ms silence then step to 0.001
            signal[frame] = 0.001;
        }

        // Low sensitivity (-12 dB) — high threshold, signal should be bypassed
        let params_low = TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: -12.0,
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin_low = TransientShaperPlugin::from_params(channels, params_low);
        plugin_low.initialize(48000).unwrap();
        let mut buffer_low = signal.clone();
        let ctx = make_context(num_frames);
        plugin_low.process_in_place(&mut buffer_low, &ctx).unwrap();

        // High sensitivity (+12 dB) — low threshold, signal should be processed
        let params_high = TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: 12.0,
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin_high = TransientShaperPlugin::from_params(channels, params_high);
        plugin_high.initialize(48000).unwrap();
        let mut buffer_high = signal.clone();
        plugin_high
            .process_in_place(&mut buffer_high, &ctx)
            .unwrap();

        // The outputs should differ
        let mut total_diff = 0.0f32;
        for i in 0..(num_frames * channels) {
            total_diff += (buffer_low[i] - buffer_high[i]).abs();
        }
        assert!(
            total_diff > 1e-6,
            "Sensitivity should affect audio output, but outputs were identical (diff={})",
            total_diff
        );
    }

    #[test]
    fn test_output_gain_applies_to_final_mix() {
        let channels = 1;
        let num_frames = 256;

        // With mix=0.0, output_gain should still affect the output
        let params = TransientShaperPluginParams {
            attack: 0.0,
            sustain: 0.0,
            sensitivity_db: 0.0,
            output_gain_db: 6.0, // +6 dB = ~2.0x linear
            mix: 0.0,            // fully dry
        };
        let mut plugin = TransientShaperPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let mut buffer = vec![0.0f32; num_frames * channels];
        for frame in 0..num_frames {
            buffer[frame] = 0.5;
        }

        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // With mix=0.0 and output_gain=6dB, output should be input * 2.0
        let expected = 0.5 * 10.0f32.powf(6.0 / 20.0);
        for frame in 0..num_frames {
            let diff = (buffer[frame] - expected).abs();
            assert!(
                diff < 1e-5,
                "frame={}: expected={}, got={}, diff={}",
                frame,
                expected,
                buffer[frame],
                diff
            );
        }
    }

    #[test]
    fn test_reset_resets_smoothers() {
        let channels = 1;
        let mut plugin = TransientShaperPlugin::new(channels);
        plugin.initialize(48000).unwrap();

        // Set attack to 50% and process a block to advance smoothers
        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(50.0))
            .unwrap();
        assert_eq!(plugin.attack_smoother.target(), 0.5);

        let num_frames = 480;
        let mut buffer = vec![0.0f32; num_frames * channels];
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // After processing, the smoother should have moved toward the target
        let before_reset = plugin.attack_smoother.current();
        assert!(
            (before_reset - 0.5).abs() > 1e-6,
            "Smoother should have moved from initial value, got={}",
            before_reset
        );

        // Now reset
        plugin.reset();

        // After reset, smoother should be at target immediately
        let after_reset = plugin.attack_smoother.current();
        assert!(
            (after_reset - 0.5).abs() < 1e-6,
            "Smoother should be reset to target after reset(), got={}",
            after_reset
        );
    }

    #[test]
    fn test_parameter_roundtrip() {
        let channels = 2;
        let mut plugin = TransientShaperPlugin::new(channels);
        plugin.initialize(48000).unwrap();

        // Set attack to 50%
        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(50.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("attack"));
        assert_eq!(val, Some(ParameterValue::Float(50.0)));

        // Set sustain to -75%
        plugin
            .set_parameter(ParameterId::from("sustain"), ParameterValue::Float(-75.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("sustain"));
        assert_eq!(val, Some(ParameterValue::Float(-75.0)));

        // Set sensitivity
        plugin
            .set_parameter(ParameterId::from("sensitivity"), ParameterValue::Float(6.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("sensitivity"));
        assert_eq!(val, Some(ParameterValue::Float(6.0)));

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
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(val, Some(ParameterValue::Float(0.5)));
    }
}
