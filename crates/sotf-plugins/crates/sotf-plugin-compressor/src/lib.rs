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

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::{compressor::PARAMS as CP, find_by_key as pk};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::f32::consts::PI;

use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_SMOOTHING_TIME_MS: f32 = 20.0;
const DEFAULT_SAMPLE_RATE: u32 = 44100;
const CACHE_UPDATE_THROTTLE: usize = 10;
const EPSILON: f32 = 1e-10;
const DB_CONVERSION_FACTOR: f32 = 20.0;
const AUTO_MAKEUP_OVERSHOOT_FACTOR: f32 = 0.5;

// ============================================================================
// Configuration
// ============================================================================

fn default_threshold_db() -> f32 {
    pk(CP, "threshold").default_f64() as f32
}

fn default_ratio() -> f32 {
    pk(CP, "ratio").default_f64() as f32
}

fn default_attack_ms() -> f32 {
    pk(CP, "attack").default_f64() as f32
}

fn default_release_ms() -> f32 {
    pk(CP, "release").default_f64() as f32
}

fn default_knee_db() -> f32 {
    pk(CP, "knee").default_f64() as f32
}

fn default_makeup_gain_db() -> f32 {
    pk(CP, "makeup_gain").default_f64() as f32
}

fn default_mix() -> f32 {
    pk(CP, "mix").default_f64() as f32
}

pub fn default_auto_makeup() -> bool {
    pk(CP, "auto_makeup").default_bool()
}

pub fn default_link_channels() -> bool {
    pk(CP, "link_channels").default_bool()
}

pub fn default_sidechain_hpf_hz() -> f32 {
    pk(CP, "sidechain_hpf_hz").default_f64() as f32
}

/// Data exposed by the compressor for monitoring
#[derive(Debug, Clone)]
pub struct CompressorData {
    /// Current gain reduction in dB (positive value, e.g., 6.0 means -6dB gain)
    /// One value per channel
    pub gain_reduction_db: Arc<Vec<f32>>,
}

impl Default for CompressorData {
    fn default() -> Self {
        Self {
            gain_reduction_db: Arc::new(Vec::new()),
        }
    }
}

impl CompressorData {
    pub fn new(channels: usize) -> Self {
        Self {
            gain_reduction_db: Arc::new(vec![0.0; channels]),
        }
    }

    pub fn update_gains(&mut self, new_gains: &[f32]) {
        if let Some(mut_gains) = Arc::get_mut(&mut self.gain_reduction_db)
            && mut_gains.len() == new_gains.len()
        {
            mut_gains.copy_from_slice(new_gains);
            return;
        }
        self.gain_reduction_db = Arc::new(new_gains.to_vec());
    }
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
    smoothing_time_ms: f32,

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

    cached_parameters: Vec<Parameter>,

    /// Gain reduction envelope in dB (positive value)
    envelope: Vec<f32>,
    /// Buffer for monitoring gain reduction without allocations
    monitoring_levels: Vec<f32>,
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    attack_coeff: f32,
    release_coeff: f32,
    sidechain_hpf_alpha: f32,

    // Smoothing
    threshold_smoother: Smoother,
    makeup_gain_smoother: Smoother,

    cache: RealTimeCache<CompressorData>,
    cache_update_counter: usize,
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
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let mut plugin = Self {
            channels,
            sample_rate,
            smoothing_time_ms: DEFAULT_SMOOTHING_TIME_MS,

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

            cached_parameters: Vec::new(),

            envelope: vec![0.0; channels],
            monitoring_levels: vec![0.0; channels],
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sidechain_hpf_alpha: 0.0,

            threshold_smoother: Smoother::new(threshold_db, DEFAULT_SMOOTHING_TIME_MS, sample_rate),
            makeup_gain_smoother: Smoother::new(
                makeup_gain_db,
                DEFAULT_SMOOTHING_TIME_MS,
                sample_rate,
            ),
            cache: RealTimeCache::new(CompressorData::new(channels)),
            cache_update_counter: 0,
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Set smoothing time for parameters (useful for testing)
    pub fn with_smoothing_time(mut self, time_ms: f32) -> Self {
        self.smoothing_time_ms = time_ms;
        self.threshold_smoother.set_time(time_ms, self.sample_rate);
        self.makeup_gain_smoother
            .set_time(time_ms, self.sample_rate);
        self
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(CP, "threshold").min_f64() as f32,
                pk(CP, "threshold").max_f64() as f32,
            )
            .with_description("Level above which compression starts (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(CP, "ratio").min_f64() as f32,
                pk(CP, "ratio").max_f64() as f32,
            )
            .with_description("Compression ratio (1:1 to 20:1)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(CP, "attack").min_f64() as f32,
                pk(CP, "attack").max_f64() as f32,
            )
            .with_description("Attack time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(CP, "release").min_f64() as f32,
                pk(CP, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "knee",
                "Knee",
                self.knee_db,
                pk(CP, "knee").min_f64() as f32,
                pk(CP, "knee").max_f64() as f32,
            )
            .with_description("Soft knee width (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "makeup_gain",
                "Makeup Gain",
                self.makeup_gain_db,
                pk(CP, "makeup_gain").min_f64() as f32,
                pk(CP, "makeup_gain").max_f64() as f32,
            )
            .with_description("Output gain compensation (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(CP, "mix").min_f64() as f32,
                pk(CP, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = compressed)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("auto_makeup", "Auto Makeup", self.auto_makeup)
                .with_description("Automatically compensate for gain reduction")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_description("Use linked sidechain for all channels")
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "sidechain_hpf_hz",
                "Sidechain HPF",
                self.sidechain_hpf_hz,
                pk(CP, "sidechain_hpf_hz").min_f64() as f32,
                pk(CP, "sidechain_hpf_hz").max_f64() as f32,
            )
            .with_description("High-pass filter frequency for sidechain (Hz)")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::FineTuning),
        ];
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

    /// Calculate gain reduction for a given input level using fast math
    #[inline]
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
            knee_factor * knee_factor * (knee / 2.0) * slope
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

    #[inline]
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

    #[inline]
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

        // One-pole smoothing for the envelope
        self.envelope[channel] = target_gr + coeff * (self.envelope[channel] - target_gr);

        let wet_gain_linear =
            fast_pow10(-self.envelope[channel] / DB_CONVERSION_FACTOR) * makeup_gain_linear;

        let wet = input_sample * wet_gain_linear;
        dry_mix * input_sample + wet_mix * wet
    }
}

impl InPlacePlugin for CompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Compressor", "1.2.0", "SotF").with_description(
            "Optimized dynamic range compressor with fast math and block-based smoothing",
        )
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_threshold {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "threshold").default_f64() as f32);
            if val.is_finite() {
                self.threshold_db = val;
                self.threshold_smoother.set_target(val);
            }
        } else if id == self.param_ratio {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "ratio").default_f64() as f32);
            if val.is_finite() {
                self.ratio = val.max(1.0);
            }
        } else if id == self.param_attack {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "attack").default_f64() as f32);
            if val.is_finite() {
                self.attack_ms = val;
                self.update_coefficients();
            }
        } else if id == self.param_release {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "release").default_f64() as f32);
            if val.is_finite() {
                self.release_ms = val;
                self.update_coefficients();
            }
        } else if id == self.param_knee {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "knee").default_f64() as f32);
            if val.is_finite() {
                self.knee_db = val.max(0.0);
            }
        } else if id == self.param_makeup_gain {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "makeup_gain").default_f64() as f32);
            if val.is_finite() {
                self.makeup_gain_db = val;
                self.makeup_gain_smoother.set_target(val);
            }
        } else if id == self.param_mix {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "mix").default_f64() as f32);
            if val.is_finite() {
                self.mix = val.clamp(0.0, 1.0);
            }
        } else if id == self.param_auto_makeup {
            self.auto_makeup = value
                .as_bool()
                .unwrap_or(pk(CP, "auto_makeup").default_bool());
        } else if id == self.param_link_channels {
            self.link_channels = value
                .as_bool()
                .unwrap_or(pk(CP, "link_channels").default_bool());
        } else if id == self.param_sidechain_hpf_hz {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "sidechain_hpf_hz").default_f64() as f32);
            if val.is_finite() {
                self.sidechain_hpf_hz = val.max(0.0);
                self.update_coefficients();
            }
        }
        self.rebuild_cached_parameters();
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
        self.threshold_smoother
            .set_time(self.smoothing_time_ms, sample_rate);
        self.makeup_gain_smoother
            .set_time(self.smoothing_time_ms, sample_rate);
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
        enable_ftz_daz();

        let num_frames = context.num_frames;

        let thresh = self.threshold_smoother.next_n(num_frames);
        let makeup_gain = self.makeup_gain_smoother.next_n(num_frames);

        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        let auto_makeup_db = if self.auto_makeup {
            let ratio = self.ratio.max(1.0);
            let compression_slope = 1.0 - 1.0 / ratio;
            let avg_overshoot = (-thresh).max(0.0) * AUTO_MAKEUP_OVERSHOOT_FACTOR;
            avg_overshoot * compression_slope
        } else {
            0.0
        };
        let makeup_gain_linear = fast_pow10((makeup_gain + auto_makeup_db) / DB_CONVERSION_FACTOR);

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let mut detection_level = 0.0_f32;
                for ch in 0..self.channels {
                    let sample_idx = frame * self.channels + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[sample_idx]);
                    detection_level = detection_level.max(filtered.abs());
                }

                let input_db = DB_CONVERSION_FACTOR * fast_log10(detection_level.max(EPSILON));
                let target_gr = self.calculate_gain_reduction(input_db, thresh);

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
                    let filtered = self.apply_sidechain_filter(ch, input_sample);
                    let input_db = DB_CONVERSION_FACTOR * fast_log10(filtered.abs().max(EPSILON));
                    let target_gr = self.calculate_gain_reduction(input_db, thresh);

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

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= CACHE_UPDATE_THROTTLE {
            self.cache_update_counter = 0;

            if self.link_channels {
                self.monitoring_levels.fill(self.envelope[0]);
            } else {
                self.monitoring_levels.copy_from_slice(&self.envelope);
            }

            self.cache.update(|d| {
                d.update_gains(&self.monitoring_levels);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use sotf_host::*;

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

    #[test]
    fn test_compressor_processing_varied_buffers() {
        use sotf_host::{InPlacePluginAdapter, Plugin, test_varied_buffer_sizes};
        let sample_rate = 48000.0;
        let channels = 2;
        let mut inner = CompressorPlugin::new(channels, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0)
            .with_smoothing_time(0.0);
        inner.initialize(sample_rate as u32).unwrap();
        let mut plugin = InPlacePluginAdapter::new(inner);

        // Generate a 1 second sine wave at -10dB (above threshold)
        let mut signal_gen = SignalGen::new_sine(sample_rate, 1000.0, 0.316); // -10dB approx
        let input = signal_gen.generate(4800 * channels); // 100ms is enough for CI

        // Generate reference output with standard block size
        let mut expected_output = vec![0.0; input.len()];
        let ctx = ProcessContext {
            sample_rate: sample_rate as u32,
            num_frames: 4800,
        };
        plugin.process(&input, &mut expected_output, &ctx).unwrap();

        // Reset and test varied buffer sizes
        plugin.reset();
        test_varied_buffer_sizes(&mut plugin, sample_rate, &input, &expected_output);
    }

    #[test]
    fn test_compressor_rt_safety() {
        use sotf_host::{InPlacePluginAdapter, Plugin, assert_no_allocs};
        let sample_rate = 48000;
        let channels = 2;
        let mut inner = CompressorPlugin::new(channels, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        inner.initialize(sample_rate).unwrap();
        let mut plugin = InPlacePluginAdapter::new(inner);

        let input = vec![0.1; 512 * channels];
        let mut output = vec![0.0; 512 * channels];
        let ctx = ProcessContext {
            sample_rate,
            num_frames: 512,
        };

        // Warm up
        for _ in 0..10 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }

        assert_no_allocs("CompressorPlugin::process", || {
            plugin.process(&input, &mut output, &ctx).unwrap();
        });
    }
}
