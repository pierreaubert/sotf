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

use super::param_specs::compressor::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use super::smoothing::Smoother;
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

fn default_knee_db() -> f32 {
    KNEE_DEFAULT
}

fn default_makeup_gain_db() -> f32 {
    MAKEUP_GAIN_DEFAULT
}

fn default_mix() -> f32 {
    MIX_DEFAULT
}

pub fn default_auto_makeup() -> bool {
    AUTO_MAKEUP_DEFAULT
}

pub fn default_link_channels() -> bool {
    LINK_CHANNELS_DEFAULT
}

pub fn default_sidechain_hpf_hz() -> f32 {
    SIDECHAIN_HPF_HZ_DEFAULT
}

/// Data exposed by the compressor for monitoring
#[derive(Debug, Clone)]
pub struct CompressorData {
    /// Current gain reduction in dB (positive value, e.g., 6.0 means -6dB gain)
    /// One value per channel
    pub gain_reduction_db: Vec<f32>,
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

    // Smoothing
    threshold_smoother: Smoother,
    makeup_gain_smoother: Smoother,
}

impl CompressorPlugin {
    /// Create a new compressor plugin
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
        makeup_gain_db: f32,
    ) -> Self {
        let sample_rate = 44100;
        Self {
            channels,
            sample_rate,

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

            threshold_smoother: Smoother::new(threshold_db, 20.0, sample_rate),
            makeup_gain_smoother: Smoother::new(makeup_gain_db, 20.0, sample_rate),
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
    fn calculate_gain_reduction(&self, input_db: f32, threshold: f32) -> f32 {
        let knee = self.knee_db.max(0.0);
        let ratio = self.ratio.max(1.0);
        let slope = 1.0 - 1.0 / ratio;

        if knee < 0.1 {
            if input_db <= threshold {
                0.0
            } else {
                let overshoot = input_db - threshold;
                overshoot * slope
            }
        } else if input_db < threshold - knee / 2.0 {
            0.0
        } else if input_db > threshold + knee / 2.0 {
            let overshoot = input_db - threshold;
            overshoot * slope
        } else {
            let overshoot = input_db - threshold + knee / 2.0;
            let knee_factor = overshoot / knee;
            knee_factor * knee_factor * knee / 2.0 * slope
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
        PluginInfo::new("Compressor", "1.1.0", "SotF")
            .with_description("Dynamic range compressor with soft knee and smoothing")
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
            .with_description("Level above which compression starts (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("ratio", "Ratio", RATIO_DEFAULT, RATIO_MIN, RATIO_MAX)
                .with_description("Compression ratio (1:1 to 20:1)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("attack", "Attack", ATTACK_DEFAULT, ATTACK_MIN, ATTACK_MAX)
                .with_description("Attack time (ms)")
                .with_group("Timing")
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
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("knee", "Knee", KNEE_DEFAULT, KNEE_MIN, KNEE_MAX)
                .with_description("Soft knee width (dB)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "makeup_gain",
                "Makeup Gain",
                MAKEUP_GAIN_DEFAULT,
                MAKEUP_GAIN_MIN,
                MAKEUP_GAIN_MAX,
            )
            .with_description("Output gain compensation (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = compressed)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("auto_makeup", "Auto Makeup", AUTO_MAKEUP_DEFAULT)
                .with_description("Automatically compensate for gain reduction")
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
            let val = value.as_float().ok_or("Invalid threshold value")?;
            self.threshold_db = val;
            self.threshold_smoother.set_target(val);
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
            let val = value.as_float().ok_or("Invalid makeup gain value")?;
            self.makeup_gain_db = val;
            self.makeup_gain_smoother.set_target(val);
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
        self.threshold_smoother.set_time(20.0, sample_rate);
        self.makeup_gain_smoother.set_time(20.0, sample_rate);
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
    ) -> PluginResult<usize> {
        let num_frames = context.num_frames;
        let ratio = self.ratio.max(1.0);
        let compression_slope = 1.0 - 1.0 / ratio;

        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                // Tick smoothers per sample for artifact-free parameter changes
                let threshold = self.threshold_smoother.next();
                let makeup_gain = self.makeup_gain_smoother.next();

                let auto_makeup_db = if self.auto_makeup {
                    let avg_overshoot = (-threshold).max(0.0) * 0.5;
                    avg_overshoot * compression_slope
                } else {
                    0.0
                };
                let makeup_gain_linear = 10.0_f32.powf((makeup_gain + auto_makeup_db) / 20.0);

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
                let target_gr = self.calculate_gain_reduction(input_db, threshold);

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
                // Tick smoothers per sample for artifact-free parameter changes
                let threshold = self.threshold_smoother.next();
                let makeup_gain = self.makeup_gain_smoother.next();

                let auto_makeup_db = if self.auto_makeup {
                    let avg_overshoot = (-threshold).max(0.0) * 0.5;
                    avg_overshoot * compression_slope
                } else {
                    0.0
                };
                let makeup_gain_linear = 10.0_f32.powf((makeup_gain + auto_makeup_db) / 20.0);

                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let input_sample = buffer[sample_idx];
                    let sidechain_sample = self.apply_sidechain_filter(ch, input_sample);

                    let input_level = sidechain_sample.abs().max(1e-10);
                    let input_db = 20.0 * input_level.log10();

                    let target_gr = self.calculate_gain_reduction(input_db, threshold);

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

        // Flush denormals to prevent CPU performance spikes and audio crackle
        // Compressor gain reduction and envelope calculations can produce denormal numbers
        flush_denormals_inplace(buffer);

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        // Expose current gain reduction envelope
        Some(Arc::new(CompressorData {
            gain_reduction_db: self.envelope.clone(),
        }))
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
        let gr = compressor.calculate_gain_reduction(-30.0, -20.0);
        assert_eq!(gr, 0.0);

        // At threshold - no compression
        let gr = compressor.calculate_gain_reduction(-20.0, -20.0);
        assert_eq!(gr, 0.0);

        // 12 dB above threshold with 4:1 ratio
        // Gain reduction = 12 * (1 - 1/4) = 9 dB
        let gr = compressor.calculate_gain_reduction(-8.0, -20.0);
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
