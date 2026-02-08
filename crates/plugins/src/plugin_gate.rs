// ============================================================================
// Gate Plugin
// ============================================================================
//
// Noise gate that silences audio below a specified threshold.
// Useful for removing background noise and mic bleed.
//
// Parameters:
// - threshold: Level below which the gate closes (dB)
// - ratio: Gate depth ratio (1.0 = no effect, inf = complete silence)
// - attack: Time to open the gate (ms)
// - hold: Time to keep gate open after signal drops (ms)
// - release: Time to close the gate (ms)
// - mix: Dry/wet mix (0.0 = dry, 1.0 = wet)
// - link_channels: Link channels for stereo detection (true = linked, false = unlinked)
// - sidechain_hpf_hz: Sidechain high-pass filter cutoff frequency (Hz)

use super::param_specs::gate::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

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

/// Data exposed by the gate for monitoring
#[derive(Debug, Clone)]
pub struct GateData {
    /// Current input level in dB (one per channel)
    pub input_levels_db: Vec<f32>,
    /// Current gate status (true = open, false = closed)
    pub is_open: bool,
    /// Current attenuation in dB (positive value, e.g., 60.0 means -60dB gain)
    /// This is the envelope value, so it reflects attack/release smoothing
    pub attenuation_db: Vec<f32>,
}

/// Configuration parameters for GatePlugin
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

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Noise gate with hold time
pub struct GatePlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_threshold: ParameterId,
    threshold_db: f32,

    param_ratio: ParameterId,
    ratio: f32, // 1.0 = no effect, inf = complete silence

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

    // State per channel
    envelope: Vec<f32>,       // Current gate envelope per channel
    hold_counter: Vec<usize>, // Samples remaining in hold state
    attack_coeff: f32,
    release_coeff: f32,
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    sidechain_hpf_alpha: f32,
}

impl GatePlugin {
    /// Create a new gate plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `threshold_db` - Threshold in dB (default: -40.0)
    /// * `ratio` - Gate depth ratio (default: 10.0, use large values for hard gate)
    /// * `attack_ms` - Attack time in milliseconds (default: 1.0)
    /// * `hold_ms` - Hold time in milliseconds (default: 10.0)
    /// * `release_ms` - Release time in milliseconds (default: 100.0)
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Self {
        Self {
            channels,
            sample_rate: 44100, // Updated in initialize()

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
            hold_counter: vec![0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            sidechain_hpf_alpha: 0.0,
        }
    }

    /// Create a new gate plugin from configuration parameters
    pub fn from_params(channels: usize, params: GatePluginParams) -> Self {
        let mut plugin = Self::new(
            channels,
            params.threshold_db,
            params.ratio,
            params.attack_ms,
            params.hold_ms,
            params.release_ms,
        );

        plugin.mix = params.mix.clamp(0.0, 1.0);
        plugin.link_channels = params.link_channels;
        plugin.sidechain_hpf_hz = params.sidechain_hpf_hz.max(0.0);

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

    /// Calculate gate attenuation for a given input level
    fn calculate_gate_attenuation(&self, input_db: f32) -> f32 {
        if input_db >= self.threshold_db {
            // Above threshold - gate is open (no attenuation)
            0.0
        } else {
            // Below threshold - apply attenuation
            let below_threshold = self.threshold_db - input_db;
            let ratio = self.ratio.max(1.0);
            below_threshold * (1.0 - 1.0 / ratio)
        }
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_ms, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);
        let fc = self.sidechain_hpf_hz.max(0.0);
        if fc > 0.0 && self.sample_rate > 0 {
            let dt = 1.0 / self.sample_rate as f32;
            let rc = 1.0 / (2.0 * PI * fc);
            self.sidechain_hpf_alpha = rc / (rc + dt);
        } else {
            self.sidechain_hpf_alpha = 0.0;
        }
    }

    fn apply_sidechain_filter(&mut self, channel: usize, sample: f32) -> f32 {
        if self.sidechain_hpf_alpha <= 0.0 {
            return sample;
        }

        let prev_in = self.sidechain_hpf_prev_input[channel];
        let prev_out = self.sidechain_hpf_prev_output[channel];
        let alpha = self.sidechain_hpf_alpha;

        let y = alpha * (prev_out + sample - prev_in);
        self.sidechain_hpf_prev_input[channel] = sample;
        self.sidechain_hpf_prev_output[channel] = y;
        y
    }

    /// Get hold time in samples
    fn hold_samples(&self) -> usize {
        (self.hold_ms * 0.001 * self.sample_rate as f32) as usize
    }
}

impl InPlacePlugin for GatePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gate", "1.0.0", "SotF")
            .with_description("Noise gate with hold time for removing background noise")
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
            .with_description("Level below which gate closes (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("ratio", "Ratio", RATIO_DEFAULT, RATIO_MIN, RATIO_MAX)
                .with_description("Gate depth ratio (higher = more attenuation)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("attack", "Attack", ATTACK_DEFAULT, ATTACK_MIN, ATTACK_MAX)
                .with_description("Time to open gate (ms)")
                .with_group("Timing")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("hold", "Hold", HOLD_DEFAULT, HOLD_MIN, HOLD_MAX)
                .with_description("Time to keep gate open after signal drops (ms)")
                .with_group("Timing")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "release",
                "Release",
                RELEASE_DEFAULT,
                RELEASE_MIN,
                RELEASE_MAX,
            )
            .with_description("Time to close gate (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = gated)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("link_channels", "Link Channels", LINK_CHANNELS_DEFAULT)
                .with_description("Use linked sidechain for all channels")
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "sidechain_hpf_hz",
                "Sidechain HPF",
                SIDECHAIN_HPF_HZ_DEFAULT,
                SIDECHAIN_HPF_HZ_MIN,
                SIDECHAIN_HPF_HZ_MAX,
            )
            .with_description("High-pass filter frequency for sidechain (Hz)")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_threshold {
            self.threshold_db = value.as_float().ok_or("Invalid threshold value")?;
        } else if id == self.param_ratio {
            self.ratio = value.as_float().ok_or("Invalid ratio value")?.max(1.0);
        } else if id == self.param_attack {
            self.attack_ms = value.as_float().ok_or("Invalid attack value")?;
            self.update_coefficients();
        } else if id == self.param_hold {
            self.hold_ms = value.as_float().ok_or("Invalid hold value")?;
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release value")?;
            self.update_coefficients();
        } else if id == self.param_mix {
            self.mix = value.as_float().ok_or("Invalid mix value")?.clamp(0.0, 1.0);
        } else if id == self.param_link_channels {
            self.link_channels = value.as_bool().ok_or("Invalid link channels value")?;
        } else if id == self.param_sidechain_hpf_hz {
            self.sidechain_hpf_hz = value
                .as_float()
                .ok_or("Invalid sidechain high-pass value")?
                .max(0.0);
            self.update_coefficients();
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
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
        let num_frames = context.num_frames;
        let hold_samples = self.hold_samples();
        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        for frame in 0..num_frames {
            if self.link_channels && self.channels > 1 {
                let mut detection_level = 0.0_f32;

                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let sidechain_sample = self.apply_sidechain_filter(ch, input_sample);
                    let level = sidechain_sample.abs();
                    detection_level = detection_level.max(level);
                }

                let detection_level = detection_level.max(1e-10);
                let input_db = 20.0 * detection_level.log10();
                let target_attenuation = self.calculate_gate_attenuation(input_db);

                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];

                    // State machine for gate behavior
                    let target_envelope = if input_db >= self.threshold_db {
                        // Signal above threshold - open gate (reset hold)
                        self.hold_counter[ch] = hold_samples;
                        0.0
                    } else if self.hold_counter[ch] > 0 {
                        // In hold period - keep gate open
                        self.hold_counter[ch] -= 1;
                        0.0
                    } else {
                        // Below threshold and hold expired - close gate
                        target_attenuation
                    };

                    // Smooth envelope follower
                    let coeff = if target_envelope > self.envelope[ch] {
                        self.release_coeff // Closing gate (increasing attenuation)
                    } else {
                        self.attack_coeff // Opening gate (decreasing attenuation)
                    };

                    self.envelope[ch] =
                        target_envelope + coeff * (self.envelope[ch] - target_envelope);

                    // Apply gate with dry/wet mix
                    let gain_linear = 10.0_f32.powf(-self.envelope[ch] / 20.0);
                    let wet = input_sample * gain_linear;
                    buffer[sample_idx] = dry_mix * input_sample + wet_mix * wet;
                }
            } else {
                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let sidechain_sample = self.apply_sidechain_filter(ch, input_sample);

                    // Convert to dB
                    let input_level = sidechain_sample.abs().max(1e-10);
                    let input_db = 20.0 * input_level.log10();

                    // Calculate target attenuation
                    let target_attenuation = self.calculate_gate_attenuation(input_db);

                    // State machine for gate behavior
                    let target_envelope = if input_db >= self.threshold_db {
                        // Signal above threshold - open gate (reset hold)
                        self.hold_counter[ch] = hold_samples;
                        0.0
                    } else if self.hold_counter[ch] > 0 {
                        // In hold period - keep gate open
                        self.hold_counter[ch] -= 1;
                        0.0
                    } else {
                        // Below threshold and hold expired - close gate
                        target_attenuation
                    };

                    // Smooth envelope follower
                    let coeff = if target_envelope > self.envelope[ch] {
                        self.release_coeff // Closing gate (increasing attenuation)
                    } else {
                        self.attack_coeff // Opening gate (decreasing attenuation)
                    };

                    self.envelope[ch] =
                        target_envelope + coeff * (self.envelope[ch] - target_envelope);

                    // Apply gate with dry/wet mix
                    let gain_linear = 10.0_f32.powf(-self.envelope[ch] / 20.0);
                    let wet = input_sample * gain_linear;
                    buffer[sample_idx] = dry_mix * input_sample + wet_mix * wet;
                }
            }
        }

        // Flush denormals to prevent CPU performance spikes and audio crackle
        // Gate envelope calculations can produce denormal numbers
        flush_denormals_inplace(buffer);

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        // We don't store input levels persistently in the struct yet,
        // so for now we'll just report the attenuation envelope.
        // In a future update, we should track input levels for the UI too.

        // Gate is considered "open" if any channel has 0dB attenuation
        let is_open = self.envelope.iter().any(|&atten| atten < 0.1);

        // Placeholder for input levels (would need to track these in process_in_place)
        let input_levels_db = vec![-100.0; self.channels];

        Some(Arc::new(GateData {
            input_levels_db,
            is_open,
            attenuation_db: self.envelope.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_creation() {
        let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
        assert_eq!(gate.channels(), 2);
        assert_eq!(gate.threshold_db, -40.0);
        assert_eq!(gate.ratio, 10.0);
    }

    #[test]
    fn test_gate_attenuation() {
        let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);

        // Above threshold - no attenuation
        let atten = gate.calculate_gate_attenuation(-30.0);
        assert_eq!(atten, 0.0);

        // Below threshold - attenuate
        // 10 dB below threshold with 10:1 ratio
        // Attenuation = 10 * (1 - 1/10) = 9 dB
        let atten = gate.calculate_gate_attenuation(-50.0);
        assert!((atten - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_gate_additional_parameters_defaults() {
        let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);

        assert_eq!(gate.mix, 1.0);
        assert!(gate.link_channels);
        assert_eq!(gate.sidechain_hpf_hz, 0.0);
    }

    #[test]
    fn test_gate_mix_and_sidechain_parameters_set_get() {
        let mut gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
        gate.initialize(48000).unwrap();

        gate.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        gate.set_parameter(
            ParameterId::from("sidechain_hpf_hz"),
            ParameterValue::Float(120.0),
        )
        .unwrap();

        let mix = gate.get_parameter(&ParameterId::from("mix"));
        let sidechain = gate.get_parameter(&ParameterId::from("sidechain_hpf_hz"));

        assert_eq!(mix, Some(ParameterValue::Float(0.5)));
        assert_eq!(sidechain, Some(ParameterValue::Float(120.0)));
    }

    #[test]
    fn test_gate_process_loud_signal_passes() {
        // Test that a loud signal above threshold passes through unattenuated
        let mut gate = GatePlugin::new(2, -20.0, 100.0, 1.0, 10.0, 50.0);
        gate.initialize(48000).unwrap();

        let sample_rate = 48000.0;
        let duration = 0.5; // 500ms
        let num_frames = (duration * sample_rate) as usize;
        let channels = 2;

        // Generate loud sine wave at 0.5 amplitude (-6 dB, above -20 dB threshold)
        let mut buffer: Vec<f32> = (0..num_frames)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
                vec![sample, sample] // stereo
            })
            .collect();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        gate.process_in_place(&mut buffer, &context).unwrap();

        // Calculate RMS of output (skip first 100ms for gate to settle)
        let skip_frames = (0.1 * sample_rate) as usize;
        let rms: f32 = buffer
            .chunks(channels)
            .skip(skip_frames)
            .map(|frame| frame[0] * frame[0])
            .sum::<f32>()
            / (num_frames - skip_frames) as f32;
        let rms = rms.sqrt();

        // Expected RMS for sine at 0.5 amplitude: 0.5 / sqrt(2) ≈ 0.354
        let expected_rms = 0.354;
        assert!(
            (rms - expected_rms).abs() < 0.05,
            "Loud signal should pass through gate. Expected RMS ~{:.3}, got {:.3}",
            expected_rms,
            rms
        );
    }

    #[test]
    fn test_gate_process_quiet_signal_attenuated() {
        // Test that a quiet signal below threshold is attenuated
        let mut gate = GatePlugin::new(2, -20.0, 100.0, 1.0, 10.0, 50.0);
        gate.initialize(48000).unwrap();

        let sample_rate = 48000.0;
        let duration = 1.0; // 1 second to let gate fully close
        let num_frames = (duration * sample_rate) as usize;
        let channels = 2;

        // Generate quiet sine wave at 0.05 amplitude (-26 dB, below -20 dB threshold)
        let mut buffer: Vec<f32> = (0..num_frames)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.05;
                vec![sample, sample] // stereo
            })
            .collect();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        gate.process_in_place(&mut buffer, &context).unwrap();

        // Calculate RMS of output (analyze last 500ms when gate should be fully closed)
        let analyze_start = (0.5 * sample_rate) as usize;
        let rms: f32 = buffer
            .chunks(channels)
            .skip(analyze_start)
            .map(|frame| frame[0] * frame[0])
            .sum::<f32>()
            / (num_frames - analyze_start) as f32;
        let rms = rms.sqrt();

        // Input RMS: 0.05 / sqrt(2) ≈ 0.035 (-29 dB)
        // With threshold -20 dB and ratio 100:1, signal at -26 dB is 6 dB below threshold
        // Attenuation = 6 * (1 - 1/100) ≈ 5.94 dB
        // Expected output: -29 - 5.94 ≈ -35 dB → RMS ≈ 0.018
        // Actually with 100:1 ratio, it should be almost silent
        let input_rms = 0.035;
        assert!(
            rms < input_rms * 0.5, // Should be significantly attenuated
            "Quiet signal should be attenuated by gate. Input RMS ~{:.4}, output RMS {:.4}",
            input_rms,
            rms
        );
    }

    #[test]
    fn test_gate_process_loud_then_quiet() {
        // Test a signal that starts loud (gate open) then goes quiet (gate closes)
        let mut gate = GatePlugin::new(2, -20.0, 100.0, 1.0, 10.0, 50.0);
        gate.initialize(48000).unwrap();

        let sample_rate = 48000.0;
        let duration = 2.0; // 2 seconds total
        let num_frames = (duration * sample_rate) as usize;
        let channels = 2;

        let loud_amp = 0.5; // -6 dB (above threshold)
        let quiet_amp = 0.05; // -26 dB (below threshold)

        // First half loud, second half quiet
        let mut buffer: Vec<f32> = (0..num_frames)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let amp = if t < 1.0 { loud_amp } else { quiet_amp };
                let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * amp;
                vec![sample, sample]
            })
            .collect();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        gate.process_in_place(&mut buffer, &context).unwrap();

        // Analyze first half (0.2s - 0.8s) - should be loud
        let first_start = (0.2 * sample_rate) as usize;
        let first_end = (0.8 * sample_rate) as usize;
        let first_rms: f32 = buffer
            .chunks(channels)
            .skip(first_start)
            .take(first_end - first_start)
            .map(|frame| frame[0] * frame[0])
            .sum::<f32>()
            / (first_end - first_start) as f32;
        let first_rms = first_rms.sqrt();

        // Analyze second half (1.2s - 1.8s) - should be quiet (gated)
        let second_start = (1.2 * sample_rate) as usize;
        let second_end = (1.8 * sample_rate) as usize;
        let second_rms: f32 = buffer
            .chunks(channels)
            .skip(second_start)
            .take(second_end - second_start)
            .map(|frame| frame[0] * frame[0])
            .sum::<f32>()
            / (second_end - second_start) as f32;
        let second_rms = second_rms.sqrt();

        let first_db = 20.0 * first_rms.log10();
        let second_db = 20.0 * second_rms.log10();
        let reduction_db = first_db - second_db;

        println!(
            "First half RMS: {:.4} ({:.2} dB), Second half RMS: {:.4} ({:.2} dB), Reduction: {:.2} dB",
            first_rms, first_db, second_rms, second_db, reduction_db
        );

        // First half should be around -9 dB (0.5 / sqrt(2))
        assert!(
            first_rms > 0.3,
            "First half should be loud, got RMS {:.4}",
            first_rms
        );

        // Second half should be significantly attenuated - at least 10 dB reduction
        assert!(
            reduction_db > 10.0,
            "Gate should reduce quiet signal by at least 10 dB, got {:.2} dB reduction",
            reduction_db
        );
    }

    #[test]
    fn test_gate_various_sample_rates() {
        for &sample_rate in &[22050, 44100, 48000, 96000, 192000] {
            let mut gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
            gate.initialize(sample_rate).unwrap();

            let num_frames = 512;
            let mut buffer: Vec<f32> = (0..num_frames * 2)
                .map(|i| {
                    let t = i as f32 / (sample_rate as f32 * 2.0);
                    (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
                })
                .collect();

            let context = ProcessContext {
                sample_rate,
                num_frames,
            };

            gate.process_in_place(&mut buffer, &context).unwrap();

            for s in &buffer {
                assert!(s.is_finite(), "Non-finite value at sample rate {}", sample_rate);
            }
        }
    }

    #[test]
    fn test_gate_time_to_coeff() {
        let coeff = GatePlugin::time_to_coeff(0.0, 48000);
        assert_eq!(coeff, 0.0);

        let coeff = GatePlugin::time_to_coeff(50.0, 48000);
        assert!(coeff > 0.0 && coeff < 1.0);

        // Higher sample rate should give different coefficient for same time
        let coeff_44k = GatePlugin::time_to_coeff(10.0, 44100);
        let coeff_96k = GatePlugin::time_to_coeff(10.0, 96000);
        assert!((coeff_44k - coeff_96k).abs() > 0.0001);
    }

    #[test]
    fn test_gate_from_params() {
        let params = GatePluginParams {
            threshold_db: -30.0,
            ratio: 20.0,
            attack_ms: 0.5,
            hold_ms: 20.0,
            release_ms: 200.0,
            mix: 0.8,
            link_channels: false,
            sidechain_hpf_hz: 100.0,
        };
        let gate = GatePlugin::from_params(2, params);
        assert_eq!(gate.threshold_db, -30.0);
        assert_eq!(gate.ratio, 20.0);
        assert_eq!(gate.mix, 0.8);
        assert!(!gate.link_channels);
        assert_eq!(gate.sidechain_hpf_hz, 100.0);
    }

    #[test]
    fn test_gate_reset() {
        let mut gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
        gate.initialize(48000).unwrap();

        // Process some data
        let num_frames = 256;
        let mut buffer = vec![0.5_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        gate.process_in_place(&mut buffer, &context).unwrap();

        gate.reset();

        // Envelope should be reset
        for &env in &gate.envelope {
            assert_eq!(env, 0.0);
        }
        for &hc in &gate.hold_counter {
            assert_eq!(hc, 0);
        }
    }

    #[test]
    fn test_gate_get_data() {
        let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
        let data = gate.get_data();
        assert!(data.is_some());
        let data = data.unwrap();
        let gate_data = data.downcast_ref::<GateData>().unwrap();
        assert_eq!(gate_data.attenuation_db.len(), 2);
    }

    #[test]
    fn test_gate_unlinked_channels() {
        let mut gate = GatePlugin::new(2, -20.0, 100.0, 1.0, 5.0, 50.0);
        gate.link_channels = false;
        gate.initialize(48000).unwrap();

        let num_frames = 1024;
        let mut buffer = vec![0.0_f32; num_frames * 2];

        // Left channel loud, right channel quiet
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            buffer[i * 2] = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5; // -6dB
            buffer[i * 2 + 1] = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.01; // -40dB
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        gate.process_in_place(&mut buffer, &context).unwrap();

        // With unlinked channels, left should be louder than right
        let left_energy: f32 = (0..num_frames).map(|i| buffer[i * 2] * buffer[i * 2]).sum();
        let right_energy: f32 = (0..num_frames)
            .map(|i| buffer[i * 2 + 1] * buffer[i * 2 + 1])
            .sum();

        assert!(
            left_energy > right_energy * 10.0,
            "Left should be much louder than right with unlinked gate"
        );
    }

    mod proptest_gate {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Property: gate should attenuate signal below threshold
            #[test]
            fn attenuates_below_threshold(
                threshold_db in -60.0f32..-10.0,
            ) {
                // Input well below threshold
                let input_amplitude = 10.0f32.powf((threshold_db - 20.0) / 20.0);
                let mut gate = GatePlugin::new(1, threshold_db, 100.0, 1.0, 0.0, 50.0);
                gate.initialize(48000).unwrap();

                // Process many blocks for gate to fully close
                let num_frames = 8192;
                let mut buffer = vec![input_amplitude; num_frames];
                let context = ProcessContext { sample_rate: 48000, num_frames };
                gate.process_in_place(&mut buffer, &context).unwrap();

                // Last samples should be significantly attenuated
                let output_rms: f32 = buffer[num_frames - 512..]
                    .iter()
                    .map(|s| s * s)
                    .sum::<f32>()
                    / 512.0;
                let input_rms = input_amplitude * input_amplitude;

                prop_assert!(
                    output_rms < input_rms * 0.5,
                    "Gate should attenuate below threshold: output_rms={:.6}, input_rms={:.6}",
                    output_rms, input_rms
                );
            }

            /// Property: gate should not produce NaN or Inf
            #[test]
            fn no_nan_or_inf(
                threshold_db in -80.0f32..0.0,
                ratio in 1.0f32..100.0,
                input_amplitude in 0.001f32..2.0,
            ) {
                let mut gate = GatePlugin::new(1, threshold_db, ratio, 1.0, 0.0, 50.0);
                gate.initialize(48000).unwrap();

                let num_frames = 1024;
                let mut buffer = vec![input_amplitude; num_frames];
                let context = ProcessContext { sample_rate: 48000, num_frames };
                gate.process_in_place(&mut buffer, &context).unwrap();

                for &sample in &buffer {
                    prop_assert!(!sample.is_nan(), "NaN in output");
                    prop_assert!(!sample.is_infinite(), "Inf in output");
                }
            }
        }
    }
}
