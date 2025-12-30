// ============================================================================
// Expander Plugin
// ============================================================================
//
// Dynamic range expander that reduces the volume of signals below a threshold.
// More gradual than a gate, useful for reducing background noise while
// maintaining natural sound.
//
// Parameters:
// - threshold: Level below which expansion starts (dB)
// - ratio: Expansion ratio (1.0 = no expansion, inf = gate)
// - attack: Time to reach full expansion (ms)
// - release: Time to return to no expansion (ms)
// - range: Maximum attenuation (dB) - floor limit
// - knee: Soft knee width for smoother expansion (dB)
// - hysteresis: Difference between open and close thresholds (dB)
// - hold: Time to keep gate open after signal drops (ms)
// - mix: Dry/wet mix (0.0 = dry, 1.0 = wet)
// - link_channels: Use a shared detector across channels
// - sidechain_hpf_hz: High-pass filter cutoff for the detector sidechain (Hz)

use super::param_specs::expander::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
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

fn default_release_ms() -> f32 {
    RELEASE_DEFAULT
}

fn default_range_db() -> f32 {
    RANGE_DEFAULT
}

fn default_knee_db() -> f32 {
    KNEE_DEFAULT
}

fn default_hysteresis_db() -> f32 {
    HYSTERESIS_DEFAULT
}

fn default_hold_ms() -> f32 {
    HOLD_DEFAULT
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

/// Data exposed by the expander for monitoring
#[derive(Debug, Clone)]
pub struct ExpanderData {
    /// Current attenuation in dB (positive value, e.g., 6.0 means -6dB gain)
    /// One value per channel
    pub attenuation_db: Vec<f32>,
    /// Current gate state (true = open/passing signal, false = closed/attenuating)
    pub is_open: bool,
    /// Current input levels in dB (one per channel)
    pub input_levels_db: Vec<f32>,
}

/// Configuration parameters for ExpanderPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpanderPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_hysteresis_db")]
    pub hysteresis_db: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
}

impl Default for ExpanderPluginParams {
    fn default() -> Self {
        Self {
            threshold_db: default_threshold_db(),
            ratio: default_ratio(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            range_db: default_range_db(),
            knee_db: default_knee_db(),
            hysteresis_db: default_hysteresis_db(),
            hold_ms: default_hold_ms(),
            mix: default_mix(),
            link_channels: default_link_channels(),
            sidechain_hpf_hz: default_sidechain_hpf_hz(),
        }
    }
}

// ============================================================================
// Hysteresis State Machine
// ============================================================================

/// State of the expander gate for hysteresis handling
#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    /// Signal is above threshold - gate is open, no attenuation
    Open,
    /// Signal dropped below threshold but in hold period
    Hold,
    /// Signal is below close threshold - gate is closing/closed
    Closing,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Dynamic range expander with hysteresis
pub struct ExpanderPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_threshold: ParameterId,
    threshold_db: f32,

    param_ratio: ParameterId,
    ratio: f32,

    param_attack: ParameterId,
    attack_ms: f32,

    param_release: ParameterId,
    release_ms: f32,

    param_range: ParameterId,
    range_db: f32,

    param_knee: ParameterId,
    knee_db: f32,

    param_hysteresis: ParameterId,
    hysteresis_db: f32,

    param_hold: ParameterId,
    hold_ms: f32,

    param_mix: ParameterId,
    mix: f32,

    param_link_channels: ParameterId,
    link_channels: bool,

    param_sidechain_hpf_hz: ParameterId,
    sidechain_hpf_hz: f32,

    // State per channel
    envelope: Vec<f32>,           // Current attenuation envelope per channel
    gate_state: Vec<GateState>,   // Hysteresis state per channel
    hold_counter: Vec<usize>,     // Samples remaining in hold state
    input_levels_db: Vec<f32>,    // Last input level for monitoring

    // Sidechain HPF state
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    sidechain_hpf_alpha: f32,

    // Coefficients
    attack_coeff: f32,
    release_coeff: f32,
}

impl ExpanderPlugin {
    /// Create a new expander plugin with default parameters
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, ExpanderPluginParams::default())
    }

    /// Create a new expander plugin with custom parameters
    pub fn with_params(channels: usize, params: ExpanderPluginParams) -> Self {
        Self {
            channels,
            sample_rate: 44100, // Updated in initialize()

            param_threshold: ParameterId::from("threshold"),
            threshold_db: params.threshold_db,

            param_ratio: ParameterId::from("ratio"),
            ratio: params.ratio,

            param_attack: ParameterId::from("attack"),
            attack_ms: params.attack_ms,

            param_release: ParameterId::from("release"),
            release_ms: params.release_ms,

            param_range: ParameterId::from("range"),
            range_db: params.range_db,

            param_knee: ParameterId::from("knee"),
            knee_db: params.knee_db,

            param_hysteresis: ParameterId::from("hysteresis"),
            hysteresis_db: params.hysteresis_db,

            param_hold: ParameterId::from("hold"),
            hold_ms: params.hold_ms,

            param_mix: ParameterId::from("mix"),
            mix: params.mix.clamp(0.0, 1.0),

            param_link_channels: ParameterId::from("link_channels"),
            link_channels: params.link_channels,

            param_sidechain_hpf_hz: ParameterId::from("sidechain_hpf_hz"),
            sidechain_hpf_hz: params.sidechain_hpf_hz.max(0.0),

            envelope: vec![0.0; channels],
            gate_state: vec![GateState::Open; channels],
            hold_counter: vec![0; channels],
            input_levels_db: vec![-100.0; channels],
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            sidechain_hpf_alpha: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
        }
    }

    /// Create from params (for compatibility with other plugins)
    pub fn from_params(channels: usize, params: ExpanderPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    /// Calculate time coefficient for envelope follower
    fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
        }
    }

    /// Calculate expansion attenuation for a given input level
    ///
    /// Downward expansion: reduces gain for signals BELOW threshold
    fn calculate_expansion_attenuation(&self, input_db: f32) -> f32 {
        let threshold = self.threshold_db;
        let knee = self.knee_db.max(0.0);
        let ratio = self.ratio.max(1.0);
        let range = self.range_db.max(0.0);

        // Expansion slope: how much to reduce signal below threshold
        // For ratio 2:1, a signal 10dB below threshold gets attenuated by 5dB
        let slope = 1.0 - 1.0 / ratio;

        let attenuation = if knee < 0.1 {
            // Hard knee expansion
            if input_db >= threshold {
                0.0
            } else {
                let below_threshold = threshold - input_db;
                below_threshold * slope
            }
        } else {
            // Soft knee expansion
            if input_db > threshold + knee / 2.0 {
                // Above threshold + knee - no expansion
                0.0
            } else if input_db < threshold - knee / 2.0 {
                // Below threshold - full expansion
                let below_threshold = threshold - input_db;
                below_threshold * slope
            } else {
                // In the knee - smooth quadratic transition
                let below = threshold + knee / 2.0 - input_db;
                let knee_factor = below / knee;
                knee_factor * knee_factor * knee / 2.0 * slope
            }
        };

        // Apply range limit (floor)
        attenuation.min(range)
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_ms, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);

        // Update sidechain HPF
        let fc = self.sidechain_hpf_hz.max(0.0);
        if fc > 0.0 && self.sample_rate > 0 {
            let dt = 1.0 / self.sample_rate as f32;
            let rc = 1.0 / (2.0 * PI * fc);
            self.sidechain_hpf_alpha = rc / (rc + dt);
        } else {
            self.sidechain_hpf_alpha = 0.0;
        }
    }

    /// Apply sidechain high-pass filter
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

    /// Process a single channel's gate state and envelope
    fn process_channel(
        &mut self,
        channel: usize,
        input_db: f32,
        hold_samples: usize,
    ) -> f32 {
        let open_threshold = self.threshold_db;
        let close_threshold = self.threshold_db - self.hysteresis_db;

        // Hysteresis state machine
        let target_attenuation = match self.gate_state[channel] {
            GateState::Open => {
                if input_db < open_threshold {
                    // Signal dropped below open threshold, start hold
                    self.gate_state[channel] = GateState::Hold;
                    self.hold_counter[channel] = hold_samples;
                    0.0 // Still open during hold
                } else {
                    0.0 // Still open
                }
            }
            GateState::Hold => {
                if input_db >= open_threshold {
                    // Signal came back up, return to open
                    self.gate_state[channel] = GateState::Open;
                    self.hold_counter[channel] = 0;
                    0.0
                } else if self.hold_counter[channel] > 0 {
                    // Still in hold period
                    self.hold_counter[channel] -= 1;
                    0.0
                } else {
                    // Hold expired, check close threshold
                    if input_db < close_threshold {
                        self.gate_state[channel] = GateState::Closing;
                        self.calculate_expansion_attenuation(input_db)
                    } else {
                        // Between open and close thresholds, stay in hold-like state
                        0.0
                    }
                }
            }
            GateState::Closing => {
                if input_db >= open_threshold {
                    // Signal came back up, open gate
                    self.gate_state[channel] = GateState::Open;
                    self.hold_counter[channel] = 0;
                    0.0
                } else {
                    // Continue closing/attenuating
                    self.calculate_expansion_attenuation(input_db)
                }
            }
        };

        // Smooth envelope follower
        // Attack = gate opening (attenuation decreasing toward 0)
        // Release = gate closing (attenuation increasing)
        let coeff = if target_attenuation > self.envelope[channel] {
            self.release_coeff // Closing gate (increasing attenuation)
        } else {
            self.attack_coeff // Opening gate (decreasing attenuation)
        };

        self.envelope[channel] =
            target_attenuation + coeff * (self.envelope[channel] - target_attenuation);

        self.envelope[channel]
    }
}

impl InPlacePlugin for ExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Expander".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Dynamic range expander with hysteresis and soft knee".to_string(),
        }
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
            .with_description("Level below which expansion starts (dB)"),
            Parameter::new_float("ratio", "Ratio", RATIO_DEFAULT, RATIO_MIN, RATIO_MAX)
                .with_description("Expansion ratio (higher = more attenuation)"),
            Parameter::new_float("attack", "Attack", ATTACK_DEFAULT, ATTACK_MIN, ATTACK_MAX)
                .with_description("Time to open gate (ms)"),
            Parameter::new_float(
                "release",
                "Release",
                RELEASE_DEFAULT,
                RELEASE_MIN,
                RELEASE_MAX,
            )
            .with_description("Time to close gate (ms)"),
            Parameter::new_float("range", "Range", RANGE_DEFAULT, RANGE_MIN, RANGE_MAX)
                .with_description("Maximum attenuation / floor limit (dB)"),
            Parameter::new_float("knee", "Knee", KNEE_DEFAULT, KNEE_MIN, KNEE_MAX)
                .with_description("Soft knee width (dB)"),
            Parameter::new_float(
                "hysteresis",
                "Hysteresis",
                HYSTERESIS_DEFAULT,
                HYSTERESIS_MIN,
                HYSTERESIS_MAX,
            )
            .with_description("Difference between open and close thresholds (dB)"),
            Parameter::new_float("hold", "Hold", HOLD_DEFAULT, HOLD_MIN, HOLD_MAX)
                .with_description("Time to keep gate open after signal drops (ms)"),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = expanded)"),
            Parameter::new_bool("link_channels", "Link Channels", LINK_CHANNELS_DEFAULT)
                .with_description("Use linked sidechain for all channels"),
            Parameter::new_float(
                "sidechain_hpf_hz",
                "Sidechain HPF",
                SIDECHAIN_HPF_HZ_DEFAULT,
                SIDECHAIN_HPF_HZ_MIN,
                SIDECHAIN_HPF_HZ_MAX,
            )
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
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release value")?;
            self.update_coefficients();
        } else if id == self.param_range {
            self.range_db = value.as_float().ok_or("Invalid range value")?.max(0.0);
        } else if id == self.param_knee {
            self.knee_db = value.as_float().ok_or("Invalid knee value")?.max(0.0);
        } else if id == self.param_hysteresis {
            self.hysteresis_db = value.as_float().ok_or("Invalid hysteresis value")?.max(0.0);
        } else if id == self.param_hold {
            self.hold_ms = value.as_float().ok_or("Invalid hold value")?.max(0.0);
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
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_range {
            Some(ParameterValue::Float(self.range_db))
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_hysteresis {
            Some(ParameterValue::Float(self.hysteresis_db))
        } else if id == &self.param_hold {
            Some(ParameterValue::Float(self.hold_ms))
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
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
        self.input_levels_db.fill(-100.0);
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

        if self.link_channels && self.channels > 1 {
            // Linked mode: use max level across channels for detection
            for frame in 0..num_frames {
                let mut detection_level = 0.0_f32;

                // Find max level across all channels
                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let sidechain_sample = self.apply_sidechain_filter(ch, input_sample);
                    let level = sidechain_sample.abs();
                    detection_level = detection_level.max(level);
                }

                // Convert to dB
                let detection_level = detection_level.max(1e-10);
                let input_db = 20.0 * detection_level.log10();

                // Process all channels with same detection level
                // Use channel 0's state for linked processing
                let attenuation = self.process_channel(0, input_db, hold_samples);

                // Copy state to all channels for consistent behavior
                for ch in 1..self.channels {
                    self.envelope[ch] = self.envelope[0];
                    self.gate_state[ch] = self.gate_state[0];
                    self.hold_counter[ch] = self.hold_counter[0];
                    self.input_levels_db[ch] = input_db;
                }
                self.input_levels_db[0] = input_db;

                // Apply gain to all channels
                let gain_linear = 10.0_f32.powf(-attenuation / 20.0);
                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let wet = input_sample * gain_linear;
                    buffer[sample_idx] = dry_mix * input_sample + wet_mix * wet;
                }
            }
        } else {
            // Unlinked mode: process each channel independently
            for frame in 0..num_frames {
                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let sidechain_sample = self.apply_sidechain_filter(ch, input_sample);

                    // Convert to dB
                    let input_level = sidechain_sample.abs().max(1e-10);
                    let input_db = 20.0 * input_level.log10();
                    self.input_levels_db[ch] = input_db;

                    // Process this channel
                    let attenuation = self.process_channel(ch, input_db, hold_samples);

                    // Apply gain with dry/wet mix
                    let gain_linear = 10.0_f32.powf(-attenuation / 20.0);
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

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        // Determine if gate is open (any channel has low attenuation)
        let is_open = self
            .gate_state
            .iter()
            .any(|&state| state == GateState::Open || state == GateState::Hold);

        Some(Arc::new(ExpanderData {
            attenuation_db: self.envelope.clone(),
            is_open,
            input_levels_db: self.input_levels_db.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expander_creation() {
        let expander = ExpanderPlugin::new(2);
        assert_eq!(expander.channels(), 2);
        assert_eq!(expander.threshold_db, THRESHOLD_DEFAULT);
        assert_eq!(expander.ratio, RATIO_DEFAULT);
    }

    #[test]
    fn test_expander_expansion_calculation() {
        let expander = ExpanderPlugin::with_params(
            2,
            ExpanderPluginParams {
                threshold_db: -40.0,
                ratio: 2.0,
                knee_db: 0.0, // Hard knee for predictable test
                range_db: 60.0,
                ..Default::default()
            },
        );

        // Above threshold - no attenuation
        let atten = expander.calculate_expansion_attenuation(-30.0);
        assert_eq!(atten, 0.0);

        // At threshold - no attenuation
        let atten = expander.calculate_expansion_attenuation(-40.0);
        assert_eq!(atten, 0.0);

        // 10 dB below threshold with 2:1 ratio
        // Attenuation = 10 * (1 - 1/2) = 5 dB
        let atten = expander.calculate_expansion_attenuation(-50.0);
        assert!((atten - 5.0).abs() < 0.01);

        // 20 dB below threshold with 2:1 ratio
        // Attenuation = 20 * (1 - 1/2) = 10 dB
        let atten = expander.calculate_expansion_attenuation(-60.0);
        assert!((atten - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_expander_range_limit() {
        let expander = ExpanderPlugin::with_params(
            2,
            ExpanderPluginParams {
                threshold_db: -40.0,
                ratio: 10.0,      // High ratio
                knee_db: 0.0,     // Hard knee
                range_db: 20.0,   // Limit to 20 dB
                ..Default::default()
            },
        );

        // Very far below threshold with high ratio
        // Without range limit: 40 * (1 - 1/10) = 36 dB
        // With range limit: capped at 20 dB
        let atten = expander.calculate_expansion_attenuation(-80.0);
        assert!((atten - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_expander_soft_knee() {
        let expander = ExpanderPlugin::with_params(
            2,
            ExpanderPluginParams {
                threshold_db: -40.0,
                ratio: 4.0,
                knee_db: 10.0, // 10 dB knee
                range_db: 60.0,
                ..Default::default()
            },
        );

        // Above knee region (threshold + knee/2 = -35 dB)
        let atten = expander.calculate_expansion_attenuation(-30.0);
        assert_eq!(atten, 0.0);

        // In knee region
        let atten = expander.calculate_expansion_attenuation(-40.0);
        assert!(atten > 0.0 && atten < 5.0);

        // Below knee region (threshold - knee/2 = -45 dB)
        let atten = expander.calculate_expansion_attenuation(-50.0);
        assert!(atten > 0.0);
    }

    #[test]
    fn test_expander_parameters_set_get() {
        let mut expander = ExpanderPlugin::new(2);
        expander.initialize(48000).unwrap();

        expander
            .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
            .unwrap();
        expander
            .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(3.0))
            .unwrap();
        expander
            .set_parameter(ParameterId::from("hysteresis"), ParameterValue::Float(6.0))
            .unwrap();

        assert_eq!(
            expander.get_parameter(&ParameterId::from("threshold")),
            Some(ParameterValue::Float(-30.0))
        );
        assert_eq!(
            expander.get_parameter(&ParameterId::from("ratio")),
            Some(ParameterValue::Float(3.0))
        );
        assert_eq!(
            expander.get_parameter(&ParameterId::from("hysteresis")),
            Some(ParameterValue::Float(6.0))
        );
    }
}
