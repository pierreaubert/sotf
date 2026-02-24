//! Plugin parameter editing interface shared between TUI and GPUI

use crate::PluginSettings;
use std::path::PathBuf;

/// Specification for a plugin parameter in the TUI
pub struct TuiParamSpec {
    pub name: String,
    pub value: String,
    pub unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuiParamType {
    Float { min: f64, max: f64, step: f64 },
    Int { min: i32, max: i32, step: i32 },
    Bool,
    Choice { count: usize },
}

pub struct TuiParamDescriptor {
    pub name: String,
    pub param_type: TuiParamType,
    pub unit: String,
    pub group: String,
}

pub trait TuiEditablePlugin {
    fn get_descriptors(&self) -> Vec<TuiParamDescriptor>;
    fn get_params(&self) -> Vec<TuiParamSpec>;
    fn adjust_param(&mut self, index: usize, delta: f64) -> bool;
    fn get_value_as_string(&self, index: usize) -> String;
    /// Return the list of choice labels for a Choice parameter.
    /// Returns empty vec for non-Choice params.
    fn get_choice_labels(&self, _index: usize) -> Vec<String> {
        Vec::new()
    }
    /// Set a parameter to an absolute value.
    /// For Float/Int: computes the delta from current value and calls adjust_param.
    /// For Bool: value > 0.5 = true.
    /// For Choice: value is the option index.
    fn set_param(&mut self, index: usize, value: f64) -> bool {
        let current_str = self.get_value_as_string(index);
        let current = current_str.parse::<f64>().unwrap_or(0.0);
        let descriptors = self.get_descriptors();
        if let Some(desc) = descriptors.get(index) {
            match desc.param_type {
                TuiParamType::Float { step, .. } => {
                    // Compute delta in units of what adjust_param expects (delta=1.0 means +step)
                    let delta = (value - current) / step;
                    if delta.abs() > 0.001 {
                        return self.adjust_param(index, delta);
                    }
                }
                TuiParamType::Int { step, .. } => {
                    let delta = (value - current) / step as f64;
                    if delta.abs() > 0.001 {
                        return self.adjust_param(index, delta);
                    }
                }
                TuiParamType::Bool => {
                    let is_true = current_str == "On" || current_str == "true"
                        || current_str == "Linked" || current_str == "Soft"
                        || current_str == "1";
                    let want_true = value > 0.5;
                    if is_true != want_true {
                        return self.adjust_param(index, 1.0);
                    }
                }
                TuiParamType::Choice { count } => {
                    let target = (value as usize).min(count.saturating_sub(1));
                    // Cycle forward until we reach the target
                    for _ in 0..count {
                        let cur = self.get_value_as_string(index);
                        let labels = self.get_choice_labels(index);
                        if let Some(cur_idx) = labels.iter().position(|l| *l == cur) {
                            if cur_idx == target {
                                return true;
                            }
                        }
                        self.adjust_param(index, 1.0);
                    }
                }
            }
        }
        false
    }
}

impl TuiEditablePlugin for PluginSettings {
    fn get_descriptors(&self) -> Vec<TuiParamDescriptor> {
        use sotf_plugins::param_specs::*;
        match self {
            PluginSettings::Gain { .. } => vec![TuiParamDescriptor {
                name: "Gain".into(),
                param_type: TuiParamType::Float {
                    min: gain::GAIN_DB_MIN as f64,
                    max: gain::GAIN_DB_MAX as f64,
                    step: 0.5,
                },
                unit: "dB".into(),
                group: "General".into(),
            }],
            PluginSettings::EQ { filters: _, max_filters, .. } => {
                let mut descs = vec![TuiParamDescriptor {
                    name: "Max Filters".into(),
                    param_type: TuiParamType::Int {
                        min: 1,
                        max: 20,
                        step: 1,
                    },
                    unit: "".into(),
                    group: "Global".into(),
                }];
                // Always show exactly max_filters descriptors
                for i in 0..*max_filters {
                    let g = format!("Filter {}", i + 1);
                    descs.push(TuiParamDescriptor {
                        name: "Frequency".into(),
                        param_type: TuiParamType::Float {
                            min: eq::FREQUENCY_MIN,
                            max: eq::FREQUENCY_MAX,
                            step: 10.0,
                        },
                        unit: "Hz".into(),
                        group: g.clone(),
                    });
                    descs.push(TuiParamDescriptor {
                        name: "Q".into(),
                        param_type: TuiParamType::Float {
                            min: eq::Q_MIN,
                            max: eq::Q_MAX,
                            step: 0.05,
                        },
                        unit: "".into(),
                        group: g.clone(),
                    });
                    descs.push(TuiParamDescriptor {
                        name: "Gain".into(),
                        param_type: TuiParamType::Float {
                            min: eq::GAIN_DB_MIN,
                            max: eq::GAIN_DB_MAX,
                            step: 0.5,
                        },
                        unit: "dB".into(),
                        group: g.clone(),
                    });
                    descs.push(TuiParamDescriptor {
                        name: "Type".into(),
                        param_type: TuiParamType::Choice { count: 9 }, // Peak, Lowshelf, Highshelf, Lowpass, Highpass, etc.
                        unit: "".into(),
                        group: g,
                    });
                }
                descs
            }
            PluginSettings::Compressor { .. } => vec![
                TuiParamDescriptor {
                    name: "Threshold".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::THRESHOLD_MIN as f64,
                        max: compressor::THRESHOLD_MAX as f64,
                        step: 1.0,
                    },
                    unit: "dB".into(),
                    group: "Dynamics".into(),
                },
                TuiParamDescriptor {
                    name: "Ratio".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::RATIO_MIN as f64,
                        max: compressor::RATIO_MAX as f64,
                        step: 0.1,
                    },
                    unit: ":1".into(),
                    group: "Dynamics".into(),
                },
                TuiParamDescriptor {
                    name: "Attack".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::ATTACK_MIN as f64,
                        max: compressor::ATTACK_MAX as f64,
                        step: 0.5,
                    },
                    unit: "ms".into(),
                    group: "Timing".into(),
                },
                TuiParamDescriptor {
                    name: "Release".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::RELEASE_MIN as f64,
                        max: compressor::RELEASE_MAX as f64,
                        step: 5.0,
                    },
                    unit: "ms".into(),
                    group: "Timing".into(),
                },
                TuiParamDescriptor {
                    name: "Knee".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::KNEE_MIN as f64,
                        max: compressor::KNEE_MAX as f64,
                        step: 0.5,
                    },
                    unit: "dB".into(),
                    group: "Dynamics".into(),
                },
                TuiParamDescriptor {
                    name: "Makeup Gain".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::MAKEUP_GAIN_MIN as f64,
                        max: compressor::MAKEUP_GAIN_MAX as f64,
                        step: 0.5,
                    },
                    unit: "dB".into(),
                    group: "Output".into(),
                },
                TuiParamDescriptor {
                    name: "Mix".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::MIX_MIN as f64,
                        max: compressor::MIX_MAX as f64,
                        step: 0.01,
                    },
                    unit: "".into(),
                    group: "Output".into(),
                },
                TuiParamDescriptor {
                    name: "Auto Makeup".into(),
                    param_type: TuiParamType::Bool,
                    unit: "".into(),
                    group: "Output".into(),
                },
                TuiParamDescriptor {
                    name: "Link Channels".into(),
                    param_type: TuiParamType::Bool,
                    unit: "".into(),
                    group: "Channels".into(),
                },
                TuiParamDescriptor {
                    name: "Sidechain HPF".into(),
                    param_type: TuiParamType::Float {
                        min: compressor::SIDECHAIN_HPF_HZ_MIN as f64,
                        max: compressor::SIDECHAIN_HPF_HZ_MAX as f64,
                        step: 5.0,
                    },
                    unit: "Hz".into(),
                    group: "Sidechain".into(),
                },
            ],
            PluginSettings::Upmixer { .. } => vec![
                TuiParamDescriptor { name: "Speaker Config".into(), param_type: TuiParamType::Choice { count: 10 }, unit: "".into(), group: "Output".into() },
                // Gains
                TuiParamDescriptor { name: "Front Direct".into(), param_type: TuiParamType::Float { min: upmixer::GAIN_FRONT_DIRECT_MIN as f64, max: upmixer::GAIN_FRONT_DIRECT_MAX as f64, step: 0.05 }, unit: "x".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "Front Ambient".into(), param_type: TuiParamType::Float { min: upmixer::GAIN_FRONT_AMBIENT_MIN as f64, max: upmixer::GAIN_FRONT_AMBIENT_MAX as f64, step: 0.05 }, unit: "x".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "Rear Ambient".into(), param_type: TuiParamType::Float { min: upmixer::GAIN_REAR_AMBIENT_MIN as f64, max: upmixer::GAIN_REAR_AMBIENT_MAX as f64, step: 0.05 }, unit: "x".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "Height Gain".into(), param_type: TuiParamType::Float { min: upmixer::GAIN_HEIGHT_MIN as f64, max: upmixer::GAIN_HEIGHT_MAX as f64, step: 0.05 }, unit: "x".into(), group: "Gains".into() },
                // LFE
                TuiParamDescriptor { name: "LFE Gain".into(), param_type: TuiParamType::Float { min: upmixer::LFE_GAIN_MIN as f64, max: upmixer::LFE_GAIN_MAX as f64, step: 0.05 }, unit: "x".into(), group: "LFE".into() },
                TuiParamDescriptor { name: "LFE Cutoff".into(), param_type: TuiParamType::Float { min: upmixer::LFE_CUTOFF_HZ_MIN as f64, max: upmixer::LFE_CUTOFF_HZ_MAX as f64, step: 5.0 }, unit: "Hz".into(), group: "LFE".into() },
                TuiParamDescriptor { name: "Subharmonic Synth".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "LFE".into() },
                TuiParamDescriptor { name: "Sub Gain".into(), param_type: TuiParamType::Float { min: upmixer::SUBHARMONIC_GAIN_MIN as f64, max: upmixer::SUBHARMONIC_GAIN_MAX as f64, step: 0.05 }, unit: "x".into(), group: "LFE".into() },
                TuiParamDescriptor { name: "Sub Freq".into(), param_type: TuiParamType::Float { min: upmixer::SUBHARMONIC_FREQ_HZ_MIN as f64, max: upmixer::SUBHARMONIC_FREQ_HZ_MAX as f64, step: 1.0 }, unit: "Hz".into(), group: "LFE".into() },
                TuiParamDescriptor { name: "Sub Attack".into(), param_type: TuiParamType::Float { min: upmixer::SUBHARMONIC_ATTACK_MS_MIN as f64, max: upmixer::SUBHARMONIC_ATTACK_MS_MAX as f64, step: 1.0 }, unit: "ms".into(), group: "LFE".into() },
                TuiParamDescriptor { name: "Sub Release".into(), param_type: TuiParamType::Float { min: upmixer::SUBHARMONIC_RELEASE_MS_MIN as f64, max: upmixer::SUBHARMONIC_RELEASE_MS_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "LFE".into() },
                // Spatial
                TuiParamDescriptor { name: "Stereo Width".into(), param_type: TuiParamType::Float { min: upmixer::STEREO_WIDTH_MIN as f64, max: upmixer::STEREO_WIDTH_MAX as f64, step: 0.05 }, unit: "".into(), group: "Spatial".into() },
                TuiParamDescriptor { name: "Center Spread".into(), param_type: TuiParamType::Float { min: upmixer::CENTER_SPREAD_MIN as f64, max: upmixer::CENTER_SPREAD_MAX as f64, step: 0.05 }, unit: "".into(), group: "Spatial".into() },
                TuiParamDescriptor { name: "Upmix Crossover".into(), param_type: TuiParamType::Float { min: upmixer::BANDPASS_HZ_MIN as f64, max: upmixer::BANDPASS_HZ_MAX as f64, step: 5.0 }, unit: "Hz".into(), group: "Spatial".into() },
                // Enhancement
                TuiParamDescriptor { name: "HR Direct".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Enhancement".into() },
                TuiParamDescriptor { name: "HR Sharpen".into(), param_type: TuiParamType::Float { min: upmixer::HR_SHARPEN_MIN as f64, max: upmixer::HR_SHARPEN_MAX as f64, step: 0.05 }, unit: "".into(), group: "Enhancement".into() },
                TuiParamDescriptor { name: "Ambient Boost".into(), param_type: TuiParamType::Float { min: upmixer::AMBIENT_BOOST_MIN as f64, max: upmixer::AMBIENT_BOOST_MAX as f64, step: 0.05 }, unit: "x".into(), group: "Enhancement".into() },
                TuiParamDescriptor { name: "Decor Mode".into(), param_type: TuiParamType::Choice { count: 2 }, unit: "".into(), group: "Enhancement".into() },
                TuiParamDescriptor { name: "Decor LFO Rate".into(), param_type: TuiParamType::Float { min: upmixer::DECORRELATION_LFO_RATE_HZ_MIN as f64, max: upmixer::DECORRELATION_LFO_RATE_HZ_MAX as f64, step: 0.01 }, unit: "Hz".into(), group: "Enhancement".into() },
                TuiParamDescriptor { name: "Velvet Duration".into(), param_type: TuiParamType::Float { min: upmixer::VELVET_NOISE_DURATION_MS_MIN as f64, max: upmixer::VELVET_NOISE_DURATION_MS_MAX as f64, step: 1.0 }, unit: "ms".into(), group: "Enhancement".into() },
                TuiParamDescriptor { name: "Velvet Density".into(), param_type: TuiParamType::Float { min: upmixer::VELVET_NOISE_DENSITY_MIN as f64, max: upmixer::VELVET_NOISE_DENSITY_MAX as f64, step: 100.0 }, unit: "".into(), group: "Enhancement".into() },
                // Height
                TuiParamDescriptor { name: "Height HF Cap".into(), param_type: TuiParamType::Float { min: upmixer::HEIGHT_HF_CAP_HZ_MIN as f64, max: upmixer::HEIGHT_HF_CAP_HZ_MAX as f64, step: 100.0 }, unit: "Hz".into(), group: "Height".into() },
                TuiParamDescriptor { name: "Height Trans Red".into(), param_type: TuiParamType::Float { min: upmixer::HEIGHT_TRANSIENT_REDUCTION_MIN as f64, max: upmixer::HEIGHT_TRANSIENT_REDUCTION_MAX as f64, step: 0.05 }, unit: "".into(), group: "Height".into() },
                TuiParamDescriptor { name: "Height Direct Leak".into(), param_type: TuiParamType::Float { min: upmixer::HEIGHT_DIRECT_LEAK_MIN as f64, max: upmixer::HEIGHT_DIRECT_LEAK_MAX as f64, step: 0.01 }, unit: "".into(), group: "Height".into() },
                // Surround
                TuiParamDescriptor { name: "Surround Bleed".into(), param_type: TuiParamType::Float { min: upmixer::SURROUND_DIRECT_BLEED_MIN as f64, max: upmixer::SURROUND_DIRECT_BLEED_MAX as f64, step: 0.05 }, unit: "".into(), group: "Surround".into() },
                TuiParamDescriptor { name: "Rear Amb Boost".into(), param_type: TuiParamType::Float { min: upmixer::REAR_AMBIENT_BOOST_MIN as f64, max: upmixer::REAR_AMBIENT_BOOST_MAX as f64, step: 0.05 }, unit: "x".into(), group: "Surround".into() },
                TuiParamDescriptor { name: "Rear Late Refl".into(), param_type: TuiParamType::Float { min: upmixer::REAR_LATE_REFLECTION_MIN as f64, max: upmixer::REAR_LATE_REFLECTION_MAX as f64, step: 0.01 }, unit: "".into(), group: "Surround".into() },
                // Dialogue
                TuiParamDescriptor { name: "Dialogue Weight".into(), param_type: TuiParamType::Float { min: upmixer::DIALOGUE_WEIGHT_MIN as f64, max: upmixer::DIALOGUE_WEIGHT_MAX as f64, step: 0.05 }, unit: "".into(), group: "Dialogue".into() },
                TuiParamDescriptor { name: "Voice Freq Min".into(), param_type: TuiParamType::Float { min: upmixer::VOICE_FREQ_MIN_HZ_MIN as f64, max: upmixer::VOICE_FREQ_MIN_HZ_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "Dialogue".into() },
                TuiParamDescriptor { name: "Voice Freq Max".into(), param_type: TuiParamType::Float { min: upmixer::VOICE_FREQ_MAX_HZ_MIN as f64, max: upmixer::VOICE_FREQ_MAX_HZ_MAX as f64, step: 50.0 }, unit: "Hz".into(), group: "Dialogue".into() },
                TuiParamDescriptor { name: "Diag Centroid W".into(), param_type: TuiParamType::Float { min: upmixer::DIALOGUE_CENTROID_WEIGHT_MIN as f64, max: upmixer::DIALOGUE_CENTROID_WEIGHT_MAX as f64, step: 0.05 }, unit: "".into(), group: "Dialogue".into() },
                TuiParamDescriptor { name: "Diag Variance W".into(), param_type: TuiParamType::Float { min: upmixer::DIALOGUE_VARIANCE_WEIGHT_MIN as f64, max: upmixer::DIALOGUE_VARIANCE_WEIGHT_MAX as f64, step: 0.05 }, unit: "".into(), group: "Dialogue".into() },
                TuiParamDescriptor { name: "Diag Coherence W".into(), param_type: TuiParamType::Float { min: upmixer::DIALOGUE_COHERENCE_WEIGHT_MIN as f64, max: upmixer::DIALOGUE_COHERENCE_WEIGHT_MAX as f64, step: 0.05 }, unit: "".into(), group: "Dialogue".into() },
                // Output
                TuiParamDescriptor { name: "Safety Cap".into(), param_type: TuiParamType::Float { min: upmixer::SAFETY_CAP_DB_MIN as f64, max: upmixer::SAFETY_CAP_DB_MAX as f64, step: 0.1 }, unit: "dB".into(), group: "Output".into() },
                TuiParamDescriptor { name: "Bypass Decor".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostics".into() },
                TuiParamDescriptor { name: "Bypass Transients".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostics".into() },
                TuiParamDescriptor { name: "Bypass All".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostics".into() },
                TuiParamDescriptor { name: "ML Detection".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostics".into() },
            ],
            PluginSettings::Limiter { .. } => vec![
                TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: limiter::THRESHOLD_MIN as f64, max: limiter::THRESHOLD_MAX as f64, step: 0.1 }, unit: "dB".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: limiter::RELEASE_MIN as f64, max: limiter::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Lookahead".into(), param_type: TuiParamType::Float { min: limiter::LOOKAHEAD_MIN as f64, max: limiter::LOOKAHEAD_MAX as f64, step: 0.5 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Soft Knee".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: limiter::MIX_MIN as f64, max: limiter::MIX_MAX as f64, step: 0.01 }, unit: "".into(), group: "Output".into() },
            ],
            PluginSettings::Gate { .. } => vec![
                TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: gate::THRESHOLD_MIN as f64, max: gate::THRESHOLD_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Ratio".into(), param_type: TuiParamType::Float { min: gate::RATIO_MIN as f64, max: gate::RATIO_MAX as f64, step: 0.1 }, unit: ":1".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: gate::ATTACK_MIN as f64, max: gate::ATTACK_MAX as f64, step: 0.1 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Hold".into(), param_type: TuiParamType::Float { min: gate::HOLD_MIN as f64, max: gate::HOLD_MAX as f64, step: 1.0 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: gate::RELEASE_MIN as f64, max: gate::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: gate::MIX_MIN as f64, max: gate::MIX_MAX as f64, step: 0.01 }, unit: "".into(), group: "Output".into() },
                TuiParamDescriptor { name: "Link Channels".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Channels".into() },
                TuiParamDescriptor { name: "Sidechain HPF".into(), param_type: TuiParamType::Float { min: gate::SIDECHAIN_HPF_HZ_MIN as f64, max: gate::SIDECHAIN_HPF_HZ_MAX as f64, step: 5.0 }, unit: "Hz".into(), group: "Sidechain".into() },
            ],
            PluginSettings::Expander { .. } => vec![
                TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: expander::THRESHOLD_MIN as f64, max: expander::THRESHOLD_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Ratio".into(), param_type: TuiParamType::Float { min: expander::RATIO_MIN as f64, max: expander::RATIO_MAX as f64, step: 0.1 }, unit: ":1".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: expander::ATTACK_MIN as f64, max: expander::ATTACK_MAX as f64, step: 0.1 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: expander::RELEASE_MIN as f64, max: expander::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Range".into(), param_type: TuiParamType::Float { min: expander::RANGE_MIN as f64, max: expander::RANGE_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Knee".into(), param_type: TuiParamType::Float { min: expander::KNEE_MIN as f64, max: expander::KNEE_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Hysteresis".into(), param_type: TuiParamType::Float { min: expander::HYSTERESIS_MIN as f64, max: expander::HYSTERESIS_MAX as f64, step: 0.1 }, unit: "dB".into(), group: "Dynamics".into() },
                TuiParamDescriptor { name: "Hold".into(), param_type: TuiParamType::Float { min: expander::HOLD_MIN as f64, max: expander::HOLD_MAX as f64, step: 1.0 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: expander::MIX_MIN as f64, max: expander::MIX_MAX as f64, step: 0.01 }, unit: "".into(), group: "Output".into() },
                TuiParamDescriptor { name: "Link Channels".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Channels".into() },
                TuiParamDescriptor { name: "Sidechain HPF".into(), param_type: TuiParamType::Float { min: expander::SIDECHAIN_HPF_HZ_MIN as f64, max: expander::SIDECHAIN_HPF_HZ_MAX as f64, step: 5.0 }, unit: "Hz".into(), group: "Sidechain".into() },
            ],
            PluginSettings::MultibandCompressor { num_bands, .. } => {
                let mut descs = vec![
                    TuiParamDescriptor { name: "Bands".into(), param_type: TuiParamType::Int { min: multiband_compressor::NUM_BANDS_MIN as i32, max: multiband_compressor::NUM_BANDS_MAX as i32, step: 1 }, unit: "".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Preset".into(), param_type: TuiParamType::Int { min: multiband_compressor::CROSSOVER_PRESET_MIN, max: multiband_compressor::CROSSOVER_PRESET_MAX, step: 1 }, unit: "".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 1".into(), param_type: TuiParamType::Float { min: multiband_compressor::CROSSOVER_FREQ_1_MIN as f64, max: multiband_compressor::CROSSOVER_FREQ_1_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 2".into(), param_type: TuiParamType::Float { min: multiband_compressor::CROSSOVER_FREQ_2_MIN as f64, max: multiband_compressor::CROSSOVER_FREQ_2_MAX as f64, step: 50.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 3".into(), param_type: TuiParamType::Float { min: multiband_compressor::CROSSOVER_FREQ_3_MIN as f64, max: multiband_compressor::CROSSOVER_FREQ_3_MAX as f64, step: 100.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 4".into(), param_type: TuiParamType::Float { min: multiband_compressor::CROSSOVER_FREQ_4_MIN as f64, max: multiband_compressor::CROSSOVER_FREQ_4_MAX as f64, step: 100.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: multiband_compressor::THRESHOLD_MIN as f64, max: multiband_compressor::THRESHOLD_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Ratio".into(), param_type: TuiParamType::Float { min: multiband_compressor::RATIO_MIN as f64, max: multiband_compressor::RATIO_MAX as f64, step: 0.1 }, unit: ":1".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: multiband_compressor::ATTACK_MIN as f64, max: multiband_compressor::ATTACK_MAX as f64, step: 0.5 }, unit: "ms".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: multiband_compressor::RELEASE_MIN as f64, max: multiband_compressor::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Knee".into(), param_type: TuiParamType::Float { min: multiband_compressor::KNEE_MIN as f64, max: multiband_compressor::KNEE_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: multiband_compressor::MIX_MIN as f64, max: multiband_compressor::MIX_MAX as f64, step: 0.01 }, unit: "".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Link Channels".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Global".into() },
                ];
                for i in 0..*num_bands {
                    let g = format!("Band {}", i + 1);
                    descs.push(TuiParamDescriptor { name: "Solo".into(), param_type: TuiParamType::Bool, unit: "".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Bypass".into(), param_type: TuiParamType::Bool, unit: "".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: multiband_compressor::THRESHOLD_MIN as f64, max: multiband_compressor::THRESHOLD_MAX as f64, step: 1.0 }, unit: "dB".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Ratio".into(), param_type: TuiParamType::Float { min: multiband_compressor::RATIO_MIN as f64, max: multiband_compressor::RATIO_MAX as f64, step: 0.1 }, unit: ":1".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: multiband_compressor::ATTACK_MIN as f64, max: multiband_compressor::ATTACK_MAX as f64, step: 0.5 }, unit: "ms".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: multiband_compressor::RELEASE_MIN as f64, max: multiband_compressor::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Knee".into(), param_type: TuiParamType::Float { min: multiband_compressor::KNEE_MIN as f64, max: multiband_compressor::KNEE_MAX as f64, step: 0.5 }, unit: "dB".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Makeup Gain".into(), param_type: TuiParamType::Float { min: -24.0, max: 24.0, step: 0.5 }, unit: "dB".into(), group: g });
                }
                descs
            }
            PluginSettings::MultibandExpander { num_bands, .. } => {
                let mut descs = vec![
                    TuiParamDescriptor { name: "Bands".into(), param_type: TuiParamType::Int { min: multiband_expander::NUM_BANDS_MIN as i32, max: multiband_expander::NUM_BANDS_MAX as i32, step: 1 }, unit: "".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Preset".into(), param_type: TuiParamType::Int { min: multiband_expander::CROSSOVER_PRESET_MIN, max: multiband_expander::CROSSOVER_PRESET_MAX, step: 1 }, unit: "".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 1".into(), param_type: TuiParamType::Float { min: multiband_expander::CROSSOVER_FREQ_1_MIN as f64, max: multiband_expander::CROSSOVER_FREQ_1_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 2".into(), param_type: TuiParamType::Float { min: multiband_expander::CROSSOVER_FREQ_2_MIN as f64, max: multiband_expander::CROSSOVER_FREQ_2_MAX as f64, step: 50.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 3".into(), param_type: TuiParamType::Float { min: multiband_expander::CROSSOVER_FREQ_3_MIN as f64, max: multiband_expander::CROSSOVER_FREQ_3_MAX as f64, step: 100.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Crossover 4".into(), param_type: TuiParamType::Float { min: multiband_expander::CROSSOVER_FREQ_4_MIN as f64, max: multiband_expander::CROSSOVER_FREQ_4_MAX as f64, step: 100.0 }, unit: "Hz".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: multiband_expander::THRESHOLD_MIN as f64, max: multiband_expander::THRESHOLD_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Ratio".into(), param_type: TuiParamType::Float { min: multiband_expander::RATIO_MIN as f64, max: multiband_expander::RATIO_MAX as f64, step: 0.1 }, unit: ":1".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: multiband_expander::ATTACK_MIN as f64, max: multiband_expander::ATTACK_MAX as f64, step: 0.1 }, unit: "ms".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: multiband_expander::RELEASE_MIN as f64, max: multiband_expander::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Range".into(), param_type: TuiParamType::Float { min: multiband_expander::RANGE_MIN as f64, max: multiband_expander::RANGE_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Knee".into(), param_type: TuiParamType::Float { min: multiband_expander::KNEE_MIN as f64, max: multiband_expander::KNEE_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Hysteresis".into(), param_type: TuiParamType::Float { min: multiband_expander::HYSTERESIS_MIN as f64, max: multiband_expander::HYSTERESIS_MAX as f64, step: 0.1 }, unit: "dB".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Hold".into(), param_type: TuiParamType::Float { min: multiband_expander::HOLD_MIN as f64, max: multiband_expander::HOLD_MAX as f64, step: 1.0 }, unit: "ms".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: multiband_expander::MIX_MIN as f64, max: multiband_expander::MIX_MAX as f64, step: 0.01 }, unit: "".into(), group: "Global".into() },
                    TuiParamDescriptor { name: "Link Channels".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Global".into() },
                ];
                for i in 0..*num_bands {
                    let g = format!("Band {}", i + 1);
                    descs.push(TuiParamDescriptor { name: "Solo".into(), param_type: TuiParamType::Bool, unit: "".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Bypass".into(), param_type: TuiParamType::Bool, unit: "".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Threshold".into(), param_type: TuiParamType::Float { min: multiband_expander::THRESHOLD_MIN as f64, max: multiband_expander::THRESHOLD_MAX as f64, step: 1.0 }, unit: "dB".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Ratio".into(), param_type: TuiParamType::Float { min: multiband_expander::RATIO_MIN as f64, max: multiband_expander::RATIO_MAX as f64, step: 0.1 }, unit: ":1".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: multiband_expander::ATTACK_MIN as f64, max: multiband_expander::ATTACK_MAX as f64, step: 0.1 }, unit: "ms".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: multiband_expander::RELEASE_MIN as f64, max: multiband_expander::RELEASE_MAX as f64, step: 5.0 }, unit: "ms".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Range".into(), param_type: TuiParamType::Float { min: multiband_expander::RANGE_MIN as f64, max: multiband_expander::RANGE_MAX as f64, step: 1.0 }, unit: "dB".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Knee".into(), param_type: TuiParamType::Float { min: multiband_expander::KNEE_MIN as f64, max: multiband_expander::KNEE_MAX as f64, step: 0.5 }, unit: "dB".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Hysteresis".into(), param_type: TuiParamType::Float { min: multiband_expander::HYSTERESIS_MIN as f64, max: multiband_expander::HYSTERESIS_MAX as f64, step: 0.1 }, unit: "dB".into(), group: g.clone() });
                    descs.push(TuiParamDescriptor { name: "Hold".into(), param_type: TuiParamType::Float { min: multiband_expander::HOLD_MIN as f64, max: multiband_expander::HOLD_MAX as f64, step: 1.0 }, unit: "ms".into(), group: g });
                }
                descs
            }
            PluginSettings::LoudnessCompensation { .. } => vec![
                TuiParamDescriptor { name: "Low Freq".into(), param_type: TuiParamType::Float { min: loudness_compensation::LOW_FREQ_MIN as f64, max: loudness_compensation::LOW_FREQ_MAX as f64, step: 5.0 }, unit: "Hz".into(), group: "Low".into() },
                TuiParamDescriptor { name: "Low Gain".into(), param_type: TuiParamType::Float { min: loudness_compensation::LOW_GAIN_MIN as f64, max: loudness_compensation::LOW_GAIN_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Low".into() },
                TuiParamDescriptor { name: "High Freq".into(), param_type: TuiParamType::Float { min: loudness_compensation::HIGH_FREQ_MIN as f64, max: loudness_compensation::HIGH_FREQ_MAX as f64, step: 100.0 }, unit: "Hz".into(), group: "High".into() },
                TuiParamDescriptor { name: "High Gain".into(), param_type: TuiParamType::Float { min: loudness_compensation::HIGH_GAIN_MIN as f64, max: loudness_compensation::HIGH_GAIN_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "High".into() },
                TuiParamDescriptor { name: "Auto Gain".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Max Auto Gain".into(), param_type: TuiParamType::Float { min: 0.0, max: 24.0, step: 1.0 }, unit: "dB".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Smoothing".into(), param_type: TuiParamType::Float { min: 1.0, max: 1000.0, step: 5.0 }, unit: "ms".into(), group: "Auto Gain".into() },
            ],
            PluginSettings::FletcherMunson { .. } => vec![
                TuiParamDescriptor { name: "Reference".into(), param_type: TuiParamType::Float { min: fletcher_munson::REFERENCE_LEVEL_DB_MIN as f64, max: fletcher_munson::REFERENCE_LEVEL_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Global".into() },
                TuiParamDescriptor { name: "Enabled".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Global".into() },
                TuiParamDescriptor { name: "Smoothing".into(), param_type: TuiParamType::Float { min: fletcher_munson::SMOOTHING_MS_MIN as f64, max: fletcher_munson::SMOOTHING_MS_MAX as f64, step: 1.0 }, unit: "ms".into(), group: "Global".into() },
                TuiParamDescriptor { name: "Auto Gain".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Max Correction".into(), param_type: TuiParamType::Float { min: fletcher_munson::AUTO_GAIN_MAX_DB_MIN as f64, max: fletcher_munson::AUTO_GAIN_MAX_DB_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "AG Smoothing".into(), param_type: TuiParamType::Float { min: fletcher_munson::AUTO_GAIN_SMOOTHING_MS_MIN as f64, max: fletcher_munson::AUTO_GAIN_SMOOTHING_MS_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Auto Gain".into() },
                // Band 1
                TuiParamDescriptor { name: "Band 1 Freq".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_FREQ_MIN, max: fletcher_munson::BAND_FREQ_MAX, step: 5.0 }, unit: "Hz".into(), group: "Band 1".into() },
                TuiParamDescriptor { name: "Band 1 Q".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_Q_MIN, max: fletcher_munson::BAND_Q_MAX, step: 0.05 }, unit: "".into(), group: "Band 1".into() },
                TuiParamDescriptor { name: "Band 1 Max".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_MAX_GAIN_MIN, max: fletcher_munson::BAND_MAX_GAIN_MAX, step: 0.5 }, unit: "dB".into(), group: "Band 1".into() },
                TuiParamDescriptor { name: "Band 1 Slope".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_SLOPE_MIN, max: fletcher_munson::BAND_SLOPE_MAX, step: 0.01 }, unit: "".into(), group: "Band 1".into() },
                // Band 2
                TuiParamDescriptor { name: "Band 2 Freq".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_FREQ_MIN, max: fletcher_munson::BAND_FREQ_MAX, step: 10.0 }, unit: "Hz".into(), group: "Band 2".into() },
                TuiParamDescriptor { name: "Band 2 Q".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_Q_MIN, max: fletcher_munson::BAND_Q_MAX, step: 0.05 }, unit: "".into(), group: "Band 2".into() },
                TuiParamDescriptor { name: "Band 2 Max".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_MAX_GAIN_MIN, max: fletcher_munson::BAND_MAX_GAIN_MAX, step: 0.5 }, unit: "dB".into(), group: "Band 2".into() },
                TuiParamDescriptor { name: "Band 2 Slope".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_SLOPE_MIN, max: fletcher_munson::BAND_SLOPE_MAX, step: 0.01 }, unit: "".into(), group: "Band 2".into() },
                // Band 3
                TuiParamDescriptor { name: "Band 3 Freq".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_FREQ_MIN, max: fletcher_munson::BAND_FREQ_MAX, step: 50.0 }, unit: "Hz".into(), group: "Band 3".into() },
                TuiParamDescriptor { name: "Band 3 Q".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_Q_MIN, max: fletcher_munson::BAND_Q_MAX, step: 0.05 }, unit: "".into(), group: "Band 3".into() },
                TuiParamDescriptor { name: "Band 3 Max".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_MAX_GAIN_MIN, max: fletcher_munson::BAND_MAX_GAIN_MAX, step: 0.5 }, unit: "dB".into(), group: "Band 3".into() },
                TuiParamDescriptor { name: "Band 3 Slope".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_SLOPE_MIN, max: fletcher_munson::BAND_SLOPE_MAX, step: 0.01 }, unit: "".into(), group: "Band 3".into() },
                // Band 4
                TuiParamDescriptor { name: "Band 4 Freq".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_FREQ_MIN, max: fletcher_munson::BAND_FREQ_MAX, step: 100.0 }, unit: "Hz".into(), group: "Band 4".into() },
                TuiParamDescriptor { name: "Band 4 Q".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_Q_MIN, max: fletcher_munson::BAND_Q_MAX, step: 0.05 }, unit: "".into(), group: "Band 4".into() },
                TuiParamDescriptor { name: "Band 4 Max".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_MAX_GAIN_MIN, max: fletcher_munson::BAND_MAX_GAIN_MAX, step: 0.5 }, unit: "dB".into(), group: "Band 4".into() },
                TuiParamDescriptor { name: "Band 4 Slope".into(), param_type: TuiParamType::Float { min: fletcher_munson::BAND_SLOPE_MIN, max: fletcher_munson::BAND_SLOPE_MAX, step: 0.01 }, unit: "".into(), group: "Band 4".into() },
            ],
            PluginSettings::BinauralDecoder { .. } => vec![
                TuiParamDescriptor { name: "SOFA File".into(), param_type: TuiParamType::Choice { count: 0 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Input Channels".into(), param_type: TuiParamType::Int { min: 2, max: 16, step: 1 }, unit: "ch".into(), group: "General".into() },
                TuiParamDescriptor { name: "Optimization".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Externalization".into(), param_type: TuiParamType::Float { min: binaural::EXTERNALIZATION_MIN as f64, max: binaural::EXTERNALIZATION_MAX as f64, step: 0.05 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Near-field".into(), param_type: TuiParamType::Float { min: binaural::NEAR_FIELD_STRENGTH_MIN as f64, max: binaural::NEAR_FIELD_STRENGTH_MAX as f64, step: 0.05 }, unit: "".into(), group: "General".into() },
            ],
            PluginSettings::Convolution { .. } => vec![
                TuiParamDescriptor { name: "IR File".into(), param_type: TuiParamType::Choice { count: 0 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: convolution::MIX_MIN as f64, max: convolution::MIX_MAX as f64, step: 0.05 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Gain".into(), param_type: TuiParamType::Float { min: convolution::GAIN_DB_MIN as f64, max: convolution::GAIN_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "General".into() },
            ],
            PluginSettings::XTC { .. } => vec![
                TuiParamDescriptor { name: "Distance".into(), param_type: TuiParamType::Float { min: xtc::DISTANCE_M_MIN, max: xtc::DISTANCE_M_MAX, step: 0.05 }, unit: "m".into(), group: "Geometry".into() },
                TuiParamDescriptor { name: "Speaker Angle".into(), param_type: TuiParamType::Float { min: xtc::SPEAKER_ANGLE_DEG_MIN, max: xtc::SPEAKER_ANGLE_DEG_MAX, step: 0.5 }, unit: "\u{00b0}".into(), group: "Geometry".into() },
                TuiParamDescriptor { name: "Head Radius".into(), param_type: TuiParamType::Float { min: xtc::HEAD_RADIUS_M_MIN, max: xtc::HEAD_RADIUS_M_MAX, step: 0.001 }, unit: "m".into(), group: "Geometry".into() },
                TuiParamDescriptor { name: "Head Offset X".into(), param_type: TuiParamType::Float { min: -0.5, max: 0.5, step: 0.01 }, unit: "m".into(), group: "Head Tracking".into() },
                TuiParamDescriptor { name: "Head Offset Z".into(), param_type: TuiParamType::Float { min: -0.5, max: 0.5, step: 0.01 }, unit: "m".into(), group: "Head Tracking".into() },
                TuiParamDescriptor { name: "Head Yaw".into(), param_type: TuiParamType::Float { min: -90.0, max: 90.0, step: 1.0 }, unit: "\u{00b0}".into(), group: "Head Tracking".into() },
                TuiParamDescriptor { name: "Beta Base".into(), param_type: TuiParamType::Float { min: xtc::BETA_BASE_MIN, max: xtc::BETA_BASE_MAX, step: 0.001 }, unit: "".into(), group: "Beta".into() },
                TuiParamDescriptor { name: "Beta Low Boost".into(), param_type: TuiParamType::Float { min: xtc::BETA_LOW_FREQ_BOOST_MIN, max: xtc::BETA_LOW_FREQ_BOOST_MAX, step: 0.5 }, unit: "".into(), group: "Beta".into() },
                TuiParamDescriptor { name: "Beta High Boost".into(), param_type: TuiParamType::Float { min: xtc::BETA_HIGH_FREQ_BOOST_MIN, max: xtc::BETA_HIGH_FREQ_BOOST_MAX, step: 0.5 }, unit: "".into(), group: "Beta".into() },
                TuiParamDescriptor { name: "Shadow Cutoff".into(), param_type: TuiParamType::Float { min: xtc::HEAD_SHADOW_CUTOFF_HZ_MIN, max: xtc::HEAD_SHADOW_CUTOFF_HZ_MAX, step: 50.0 }, unit: "Hz".into(), group: "Shadow".into() },
                TuiParamDescriptor { name: "Shadow Slope".into(), param_type: TuiParamType::Float { min: xtc::HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MIN, max: xtc::HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MAX, step: 0.5 }, unit: "dB/oct".into(), group: "Shadow".into() },
                TuiParamDescriptor { name: "Max Gain".into(), param_type: TuiParamType::Float { min: xtc::MAX_GAIN_DB_MIN, max: xtc::MAX_GAIN_DB_MAX, step: 1.0 }, unit: "dB".into(), group: "Filter".into() },
                TuiParamDescriptor { name: "Spectral Norm".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Advanced".into() },
                TuiParamDescriptor { name: "Pinna Model".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Advanced".into() },
                TuiParamDescriptor { name: "Room Reflections".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Room".into() },
                TuiParamDescriptor { name: "Room Width".into(), param_type: TuiParamType::Float { min: 2.0, max: 10.0, step: 0.1 }, unit: "m".into(), group: "Room".into() },
                TuiParamDescriptor { name: "Room Depth".into(), param_type: TuiParamType::Float { min: 2.0, max: 15.0, step: 0.1 }, unit: "m".into(), group: "Room".into() },
                TuiParamDescriptor { name: "Wall Absorption".into(), param_type: TuiParamType::Float { min: 0.0, max: 1.0, step: 0.05 }, unit: "".into(), group: "Room".into() },
                TuiParamDescriptor { name: "Reflection Beta".into(), param_type: TuiParamType::Float { min: 1.0, max: 10.0, step: 0.1 }, unit: "".into(), group: "Room".into() },
                TuiParamDescriptor { name: "Bypass XTC Filters".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostic".into() },
                TuiParamDescriptor { name: "Bypass Spectral Norm".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostic".into() },
                TuiParamDescriptor { name: "Bypass Neumann".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Diagnostic".into() },
                TuiParamDescriptor { name: "Auto Gain".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "AG Max".into(), param_type: TuiParamType::Float { min: xtc::AUTO_GAIN_MAX_DB_MIN as f64, max: xtc::AUTO_GAIN_MAX_DB_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "AG Smoothing".into(), param_type: TuiParamType::Float { min: xtc::AUTO_GAIN_SMOOTHING_MS_MIN as f64, max: xtc::AUTO_GAIN_SMOOTHING_MS_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Auto Gain".into() },
            ],
            PluginSettings::Denoiser { .. } => vec![
                TuiParamDescriptor { name: "Reduction".into(), param_type: TuiParamType::Float { min: denoiser::REDUCTION_DB_MIN as f64, max: denoiser::REDUCTION_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "General".into() },
                TuiParamDescriptor { name: "Floor".into(), param_type: TuiParamType::Float { min: denoiser::FLOOR_DB_MIN as f64, max: denoiser::FLOOR_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "General".into() },
                TuiParamDescriptor { name: "Smoothing".into(), param_type: TuiParamType::Float { min: denoiser::SMOOTHING_MIN as f64, max: denoiser::SMOOTHING_MAX as f64, step: 0.01 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Attack".into(), param_type: TuiParamType::Float { min: denoiser::ATTACK_MS_MIN as f64, max: denoiser::ATTACK_MS_MAX as f64, step: 0.5 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Release".into(), param_type: TuiParamType::Float { min: denoiser::RELEASE_MS_MIN as f64, max: denoiser::RELEASE_MS_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "Timing".into() },
                TuiParamDescriptor { name: "Low Latency".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Polyphonic".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Analysis".into() },
                TuiParamDescriptor { name: "Crack Sens.".into(), param_type: TuiParamType::Float { min: 1.0, max: 100.0, step: 1.0 }, unit: "".into(), group: "Analysis".into() },
                TuiParamDescriptor { name: "MCRA Alpha S".into(), param_type: TuiParamType::Float { min: 0.5, max: 0.99, step: 0.01 }, unit: "".into(), group: "Advanced".into() },
                TuiParamDescriptor { name: "MCRA Alpha P".into(), param_type: TuiParamType::Float { min: 0.1, max: 0.99, step: 0.01 }, unit: "".into(), group: "Advanced".into() },
                TuiParamDescriptor { name: "MCRA Window".into(), param_type: TuiParamType::Int { min: 10, max: 200, step: 1 }, unit: "fr".into(), group: "Advanced".into() },
                TuiParamDescriptor { name: "MCRA Delta".into(), param_type: TuiParamType::Float { min: 1.0, max: 20.0, step: 0.5 }, unit: "".into(), group: "Advanced".into() },
                TuiParamDescriptor { name: "Transparency".into(), param_type: TuiParamType::Float { min: denoiser::TRANSPARENCY_MIN as f64, max: denoiser::TRANSPARENCY_MAX as f64, step: 0.01 }, unit: "".into(), group: "Analysis".into() },
                TuiParamDescriptor { name: "DD SNR".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Analysis".into() },
                TuiParamDescriptor { name: "DD Alpha".into(), param_type: TuiParamType::Float { min: denoiser::DD_ALPHA_MIN as f64, max: denoiser::DD_ALPHA_MAX as f64, step: 0.001 }, unit: "".into(), group: "Analysis".into() },
                TuiParamDescriptor { name: "Psychoacoustic".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Analysis".into() },
                TuiParamDescriptor { name: "Learn Noise".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Noise Profile".into() },
                TuiParamDescriptor { name: "Use Profile".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Noise Profile".into() },
                TuiParamDescriptor { name: "Clear Profile".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Noise Profile".into() },
            ],
            PluginSettings::Pnd { .. } => vec![
                TuiParamDescriptor { name: "Correction".into(), param_type: TuiParamType::Float { min: pnd::CORRECTION_STRENGTH_MIN as f64, max: pnd::CORRECTION_STRENGTH_MAX as f64, step: 0.05 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Analysis Window".into(), param_type: TuiParamType::Float { min: pnd::ANALYSIS_WINDOW_MS_MIN as f64, max: pnd::ANALYSIS_WINDOW_MS_MAX as f64, step: 5.0 }, unit: "ms".into(), group: "General".into() },
                TuiParamDescriptor { name: "Drift Smoothing".into(), param_type: TuiParamType::Float { min: pnd::DRIFT_SMOOTHING_MIN as f64, max: pnd::DRIFT_SMOOTHING_MAX as f64, step: 0.001 }, unit: "".into(), group: "General".into() },
            ],
            PluginSettings::ABCompare { .. } => vec![
                TuiParamDescriptor { name: "Mix (A/B)".into(), param_type: TuiParamType::Float { min: ab_compare::MIX_MIN, max: ab_compare::MIX_MAX, step: 0.05 }, unit: "".into(), group: "Mix".into() },
                TuiParamDescriptor { name: "Mix Mode".into(), param_type: TuiParamType::Choice { count: 2 }, unit: "".into(), group: "Mix".into() },
                TuiParamDescriptor { name: "Selected Path".into(), param_type: TuiParamType::Choice { count: 2 }, unit: "".into(), group: "Mix".into() },
                TuiParamDescriptor { name: "Bypass".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Mix".into() },
                TuiParamDescriptor { name: "Auto Gain".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Loudness Type".into(), param_type: TuiParamType::Choice { count: 2 }, unit: "".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Max Auto Gain".into(), param_type: TuiParamType::Float { min: ab_compare::MAX_AUTO_GAIN_DB_MIN, max: ab_compare::MAX_AUTO_GAIN_DB_MAX, step: 1.0 }, unit: "dB".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Gain Smoothing".into(), param_type: TuiParamType::Float { min: ab_compare::GAIN_SMOOTHING_MS_MIN, max: ab_compare::GAIN_SMOOTHING_MS_MAX, step: 5.0 }, unit: "ms".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Mix Transition".into(), param_type: TuiParamType::Float { min: ab_compare::MIX_TRANSITION_MS_MIN, max: ab_compare::MIX_TRANSITION_MS_MAX, step: 5.0 }, unit: "ms".into(), group: "Mix".into() },
            ],
            PluginSettings::BandSplit { .. } => vec![
                TuiParamDescriptor { name: "Frequency".into(), param_type: TuiParamType::Float { min: band_split::FREQUENCY_MIN, max: band_split::FREQUENCY_MAX, step: 10.0 }, unit: "Hz".into(), group: "General".into() },
                TuiParamDescriptor { name: "Type".into(), param_type: TuiParamType::Choice { count: 2 }, unit: "".into(), group: "General".into() },
            ],
            PluginSettings::BandMerge { .. } => vec![
                TuiParamDescriptor { name: "Bands".into(), param_type: TuiParamType::Int { min: band_merge::BANDS_MIN as i32, max: band_merge::BANDS_MAX as i32, step: 1 }, unit: "".into(), group: "General".into() },
            ],
            PluginSettings::Downmix { .. } => vec![
                TuiParamDescriptor { name: "Center Gain".into(), param_type: TuiParamType::Float { min: downmix::CENTER_GAIN_DB_MIN as f64, max: downmix::CENTER_GAIN_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "Surround Gain".into(), param_type: TuiParamType::Float { min: downmix::SURROUND_GAIN_DB_MIN as f64, max: downmix::SURROUND_GAIN_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "Height Gain".into(), param_type: TuiParamType::Float { min: downmix::HEIGHT_GAIN_DB_MIN as f64, max: downmix::HEIGHT_GAIN_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "LFE Gain".into(), param_type: TuiParamType::Float { min: downmix::LFE_GAIN_DB_MIN as f64, max: downmix::LFE_GAIN_DB_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Gains".into() },
                TuiParamDescriptor { name: "Phase Coherence".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Phase".into() },
                TuiParamDescriptor { name: "Phase Blend Low".into(), param_type: TuiParamType::Float { min: downmix::PHASE_BLEND_LOW_HZ_MIN as f64, max: downmix::PHASE_BLEND_LOW_HZ_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "Phase".into() },
                TuiParamDescriptor { name: "Phase Blend High".into(), param_type: TuiParamType::Float { min: downmix::PHASE_BLEND_HIGH_HZ_MIN as f64, max: downmix::PHASE_BLEND_HIGH_HZ_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "Phase".into() },
            ],
            PluginSettings::MonoToStereo { .. } => vec![
                TuiParamDescriptor { name: "Width".into(), param_type: TuiParamType::Float { min: mono_to_stereo::STEREO_WIDTH_MIN as f64, max: mono_to_stereo::STEREO_WIDTH_MAX as f64, step: 0.05 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Haas Delay".into(), param_type: TuiParamType::Float { min: mono_to_stereo::HAAS_DELAY_MS_MIN as f64, max: mono_to_stereo::HAAS_DELAY_MS_MAX as f64, step: 0.1 }, unit: "ms".into(), group: "General".into() },
                TuiParamDescriptor { name: "Comp EQ".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "EQ".into() },
                TuiParamDescriptor { name: "Comp EQ Depth".into(), param_type: TuiParamType::Float { min: mono_to_stereo::COMP_EQ_DEPTH_DB_MIN as f64, max: mono_to_stereo::COMP_EQ_DEPTH_DB_MAX as f64, step: 0.1 }, unit: "dB".into(), group: "EQ".into() },
                TuiParamDescriptor { name: "Decor Low".into(), param_type: TuiParamType::Float { min: mono_to_stereo::DECOR_LOW_HZ_MIN as f64, max: mono_to_stereo::DECOR_LOW_HZ_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "General".into() },
                TuiParamDescriptor { name: "Decor High".into(), param_type: TuiParamType::Float { min: mono_to_stereo::DECOR_HIGH_HZ_MIN as f64, max: mono_to_stereo::DECOR_HIGH_HZ_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "General".into() },
            ],
            PluginSettings::Crossfeed { .. } => vec![
                TuiParamDescriptor { name: "Mode".into(), param_type: TuiParamType::Choice { count: 4 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Preset".into(), param_type: TuiParamType::Choice { count: 5 }, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Enabled".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "General".into() },
                TuiParamDescriptor { name: "Mix".into(), param_type: TuiParamType::Float { min: crossfeed::MIX_MIN as f64, max: crossfeed::MIX_MAX as f64, step: 0.05 }, unit: "".into(), group: "General".into() },
                // Bauer
                TuiParamDescriptor { name: "Bauer Cutoff".into(), param_type: TuiParamType::Float { min: crossfeed::BAUER_FCUT_MIN as f64, max: crossfeed::BAUER_FCUT_MAX as f64, step: 10.0 }, unit: "Hz".into(), group: "Bauer".into() },
                TuiParamDescriptor { name: "Bauer Feed".into(), param_type: TuiParamType::Float { min: crossfeed::BAUER_FEED_MIN as f64, max: crossfeed::BAUER_FEED_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Bauer".into() },
                // Meier
                TuiParamDescriptor { name: "Meier Level".into(), param_type: TuiParamType::Float { min: crossfeed::MEIER_LEVEL_MIN as f64, max: crossfeed::MEIER_LEVEL_MAX as f64, step: 1.0 }, unit: "%".into(), group: "Meier".into() },
                // Multiband
                TuiParamDescriptor { name: "MB Low Freq".into(), param_type: TuiParamType::Float { min: crossfeed::MB_LOW_FREQ_MIN as f64, max: crossfeed::MB_LOW_FREQ_MAX as f64, step: 5.0 }, unit: "Hz".into(), group: "Multiband".into() },
                TuiParamDescriptor { name: "MB Mid/High Freq".into(), param_type: TuiParamType::Float { min: crossfeed::MB_MID_HIGH_FREQ_MIN as f64, max: crossfeed::MB_MID_HIGH_FREQ_MAX as f64, step: 50.0 }, unit: "Hz".into(), group: "Multiband".into() },
                TuiParamDescriptor { name: "MB Low Feed".into(), param_type: TuiParamType::Float { min: crossfeed::MB_LOW_FEED_MIN as f64, max: crossfeed::MB_LOW_FEED_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Multiband".into() },
                TuiParamDescriptor { name: "MB Mid Feed".into(), param_type: TuiParamType::Float { min: crossfeed::MB_MID_FEED_MIN as f64, max: crossfeed::MB_MID_FEED_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Multiband".into() },
                TuiParamDescriptor { name: "MB High Feed".into(), param_type: TuiParamType::Float { min: crossfeed::MB_HIGH_FEED_MIN as f64, max: crossfeed::MB_HIGH_FEED_MAX as f64, step: 0.5 }, unit: "dB".into(), group: "Multiband".into() },
                // Auto Gain
                TuiParamDescriptor { name: "Auto Gain".into(), param_type: TuiParamType::Bool, unit: "".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Target LUFS".into(), param_type: TuiParamType::Float { min: crossfeed::AUTOGAIN_TARGET_MIN as f64, max: crossfeed::AUTOGAIN_TARGET_MAX as f64, step: 0.5 }, unit: "LUFS".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Max Gain".into(), param_type: TuiParamType::Float { min: crossfeed::AUTOGAIN_MAX_GAIN_MIN as f64, max: crossfeed::AUTOGAIN_MAX_GAIN_MAX as f64, step: 1.0 }, unit: "dB".into(), group: "Auto Gain".into() },
                TuiParamDescriptor { name: "Smoothing".into(), param_type: TuiParamType::Float { min: crossfeed::AUTOGAIN_SMOOTHING_MIN as f64, max: crossfeed::AUTOGAIN_SMOOTHING_MAX as f64, step: 10.0 }, unit: "ms".into(), group: "Auto Gain".into() },
            ],
            _ => vec![], // Placeholder for other types
        }
    }

    fn get_params(&self) -> Vec<TuiParamSpec> {
        let descriptors = self.get_descriptors();
        let mut params = Vec::with_capacity(descriptors.len());
        for (i, desc) in descriptors.into_iter().enumerate() {
            params.push(TuiParamSpec {
                name: desc.name,
                value: self.get_value_as_string(i),
                unit: desc.unit,
            });
        }
        params
    }

    fn get_value_as_string(&self, index: usize) -> String {
        match self {
            PluginSettings::Gain { gain_db, .. } => {
                if index == 0 {
                    format!("{:.1}", gain_db)
                } else {
                    String::new()
                }
            }
            PluginSettings::EQ { filters, max_filters, .. } => {
                if index == 0 {
                    return format!("{}", max_filters);
                }
                let filter_offset = index - 1;
                let filter_idx = filter_offset / 4;
                let param_idx = filter_offset % 4;
                if let Some(filter) = filters.get(filter_idx) {
                    match param_idx {
                        0 => format!("{:.0}", filter.frequency),
                        1 => format!("{:.2}", filter.q),
                        2 => format!("{:.1}", filter.gain_db),
                        3 => format!("{:?}", filter.filter_type),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                }
            }
            PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => match index {
                0 => format!("{:.1}", threshold_db),
                1 => format!("{:.1}", ratio),
                2 => format!("{:.1}", attack_ms),
                3 => format!("{:.1}", release_ms),
                4 => format!("{:.1}", knee_db),
                5 => format!("{:.1}", makeup_gain_db),
                6 => format!("{:.0}%", mix * 100.0),
                7 => (if *auto_makeup { "On" } else { "Off" }).to_string(),
                8 => (if *link_channels { "Linked" } else { "Unlinked" }).to_string(),
                9 => format!("{:.0}", sidechain_hpf_hz),
                _ => String::new(),
            },
            PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
                enable_ml_detection,
            } => match index {
                0 => speaker_config.clone(),
                1 => format!("{:.2}", gain_front_direct),
                2 => format!("{:.2}", gain_front_ambient),
                3 => format!("{:.2}", gain_rear_ambient),
                4 => format!("{:.2}", height_gain),
                5 => format!("{:.2}", lfe_gain),
                6 => format!("{:.0}", lfe_cutoff_hz),
                7 => (if *enable_subharmonic_synth { "On" } else { "Off" }).into(),
                8 => format!("{:.2}", subharmonic_gain),
                9 => format!("{:.0}", subharmonic_freq_hz),
                10 => format!("{:.1}", subharmonic_attack_ms),
                11 => format!("{:.1}", subharmonic_release_ms),
                12 => format!("{:.2}", stereo_width),
                13 => format!("{:.2}", center_spread),
                14 => format!("{:.0}", bandpass_hz),
                15 => (if *enable_hr_direct { "On" } else { "Off" }).into(),
                16 => format!("{:.2}", hr_sharpen),
                17 => format!("{:.2}", ambient_boost),
                18 => match decorrelation_mode {
                    0 => "Velvet Noise".into(),
                    1 => "LFO Phase".into(),
                    _ => "Unknown".into(),
                },
                19 => format!("{:.2}", decorrelation_lfo_rate_hz),
                20 => format!("{:.0}", velvet_noise_duration_ms),
                21 => format!("{:.0}", velvet_noise_density),
                22 => format!("{:.0}", height_hf_cap_hz),
                23 => format!("{:.2}", height_transient_reduction),
                24 => format!("{:.2}", height_direct_leak),
                25 => format!("{:.2}", surround_direct_bleed),
                26 => format!("{:.2}", rear_ambient_boost),
                27 => format!("{:.2}", rear_late_reflection),
                28 => format!("{:.2}", dialogue_weight),
                29 => format!("{:.0}", voice_freq_min_hz),
                30 => format!("{:.0}", voice_freq_max_hz),
                31 => format!("{:.2}", dialogue_centroid_weight),
                32 => format!("{:.2}", dialogue_variance_weight),
                33 => format!("{:.2}", dialogue_coherence_weight),
                34 => format!("{:.1}", safety_cap_db),
                35 => (if *bypass_decorrelation { "On" } else { "Off" }).into(),
                36 => (if *bypass_transient_detection { "On" } else { "Off" }).into(),
                37 => (if *bypass_all_processing { "On" } else { "Off" }).into(),
                38 => (if *enable_ml_detection { "On" } else { "Off" }).into(),
                _ => String::new(),
            },
            PluginSettings::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => match index {
                0 => format!("{:.1}", threshold_db),
                1 => format!("{:.0}", release_ms),
                2 => format!("{:.1}", lookahead_ms),
                3 => (if *soft { "Soft" } else { "Hard" }).into(),
                4 => format!("{:.0}%", mix * 100.0),
                _ => String::new(),
            },
            PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => match index {
                0 => format!("{:.1}", threshold_db),
                1 => format!("{:.1}", ratio),
                2 => format!("{:.1}", attack_ms),
                3 => format!("{:.0}", hold_ms),
                4 => format!("{:.0}", release_ms),
                5 => format!("{:.0}%", mix * 100.0),
                6 => (if *link_channels { "Linked" } else { "Unlinked" }).into(),
                7 => format!("{:.0}", sidechain_hpf_hz),
                _ => String::new(),
            },
            PluginSettings::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => match index {
                0 => format!("{:.0}", low_freq),
                1 => format!("{:.1}", low_gain),
                2 => format!("{:.0}", high_freq),
                3 => format!("{:.1}", high_gain),
                4 => (if *auto_gain_enabled { "On" } else { "Off" }).into(),
                5 => format!("{:.1}", auto_gain_max_db),
                6 => format!("{:.0}", auto_gain_smoothing_ms),
                _ => String::new(),
            },
            PluginSettings::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
                ..
            } => match index {
                0 => if sofa_file.is_empty() {
                    "None".to_string()
                } else {
                    PathBuf::from(sofa_file)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                },
                1 => format!("{}", input_channels),
                2 => (if *enable_optimization { "On" } else { "Off" }).into(),
                3 => format!("{:.2}", externalization),
                4 => format!("{:.2}", near_field_strength),
                _ => String::new(),
            },
            PluginSettings::Convolution {
                ir_file,
                mix,
                gain_db,
            } => match index {
                0 => if ir_file.is_empty() {
                    "None".to_string()
                } else {
                    PathBuf::from(ir_file)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                },
                1 => format!("{:.0}%", mix * 100.0),
                2 => format!("{:.1}", gain_db),
                _ => String::new(),
            },
            PluginSettings::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
                max_gain_db,
                head_offset_x,
                head_offset_z,
                head_yaw_deg,
                head_tracking_smooth_s: _,
                spectral_normalization,
                room_reflections_enabled,
                room_ir_file: _,
                room_width_m,
                room_depth_m,
                wall_absorption,
                reflection_beta_boost,
                bypass_xtc_filters,
                bypass_spectral_normalization,
                bypass_neumann_refinement,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                pinna_model_enabled,
            } => match index {
                0 => format!("{:.2}", distance_m),
                1 => format!("{:.1}", speaker_angle_deg),
                2 => format!("{:.4}", head_radius_m),
                3 => format!("{:.2}", head_offset_x),
                4 => format!("{:.2}", head_offset_z),
                5 => format!("{:.1}", head_yaw_deg),
                6 => format!("{:.4}", beta_base),
                7 => format!("{:.1}", beta_low_freq_boost),
                8 => format!("{:.1}", beta_high_freq_boost),
                9 => format!("{:.0}", head_shadow_cutoff_hz),
                10 => format!("{:.1}", head_shadow_slope_db_per_octave),
                11 => format!("{:.1}", max_gain_db),
                12 => (if *spectral_normalization { "On" } else { "Off" }).into(),
                13 => (if *pinna_model_enabled { "On" } else { "Off" }).into(),
                14 => (if *room_reflections_enabled { "On" } else { "Off" }).into(),
                15 => format!("{:.1}", room_width_m),
                16 => format!("{:.1}", room_depth_m),
                17 => format!("{:.2}", wall_absorption),
                18 => format!("{:.1}", reflection_beta_boost),
                19 => (if *bypass_xtc_filters { "On" } else { "Off" }).into(),
                20 => (if *bypass_spectral_normalization { "On" } else { "Off" }).into(),
                21 => (if *bypass_neumann_refinement { "On" } else { "Off" }).into(),
                22 => (if *auto_gain_enabled { "On" } else { "Off" }).into(),
                23 => format!("{:.1}", auto_gain_max_db),
                24 => format!("{:.0}", auto_gain_smoothing_ms),
                _ => String::new(),
            },
            PluginSettings::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
                polyphonic_detection,
                crack_sensitivity,
                mcra_alpha_s,
                mcra_alpha_p,
                mcra_l,
                mcra_delta,
                transparency,
                dd_enabled,
                dd_alpha,
                psychoacoustic_masking,
                learn_noise,
                use_captured_profile,
                clear_profile,
            } => match index {
                0 => format!("{:.1}", reduction_db),
                1 => format!("{:.1}", floor_db),
                2 => format!("{:.2}", smoothing),
                3 => format!("{:.1}", attack_ms),
                4 => format!("{:.1}", release_ms),
                5 => (if *low_latency { "On" } else { "Off" }).into(),
                6 => (if *polyphonic_detection { "On" } else { "Off" }).into(),
                7 => format!("{:.1}", crack_sensitivity),
                8 => format!("{:.2}", mcra_alpha_s),
                9 => format!("{:.2}", mcra_alpha_p),
                10 => format!("{}", mcra_l),
                11 => format!("{:.1}", mcra_delta),
                12 => format!("{:.2}", transparency),
                13 => (if *dd_enabled { "On" } else { "Off" }).into(),
                14 => format!("{:.3}", dd_alpha),
                15 => (if *psychoacoustic_masking { "On" } else { "Off" }).into(),
                16 => (if *learn_noise { "Active" } else { "Off" }).into(),
                17 => (if *use_captured_profile { "On" } else { "Off" }).into(),
                18 => (if *clear_profile { "Trigger" } else { "Off" }).into(),
                _ => String::new(),
            },
            PluginSettings::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => match index {
                0 => format!("{:.2}", correction_strength),
                1 => format!("{:.1}", analysis_window_ms),
                2 => format!("{:.3}", drift_smoothing),
                _ => String::new(),
            },
            PluginSettings::ABCompare {
                mix,
                mix_mode,
                selected_path,
                bypass,
                auto_gain_enabled,
                loudness_type,
                max_auto_gain_db,
                gain_smoothing_ms,
                mix_transition_ms,
                ..
            } => match index {
                0 => format!("{:.2}", mix),
                1 => (if *mix_mode == 0 { "Pot" } else { "Binary" }).into(),
                2 => (if *selected_path == 0 { "A" } else { "B" }).into(),
                3 => (if *bypass { "Yes" } else { "No" }).into(),
                4 => (if *auto_gain_enabled { "On" } else { "Off" }).into(),
                5 => (if *loudness_type == 0 { "Momentary" } else { "ShortTerm" }).into(),
                6 => format!("{:.1}", max_auto_gain_db),
                7 => format!("{:.0}", gain_smoothing_ms),
                8 => format!("{:.0}", mix_transition_ms),
                _ => String::new(),
            },
            PluginSettings::BandSplit {
                frequency,
                crossover_type,
                ..
            } => match index {
                0 => format!("{:.0}", frequency),
                1 => crossover_type.clone(),
                _ => String::new(),
            },
            PluginSettings::BandMerge { bands, .. } => match index {
                0 => format!("{}", bands),
                _ => String::new(),
            },
            PluginSettings::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => match index {
                0 => format!("{:.1}", center_gain_db),
                1 => format!("{:.1}", surround_gain_db),
                2 => format!("{:.1}", height_gain_db),
                3 => format!("{:.1}", lfe_gain_db),
                4 => (if *phase_coherence { "On" } else { "Off" }).into(),
                5 => format!("{:.0}", phase_blend_low_hz),
                6 => format!("{:.0}", phase_blend_high_hz),
                _ => String::new(),
            },
            PluginSettings::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => match index {
                0 => format!("{:.2}", stereo_width),
                1 => format!("{:.1}", haas_delay_ms),
                2 => (if *enable_comp_eq { "On" } else { "Off" }).into(),
                3 => format!("{:.1}", comp_eq_depth_db),
                4 => format!("{:.0}", decor_low_hz),
                5 => format!("{:.0}", decor_high_hz),
                _ => String::new(),
            },
            PluginSettings::MultibandCompressor {
                num_bands,
                crossover_preset,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                mix,
                link_channels,
                bands,
                ..
            } => {
                const GLOBAL_COUNT: usize = 13;
                const BAND_PARAMS: usize = 8;
                if index < GLOBAL_COUNT {
                    match index {
                        0 => format!("{}", num_bands),
                        1 => format!("{}", crossover_preset),
                        2 => format!("{:.0}", crossover_freq_1),
                        3 => format!("{:.0}", crossover_freq_2),
                        4 => format!("{:.0}", crossover_freq_3),
                        5 => format!("{:.0}", crossover_freq_4),
                        6 => format!("{:.1}", threshold_db),
                        7 => format!("{:.1}", ratio),
                        8 => format!("{:.1}", attack_ms),
                        9 => format!("{:.0}", release_ms),
                        10 => format!("{:.1}", knee_db),
                        11 => format!("{:.0}%", mix * 100.0),
                        12 => (if *link_channels { "Linked" } else { "Unlinked" }).into(),
                        _ => String::new(),
                    }
                } else {
                    let band_offset = index - GLOBAL_COUNT;
                    let band_idx = band_offset / BAND_PARAMS;
                    let param_in_band = band_offset % BAND_PARAMS;
                    if let Some(band) = bands.get(band_idx) {
                        match param_in_band {
                            0 => (if band.solo { "On" } else { "Off" }).into(),
                            1 => (if band.bypass { "On" } else { "Off" }).into(),
                            2 => band.threshold_db.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            3 => band.ratio.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            4 => band.attack_ms.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            5 => band.release_ms.map(|v| format!("{:.0}", v)).unwrap_or("Global".into()),
                            6 => band.knee_db.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            7 => format!("{:.1}", band.makeup_gain_db),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
                }
            }
            PluginSettings::MultibandExpander {
                num_bands,
                crossover_preset,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                bands,
                ..
            } => {
                const GLOBAL_COUNT: usize = 16;
                const BAND_PARAMS: usize = 10;
                if index < GLOBAL_COUNT {
                    match index {
                        0 => format!("{}", num_bands),
                        1 => format!("{}", crossover_preset),
                        2 => format!("{:.0}", crossover_freq_1),
                        3 => format!("{:.0}", crossover_freq_2),
                        4 => format!("{:.0}", crossover_freq_3),
                        5 => format!("{:.0}", crossover_freq_4),
                        6 => format!("{:.1}", threshold_db),
                        7 => format!("{:.1}", ratio),
                        8 => format!("{:.1}", attack_ms),
                        9 => format!("{:.0}", release_ms),
                        10 => format!("{:.1}", range_db),
                        11 => format!("{:.1}", knee_db),
                        12 => format!("{:.1}", hysteresis_db),
                        13 => format!("{:.0}", hold_ms),
                        14 => format!("{:.0}%", mix * 100.0),
                        15 => (if *link_channels { "Linked" } else { "Unlinked" }).into(),
                        _ => String::new(),
                    }
                } else {
                    let band_offset = index - GLOBAL_COUNT;
                    let band_idx = band_offset / BAND_PARAMS;
                    let param_in_band = band_offset % BAND_PARAMS;
                    if let Some(band) = bands.get(band_idx) {
                        match param_in_band {
                            0 => (if band.solo { "On" } else { "Off" }).into(),
                            1 => (if band.bypass { "On" } else { "Off" }).into(),
                            2 => band.threshold_db.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            3 => band.ratio.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            4 => band.attack_ms.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            5 => band.release_ms.map(|v| format!("{:.0}", v)).unwrap_or("Global".into()),
                            6 => band.range_db.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            7 => band.knee_db.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            8 => band.hysteresis_db.map(|v| format!("{:.1}", v)).unwrap_or("Global".into()),
                            9 => band.hold_ms.map(|v| format!("{:.0}", v)).unwrap_or("Global".into()),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
                }
            }
            PluginSettings::Crossfeed {
                mode,
                preset,
                enabled,
                mix,
                bauer_fcut_hz,
                bauer_feed_db,
                meier_level,
                mb_low_freq_hz,
                mb_mid_high_freq_hz,
                mb_low_feed_db,
                mb_mid_feed_db,
                mb_high_feed_db,
                autogain_enabled,
                autogain_target_lufs,
                autogain_max_gain_db,
                autogain_smoothing_ms,
            } => match index {
                0 => format!("{:?}", mode),
                1 => format!("{:?}", preset),
                2 => (if *enabled { "On" } else { "Off" }).into(),
                3 => format!("{:.0}%", mix * 100.0),
                4 => format!("{:.0}", bauer_fcut_hz),
                5 => format!("{:.1}", bauer_feed_db),
                6 => format!("{:.0}", meier_level),
                7 => format!("{:.0}", mb_low_freq_hz),
                8 => format!("{:.0}", mb_mid_high_freq_hz),
                9 => format!("{:.1}", mb_low_feed_db),
                10 => format!("{:.1}", mb_mid_feed_db),
                11 => format!("{:.1}", mb_high_feed_db),
                12 => (if *autogain_enabled { "On" } else { "Off" }).into(),
                13 => format!("{:.1}", autogain_target_lufs),
                14 => format!("{:.1}", autogain_max_gain_db),
                15 => format!("{:.0}", autogain_smoothing_ms),
                _ => String::new(),
            },
            PluginSettings::FletcherMunson {
                playback_volume_db: _,
                reference_level_db,
                enabled,
                band1_freq,
                band1_q,
                band1_max_gain,
                band1_slope,
                band2_freq,
                band2_q,
                band2_max_gain,
                band2_slope,
                band3_freq,
                band3_q,
                band3_max_gain,
                band3_slope,
                band4_freq,
                band4_q,
                band4_max_gain,
                band4_slope,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                ..
            } => match index {
                0 => format!("{:.1}", reference_level_db),
                1 => (if *enabled { "On" } else { "Off" }).into(),
                2 => format!("{:.0}", smoothing_ms),
                3 => (if *auto_gain_enabled { "On" } else { "Off" }).into(),
                4 => format!("{:.1}", auto_gain_max_db),
                5 => format!("{:.0}", auto_gain_smoothing_ms),
                // Band 1
                6 => format!("{:.0}", band1_freq),
                7 => format!("{:.2}", band1_q),
                8 => format!("{:.1}", band1_max_gain),
                9 => format!("{:.2}", band1_slope),
                // Band 2
                10 => format!("{:.0}", band2_freq),
                11 => format!("{:.2}", band2_q),
                12 => format!("{:.1}", band2_max_gain),
                13 => format!("{:.2}", band2_slope),
                // Band 3
                14 => format!("{:.0}", band3_freq),
                15 => format!("{:.2}", band3_q),
                16 => format!("{:.1}", band3_max_gain),
                17 => format!("{:.2}", band3_slope),
                // Band 4
                18 => format!("{:.0}", band4_freq),
                19 => format!("{:.2}", band4_q),
                20 => format!("{:.1}", band4_max_gain),
                21 => format!("{:.2}", band4_slope),
                _ => String::new(),
            },
            PluginSettings::Expander {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => match index {
                0 => format!("{:.1}", threshold_db),
                1 => format!("{:.1}", ratio),
                2 => format!("{:.1}", attack_ms),
                3 => format!("{:.0}", release_ms),
                4 => format!("{:.1}", range_db),
                5 => format!("{:.1}", knee_db),
                6 => format!("{:.1}", hysteresis_db),
                7 => format!("{:.0}", hold_ms),
                8 => format!("{:.0}%", mix * 100.0),
                9 => (if *link_channels { "Linked" } else { "Unlinked" }).into(),
                10 => format!("{:.0}", sidechain_hpf_hz),
                _ => String::new(),
            },
            _ => String::new(),
        }
    }

    fn adjust_param(&mut self, index: usize, delta: f64) -> bool {
        match self {
            PluginSettings::Gain { gain_db, .. } => {
                if index == 0 {
                    *gain_db = (*gain_db + delta * 0.5).clamp(-40.0, 40.0);
                    return true;
                }
            }
            PluginSettings::EQ { filters, max_filters, .. } => {
                if index == 0 {
                    let old_max = *max_filters;
                    *max_filters = ((*max_filters as i64) + delta as i64).clamp(1, 20) as usize;
                    
                    if *max_filters > old_max {
                        // Add new default filters if we increased the limit
                        while filters.len() < *max_filters {
                            filters.push(crate::EQFilter::new(
                                crate::BiquadFilterType::Peak,
                                1000.0,
                                1.0,
                                0.0,
                            ));
                        }
                    } else if *max_filters < old_max {
                        // Truncate filters if we decreased the limit
                        filters.truncate(*max_filters);
                    }
                    return true;
                }
                let filter_offset = index - 1;
                let filter_idx = filter_offset / 4;
                let param_idx = filter_offset % 4;
                if let Some(filter) = filters.get_mut(filter_idx) {
                    match param_idx {
                        0 => {
                            filter.frequency =
                                (filter.frequency + delta * 10.0).clamp(20.0, 20000.0)
                        }
                        1 => filter.q = (filter.q + delta * 0.1).clamp(0.1, 10.0),
                        2 => filter.gain_db = (filter.gain_db + delta * 0.5).clamp(-24.0, 24.0),
                        3 => {
                            use crate::BiquadFilterType;
                            let types = [
                                BiquadFilterType::Peak,
                                BiquadFilterType::Lowshelf,
                                BiquadFilterType::Highshelf,
                                BiquadFilterType::Lowpass,
                                BiquadFilterType::Highpass,
                                BiquadFilterType::Bandpass,
                                BiquadFilterType::Notch,
                            ];
                            let current_idx = types
                                .iter()
                                .position(|t| *t == filter.filter_type)
                                .unwrap_or(0);
                            let new_idx = if delta > 0.0 {
                                (current_idx + 1) % types.len()
                            } else {
                                (current_idx + types.len() - 1) % types.len()
                            };
                            filter.filter_type = types[new_idx];
                        }
                        _ => return false,
                    }
                    return true;
                }
            }
            PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => {
                match index {
                    0 => *threshold_db = (*threshold_db + delta).clamp(-60.0, 0.0),
                    1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 20.0),
                    2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                    3 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                    4 => *knee_db = (*knee_db + delta * 0.1).clamp(0.0, 12.0),
                    5 => *makeup_gain_db = (*makeup_gain_db + delta * 0.1).clamp(-20.0, 20.0),
                    6 => *mix = (*mix + delta * 0.01).clamp(0.0, 1.0),
                    7 => *auto_makeup = !*auto_makeup,
                    8 => *link_channels = !*link_channels,
                    9 => *sidechain_hpf_hz = (*sidechain_hpf_hz + delta).clamp(20.0, 500.0),
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => {
                match index {
                    0 => *threshold_db = (*threshold_db + delta * 0.1).clamp(-20.0, 0.0),
                    1 => *release_ms = (*release_ms + delta).clamp(1.0, 500.0),
                    2 => *lookahead_ms = (*lookahead_ms + delta * 0.1).clamp(0.0, 20.0),
                    3 => *soft = !*soft,
                    4 => *mix = (*mix + delta * 0.05).clamp(0.0, 1.0),
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
                enable_ml_detection,
            } => {
                use sotf_plugins::param_specs::upmixer::*;
                // Indices MUST match get_descriptors() order
                match index {
                    0 => {
                        let configs = [
                            "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                            "9.1.4", "9.1.6",
                        ];
                        let current_idx = configs
                            .iter()
                            .position(|&c| c == speaker_config.as_str())
                            .unwrap_or(2);
                        let new_idx = if delta > 0.0 {
                            (current_idx + 1) % configs.len()
                        } else {
                            (current_idx + configs.len() - 1) % configs.len()
                        };
                        *speaker_config = configs[new_idx].to_string();
                    }
                    // Gains
                    1 => *gain_front_direct = (*gain_front_direct + delta * 0.05).clamp(GAIN_FRONT_DIRECT_MIN as f64, GAIN_FRONT_DIRECT_MAX as f64),
                    2 => *gain_front_ambient = (*gain_front_ambient + delta * 0.05).clamp(GAIN_FRONT_AMBIENT_MIN as f64, GAIN_FRONT_AMBIENT_MAX as f64),
                    3 => *gain_rear_ambient = (*gain_rear_ambient + delta * 0.05).clamp(GAIN_REAR_AMBIENT_MIN as f64, GAIN_REAR_AMBIENT_MAX as f64),
                    4 => *height_gain = (*height_gain + delta * 0.05).clamp(GAIN_HEIGHT_MIN as f64, GAIN_HEIGHT_MAX as f64),
                    // LFE
                    5 => *lfe_gain = (*lfe_gain + delta * 0.05).clamp(LFE_GAIN_MIN as f64, LFE_GAIN_MAX as f64),
                    6 => *lfe_cutoff_hz = (*lfe_cutoff_hz + delta * 5.0).clamp(LFE_CUTOFF_HZ_MIN as f64, LFE_CUTOFF_HZ_MAX as f64),
                    7 => *enable_subharmonic_synth = !*enable_subharmonic_synth,
                    8 => *subharmonic_gain = (*subharmonic_gain + delta * 0.05).clamp(SUBHARMONIC_GAIN_MIN as f64, SUBHARMONIC_GAIN_MAX as f64),
                    9 => *subharmonic_freq_hz = (*subharmonic_freq_hz + delta * 1.0).clamp(SUBHARMONIC_FREQ_HZ_MIN as f64, SUBHARMONIC_FREQ_HZ_MAX as f64),
                    10 => *subharmonic_attack_ms = (*subharmonic_attack_ms + delta * 1.0).clamp(SUBHARMONIC_ATTACK_MS_MIN as f64, SUBHARMONIC_ATTACK_MS_MAX as f64),
                    11 => *subharmonic_release_ms = (*subharmonic_release_ms + delta * 5.0).clamp(SUBHARMONIC_RELEASE_MS_MIN as f64, SUBHARMONIC_RELEASE_MS_MAX as f64),
                    // Spatial
                    12 => *stereo_width = (*stereo_width + delta * 0.05).clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64),
                    13 => *center_spread = (*center_spread + delta * 0.05).clamp(CENTER_SPREAD_MIN as f64, CENTER_SPREAD_MAX as f64),
                    14 => *bandpass_hz = (*bandpass_hz + delta * 5.0).clamp(BANDPASS_HZ_MIN as f64, BANDPASS_HZ_MAX as f64),
                    // Enhancement
                    15 => *enable_hr_direct = !*enable_hr_direct,
                    16 => *hr_sharpen = (*hr_sharpen + delta * 0.05).clamp(HR_SHARPEN_MIN as f64, HR_SHARPEN_MAX as f64),
                    17 => *ambient_boost = (*ambient_boost + delta * 0.05).clamp(AMBIENT_BOOST_MIN as f64, AMBIENT_BOOST_MAX as f64),
                    18 => *decorrelation_mode = (*decorrelation_mode + 1) % 2,
                    19 => *decorrelation_lfo_rate_hz = (*decorrelation_lfo_rate_hz + delta * 0.01).clamp(DECORRELATION_LFO_RATE_HZ_MIN as f64, DECORRELATION_LFO_RATE_HZ_MAX as f64),
                    20 => *velvet_noise_duration_ms = (*velvet_noise_duration_ms + delta * 1.0).clamp(VELVET_NOISE_DURATION_MS_MIN as f64, VELVET_NOISE_DURATION_MS_MAX as f64),
                    21 => *velvet_noise_density = (*velvet_noise_density + delta * 100.0).clamp(VELVET_NOISE_DENSITY_MIN as f64, VELVET_NOISE_DENSITY_MAX as f64),
                    // Height
                    22 => *height_hf_cap_hz = (*height_hf_cap_hz + delta * 100.0).clamp(HEIGHT_HF_CAP_HZ_MIN as f64, HEIGHT_HF_CAP_HZ_MAX as f64),
                    23 => *height_transient_reduction = (*height_transient_reduction + delta * 0.05).clamp(HEIGHT_TRANSIENT_REDUCTION_MIN as f64, HEIGHT_TRANSIENT_REDUCTION_MAX as f64),
                    24 => *height_direct_leak = (*height_direct_leak + delta * 0.01).clamp(HEIGHT_DIRECT_LEAK_MIN as f64, HEIGHT_DIRECT_LEAK_MAX as f64),
                    // Surround
                    25 => *surround_direct_bleed = (*surround_direct_bleed + delta * 0.05).clamp(SURROUND_DIRECT_BLEED_MIN as f64, SURROUND_DIRECT_BLEED_MAX as f64),
                    26 => *rear_ambient_boost = (*rear_ambient_boost + delta * 0.05).clamp(REAR_AMBIENT_BOOST_MIN as f64, REAR_AMBIENT_BOOST_MAX as f64),
                    27 => *rear_late_reflection = (*rear_late_reflection + delta * 0.01).clamp(REAR_LATE_REFLECTION_MIN as f64, REAR_LATE_REFLECTION_MAX as f64),
                    // Dialogue
                    28 => *dialogue_weight = (*dialogue_weight + delta * 0.05).clamp(DIALOGUE_WEIGHT_MIN as f64, DIALOGUE_WEIGHT_MAX as f64),
                    29 => *voice_freq_min_hz = (*voice_freq_min_hz + delta * 10.0).clamp(VOICE_FREQ_MIN_HZ_MIN as f64, VOICE_FREQ_MIN_HZ_MAX as f64),
                    30 => *voice_freq_max_hz = (*voice_freq_max_hz + delta * 50.0).clamp(VOICE_FREQ_MAX_HZ_MIN as f64, VOICE_FREQ_MAX_HZ_MAX as f64),
                    31 => *dialogue_centroid_weight = (*dialogue_centroid_weight + delta * 0.05).clamp(DIALOGUE_CENTROID_WEIGHT_MIN as f64, DIALOGUE_CENTROID_WEIGHT_MAX as f64),
                    32 => *dialogue_variance_weight = (*dialogue_variance_weight + delta * 0.05).clamp(DIALOGUE_VARIANCE_WEIGHT_MIN as f64, DIALOGUE_VARIANCE_WEIGHT_MAX as f64),
                    33 => *dialogue_coherence_weight = (*dialogue_coherence_weight + delta * 0.05).clamp(DIALOGUE_COHERENCE_WEIGHT_MIN as f64, DIALOGUE_COHERENCE_WEIGHT_MAX as f64),
                    // Output
                    34 => *safety_cap_db = (*safety_cap_db + delta * 0.1).clamp(SAFETY_CAP_DB_MIN as f64, SAFETY_CAP_DB_MAX as f64),
                    35 => *bypass_decorrelation = !*bypass_decorrelation,
                    36 => *bypass_transient_detection = !*bypass_transient_detection,
                    37 => *bypass_all_processing = !*bypass_all_processing,
                    38 => *enable_ml_detection = !*enable_ml_detection,
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => {
                match index {
                    0 => *threshold_db = (*threshold_db + delta).clamp(-60.0, 0.0),
                    1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 20.0),
                    2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                    3 => *hold_ms = (*hold_ms + delta).clamp(0.0, 500.0),
                    4 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                    5 => *mix = (*mix + delta * 0.01).clamp(0.0, 1.0),
                    6 => *link_channels = !*link_channels,
                    7 => *sidechain_hpf_hz = (*sidechain_hpf_hz + delta).clamp(20.0, 500.0),
                    _ => return false,
                }
                return true;
            }
            PluginSettings::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => {
                use sotf_plugins::param_specs::loudness_compensation::*;
                match index {
                    0 => *low_freq = (*low_freq + delta * 5.0).clamp(LOW_FREQ_MIN as f64, LOW_FREQ_MAX as f64),
                    1 => *low_gain = (*low_gain + delta * 0.5).clamp(LOW_GAIN_MIN as f64, LOW_GAIN_MAX as f64),
                    2 => *high_freq = (*high_freq + delta * 100.0).clamp(HIGH_FREQ_MIN as f64, HIGH_FREQ_MAX as f64),
                    3 => *high_gain = (*high_gain + delta * 0.5).clamp(HIGH_GAIN_MIN as f64, HIGH_GAIN_MAX as f64),
                    4 => *auto_gain_enabled = !*auto_gain_enabled,
                    5 => *auto_gain_max_db = (*auto_gain_max_db + delta).clamp(0.0, 24.0),
                    6 => *auto_gain_smoothing_ms = (*auto_gain_smoothing_ms + delta * 5.0).clamp(1.0, 1000.0),
                    _ => return false,
                }
                return true;
            }
            PluginSettings::BinauralDecoder {
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
                ..
            } => {
                match index {
                    0 => return false, // SOFA file - not adjustable with delta
                    1 => {
                        *input_channels =
                            (*input_channels as i64 + delta as i64).clamp(2, 16) as usize
                    }
                    2 => *enable_optimization = !*enable_optimization,
                    3 => *externalization = (*externalization + delta * 0.05).clamp(0.0, 1.0),
                    4 => {
                        *near_field_strength = (*near_field_strength + delta * 0.05).clamp(0.0, 1.0)
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Convolution { mix, gain_db, .. } => {
                match index {
                    0 => return false, // IR file - not adjustable with delta
                    1 => *mix = (*mix + delta * 0.05).clamp(0.0, 1.0),
                    2 => *gain_db = (*gain_db + delta * 0.5).clamp(-40.0, 40.0),
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => {
                use sotf_plugins::param_specs::downmix::*;
                match index {
                    0 => {
                        *center_gain_db = (*center_gain_db + delta * 0.5)
                            .clamp(CENTER_GAIN_DB_MIN as f64, CENTER_GAIN_DB_MAX as f64)
                    }
                    1 => {
                        *surround_gain_db = (*surround_gain_db + delta * 0.5)
                            .clamp(SURROUND_GAIN_DB_MIN as f64, SURROUND_GAIN_DB_MAX as f64)
                    }
                    2 => {
                        *height_gain_db = (*height_gain_db + delta * 0.5)
                            .clamp(HEIGHT_GAIN_DB_MIN as f64, HEIGHT_GAIN_DB_MAX as f64)
                    }
                    3 => {
                        *lfe_gain_db = (*lfe_gain_db + delta * 0.5)
                            .clamp(LFE_GAIN_DB_MIN as f64, LFE_GAIN_DB_MAX as f64)
                    }
                    4 => *phase_coherence = !*phase_coherence,
                    5 => {
                        *phase_blend_low_hz = (*phase_blend_low_hz + delta * 10.0)
                            .clamp(PHASE_BLEND_LOW_HZ_MIN as f64, PHASE_BLEND_LOW_HZ_MAX as f64)
                    }
                    6 => {
                        *phase_blend_high_hz = (*phase_blend_high_hz + delta * 10.0).clamp(
                            PHASE_BLEND_HIGH_HZ_MIN as f64,
                            PHASE_BLEND_HIGH_HZ_MAX as f64,
                        )
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => {
                use sotf_plugins::param_specs::mono_to_stereo::*;
                match index {
                    0 => {
                        *stereo_width = (*stereo_width + delta * 0.05)
                            .clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64)
                    }
                    1 => {
                        *haas_delay_ms = (*haas_delay_ms + delta * 0.1)
                            .clamp(HAAS_DELAY_MS_MIN as f64, HAAS_DELAY_MS_MAX as f64)
                    }
                    2 => *enable_comp_eq = !*enable_comp_eq,
                    3 => {
                        *comp_eq_depth_db = (*comp_eq_depth_db + delta * 0.1)
                            .clamp(COMP_EQ_DEPTH_DB_MIN as f64, COMP_EQ_DEPTH_DB_MAX as f64)
                    }
                    4 => {
                        *decor_low_hz = (*decor_low_hz + delta * 10.0)
                            .clamp(DECOR_LOW_HZ_MIN as f64, DECOR_LOW_HZ_MAX as f64)
                    }
                    5 => {
                        *decor_high_hz = (*decor_high_hz + delta * 10.0)
                            .clamp(DECOR_HIGH_HZ_MIN as f64, DECOR_HIGH_HZ_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
                polyphonic_detection,
                crack_sensitivity,
                mcra_alpha_s,
                mcra_alpha_p,
                mcra_l,
                mcra_delta,
                transparency,
                dd_enabled,
                dd_alpha,
                psychoacoustic_masking,
                learn_noise,
                use_captured_profile,
                clear_profile,
            } => {
                use sotf_plugins::param_specs::denoiser::*;
                match index {
                    0 => {
                        *reduction_db = (*reduction_db + delta)
                            .clamp(REDUCTION_DB_MIN as f64, REDUCTION_DB_MAX as f64)
                    }
                    1 => {
                        *floor_db =
                            (*floor_db + delta).clamp(FLOOR_DB_MIN as f64, FLOOR_DB_MAX as f64)
                    }
                    2 => {
                        *smoothing = (*smoothing + delta * 0.01)
                            .clamp(SMOOTHING_MIN as f64, SMOOTHING_MAX as f64)
                    }
                    3 => {
                        *attack_ms = (*attack_ms + delta * 0.1)
                            .clamp(ATTACK_MS_MIN as f64, ATTACK_MS_MAX as f64)
                    }
                    4 => {
                        *release_ms = (*release_ms + delta)
                            .clamp(RELEASE_MS_MIN as f64, RELEASE_MS_MAX as f64)
                    }
                    5 => *low_latency = !*low_latency,
                    6 => *polyphonic_detection = !*polyphonic_detection,
                    7 => *crack_sensitivity = (*crack_sensitivity + delta).clamp(1.0, 100.0),
                    8 => *mcra_alpha_s = (*mcra_alpha_s + delta * 0.01).clamp(0.5, 0.99),
                    9 => *mcra_alpha_p = (*mcra_alpha_p + delta * 0.01).clamp(0.1, 0.99),
                    10 => *mcra_l = ((*mcra_l as i64) + delta as i64).clamp(10, 200) as usize,
                    11 => *mcra_delta = (*mcra_delta + delta * 0.5).clamp(1.0, 20.0),
                    12 => {
                        *transparency = (*transparency + delta * 0.05)
                            .clamp(TRANSPARENCY_MIN as f64, TRANSPARENCY_MAX as f64)
                    }
                    13 => *dd_enabled = !*dd_enabled,
                    14 => {
                        *dd_alpha = (*dd_alpha + delta * 0.01)
                            .clamp(DD_ALPHA_MIN as f64, DD_ALPHA_MAX as f64)
                    }
                    15 => *psychoacoustic_masking = !*psychoacoustic_masking,
                    16 => *learn_noise = !*learn_noise,
                    17 => *use_captured_profile = !*use_captured_profile,
                    18 => *clear_profile = !*clear_profile,
                    _ => return false,
                }
                return true;
            }
            PluginSettings::BandSplit {
                frequency,
                crossover_type,
                ..
            } => {
                use sotf_plugins::param_specs::band_split::*;
                match index {
                    0 => {
                        *frequency = (*frequency + delta * 10.0).clamp(FREQUENCY_MIN, FREQUENCY_MAX)
                    }
                    1 => {
                        *crossover_type = if crossover_type == "LR24" {
                            "LR48".to_string()
                        } else {
                            "LR24".to_string()
                        };
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::BandMerge { bands, .. } => {
                use sotf_plugins::param_specs::band_merge::*;
                match index {
                    0 => {
                        *bands = ((*bands as i64) + delta as i64)
                            .clamp(BANDS_MIN as i64, BANDS_MAX as i64)
                            as usize
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Expander {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => {
                use sotf_plugins::param_specs::expander::*;
                match index {
                    0 => {
                        *threshold_db = (*threshold_db + delta)
                            .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64)
                    }
                    1 => *ratio = (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64),
                    2 => {
                        *attack_ms =
                            (*attack_ms + delta * 0.1).clamp(ATTACK_MIN as f64, ATTACK_MAX as f64)
                    }
                    3 => {
                        *release_ms =
                            (*release_ms + delta).clamp(RELEASE_MIN as f64, RELEASE_MAX as f64)
                    }
                    4 => *range_db = (*range_db + delta).clamp(RANGE_MIN as f64, RANGE_MAX as f64),
                    5 => {
                        *knee_db = (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64)
                    }
                    6 => {
                        *hysteresis_db = (*hysteresis_db + delta * 0.1)
                            .clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64)
                    }
                    7 => *hold_ms = (*hold_ms + delta).clamp(HOLD_MIN as f64, HOLD_MAX as f64),
                    8 => *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    9 => *link_channels = !*link_channels,
                    10 => {
                        *sidechain_hpf_hz = (*sidechain_hpf_hz + delta)
                            .clamp(SIDECHAIN_HPF_HZ_MIN as f64, SIDECHAIN_HPF_HZ_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::MultibandCompressor {
                num_bands,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                mix,
                link_channels,
                bands,
                ..
            } => {
                use sotf_plugins::param_specs::multiband_compressor::*;
                const GLOBAL_COUNT: usize = 12;
                const BAND_PARAMS: usize = 8; // solo, bypass, threshold, ratio, attack, release, knee, makeup
                match index {
                    0 => {
                        let new_bands = ((*num_bands as i64) + delta as i64)
                            .clamp(NUM_BANDS_MIN as i64, NUM_BANDS_MAX as i64)
                            as usize;
                        *num_bands = new_bands;
                        // Resize bands vector to match
                        bands.resize_with(new_bands, Default::default);
                    }
                    1 => {
                        *crossover_freq_1 = (*crossover_freq_1 + delta * 5.0)
                            .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64)
                    }
                    2 => {
                        *crossover_freq_2 = (*crossover_freq_2 + delta * 10.0)
                            .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64)
                    }
                    3 => {
                        *crossover_freq_3 = (*crossover_freq_3 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64)
                    }
                    4 => {
                        *crossover_freq_4 = (*crossover_freq_4 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64)
                    }
                    5 => {
                        *threshold_db = (*threshold_db + delta)
                            .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64)
                    }
                    6 => *ratio = (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64),
                    7 => {
                        *attack_ms =
                            (*attack_ms + delta * 0.1).clamp(ATTACK_MIN as f64, ATTACK_MAX as f64)
                    }
                    8 => {
                        *release_ms =
                            (*release_ms + delta).clamp(RELEASE_MIN as f64, RELEASE_MAX as f64)
                    }
                    9 => {
                        *knee_db = (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64)
                    }
                    10 => *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    11 => *link_channels = !*link_channels,
                    _ => {
                        // Per-band parameters
                        let band_offset = index - GLOBAL_COUNT;
                        let band_idx = band_offset / BAND_PARAMS;
                        let param_in_band = band_offset % BAND_PARAMS;
                        // Ensure bands vec is large enough
                        if band_idx >= bands.len() {
                            bands.resize_with(band_idx + 1, Default::default);
                        }
                        let band = &mut bands[band_idx];
                        match param_in_band {
                            0 => band.solo = !band.solo,
                            1 => band.bypass = !band.bypass,
                            2 => { // threshold: toggle between Global/override
                                band.threshold_db = match band.threshold_db {
                                    None => Some(*threshold_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < THRESHOLD_MIN { None } else { Some(new_v.clamp(THRESHOLD_MIN, THRESHOLD_MAX)) }
                                    }
                                };
                            }
                            3 => { // ratio
                                band.ratio = match band.ratio {
                                    None => Some(*ratio as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < RATIO_MIN { None } else { Some(new_v.clamp(RATIO_MIN, RATIO_MAX)) }
                                    }
                                };
                            }
                            4 => { // attack
                                band.attack_ms = match band.attack_ms {
                                    None => Some(*attack_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < ATTACK_MIN { None } else { Some(new_v.clamp(ATTACK_MIN, ATTACK_MAX)) }
                                    }
                                };
                            }
                            5 => { // release
                                band.release_ms = match band.release_ms {
                                    None => Some(*release_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < RELEASE_MIN { None } else { Some(new_v.clamp(RELEASE_MIN, RELEASE_MAX)) }
                                    }
                                };
                            }
                            6 => { // knee
                                band.knee_db = match band.knee_db {
                                    None => Some(*knee_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < KNEE_MIN { None } else { Some(new_v.clamp(KNEE_MIN, KNEE_MAX)) }
                                    }
                                };
                            }
                            7 => { // makeup gain
                                band.makeup_gain_db = (band.makeup_gain_db + delta as f32 * 0.5).clamp(-24.0, 24.0);
                            }
                            _ => return false,
                        }
                    }
                }
                return true;
            }
            PluginSettings::MultibandExpander {
                num_bands,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                bands,
                ..
            } => {
                use sotf_plugins::param_specs::multiband_expander::*;
                const GLOBAL_COUNT: usize = 15;
                const BAND_PARAMS: usize = 10; // solo, bypass, threshold, ratio, attack, release, range, knee, hysteresis, hold
                match index {
                    0 => {
                        let new_bands = ((*num_bands as i64) + delta as i64)
                            .clamp(NUM_BANDS_MIN as i64, NUM_BANDS_MAX as i64)
                            as usize;
                        *num_bands = new_bands;
                        bands.resize_with(new_bands, Default::default);
                    }
                    1 => {
                        *crossover_freq_1 = (*crossover_freq_1 + delta * 5.0)
                            .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64)
                    }
                    2 => {
                        *crossover_freq_2 = (*crossover_freq_2 + delta * 10.0)
                            .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64)
                    }
                    3 => {
                        *crossover_freq_3 = (*crossover_freq_3 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64)
                    }
                    4 => {
                        *crossover_freq_4 = (*crossover_freq_4 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64)
                    }
                    5 => {
                        *threshold_db = (*threshold_db + delta)
                            .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64)
                    }
                    6 => *ratio = (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64),
                    7 => {
                        *attack_ms =
                            (*attack_ms + delta * 0.1).clamp(ATTACK_MIN as f64, ATTACK_MAX as f64)
                    }
                    8 => {
                        *release_ms =
                            (*release_ms + delta).clamp(RELEASE_MIN as f64, RELEASE_MAX as f64)
                    }
                    9 => *range_db = (*range_db + delta).clamp(RANGE_MIN as f64, RANGE_MAX as f64),
                    10 => {
                        *knee_db = (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64)
                    }
                    11 => {
                        *hysteresis_db = (*hysteresis_db + delta * 0.1)
                            .clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64)
                    }
                    12 => *hold_ms = (*hold_ms + delta).clamp(HOLD_MIN as f64, HOLD_MAX as f64),
                    13 => *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    14 => *link_channels = !*link_channels,
                    _ => {
                        // Per-band parameters
                        let band_offset = index - GLOBAL_COUNT;
                        let band_idx = band_offset / BAND_PARAMS;
                        let param_in_band = band_offset % BAND_PARAMS;
                        if band_idx >= bands.len() {
                            bands.resize_with(band_idx + 1, Default::default);
                        }
                        let band = &mut bands[band_idx];
                        match param_in_band {
                            0 => band.solo = !band.solo,
                            1 => band.bypass = !band.bypass,
                            2 => { // threshold
                                band.threshold_db = match band.threshold_db {
                                    None => Some(*threshold_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < THRESHOLD_MIN { None } else { Some(new_v.clamp(THRESHOLD_MIN, THRESHOLD_MAX)) }
                                    }
                                };
                            }
                            3 => { // ratio
                                band.ratio = match band.ratio {
                                    None => Some(*ratio as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < RATIO_MIN { None } else { Some(new_v.clamp(RATIO_MIN, RATIO_MAX)) }
                                    }
                                };
                            }
                            4 => { // attack
                                band.attack_ms = match band.attack_ms {
                                    None => Some(*attack_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < ATTACK_MIN { None } else { Some(new_v.clamp(ATTACK_MIN, ATTACK_MAX)) }
                                    }
                                };
                            }
                            5 => { // release
                                band.release_ms = match band.release_ms {
                                    None => Some(*release_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < RELEASE_MIN { None } else { Some(new_v.clamp(RELEASE_MIN, RELEASE_MAX)) }
                                    }
                                };
                            }
                            6 => { // range
                                band.range_db = match band.range_db {
                                    None => Some(*range_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < RANGE_MIN { None } else { Some(new_v.clamp(RANGE_MIN, RANGE_MAX)) }
                                    }
                                };
                            }
                            7 => { // knee
                                band.knee_db = match band.knee_db {
                                    None => Some(*knee_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < KNEE_MIN { None } else { Some(new_v.clamp(KNEE_MIN, KNEE_MAX)) }
                                    }
                                };
                            }
                            8 => { // hysteresis
                                band.hysteresis_db = match band.hysteresis_db {
                                    None => Some(*hysteresis_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < HYSTERESIS_MIN { None } else { Some(new_v.clamp(HYSTERESIS_MIN, HYSTERESIS_MAX)) }
                                    }
                                };
                            }
                            9 => { // hold
                                band.hold_ms = match band.hold_ms {
                                    None => Some(*hold_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < HOLD_MIN { None } else { Some(new_v.clamp(HOLD_MIN, HOLD_MAX)) }
                                    }
                                };
                            }
                            _ => return false,
                        }
                    }
                }
                return true;
            }
            PluginSettings::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
                max_gain_db,
                head_offset_x,
                head_offset_z,
                head_yaw_deg,
                head_tracking_smooth_s: _,
                spectral_normalization,
                room_reflections_enabled,
                room_ir_file: _,
                room_width_m,
                room_depth_m,
                wall_absorption,
                reflection_beta_boost,
                bypass_xtc_filters,
                bypass_spectral_normalization,
                bypass_neumann_refinement,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                pinna_model_enabled,
            } => {
                use sotf_plugins::param_specs::xtc::*;
                match index {
                    0 => {
                        *distance_m =
                            (*distance_m + delta * 0.1).clamp(DISTANCE_M_MIN, DISTANCE_M_MAX)
                    }
                    1 => {
                        *speaker_angle_deg = (*speaker_angle_deg + delta)
                            .clamp(SPEAKER_ANGLE_DEG_MIN, SPEAKER_ANGLE_DEG_MAX)
                    }
                    2 => {
                        *head_radius_m = (*head_radius_m + delta * 0.001)
                            .clamp(HEAD_RADIUS_M_MIN, HEAD_RADIUS_M_MAX)
                    }
                    3 => *head_offset_x = (*head_offset_x + delta * 0.01).clamp(-0.5, 0.5),
                    4 => *head_offset_z = (*head_offset_z + delta * 0.01).clamp(-0.5, 0.5),
                    5 => *head_yaw_deg = (*head_yaw_deg + delta).clamp(-90.0, 90.0),
                    6 => {
                        *beta_base =
                            (*beta_base + delta * 0.0001).clamp(BETA_BASE_MIN, BETA_BASE_MAX)
                    }
                    7 => {
                        *beta_low_freq_boost = (*beta_low_freq_boost + delta)
                            .clamp(BETA_LOW_FREQ_BOOST_MIN, BETA_LOW_FREQ_BOOST_MAX)
                    }
                    8 => {
                        *beta_high_freq_boost = (*beta_high_freq_boost + delta)
                            .clamp(BETA_HIGH_FREQ_BOOST_MIN, BETA_HIGH_FREQ_BOOST_MAX)
                    }
                    9 => {
                        *head_shadow_cutoff_hz = (*head_shadow_cutoff_hz + delta * 100.0)
                            .clamp(HEAD_SHADOW_CUTOFF_HZ_MIN, HEAD_SHADOW_CUTOFF_HZ_MAX)
                    }
                    10 => {
                        *head_shadow_slope_db_per_octave =
                            (*head_shadow_slope_db_per_octave + delta * 0.5).clamp(
                                HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MIN,
                                HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MAX,
                            )
                    }
                    11 => {
                        *max_gain_db =
                            (*max_gain_db + delta).clamp(MAX_GAIN_DB_MIN, MAX_GAIN_DB_MAX)
                    }
                    12 => *spectral_normalization = !*spectral_normalization,
                    13 => *pinna_model_enabled = !*pinna_model_enabled,
                    14 => *room_reflections_enabled = !*room_reflections_enabled,
                    15 => *room_width_m = (*room_width_m + delta * 0.1).clamp(2.0, 10.0),
                    16 => *room_depth_m = (*room_depth_m + delta * 0.1).clamp(2.0, 15.0),
                    17 => *wall_absorption = (*wall_absorption + delta * 0.05).clamp(0.0, 1.0),
                    18 => *reflection_beta_boost = (*reflection_beta_boost + delta * 0.1).clamp(1.0, 10.0),
                    19 => *bypass_xtc_filters = !*bypass_xtc_filters,
                    20 => *bypass_spectral_normalization = !*bypass_spectral_normalization,
                    21 => *bypass_neumann_refinement = !*bypass_neumann_refinement,
                    22 => *auto_gain_enabled = !*auto_gain_enabled,
                    23 => {
                        *auto_gain_max_db = (*auto_gain_max_db + delta)
                            .clamp(AUTO_GAIN_MAX_DB_MIN as f64, AUTO_GAIN_MAX_DB_MAX as f64)
                    }
                    24 => {
                        *auto_gain_smoothing_ms = (*auto_gain_smoothing_ms + delta * 5.0)
                            .clamp(AUTO_GAIN_SMOOTHING_MS_MIN as f64, AUTO_GAIN_SMOOTHING_MS_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => {
                use sotf_plugins::param_specs::pnd::*;
                match index {
                    0 => {
                        *correction_strength = (*correction_strength + delta * 0.05).clamp(
                            CORRECTION_STRENGTH_MIN as f64,
                            CORRECTION_STRENGTH_MAX as f64,
                        )
                    }
                    1 => {
                        *analysis_window_ms = (*analysis_window_ms + delta * 5.0)
                            .clamp(ANALYSIS_WINDOW_MS_MIN as f64, ANALYSIS_WINDOW_MS_MAX as f64)
                    }
                    2 => {
                        *drift_smoothing = (*drift_smoothing + delta * 0.01)
                            .clamp(DRIFT_SMOOTHING_MIN as f64, DRIFT_SMOOTHING_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::ABCompare {
                mix,
                mix_mode,
                selected_path,
                bypass,
                auto_gain_enabled,
                loudness_type,
                max_auto_gain_db,
                gain_smoothing_ms,
                mix_transition_ms,
                path_a_config: _,
                path_b_config: _,
            } => {
                use sotf_plugins::param_specs::ab_compare::*;
                match index {
                    0 => *mix = (*mix + delta * 0.05).clamp(MIX_MIN, MIX_MAX),
                    1 => *mix_mode = if *mix_mode == 0 { 1 } else { 0 },
                    2 => *selected_path = if *selected_path == 0 { 1 } else { 0 },
                    3 => *bypass = !*bypass,
                    4 => *auto_gain_enabled = !*auto_gain_enabled,
                    5 => *loudness_type = if *loudness_type == 0 { 1 } else { 0 },
                    6 => {
                        *max_auto_gain_db = (*max_auto_gain_db + delta)
                            .clamp(MAX_AUTO_GAIN_DB_MIN, MAX_AUTO_GAIN_DB_MAX)
                    }
                    7 => {
                        *gain_smoothing_ms = (*gain_smoothing_ms + delta * 5.0)
                            .clamp(GAIN_SMOOTHING_MS_MIN, GAIN_SMOOTHING_MS_MAX)
                    }
                    8 => {
                        *mix_transition_ms = (*mix_transition_ms + delta * 5.0)
                            .clamp(MIX_TRANSITION_MS_MIN, MIX_TRANSITION_MS_MAX)
                    }
                    9 | 10 => return false, // path_a/path_b configs are strings, not adjustable with delta
                    _ => return false,
                }
                return true;
            }
            PluginSettings::SpectrumAnalyzer {
                num_bins,
                min_freq,
                max_freq,
                smoothing,
                tilt_correction,
                tilt_reference,
            } => {
                use sotf_plugins::param_specs::spectrum::*;
                use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};
                match index {
                    0 => {
                        *num_bins = ((*num_bins as i64) + delta as i64)
                            .clamp(NUM_BINS_MIN as i64, NUM_BINS_MAX as i64)
                            as usize
                    }
                    1 => *min_freq = (*min_freq + delta as f32).clamp(MIN_FREQ_MIN, MIN_FREQ_MAX),
                    2 => {
                        *max_freq =
                            (*max_freq + delta as f32 * 100.0).clamp(MAX_FREQ_MIN, MAX_FREQ_MAX)
                    }
                    3 => {
                        *smoothing =
                            (*smoothing + delta as f32 * 0.01).clamp(SMOOTHING_MIN, SMOOTHING_MAX)
                    }
                    4 => {
                        // Cycle through tilt correction modes
                        let modes = [
                            SpectralTiltCorrection::None,
                            SpectralTiltCorrection::ThreeDbPerOctave,
                            SpectralTiltCorrection::SixDbPerOctave,
                            SpectralTiltCorrection::Pink,
                        ];
                        let current = modes.iter().position(|m| m == tilt_correction).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *tilt_correction = modes[next];
                    }
                    5 => {
                        // Cycle through tilt reference modes
                        let modes = [
                            TiltReferenceFreq::Standard,
                            TiltReferenceFreq::OneKilohertz,
                            TiltReferenceFreq::TwoKilohertz,
                            TiltReferenceFreq::MinFreq,
                        ];
                        let current = modes.iter().position(|m| m == tilt_reference).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *tilt_reference = modes[next];
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::LoudnessMonitor => return false,
            PluginSettings::ChannelMuteSolo { enabled, .. } => {
                match index {
                    0 => *enabled = !*enabled,
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } => {
                use sotf_plugins::param_specs::hal::*;
                match index {
                    0 => {
                        *input_channels = ((*input_channels as i64) + delta as i64)
                            .clamp(CHANNELS_MIN as i64, CHANNELS_MAX as i64)
                            as usize
                    }
                    1 => {
                        *output_channels = ((*output_channels as i64) + delta as i64)
                            .clamp(CHANNELS_MIN as i64, CHANNELS_MAX as i64)
                            as usize
                    }
                    _ => return false,
                }
                return true;
            }
            PluginSettings::FletcherMunson {
                playback_volume_db: _,
                reference_level_db,
                enabled,
                band1_freq,
                band1_q,
                band1_max_gain,
                band1_slope,
                band2_freq,
                band2_q,
                band2_max_gain,
                band2_slope,
                band3_freq,
                band3_q,
                band3_max_gain,
                band3_slope,
                band4_freq,
                band4_q,
                band4_max_gain,
                band4_slope,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                ..
            } => {
                use sotf_plugins::param_specs::fletcher_munson::*;
                match index {
                    0 => {
                        *reference_level_db = (*reference_level_db + delta * 0.5)
                            .clamp(REFERENCE_LEVEL_DB_MIN as f64, REFERENCE_LEVEL_DB_MAX as f64)
                    }
                    1 => *enabled = !*enabled,
                    2 => *smoothing_ms = (*smoothing_ms + delta).clamp(SMOOTHING_MS_MIN as f64, SMOOTHING_MS_MAX as f64),
                    3 => *auto_gain_enabled = !*auto_gain_enabled,
                    4 => {
                        *auto_gain_max_db = (*auto_gain_max_db + delta)
                            .clamp(AUTO_GAIN_MAX_DB_MIN as f64, AUTO_GAIN_MAX_DB_MAX as f64)
                    }
                    5 => *auto_gain_smoothing_ms = (*auto_gain_smoothing_ms + delta * 5.0).clamp(AUTO_GAIN_SMOOTHING_MS_MIN as f64, AUTO_GAIN_SMOOTHING_MS_MAX as f64),
                    // Band 1
                    6 => *band1_freq = (*band1_freq + delta * 5.0).clamp(BAND_FREQ_MIN, BAND_FREQ_MAX),
                    7 => *band1_q = (*band1_q + delta * 0.05).clamp(BAND_Q_MIN, BAND_Q_MAX),
                    8 => *band1_max_gain = (*band1_max_gain + delta * 0.5).clamp(BAND_MAX_GAIN_MIN, BAND_MAX_GAIN_MAX),
                    9 => *band1_slope = (*band1_slope + delta * 0.01).clamp(BAND_SLOPE_MIN, BAND_SLOPE_MAX),
                    // Band 2
                    10 => *band2_freq = (*band2_freq + delta * 10.0).clamp(BAND_FREQ_MIN, BAND_FREQ_MAX),
                    11 => *band2_q = (*band2_q + delta * 0.05).clamp(BAND_Q_MIN, BAND_Q_MAX),
                    12 => *band2_max_gain = (*band2_max_gain + delta * 0.5).clamp(BAND_MAX_GAIN_MIN, BAND_MAX_GAIN_MAX),
                    13 => *band2_slope = (*band2_slope + delta * 0.01).clamp(BAND_SLOPE_MIN, BAND_SLOPE_MAX),
                    // Band 3
                    14 => *band3_freq = (*band3_freq + delta * 50.0).clamp(BAND_FREQ_MIN, BAND_FREQ_MAX),
                    15 => *band3_q = (*band3_q + delta * 0.05).clamp(BAND_Q_MIN, BAND_Q_MAX),
                    16 => *band3_max_gain = (*band3_max_gain + delta * 0.5).clamp(BAND_MAX_GAIN_MIN, BAND_MAX_GAIN_MAX),
                    17 => *band3_slope = (*band3_slope + delta * 0.01).clamp(BAND_SLOPE_MIN, BAND_SLOPE_MAX),
                    // Band 4
                    18 => *band4_freq = (*band4_freq + delta * 100.0).clamp(BAND_FREQ_MIN, BAND_FREQ_MAX),
                    19 => *band4_q = (*band4_q + delta * 0.05).clamp(BAND_Q_MIN, BAND_Q_MAX),
                    20 => *band4_max_gain = (*band4_max_gain + delta * 0.5).clamp(BAND_MAX_GAIN_MIN, BAND_MAX_GAIN_MAX),
                    21 => *band4_slope = (*band4_slope + delta * 0.01).clamp(BAND_SLOPE_MIN, BAND_SLOPE_MAX),
                    _ => return false,
                }
                return true;
            }
            PluginSettings::Crossfeed {
                mode,
                preset,
                enabled,
                mix,
                bauer_fcut_hz,
                bauer_feed_db,
                meier_level,
                mb_low_freq_hz,
                mb_mid_high_freq_hz,
                mb_low_feed_db,
                mb_mid_feed_db,
                mb_high_feed_db,
                autogain_enabled,
                autogain_target_lufs,
                autogain_max_gain_db,
                autogain_smoothing_ms,
            } => {
                use sotf_plugins::param_specs::crossfeed::*;
                use sotf_plugins::{CrossfeedMode, CrossfeedPreset};
                match index {
                    0 => {
                        let modes = [
                            CrossfeedMode::Off,
                            CrossfeedMode::Bauer,
                            CrossfeedMode::Meier,
                            CrossfeedMode::Mb,
                        ];
                        let current = modes.iter().position(|m| m == mode).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *mode = modes[next];
                    }
                    1 => {
                        let presets = [
                            CrossfeedPreset::Default,
                            CrossfeedPreset::Cmoy,
                            CrossfeedPreset::Meier,
                            CrossfeedPreset::Mb,
                            CrossfeedPreset::Off,
                        ];
                        let current = presets.iter().position(|p| p == preset).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % presets.len()
                        } else {
                            (current + presets.len() - 1) % presets.len()
                        };
                        *preset = presets[next];
                        // Apply preset logic if not custom-tweaked immediately after
                        let p_params = sotf_plugins::CrossfeedPluginParams::from_preset(*preset);
                        *mode = p_params.mode;
                        *bauer_fcut_hz = p_params.bauer_fcut_hz as f64;
                        *bauer_feed_db = p_params.bauer_feed_db as f64;
                        *meier_level = p_params.meier_level as f64;
                        *mb_low_freq_hz = p_params.mb_low_freq_hz as f64;
                        *mb_mid_high_freq_hz = p_params.mb_mid_high_freq_hz as f64;
                        *mb_low_feed_db = p_params.mb_low_feed_db as f64;
                        *mb_mid_feed_db = p_params.mb_mid_feed_db as f64;
                        *mb_high_feed_db = p_params.mb_high_feed_db as f64;
                    }
                    2 => *enabled = !*enabled,
                    3 => *mix = (*mix + delta * 0.05).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    4 => {
                        *bauer_fcut_hz = (*bauer_fcut_hz + delta * 10.0)
                            .clamp(BAUER_FCUT_MIN as f64, BAUER_FCUT_MAX as f64)
                    }
                    5 => {
                        *bauer_feed_db = (*bauer_feed_db + delta * 0.5)
                            .clamp(BAUER_FEED_MIN as f64, BAUER_FEED_MAX as f64)
                    }
                    6 => {
                        *meier_level = (*meier_level + delta)
                            .clamp(MEIER_LEVEL_MIN as f64, MEIER_LEVEL_MAX as f64)
                    }
                    7 => {
                        *mb_low_freq_hz = (*mb_low_freq_hz + delta * 5.0)
                            .clamp(MB_LOW_FREQ_MIN as f64, MB_LOW_FREQ_MAX as f64)
                    }
                    8 => {
                        *mb_mid_high_freq_hz = (*mb_mid_high_freq_hz + delta * 50.0)
                            .clamp(MB_MID_HIGH_FREQ_MIN as f64, MB_MID_HIGH_FREQ_MAX as f64)
                    }
                    9 => {
                        *mb_low_feed_db = (*mb_low_feed_db + delta * 0.5)
                            .clamp(MB_LOW_FEED_MIN as f64, MB_LOW_FEED_MAX as f64)
                    }
                    10 => {
                        *mb_mid_feed_db = (*mb_mid_feed_db + delta * 0.5)
                            .clamp(MB_MID_FEED_MIN as f64, MB_MID_FEED_MAX as f64)
                    }
                    11 => {
                        *mb_high_feed_db = (*mb_high_feed_db + delta * 0.5)
                            .clamp(MB_HIGH_FEED_MIN as f64, MB_HIGH_FEED_MAX as f64)
                    }
                    12 => *autogain_enabled = !*autogain_enabled,
                    13 => {
                        *autogain_target_lufs = (*autogain_target_lufs + delta * 0.5)
                            .clamp(AUTOGAIN_TARGET_MIN as f64, AUTOGAIN_TARGET_MAX as f64)
                    }
                    14 => {
                        *autogain_max_gain_db = (*autogain_max_gain_db + delta)
                            .clamp(AUTOGAIN_MAX_GAIN_MIN as f64, AUTOGAIN_MAX_GAIN_MAX as f64)
                    }
                    15 => {
                        *autogain_smoothing_ms = (*autogain_smoothing_ms + delta * 10.0).clamp(
                            AUTOGAIN_SMOOTHING_MIN as f64,
                            AUTOGAIN_SMOOTHING_MAX as f64,
                        )
                    }
                    _ => return false,
                }
                return true;
            }
        }
        false
    }

    fn get_choice_labels(&self, index: usize) -> Vec<String> {
        match self {
            PluginSettings::EQ { filters, .. } => {
                if index == 0 {
                    return Vec::new();
                }
                let filter_offset = index - 1;
                let param_idx = filter_offset % 4;
                if param_idx == 3 && (filter_offset / 4) < filters.len() {
                    return vec![
                        "Peak".into(),
                        "Lowshelf".into(),
                        "Highshelf".into(),
                        "Lowpass".into(),
                        "Highpass".into(),
                        "Bandpass".into(),
                        "Notch".into(),
                    ];
                }
                Vec::new()
            }
            PluginSettings::Upmixer { .. } => match index {
                0 => vec![
                    "2.0".into(), "5.0".into(), "5.1".into(), "7.1".into(),
                    "5.1.2".into(), "5.1.4".into(), "7.1.2".into(), "7.1.4".into(),
                    "9.1.4".into(), "9.1.6".into(),
                ],
                18 => vec!["Velvet Noise".into(), "LFO Phase".into()],
                _ => Vec::new(),
            },
            PluginSettings::ABCompare { .. } => match index {
                1 => vec!["Pot".into(), "Binary".into()],
                2 => vec!["A".into(), "B".into()],
                5 => vec!["Momentary".into(), "ShortTerm".into()],
                _ => Vec::new(),
            },
            PluginSettings::BandSplit { .. } => match index {
                1 => vec!["LR24".into(), "LR48".into()],
                _ => Vec::new(),
            },
            PluginSettings::Crossfeed { .. } => match index {
                0 => vec!["Off".into(), "Bauer".into(), "Meier".into(), "Mb".into()],
                1 => vec!["Default".into(), "Cmoy".into(), "Meier".into(), "Mb".into(), "Off".into()],
                _ => Vec::new(),
            },
            PluginSettings::SpectrumAnalyzer { .. } => match index {
                4 => vec!["None".into(), "ThreeDbPerOctave".into(), "SixDbPerOctave".into(), "Pink".into()],
                5 => vec!["Standard".into(), "OneKilohertz".into(), "TwoKilohertz".into(), "MinFreq".into()],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}
