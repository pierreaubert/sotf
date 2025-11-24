// ============================================================================
// Compressor Plugin
// ============================================================================
//
// Dynamic range compressor that reduces the volume of loud signals.
//
// Parameters:
// - threshold: Level above which compression starts (dB)
// - ratio: Compression ratio (1.0 = no compression, 10.0 = 10:1)
// - attack: Time to reach full compression (ms)
// - release: Time to return to no compression (ms)
// - knee: Soft knee width for smoother compression (dB)
// - makeup_gain: Output gain to compensate for volume reduction (dB)
// - mix: Dry/wet mix between unprocessed and compressed signal (0 = dry, 1 = wet)
// - auto_makeup: Automatically add makeup gain based on threshold/ratio
// - link_channels: Use a shared detector across channels to avoid image shifts
// - sidechain_hpf_hz: High-pass filter cutoff for the detector sidechain (Hz)

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

// ============================================================================
// Configuration
// ============================================================================

fn default_threshold_db() -> f32 {
    -20.0
}

fn default_ratio() -> f32 {
    4.0
}

fn default_attack_ms() -> f32 {
    5.0
}

fn default_release_ms() -> f32 {
    50.0
}

fn default_knee_db() -> f32 {
    6.0
}

fn default_makeup_gain_db() -> f32 {
    0.0
}

fn default_mix() -> f32 {
    1.0
}

fn default_auto_makeup() -> bool {
    false
}

fn default_link_channels() -> bool {
    true
}

fn default_sidechain_hpf_hz() -> f32 {
    80.0
}

/// Configuration parameters for CompressorPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressorPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_makeup_gain_db")]
    pub makeup_gain_db: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_auto_makeup")]
    pub auto_makeup: bool,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Dynamic range compressor
pub struct CompressorPlugin {
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

    param_knee: ParameterId,
    knee_db: f32,

    param_makeup_gain: ParameterId,
    makeup_gain_db: f32,

    param_mix: ParameterId,
    mix: f32,

    param_auto_makeup: ParameterId,
    auto_makeup: bool,

    param_link_channels: ParameterId,
    link_channels: bool,

    param_sidechain_hpf_hz: ParameterId,
    sidechain_hpf_hz: f32,

    // State per channel
    envelope: Vec<f32>, // Current gain reduction envelope per channel
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    attack_coeff: f32,
    release_coeff: f32,
    sidechain_hpf_alpha: f32,
}

impl CompressorPlugin {
    /// Create a new compressor plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `threshold_db` - Threshold in dB (default: -20.0)
    /// * `ratio` - Compression ratio (default: 4.0)
    /// * `attack_ms` - Attack time in milliseconds (default: 5.0)
    /// * `release_ms` - Release time in milliseconds (default: 50.0)
    /// * `knee_db` - Soft knee width in dB (default: 6.0)
    /// * `makeup_gain_db` - Makeup gain in dB (default: 0.0)
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
        makeup_gain_db: f32,
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

            param_release: ParameterId::from("release"),
            release_ms,

            param_knee: ParameterId::from("knee"),
            knee_db,

            param_makeup_gain: ParameterId::from("makeup_gain"),
            makeup_gain_db,

            param_mix: ParameterId::from("mix"),
            mix: 1.0,

            param_auto_makeup: ParameterId::from("auto_makeup"),
            auto_makeup: false,

            param_link_channels: ParameterId::from("link_channels"),
            link_channels: true,

            param_sidechain_hpf_hz: ParameterId::from("sidechain_hpf_hz"),
            sidechain_hpf_hz: 80.0,

            envelope: vec![0.0; channels],
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sidechain_hpf_alpha: 0.0,
        }
    }

    /// Create a new compressor plugin from configuration parameters
    pub fn from_params(channels: usize, params: CompressorPluginParams) -> Self {
        let mut plugin = Self::new(
            channels,
            params.threshold_db,
            params.ratio,
            params.attack_ms,
            params.release_ms,
            params.knee_db,
            params.makeup_gain_db,
        );

        plugin.mix = params.mix.clamp(0.0, 1.0);
        plugin.auto_makeup = params.auto_makeup;
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

    /// Calculate gain reduction for a given input level
    fn calculate_gain_reduction(&self, input_db: f32) -> f32 {
        let threshold = self.threshold_db;
        let knee = self.knee_db.max(0.0);
        let ratio = self.ratio.max(1.0);
        let slope = 1.0 - 1.0 / ratio;

        // Handle hard knee (knee = 0) separately
        if knee < 0.1 {
            // Hard knee compression
            if input_db <= threshold {
                0.0
            } else {
                let overshoot = input_db - threshold;
                overshoot * slope
            }
        } else {
            // Soft knee compression
            if input_db < threshold - knee / 2.0 {
                // Below threshold - no compression
                0.0
            } else if input_db > threshold + knee / 2.0 {
                // Above threshold + knee - full compression
                let overshoot = input_db - threshold;
                overshoot * slope
            } else {
                // In the knee - smooth transition
                let overshoot = input_db - threshold + knee / 2.0;
                let knee_factor = overshoot / knee;
                knee_factor * knee_factor * knee / 2.0 * slope
            }
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

    fn apply_gain_for_channel(
        &mut self,
        channel: usize,
        target_gr: f32,
        makeup_gain_linear: f32,
        input_sample: f32,
        dry_mix: f32,
        wet_mix: f32,
    ) -> f32 {
        let coeff = if target_gr > self.envelope[channel] {
            self.attack_coeff
        } else {
            self.release_coeff
        };

        self.envelope[channel] = target_gr + coeff * (self.envelope[channel] - target_gr);

        let wet_gain_linear = 10.0_f32.powf(-self.envelope[channel] / 20.0) * makeup_gain_linear;

        let wet = input_sample * wet_gain_linear;
        dry_mix * input_sample + wet_mix * wet
    }
}

impl InPlacePlugin for CompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Compressor".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Dynamic range compressor with soft knee".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("threshold", "Threshold", -20.0, -60.0, 0.0)
                .with_description("Level above which compression starts (dB)"),
            Parameter::new_float("ratio", "Ratio", 4.0, 1.0, 20.0)
                .with_description("Compression ratio (1:1 to 20:1)"),
            Parameter::new_float("attack", "Attack", 5.0, 0.1, 100.0)
                .with_description("Attack time (ms)"),
            Parameter::new_float("release", "Release", 50.0, 10.0, 1000.0)
                .with_description("Release time (ms)"),
            Parameter::new_float("knee", "Knee", 6.0, 0.0, 20.0)
                .with_description("Soft knee width (dB)"),
            Parameter::new_float("makeup_gain", "Makeup Gain", 0.0, -24.0, 24.0)
                .with_description("Output gain compensation (dB)"),
            Parameter::new_float("mix", "Mix", 1.0, 0.0, 1.0)
                .with_description("Dry/wet mix (0 = dry, 1 = compressed)"),
            Parameter::new_bool("auto_makeup", "Auto Makeup", false)
                .with_description("Automatically compensate for gain reduction"),
            Parameter::new_bool("link_channels", "Link Channels", true)
                .with_description("Use linked sidechain for all channels"),
            Parameter::new_float("sidechain_hpf_hz", "Sidechain HPF", 80.0, 0.0, 200.0)
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
        } else if id == self.param_knee {
            self.knee_db = value.as_float().ok_or("Invalid knee value")?.max(0.0);
        } else if id == self.param_makeup_gain {
            self.makeup_gain_db = value.as_float().ok_or("Invalid makeup gain value")?;
        } else if id == self.param_mix {
            self.mix = value.as_float().ok_or("Invalid mix value")?.clamp(0.0, 1.0);
        } else if id == self.param_auto_makeup {
            self.auto_makeup = value.as_bool().ok_or("Invalid auto makeup value")?;
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
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_makeup_gain {
            Some(ParameterValue::Float(self.makeup_gain_db))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if id == &self.param_auto_makeup {
            Some(ParameterValue::Bool(self.auto_makeup))
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
        self.sidechain_hpf_prev_input.fill(0.0);
        self.sidechain_hpf_prev_output.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;
        let ratio = self.ratio.max(1.0);
        let compression_slope = 1.0 - 1.0 / ratio;
        let auto_makeup_db = if self.auto_makeup {
            let avg_overshoot = (-self.threshold_db).max(0.0) * 0.5;
            avg_overshoot * compression_slope
        } else {
            0.0
        };
        let makeup_gain_linear = 10.0_f32.powf((self.makeup_gain_db + auto_makeup_db) / 20.0);
        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
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
                let target_gr = self.calculate_gain_reduction(input_db);

                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];

                    buffer[sample_idx] = self.apply_gain_for_channel(
                        ch,
                        target_gr,
                        makeup_gain_linear,
                        input_sample,
                        dry_mix,
                        wet_mix,
                    );
                }
            }
        } else {
            for frame in 0..num_frames {
                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let sidechain_sample = self.apply_sidechain_filter(ch, input_sample);

                    let input_level = sidechain_sample.abs().max(1e-10);
                    let input_db = 20.0 * input_level.log10();

                    let target_gr = self.calculate_gain_reduction(input_db);

                    buffer[sample_idx] = self.apply_gain_for_channel(
                        ch,
                        target_gr,
                        makeup_gain_linear,
                        input_sample,
                        dry_mix,
                        wet_mix,
                    );
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
    fn test_compressor_creation() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        assert_eq!(compressor.channels(), 2);
        assert_eq!(compressor.threshold_db, -20.0);
        assert_eq!(compressor.ratio, 4.0);
    }

    #[test]
    fn test_compressor_gain_reduction() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0); // No knee for simple test

        // Below threshold - no compression
        let gr = compressor.calculate_gain_reduction(-30.0);
        assert_eq!(gr, 0.0);

        // At threshold - no compression
        let gr = compressor.calculate_gain_reduction(-20.0);
        assert_eq!(gr, 0.0);

        // 12 dB above threshold with 4:1 ratio
        // Gain reduction = 12 * (1 - 1/4) = 9 dB
        let gr = compressor.calculate_gain_reduction(-8.0);
        assert!((gr - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_compressor_additional_parameters_defaults() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);

        assert_eq!(compressor.mix, 1.0);
        assert!(!compressor.auto_makeup);
        assert!(compressor.link_channels);
        assert_eq!(compressor.sidechain_hpf_hz, 80.0);
    }

    #[test]
    fn test_compressor_mix_and_sidechain_parameters_set_get() {
        let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        compressor.initialize(48000).unwrap();

        compressor
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        compressor
            .set_parameter(
                ParameterId::from("sidechain_hpf_hz"),
                ParameterValue::Float(120.0),
            )
            .unwrap();

        let mix = compressor.get_parameter(&ParameterId::from("mix"));
        let sidechain = compressor.get_parameter(&ParameterId::from("sidechain_hpf_hz"));

        assert_eq!(mix, Some(ParameterValue::Float(0.5)));
        assert_eq!(sidechain, Some(ParameterValue::Float(120.0)));
    }
}
