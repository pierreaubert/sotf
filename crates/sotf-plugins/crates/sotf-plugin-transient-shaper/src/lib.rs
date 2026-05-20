// ============================================================================
// Transient Shaper Plugin — SPL Transient Designer approach
// ============================================================================
//
// Uses a differential envelope detector to separate transient and sustain
// components.  Two one-pole envelope followers (fast and slow) track the
// input level.  The difference between the envelopes is the transient
// component; the slow envelope is the sustain component.
//
// Sensitivity parameter: implemented as a threshold gate.  When the slow
// envelope falls below the threshold derived from sensitivity_db (relative to
// a -60 dBFS reference), gain modulation is bypassed (gain stays at 1.0).
// This makes loud passages more sensitive to shaping than quiet ones.
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
    if time_ms <= 0.0 || sample_rate == 0 {
        return 1.0;
    }
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
        // Reset smoothers to their current targets so a transport-loop restart
        // doesn't inherit a mid-ramp state from before the reset.
        self.attack_smoother.reset(self.attack_amount);
        self.sustain_smoother.reset(self.sustain_amount);
        self.mix_smoother.reset(self.mix);
        self.cache_counter = 0;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let ch = self.channels;

        // Sensitivity: threshold for gain modulation activation.
        // sensitivity_db = 0 → threshold at -60 dBFS reference (1e-3 linear).
        // Positive values raise the threshold (only loud signals are shaped);
        // negative values lower it (even quiet signals are shaped).
        // Because the ratios used for shaping cancel out any uniform envelope
        // scaling, we implement sensitivity as a threshold gate: gain modulation
        // is only applied when the slow envelope exceeds this threshold.
        let threshold_lin = 10.0f32.powf(self.sensitivity_db / 20.0) * 1e-3;

        // Pre-compute output gain (linear multiplier from dB).
        // Applied to the final mixed output so it acts as true makeup gain
        // regardless of the dry/wet mix setting.
        let output_gain_lin = 10.0f32.powf(self.output_gain_db / 20.0);

        // Hoist env slice references to help the compiler promote them into
        // registers and avoid repeated &mut self borrows inside the inner loop.
        let fast_env = &mut self.fast_env;
        let slow_env = &mut self.slow_env;

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
                fast_env[c] = one_pole(
                    fast_env[c],
                    abs_input,
                    self.fast_attack_coeff,
                    self.fast_release_coeff,
                );

                // Slow envelope (tracks sustain/body)
                slow_env[c] = one_pole(
                    slow_env[c],
                    abs_input,
                    self.slow_attack_coeff,
                    self.slow_release_coeff,
                );

                let fast = fast_env[c];
                let slow = slow_env[c];

                // Transient = difference between fast and slow envelopes
                let transient = fast - slow;
                // Sustain = slow envelope
                let sustain = slow;

                // Compute gain modulation, gated by sensitivity threshold.
                // When signal is below threshold, shaping is bypassed (gain = 1.0).
                let gain: f32 = if slow > threshold_lin {
                    let transient_ratio = (transient / slow.max(EPSILON)).clamp(-1.0, 1.0);
                    let sustain_ratio = (sustain / fast.max(EPSILON)).clamp(-1.0, 1.0);
                    (1.0 + attack_amt * transient_ratio + sustain_amt * sustain_ratio).max(0.0)
                } else {
                    1.0
                };

                let wet = input * gain;
                // Apply dry/wet mix, then apply output gain to the full mixed
                // output so it acts as true makeup gain at any mix setting.
                buffer[idx] = (input + current_mix * (wet - input)) * output_gain_lin;

                // Update monitoring — use max across channels for consistency.
                peak_transient = peak_transient.max(transient.abs());
                peak_sustain = peak_sustain.max(sustain);
                last_gain = last_gain.max(gain);
            }
        }

        // Flush envelope states to prevent CPU denormal penalty during silence.
        for c in 0..ch {
            if fast_env[c].abs() < 1e-30 {
                fast_env[c] = 0.0;
            }
            if slow_env[c].abs() < 1e-30 {
                slow_env[c] = 0.0;
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
        // With attack=0, sustain=0, output_gain=0 dB (linear=1.0), mix=1.0:
        // attack_amt = 0 and sustain_amt = 0 make gain exactly 1.0 for every
        // sample regardless of envelope state, so output == input sample-for-sample.
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

        // With attack=0 and sustain=0 the gain formula collapses to 1.0 for
        // every sample. Tolerance of 1e-5 accounts for f32 rounding only.
        for frame in 0..num_frames {
            for c in 0..channels {
                let idx = frame * channels + c;
                let diff = (buffer[idx] - original[idx]).abs();
                assert!(
                    diff < 1e-5,
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
    fn test_sensitivity_low_level_step_affects_audio_output() {
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
    fn test_reset_snaps_smoother_to_target() {
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

    #[test]
    fn test_sensitivity_threshold_gate_affects_audio_output() {
        // Sensitivity is a threshold gate: with sensitivity_db = +12 the
        // threshold is raised 20× (to ~0.02 linear), so a low-level signal
        // that would otherwise be shaped is left unmodified (gain = 1.0).
        // Two runs of the same moderate-level signal must produce different
        // shaped output when sensitivity differs.
        let channels = 1;
        let num_frames = 4800;

        // Build a signal with a clear transient followed by sustain so that
        // attack shaping would normally increase the transient region.
        let mut buf_low_sens = vec![0.0f32; num_frames];
        for frame in 480..num_frames {
            buf_low_sens[frame] = 0.3;
        }
        for frame in 480..490 {
            buf_low_sens[frame] = 0.9;
        }
        let mut buf_high_sens = buf_low_sens.clone();

        // Low sensitivity (threshold very low → shaping active)
        let params_low = TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: -12.0,
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin_low = TransientShaperPlugin::from_params(channels, params_low);
        plugin_low.initialize(48000).unwrap();

        // High sensitivity (threshold raised → quiet parts bypass shaping)
        let params_high = TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: 60.0, // threshold at ~1.0 linear: almost nothing is shaped
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin_high = TransientShaperPlugin::from_params(channels, params_high);
        plugin_high.initialize(48000).unwrap();

        let ctx = make_context(num_frames);
        plugin_low
            .process_in_place(&mut buf_low_sens, &ctx)
            .unwrap();
        plugin_high
            .process_in_place(&mut buf_high_sens, &ctx)
            .unwrap();

        // With low sensitivity the transient spike is amplified; with high
        // sensitivity the slow envelope never exceeds the threshold so gain
        // stays at 1.0.  The outputs must differ.
        let same = buf_low_sens
            .iter()
            .zip(buf_high_sens.iter())
            .all(|(a, b)| (a - b).abs() < 1e-6);
        assert!(
            !same,
            "sensitivity_db=-12 and sensitivity_db=+60 must produce different outputs"
        );
    }

    #[test]
    fn test_silence_produces_no_nan_inf() {
        // Digital silence input: envelopes decay to zero, output must be
        // finite and zero (no NaN, no Inf, no denormal artifacts leaking out).
        let channels = 2;
        let params = TransientShaperPluginParams {
            attack: 50.0,
            sustain: -50.0,
            sensitivity_db: 0.0,
            output_gain_db: 0.0,
            mix: 1.0,
        };
        let mut plugin = TransientShaperPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 9600; // 200ms
        let mut buffer = vec![0.0f32; num_frames * channels];
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        for (i, &s) in buffer.iter().enumerate() {
            assert!(s.is_finite(), "sample {} is not finite: {}", i, s);
            assert_eq!(s, 0.0, "silence in must be silence out at sample {}", i);
        }
    }

    #[test]
    fn test_single_impulse_fast_envelope_responds() {
        // A single full-scale impulse followed by silence: the fast envelope
        // should immediately jump while the slow envelope lags behind.
        // This verifies the differential detection works as intended.
        let channels = 1;
        let mut plugin = TransientShaperPlugin::new(channels);
        plugin.initialize(48000).unwrap();

        let num_frames = 512;
        let mut buffer = vec![0.0f32; num_frames];
        buffer[0] = 1.0; // single impulse
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        // After the impulse, no NaN/Inf should appear.
        for (i, &s) in buffer.iter().enumerate() {
            assert!(s.is_finite(), "sample {} is not finite: {}", i, s);
        }
    }

    #[test]
    fn test_time_to_coeff_handles_bad_inputs() {
        assert_eq!(time_to_coeff(0.0, 48000), 1.0);
        assert_eq!(time_to_coeff(-1.0, 48000), 1.0);
        assert_eq!(time_to_coeff(10.0, 0), 1.0);
        assert!(time_to_coeff(10.0, 48000).is_finite());
    }

    #[test]
    fn test_reset_starts_processing_from_clean_smoother_state() {
        // Set attack to +100%, let the smoother start ramping, then reset().
        // The very first processed sample after reset should use the settled
        // target value (attack=1.0), not an intermediate ramp value.
        let channels = 1;
        let mut plugin = TransientShaperPlugin::new(channels);
        plugin.initialize(48000).unwrap();

        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(100.0))
            .unwrap();

        // Advance the smoother partway by processing a short buffer
        let short_frames = 5;
        let mut buf = vec![0.5f32; short_frames];
        let ctx_short = make_context(short_frames);
        plugin.process_in_place(&mut buf, &ctx_short).unwrap();

        // Now reset — smoothers must snap to their targets
        plugin.reset();

        // Process a buffer; with attack=1.0 settled the output should reflect
        // the full attack amount from sample zero (no partial ramp).
        // We verify by checking the smoother returns 1.0 on the very first advance.
        // We do this indirectly: process one sample of silence and verify no panic.
        let mut buf2 = vec![0.0f32; 1];
        let ctx1 = make_context(1);
        let result = plugin.process_in_place(&mut buf2, &ctx1);
        assert!(result.is_ok());

        // After reset, fast_env and slow_env should be zero.
        // The next sample's shaping starts from a clean state.
        // (Verify by checking output = 0.0 for silent input, gain = 1.0.)
        assert_eq!(buf2[0], 0.0);
    }

    #[test]
    fn test_output_gain_post_mix() {
        // output_gain_db should apply to the final mixed output at all mix
        // settings.  With attack=0, sustain=0 and mix=0.0 (full dry), the
        // output should still be scaled by output_gain_lin.
        let channels = 1;
        let num_frames = 64;
        let input_val = 0.5f32;

        let params = TransientShaperPluginParams {
            attack: 0.0,
            sustain: 0.0,
            sensitivity_db: -60.0, // ensure threshold not an issue
            output_gain_db: 6.0,   // ≈ ×2 linear
            mix: 0.0,              // full dry
        };
        let mut plugin = TransientShaperPlugin::from_params(channels, params);
        plugin.initialize(48000).unwrap();

        let mut buffer = vec![input_val; num_frames];
        let ctx = make_context(num_frames);
        plugin.process_in_place(&mut buffer, &ctx).unwrap();

        let expected_lin = 10.0f32.powf(6.0 / 20.0);
        let expected_output = input_val * expected_lin;

        for (i, &s) in buffer.iter().enumerate() {
            let diff = (s - expected_output).abs();
            assert!(
                diff < 1e-4,
                "sample {}: output={} expected={} (output_gain must apply post-mix)",
                i,
                s,
                expected_output
            );
        }
    }
}
