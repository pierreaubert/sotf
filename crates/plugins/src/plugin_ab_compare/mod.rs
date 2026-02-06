//! ============================================================================
//! A/B Comparison Plugin
//! ============================================================================
//!
//! This plugin allows fair comparison between two audio processing chains
//! with automatic loudness matching. Each path (A or B) can be:
//! - A single plugin
//! - A rack (linear chain of plugins)
//! - A graph (full DAG topology)

mod config;
mod factory;
#[cfg(test)]
mod tests;

pub use config::*;
use factory::build_path_from_config;

use crate::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use crate::host::DawHost;
use crate::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use crate::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Main Plugin Struct
// ============================================================================

/// A/B Comparison Plugin
///
/// Allows fair comparison between two audio processing chains with automatic
/// loudness matching. Each path (A or B) can be a single plugin, a rack
/// (linear chain), or a full graph.
pub struct ABComparePlugin {
    // Configuration
    num_channels: usize,
    sample_rate: u32,

    // Processing paths - use DawHost for flexibility
    host_a: DawHost,
    host_b: DawHost,

    // Path configurations (stored for runtime changes)
    path_a_config: PathConfig,
    path_b_config: PathConfig,

    // Auto-gain for matching B to A's loudness
    // Uses A's output as "input reference" and B's output as "output to compensate"
    // Also provides loudness and peak data for both paths
    auto_gain: AutoGain,

    // State
    mix_mode: MixMode,
    mix: f32,
    mix_smoother: Smoother,
    selected_path: i32,
    bypass: bool,
    mix_transition_ms: f32,

    // Internal buffers
    buffer_a: Vec<f32>,
    buffer_b: Vec<f32>,

    // Cached peak values
    last_peak_a: f64,
    last_peak_b: f64,
}

impl ABComparePlugin {
    /// Create a new A/B Compare plugin with default settings
    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::from_params(num_channels, ABComparePluginParams::default())
    }

    /// Create from parameters
    pub fn from_params(num_channels: usize, params: ABComparePluginParams) -> Result<Self, String> {
        let sample_rate = 48000; // Will be updated in initialize()

        let host_a = build_path_from_config(&params.path_a, num_channels, sample_rate)?;
        let host_b = build_path_from_config(&params.path_b, num_channels, sample_rate)?;

        // Create AutoGain for matching B's loudness to A's loudness
        // A's output is the "input reference", B's output is "what to compensate"
        let auto_gain_params = AutoGainParams {
            enabled: params.auto_gain_enabled,
            loudness_type: params.loudness_type,
            max_gain_db: params.max_auto_gain_db,
            smoothing_ms: params.gain_smoothing_ms,
        };
        let auto_gain = AutoGain::new(num_channels, sample_rate, auto_gain_params)?;

        let mix_smoother = Smoother::new(params.mix, params.mix_transition_ms, sample_rate);

        Ok(Self {
            num_channels,
            sample_rate,
            host_a,
            host_b,
            path_a_config: params.path_a,
            path_b_config: params.path_b,
            auto_gain,
            mix_mode: params.mix_mode,
            mix: params.mix,
            mix_smoother,
            selected_path: params.selected_path,
            bypass: params.bypass,
            mix_transition_ms: params.mix_transition_ms,
            buffer_a: Vec::new(),
            buffer_b: Vec::new(),
            last_peak_a: 0.0,
            last_peak_b: 0.0,
        })
    }

    /// Rebuild path A from config
    fn rebuild_path_a(&mut self) -> Result<(), String> {
        self.host_a =
            build_path_from_config(&self.path_a_config, self.num_channels, self.sample_rate)?;
        Ok(())
    }

    /// Rebuild path B from config
    fn rebuild_path_b(&mut self) -> Result<(), String> {
        self.host_b =
            build_path_from_config(&self.path_b_config, self.num_channels, self.sample_rate)?;
        Ok(())
    }
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

impl Plugin for ABComparePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("A/B Compare", "1.0.0", "SotF")
            .with_description("A/B comparison with automatic loudness matching")
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("mix", "A/B Mix", 0.0, -1.0, 1.0)
                .with_description("Mix between A and B: -1.0 = A, 0.0 = 50/50, +1.0 = B")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int("mix_mode", "Mix Mode", 0, 0, 1)
                .with_description("0 = Potentiometer (continuous), 1 = Binary (A/B switch)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int("selected_path", "Selected Path", 0, 0, 1)
                .with_description("0 = A, 1 = B (only used in binary mode)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("bypass", "Bypass", false)
                .with_description("Bypass A/B processing, output original input")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", true)
                .with_description("Automatically match loudness between A and B")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int("loudness_type", "Loudness Type", 0, 0, 1)
                .with_description("0 = Momentary (400ms), 1 = Short-term (3s)")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("max_auto_gain_db", "Max Auto Gain", 12.0, 0.0, 24.0)
                .with_description("Maximum loudness correction in dB")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("gain_smoothing_ms", "Gain Smoothing", 100.0, 10.0, 500.0)
                .with_description("Auto-gain smoothing time in milliseconds")
                .with_group("Loudness Matching")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("mix_transition_ms", "Mix Transition", 50.0, 5.0, 500.0)
                .with_description("A/B transition smoothing time in milliseconds")
                .with_group("Timing")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_string(
                "path_a_config",
                "Path A Config",
                r#"{"type":"None"}"#.to_string(),
            )
            .with_description("JSON configuration for path A")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_string(
                "path_b_config",
                "Path B Config",
                r#"{"type":"None"}"#.to_string(),
            )
            .with_description("JSON configuration for path B")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        match id.0.as_str() {
            "mix" => {
                if let ParameterValue::Float(v) = value {
                    self.mix = v.clamp(-1.0, 1.0);
                    self.mix_smoother.set_target(self.mix);
                }
            }
            "mix_mode" => {
                if let ParameterValue::Int(v) = value {
                    self.mix_mode = if v == 0 {
                        MixMode::Potentiometer
                    } else {
                        MixMode::Binary
                    };
                }
            }
            "selected_path" => {
                if let ParameterValue::Int(v) = value {
                    self.selected_path = v.clamp(0, 1);
                    // Update mix target for binary mode
                    if self.mix_mode == MixMode::Binary {
                        let target = if self.selected_path == 0 { -1.0 } else { 1.0 };
                        self.mix_smoother.set_target(target);
                    }
                }
            }
            "bypass" => {
                if let ParameterValue::Bool(v) = value {
                    self.bypass = v;
                }
            }
            "auto_gain_enabled" => {
                if let ParameterValue::Bool(v) = value {
                    self.auto_gain.set_enabled(v);
                }
            }
            "loudness_type" => {
                if let ParameterValue::Int(v) = value {
                    let loudness_type = if v == 0 {
                        AutoGainLoudnessType::Momentary
                    } else {
                        AutoGainLoudnessType::ShortTerm
                    };
                    self.auto_gain.set_loudness_type(loudness_type);
                }
            }
            "max_auto_gain_db" => {
                if let ParameterValue::Float(v) = value {
                    self.auto_gain.set_max_gain_db(v.clamp(0.0, 24.0));
                }
            }
            "gain_smoothing_ms" => {
                if let ParameterValue::Float(v) = value {
                    self.auto_gain.set_smoothing_ms(v.clamp(10.0, 500.0));
                }
            }
            "mix_transition_ms" => {
                if let ParameterValue::Float(v) = value {
                    self.mix_transition_ms = v.clamp(5.0, 500.0);
                    self.mix_smoother
                        .set_time(self.mix_transition_ms, self.sample_rate);
                }
            }
            "path_a_config" => {
                if let ParameterValue::String(json) = value {
                    let config: PathConfig = serde_json::from_str(&json)
                        .map_err(|e| format!("Invalid path A config JSON: {}", e))?;
                    self.path_a_config = config;
                    self.rebuild_path_a()?;
                }
            }
            "path_b_config" => {
                if let ParameterValue::String(json) = value {
                    let config: PathConfig = serde_json::from_str(&json)
                        .map_err(|e| format!("Invalid path B config JSON: {}", e))?;
                    self.path_b_config = config;
                    self.rebuild_path_b()?;
                }
            }
            _ => return Err(format!("Unknown parameter: {}", id.0)),
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.0.as_str() {
            "mix" => Some(ParameterValue::Float(self.mix)),
            "mix_mode" => Some(ParameterValue::Int(match self.mix_mode {
                MixMode::Potentiometer => 0,
                MixMode::Binary => 1,
            })),
            "selected_path" => Some(ParameterValue::Int(self.selected_path)),
            "bypass" => Some(ParameterValue::Bool(self.bypass)),
            "auto_gain_enabled" => Some(ParameterValue::Bool(self.auto_gain.is_enabled())),
            "loudness_type" => Some(ParameterValue::Int(0)), // TODO: store and return actual value
            "max_auto_gain_db" => Some(ParameterValue::Float(12.0)), // TODO: store and return actual value
            "gain_smoothing_ms" => Some(ParameterValue::Float(100.0)), // TODO: store and return actual value
            "mix_transition_ms" => Some(ParameterValue::Float(self.mix_transition_ms)),
            "path_a_config" => serde_json::to_string(&self.path_a_config)
                .ok()
                .map(ParameterValue::String),
            "path_b_config" => serde_json::to_string(&self.path_b_config)
                .ok()
                .map(ParameterValue::String),
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Rebuild paths with new sample rate
        self.rebuild_path_a()?;
        self.rebuild_path_b()?;

        // Update auto-gain sample rate
        self.auto_gain
            .set_sample_rate(sample_rate)
            .map_err(|e| format!("Failed to update auto-gain sample rate: {}", e))?;

        // Reset mix smoother with new sample rate
        self.mix_smoother = Smoother::new(self.mix, self.mix_transition_ms, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        // Reset hosts
        self.host_a.reset();
        self.host_b.reset();

        // Reset auto-gain (also resets loudness monitors)
        self.auto_gain.reset();

        // Reset mix smoother
        self.mix_smoother.reset(self.mix);

        // Reset peak values
        self.last_peak_a = 0.0;
        self.last_peak_b = 0.0;

        // Clear buffers
        self.buffer_a.clear();
        self.buffer_b.clear();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let expected_samples = context.num_frames * self.num_channels;

        // Verify input/output size
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }
        if output.len() != expected_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                expected_samples,
                output.len()
            ));
        }

        // Handle bypass
        if self.bypass {
            output.copy_from_slice(input);
            return Ok(context.num_frames);
        }

        // Resize buffers if needed
        if self.buffer_a.len() != expected_samples {
            self.buffer_a.resize(expected_samples, 0.0);
            self.buffer_b.resize(expected_samples, 0.0);
        }

        // Process path A
        self.host_a.process(input, &mut self.buffer_a)?;

        // Process path B
        self.host_b.process(input, &mut self.buffer_b)?;

        // Measure loudness and peaks using AutoGain
        // A's output is the "input reference" (what we want B to match)
        // B's output is the "output to compensate"
        self.auto_gain.measure_input(&self.buffer_a)?;
        self.auto_gain.measure_output(&self.buffer_b)?;

        // Cache peak values for get_data()
        self.last_peak_a = self.auto_gain.last_input_peak();
        self.last_peak_b = self.auto_gain.last_output_peak();

        // Determine target mix value
        let target_mix = match self.mix_mode {
            MixMode::Potentiometer => self.mix,
            MixMode::Binary => {
                if self.selected_path == 0 {
                    -1.0
                } else {
                    1.0
                }
            }
        };
        self.mix_smoother.set_target(target_mix);

        // Process sample-by-sample
        for frame in 0..context.num_frames {
            // Get smoothed gain from AutoGain
            let gain_linear = self.auto_gain.next_gain_linear();

            // Get smoothed mix value
            let current_mix = self.mix_smoother.next();

            // Equal-power crossfade
            // mix: -1 = pure A, +1 = pure B
            // Convert to 0..1 range for angle calculation
            let mix_01 = (current_mix + 1.0) / 2.0; // 0 = A, 1 = B
            let angle = mix_01 * std::f32::consts::FRAC_PI_2; // 0 to PI/2
            let gain_a = angle.cos();
            let gain_b = angle.sin();

            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let sample_a = self.buffer_a[idx];
                let sample_b = self.buffer_b[idx] * gain_linear;
                output[idx] = sample_a * gain_a + sample_b * gain_b;
            }
        }

        Ok(context.num_frames)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let data = ABCompareData {
            loudness_a_lufs: self.auto_gain.last_input_lufs(),
            loudness_b_lufs: self.auto_gain.last_output_lufs(),
            auto_gain_db: self.auto_gain.current_gain_db(),
            peak_a: self.last_peak_a,
            peak_b: self.last_peak_b,
            current_mix: self.mix_smoother.current(),
            bypass_active: self.bypass,
        };
        Some(Arc::new(data))
    }

    fn latency_samples(&self) -> usize {
        // Total latency is the max of both paths
        let latency_a = self.host_a.total_latency_samples();
        let latency_b = self.host_b.total_latency_samples();
        latency_a.max(latency_b)
    }
}
