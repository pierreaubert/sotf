//! Upmixer plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Choice label constants
//! - Serializable state (Params struct with serde defaults)
//! - Index↔field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

mod consts;
mod d;
#[cfg(test)]
mod tests;

pub use consts::*;

use d::d_ambient_boost;
use d::d_auto_gain_enabled;
use d::d_auto_gain_max_db;
use d::d_auto_gain_smoothing_ms;
use d::d_bandpass_hz;
use d::d_bypass_all_processing;
use d::d_bypass_decorrelation;
use d::d_bypass_transient_detection;
use d::d_center_spread;
use d::d_decorrelation_lfo_rate_hz;
use d::d_decorrelation_mode;
use d::d_dialogue_centroid_weight;
use d::d_dialogue_coherence_weight;
use d::d_dialogue_variance_weight;
use d::d_dialogue_weight;
use d::d_enable_hr_direct;
use d::d_enable_ml_detection;
use d::d_enable_subharmonic_synth;
use d::d_frequency_resolution;
use d::d_gain_front_ambient;
use d::d_gain_front_direct;
use d::d_gain_rear_ambient;
use d::d_height_direct_leak;
use d::d_height_gain;
use d::d_height_hf_cap_hz;
use d::d_height_transient_reduction;
use d::d_hr_sharpen;
use d::d_lfe_cutoff_hz;
use d::d_lfe_gain;
use d::d_low_latency;
use d::d_multi_source_extraction;
use d::d_multi_source_threshold;
use d::d_rear_ambient_boost;
use d::d_rear_late_reflection;
use d::d_safety_cap_db;
use d::d_speaker_config;
use d::d_stereo_width;
use d::d_subharmonic_attack_ms;
use d::d_subharmonic_freq_hz;
use d::d_subharmonic_gain;
use d::d_subharmonic_release_ms;
use d::d_surround_direct_bleed;
use d::d_velvet_noise_density;
use d::d_velvet_noise_duration_ms;
use d::d_voice_freq_max_hz;
use d::d_voice_freq_min_hz;

/// Upmixer plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_speaker_config")]
    pub speaker_config: usize,
    #[serde(default = "d_gain_front_direct")]
    pub gain_front_direct: f64,
    #[serde(default = "d_gain_front_ambient")]
    pub gain_front_ambient: f64,
    #[serde(default = "d_gain_rear_ambient")]
    pub gain_rear_ambient: f64,
    #[serde(default = "d_height_gain")]
    pub height_gain: f64,
    #[serde(default = "d_lfe_gain")]
    pub lfe_gain: f64,
    #[serde(default = "d_lfe_cutoff_hz")]
    pub lfe_cutoff_hz: f64,
    #[serde(default = "d_enable_subharmonic_synth")]
    pub enable_subharmonic_synth: bool,
    #[serde(default = "d_subharmonic_gain")]
    pub subharmonic_gain: f64,
    #[serde(default = "d_subharmonic_freq_hz")]
    pub subharmonic_freq_hz: f64,
    #[serde(default = "d_subharmonic_attack_ms")]
    pub subharmonic_attack_ms: f64,
    #[serde(default = "d_subharmonic_release_ms")]
    pub subharmonic_release_ms: f64,
    #[serde(default = "d_stereo_width")]
    pub stereo_width: f64,
    #[serde(default = "d_center_spread")]
    pub center_spread: f64,
    #[serde(default = "d_bandpass_hz")]
    pub bandpass_hz: f64,
    #[serde(default = "d_enable_hr_direct")]
    pub enable_hr_direct: bool,
    #[serde(default = "d_hr_sharpen")]
    pub hr_sharpen: f64,
    #[serde(default = "d_ambient_boost")]
    pub ambient_boost: f64,
    #[serde(default = "d_decorrelation_mode")]
    pub decorrelation_mode: usize,
    #[serde(default = "d_decorrelation_lfo_rate_hz")]
    pub decorrelation_lfo_rate_hz: f64,
    #[serde(default = "d_velvet_noise_duration_ms")]
    pub velvet_noise_duration_ms: f64,
    #[serde(default = "d_velvet_noise_density")]
    pub velvet_noise_density: f64,
    #[serde(default = "d_height_hf_cap_hz")]
    pub height_hf_cap_hz: f64,
    #[serde(default = "d_height_transient_reduction")]
    pub height_transient_reduction: f64,
    #[serde(default = "d_height_direct_leak")]
    pub height_direct_leak: f64,
    #[serde(default = "d_surround_direct_bleed")]
    pub surround_direct_bleed: f64,
    #[serde(default = "d_rear_ambient_boost")]
    pub rear_ambient_boost: f64,
    #[serde(default = "d_rear_late_reflection")]
    pub rear_late_reflection: f64,
    #[serde(default = "d_dialogue_weight")]
    pub dialogue_weight: f64,
    #[serde(default = "d_voice_freq_min_hz")]
    pub voice_freq_min_hz: f64,
    #[serde(default = "d_voice_freq_max_hz")]
    pub voice_freq_max_hz: f64,
    #[serde(default = "d_dialogue_centroid_weight")]
    pub dialogue_centroid_weight: f64,
    #[serde(default = "d_dialogue_variance_weight")]
    pub dialogue_variance_weight: f64,
    #[serde(default = "d_dialogue_coherence_weight")]
    pub dialogue_coherence_weight: f64,
    #[serde(default = "d_safety_cap_db")]
    pub safety_cap_db: f64,
    #[serde(default = "d_low_latency")]
    pub low_latency: bool,
    #[serde(default = "d_frequency_resolution")]
    pub frequency_resolution: usize,
    #[serde(default = "d_bypass_decorrelation")]
    pub bypass_decorrelation: bool,
    #[serde(default = "d_bypass_transient_detection")]
    pub bypass_transient_detection: bool,
    #[serde(default = "d_bypass_all_processing")]
    pub bypass_all_processing: bool,
    #[serde(default = "d_enable_ml_detection")]
    pub enable_ml_detection: bool,
    #[serde(default = "d_multi_source_extraction")]
    pub multi_source_extraction: bool,
    #[serde(default = "d_multi_source_threshold")]
    pub multi_source_threshold: f64,
    #[serde(default)]
    pub binaural_preview: bool,
    #[serde(default = "d_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "d_auto_gain_max_db")]
    pub auto_gain_max_db: f64,
    #[serde(default = "d_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            speaker_config: d_speaker_config(),
            gain_front_direct: d_gain_front_direct(),
            gain_front_ambient: d_gain_front_ambient(),
            gain_rear_ambient: d_gain_rear_ambient(),
            height_gain: d_height_gain(),
            lfe_gain: d_lfe_gain(),
            lfe_cutoff_hz: d_lfe_cutoff_hz(),
            enable_subharmonic_synth: d_enable_subharmonic_synth(),
            subharmonic_gain: d_subharmonic_gain(),
            subharmonic_freq_hz: d_subharmonic_freq_hz(),
            subharmonic_attack_ms: d_subharmonic_attack_ms(),
            subharmonic_release_ms: d_subharmonic_release_ms(),
            stereo_width: d_stereo_width(),
            center_spread: d_center_spread(),
            bandpass_hz: d_bandpass_hz(),
            enable_hr_direct: d_enable_hr_direct(),
            hr_sharpen: d_hr_sharpen(),
            ambient_boost: d_ambient_boost(),
            decorrelation_mode: d_decorrelation_mode(),
            decorrelation_lfo_rate_hz: d_decorrelation_lfo_rate_hz(),
            velvet_noise_duration_ms: d_velvet_noise_duration_ms(),
            velvet_noise_density: d_velvet_noise_density(),
            height_hf_cap_hz: d_height_hf_cap_hz(),
            height_transient_reduction: d_height_transient_reduction(),
            height_direct_leak: d_height_direct_leak(),
            surround_direct_bleed: d_surround_direct_bleed(),
            rear_ambient_boost: d_rear_ambient_boost(),
            rear_late_reflection: d_rear_late_reflection(),
            dialogue_weight: d_dialogue_weight(),
            voice_freq_min_hz: d_voice_freq_min_hz(),
            voice_freq_max_hz: d_voice_freq_max_hz(),
            dialogue_centroid_weight: d_dialogue_centroid_weight(),
            dialogue_variance_weight: d_dialogue_variance_weight(),
            dialogue_coherence_weight: d_dialogue_coherence_weight(),
            safety_cap_db: d_safety_cap_db(),
            low_latency: d_low_latency(),
            frequency_resolution: d_frequency_resolution(),
            bypass_decorrelation: d_bypass_decorrelation(),
            bypass_transient_detection: d_bypass_transient_detection(),
            bypass_all_processing: d_bypass_all_processing(),
            enable_ml_detection: d_enable_ml_detection(),
            multi_source_extraction: d_multi_source_extraction(),
            multi_source_threshold: d_multi_source_threshold(),
            binaural_preview: false,
            auto_gain_enabled: d_auto_gain_enabled(),
            auto_gain_max_db: d_auto_gain_max_db(),
            auto_gain_smoothing_ms: d_auto_gain_smoothing_ms(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "upmixer";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.speaker_config as f64),
            1 => Some(self.gain_front_direct),
            2 => Some(self.gain_front_ambient),
            3 => Some(self.gain_rear_ambient),
            4 => Some(self.height_gain),
            5 => Some(self.lfe_gain),
            6 => Some(self.lfe_cutoff_hz),
            7 => Some(if self.enable_subharmonic_synth {
                1.0
            } else {
                0.0
            }),
            8 => Some(self.subharmonic_gain),
            9 => Some(self.subharmonic_freq_hz),
            10 => Some(self.subharmonic_attack_ms),
            11 => Some(self.subharmonic_release_ms),
            12 => Some(self.stereo_width),
            13 => Some(self.center_spread),
            14 => Some(self.bandpass_hz),
            15 => Some(if self.enable_hr_direct { 1.0 } else { 0.0 }),
            16 => Some(self.hr_sharpen),
            17 => Some(self.ambient_boost),
            18 => Some(self.decorrelation_mode as f64),
            19 => Some(self.decorrelation_lfo_rate_hz),
            20 => Some(self.velvet_noise_duration_ms),
            21 => Some(self.velvet_noise_density),
            22 => Some(self.height_hf_cap_hz),
            23 => Some(self.height_transient_reduction),
            24 => Some(self.height_direct_leak),
            25 => Some(self.surround_direct_bleed),
            26 => Some(self.rear_ambient_boost),
            27 => Some(self.rear_late_reflection),
            28 => Some(self.dialogue_weight),
            29 => Some(self.voice_freq_min_hz),
            30 => Some(self.voice_freq_max_hz),
            31 => Some(self.dialogue_centroid_weight),
            32 => Some(self.dialogue_variance_weight),
            33 => Some(self.dialogue_coherence_weight),
            34 => Some(self.safety_cap_db),
            35 => Some(if self.low_latency { 1.0 } else { 0.0 }),
            36 => Some(self.frequency_resolution as f64),
            37 => Some(if self.bypass_decorrelation { 1.0 } else { 0.0 }),
            38 => Some(if self.bypass_transient_detection {
                1.0
            } else {
                0.0
            }),
            39 => Some(if self.bypass_all_processing { 1.0 } else { 0.0 }),
            40 => Some(if self.enable_ml_detection { 1.0 } else { 0.0 }),
            41 => Some(if self.multi_source_extraction {
                1.0
            } else {
                0.0
            }),
            42 => Some(self.multi_source_threshold),
            43 => Some(if self.binaural_preview { 1.0 } else { 0.0 }),
            44 => Some(if self.auto_gain_enabled { 1.0 } else { 0.0 }),
            45 => Some(self.auto_gain_max_db),
            46 => Some(self.auto_gain_smoothing_ms),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.speaker_config = value as usize,
            1 => self.gain_front_direct = value,
            2 => self.gain_front_ambient = value,
            3 => self.gain_rear_ambient = value,
            4 => self.height_gain = value,
            5 => self.lfe_gain = value,
            6 => self.lfe_cutoff_hz = value,
            7 => self.enable_subharmonic_synth = value > 0.5,
            8 => self.subharmonic_gain = value,
            9 => self.subharmonic_freq_hz = value,
            10 => self.subharmonic_attack_ms = value,
            11 => self.subharmonic_release_ms = value,
            12 => self.stereo_width = value,
            13 => self.center_spread = value,
            14 => self.bandpass_hz = value,
            15 => self.enable_hr_direct = value > 0.5,
            16 => self.hr_sharpen = value,
            17 => self.ambient_boost = value,
            18 => self.decorrelation_mode = value as usize,
            19 => self.decorrelation_lfo_rate_hz = value,
            20 => self.velvet_noise_duration_ms = value,
            21 => self.velvet_noise_density = value,
            22 => self.height_hf_cap_hz = value,
            23 => self.height_transient_reduction = value,
            24 => self.height_direct_leak = value,
            25 => self.surround_direct_bleed = value,
            26 => self.rear_ambient_boost = value,
            27 => self.rear_late_reflection = value,
            28 => self.dialogue_weight = value,
            29 => self.voice_freq_min_hz = value,
            30 => self.voice_freq_max_hz = value,
            31 => self.dialogue_centroid_weight = value,
            32 => self.dialogue_variance_weight = value,
            33 => self.dialogue_coherence_weight = value,
            34 => self.safety_cap_db = value,
            35 => self.low_latency = value > 0.5,
            36 => self.frequency_resolution = value as usize,
            37 => self.bypass_decorrelation = value > 0.5,
            38 => self.bypass_transient_detection = value > 0.5,
            39 => self.bypass_all_processing = value > 0.5,
            40 => self.enable_ml_detection = value > 0.5,
            41 => self.multi_source_extraction = value > 0.5,
            42 => self.multi_source_threshold = value,
            43 => self.binaural_preview = value > 0.5,
            44 => self.auto_gain_enabled = value > 0.5,
            45 => self.auto_gain_max_db = value,
            46 => self.auto_gain_smoothing_ms = value,
            _ => {}
        }
    }
}
