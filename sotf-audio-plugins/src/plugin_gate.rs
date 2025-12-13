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
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

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
        PluginInfo {
            name: "Gate".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Noise gate with hold time for removing background noise".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("threshold", "Threshold", THRESHOLD_DEFAULT, THRESHOLD_MIN, THRESHOLD_MAX)
                .with_description("Level below which gate closes (dB)"),
            Parameter::new_float("ratio", "Ratio", RATIO_DEFAULT, RATIO_MIN, RATIO_MAX)
                .with_description("Gate depth ratio (higher = more attenuation)"),
            Parameter::new_float("attack", "Attack", ATTACK_DEFAULT, ATTACK_MIN, ATTACK_MAX)
                .with_description("Time to open gate (ms)"),
            Parameter::new_float("hold", "Hold", HOLD_DEFAULT, HOLD_MIN, HOLD_MAX)
                .with_description("Time to keep gate open after signal drops (ms)"),
            Parameter::new_float("release", "Release", RELEASE_DEFAULT, RELEASE_MIN, RELEASE_MAX)
                .with_description("Time to close gate (ms)"),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = gated)"),
            Parameter::new_bool("link_channels", "Link Channels", LINK_CHANNELS_DEFAULT)
                .with_description("Use linked sidechain for all channels"),
            Parameter::new_float("sidechain_hpf_hz", "Sidechain HPF", SIDECHAIN_HPF_HZ_DEFAULT, SIDECHAIN_HPF_HZ_MIN, SIDECHAIN_HPF_HZ_MAX)
                .with_description("High-pass filter frequency for sidechain (Hz)"),
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
    ) -> PluginResult<()> {
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

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        0
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
}
