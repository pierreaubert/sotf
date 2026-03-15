//! Generic parameter accessor methods for `PluginSettings`.
//!
//! These methods map between parameter indices (as defined by the `PARAMS` arrays
//! in `param_specs`) and the actual fields of each `PluginSettings` variant.
//! Consumer code (TUI, GPUI, CLI) uses these instead of hardcoding per-plugin
//! field access.
//!
//! The `impl_param_accessors!` macro generates `param_specs()`, `layout()`,
//! `param_value()`, and `set_param_value()` from a single field declaration
//! per plugin, eliminating index ordering mismatches between getter and setter.

use crate::plugins::PluginSettings;
use sotf_plugins::param_specs::{self, ParamSpec};
use sotf_plugins::plugin_layout::PluginLayout;
use sotf_plugins::{CrossfeedMode, CrossfeedPreset};

// ============================================================================
// Enum/String <-> index helpers
// ============================================================================

const SPEAKER_CONFIGS: &[&str] = &[
    "2.0", "5.0", "5.1", "7.0", "7.1", "7.1.2", "7.1.4", "9.1", "9.1.4", "9.1.6",
];

fn speaker_config_to_index(config: &str) -> f64 {
    SPEAKER_CONFIGS
        .iter()
        .position(|&c| c == config)
        .unwrap_or(2) as f64 // default to 5.1
}

fn index_to_speaker_config(index: f64) -> String {
    let idx = index as usize;
    SPEAKER_CONFIGS.get(idx).unwrap_or(&"5.1").to_string()
}

fn crossfeed_mode_to_index(mode: &CrossfeedMode) -> f64 {
    match mode {
        CrossfeedMode::Off => 0.0,
        CrossfeedMode::Bauer => 1.0,
        CrossfeedMode::Meier => 2.0,
        CrossfeedMode::Mb => 3.0,
    }
}

fn index_to_crossfeed_mode(index: f64) -> CrossfeedMode {
    match index as usize {
        0 => CrossfeedMode::Off,
        1 => CrossfeedMode::Bauer,
        2 => CrossfeedMode::Meier,
        3 => CrossfeedMode::Mb,
        _ => CrossfeedMode::Off,
    }
}

fn crossfeed_preset_to_index(preset: &CrossfeedPreset) -> f64 {
    match preset {
        CrossfeedPreset::Default => 0.0,
        CrossfeedPreset::Cmoy => 1.0,
        CrossfeedPreset::Meier => 2.0,
        CrossfeedPreset::Mb => 3.0,
        CrossfeedPreset::Off => 4.0,
    }
}

fn index_to_crossfeed_preset(index: f64) -> CrossfeedPreset {
    match index as usize {
        0 => CrossfeedPreset::Default,
        1 => CrossfeedPreset::Cmoy,
        2 => CrossfeedPreset::Meier,
        3 => CrossfeedPreset::Mb,
        4 => CrossfeedPreset::Off,
        _ => CrossfeedPreset::Default,
    }
}

const CROSSOVER_TYPES: &[&str] = &["LR24", "LR48"];

fn crossover_type_to_index(ct: &str) -> f64 {
    CROSSOVER_TYPES.iter().position(|&c| c == ct).unwrap_or(0) as f64
}

fn index_to_crossover_type(index: f64) -> String {
    let idx = index as usize;
    CROSSOVER_TYPES.get(idx).unwrap_or(&"LR24").to_string()
}

// ============================================================================
// Bool <-> f64 helpers
// ============================================================================

#[inline]
fn b2f(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

#[inline]
fn f2b(f: f64) -> bool {
    f > 0.5
}

// ============================================================================
// Type conversion macros (field <-> f64)
// ============================================================================

macro_rules! field_to_f64 {
    ($field:ident, f64)   => { Some(*$field) };
    ($field:ident, bool)  => { Some(b2f(*$field)) };
    ($field:ident, usize) => { Some(*$field as f64) };
    ($field:ident, i32)   => { Some(*$field as f64) };
    ($field:ident, skip)  => { None };
    ($field:ident, [enum $to:path, $from:path]) => { Some($to($field)) };
    ($field:ident, [str $to:path, $from:path])  => { Some($to($field)) };
}

macro_rules! f64_to_field {
    ($field:ident, $val:ident, f64)   => { *$field = $val };
    ($field:ident, $val:ident, bool)  => { *$field = f2b($val) };
    ($field:ident, $val:ident, usize) => { *$field = $val as usize };
    ($field:ident, $val:ident, i32)   => { *$field = $val as i32 };
    ($field:ident, $val:ident, skip)  => { };
    ($field:ident, $val:ident, [enum $to:path, $from:path]) => { *$field = $from($val) };
    ($field:ident, $val:ident, [str $to:path, $from:path])  => { *$field = $from($val) };
}

// ============================================================================
// Main macro: generates param_specs(), layout(), param_value(), set_param_value()
// ============================================================================

macro_rules! impl_param_accessors {
    (
        $(
            $Variant:ident {
                params: $params:expr,
                layout: $layout_val:expr,
                fields: [$($field:ident : $ty:tt),* $(,)?]
            }
        ),+
        $(,)?
        ;
        no_params_unit: [$($NoParamUnit:ident),* $(,)?];
        no_params_struct: [$($NoParamStruct:ident),* $(,)?]
    ) => {
        impl PluginSettings {
            /// Return the static `PARAMS` array for this plugin variant.
            ///
            /// For dynamic-param plugins (EQ, MultibandCompressor, MultibandExpander),
            /// returns only the global params. Per-band params use the `BAND_TEMPLATE`
            /// arrays and are handled separately by consumer code.
            ///
            /// Returns an empty slice for plugins without user-editable params
            /// (LoudnessMonitor, SpectrumAnalyzer, ChannelMuteSolo, Matrix).
            pub fn param_specs(&self) -> &'static [ParamSpec] {
                match self {
                    $(Self::$Variant { .. } => $params,)+
                    $(Self::$NoParamUnit => &[],)*
                    $(Self::$NoParamStruct { .. } => &[],)*
                }
            }

            /// Return the declarative `PluginLayout` for this variant, if one is defined.
            ///
            /// Returns `None` for plugins that keep custom renderers (EQ, SpectrumAnalyzer,
            /// Matrix, ChannelMuteSolo, LoudnessMonitor) or dynamic-param plugins that
            /// require band selection UI (MultibandCompressor, MultibandExpander).
            pub fn layout(&self) -> Option<&'static PluginLayout> {
                match self {
                    $(Self::$Variant { .. } => $layout_val,)+
                    $(Self::$NoParamUnit => None,)*
                    $(Self::$NoParamStruct { .. } => None,)*
                }
            }

            /// Read the current value of parameter at `index` as f64.
            ///
            /// Returns `None` if the index is out of range or the parameter is a FilePath type.
            /// Bool parameters are returned as 1.0 (true) or 0.0 (false).
            /// Choice parameters are returned as their numeric index.
            #[allow(unused_variables)]
            pub fn param_value(&self, index: usize) -> Option<f64> {
                match self {
                    $(
                        Self::$Variant { $($field,)* .. } => {
                            impl_param_accessors!(@get index; 0usize; $($field : $ty,)*)
                        }
                    )+
                    $(Self::$NoParamUnit => None,)*
                    $(Self::$NoParamStruct { .. } => None,)*
                }
            }

            /// Set the value of parameter at `index` from an f64 value.
            ///
            /// Does nothing for FilePath params, out-of-range indices, or non-editable plugins.
            /// Bool parameters: values > 0.5 are treated as true.
            /// Choice parameters: value is cast to the appropriate integer/enum type.
            #[allow(unused_variables)]
            pub fn set_param_value(&mut self, index: usize, value: f64) {
                match self {
                    $(
                        Self::$Variant { $($field,)* .. } => {
                            impl_param_accessors!(@set index, value; 0usize; $($field : $ty,)*)
                        }
                    )+
                    $(Self::$NoParamUnit => {},)*
                    $(Self::$NoParamStruct { .. } => {},)*
                }
            }
        }
    };

    // --- Recursive get: generates if/else chain returning Option<f64> ---
    (@get $idx:ident; $n:expr; ) => { None };
    (@get $idx:ident; $n:expr; $field:ident : $ty:tt, $($rest:ident : $rest_ty:tt,)*) => {
        if $idx == $n {
            field_to_f64!($field, $ty)
        } else {
            impl_param_accessors!(@get $idx; $n + 1usize; $($rest : $rest_ty,)*)
        }
    };

    // --- Recursive set: generates if/else chain returning () ---
    (@set $idx:ident, $val:ident; $n:expr; ) => { () };
    (@set $idx:ident, $val:ident; $n:expr; $field:ident : $ty:tt, $($rest:ident : $rest_ty:tt,)*) => {
        if $idx == $n {
            f64_to_field!($field, $val, $ty);
        } else {
            impl_param_accessors!(@set $idx, $val; $n + 1usize; $($rest : $rest_ty,)*)
        }
    };
}

// ============================================================================
// Macro invocation: single source of truth for field <-> index mapping
// ============================================================================

impl_param_accessors! {
    Gain {
        params: param_specs::gain::PARAMS,
        layout: Some(&param_specs::gain::LAYOUT),
        fields: [gain_db: f64]
    },
    Compressor {
        params: param_specs::compressor::PARAMS,
        layout: Some(&param_specs::compressor::LAYOUT),
        fields: [
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            knee_db: f64, makeup_gain_db: f64, mix: f64,
            auto_makeup: bool, link_channels: bool, sidechain_hpf_hz: f64,
        ]
    },
    Gate {
        params: param_specs::gate::PARAMS,
        layout: Some(&param_specs::gate::LAYOUT),
        fields: [
            threshold_db: f64, ratio: f64, attack_ms: f64, hold_ms: f64,
            release_ms: f64, mix: f64, link_channels: bool, sidechain_hpf_hz: f64,
        ]
    },
    Expander {
        params: param_specs::expander::PARAMS,
        layout: Some(&param_specs::expander::LAYOUT),
        fields: [
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            range_db: f64, knee_db: f64, hysteresis_db: f64, hold_ms: f64,
            mix: f64, auto_makeup: bool, link_channels: bool, sidechain_hpf_hz: f64,
        ]
    },
    Limiter {
        params: param_specs::limiter::PARAMS,
        layout: Some(&param_specs::limiter::LAYOUT),
        fields: [threshold_db: f64, release_ms: f64, lookahead_ms: f64, soft: bool, mix: f64]
    },
    LoudnessCompensation {
        params: param_specs::loudness_compensation::PARAMS,
        layout: Some(&param_specs::loudness_compensation::LAYOUT),
        fields: [
            low_freq: f64, low_gain: f64, high_freq: f64, high_gain: f64,
            auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
        ]
    },
    FletcherMunson {
        params: param_specs::fletcher_munson::PARAMS,
        layout: Some(&param_specs::fletcher_munson::LAYOUT),
        fields: [
            playback_volume_db: f64, reference_level_db: f64, enabled: bool, smoothing_ms: f64,
            auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
            auto_gain_loudness_type: i32,
            band1_freq: f64, band1_q: f64, band1_max_gain: f64, band1_slope: f64,
            band2_freq: f64, band2_q: f64, band2_max_gain: f64, band2_slope: f64,
            band3_freq: f64, band3_q: f64, band3_max_gain: f64, band3_slope: f64,
            band4_freq: f64, band4_q: f64, band4_max_gain: f64, band4_slope: f64,
        ]
    },
    Upmixer {
        params: param_specs::upmixer::PARAMS,
        layout: Some(&param_specs::upmixer::LAYOUT),
        fields: [
            speaker_config: [str speaker_config_to_index, index_to_speaker_config],
            gain_front_direct: f64, gain_front_ambient: f64, gain_rear_ambient: f64,
            height_gain: f64, lfe_gain: f64, lfe_cutoff_hz: f64,
            enable_subharmonic_synth: bool, subharmonic_gain: f64, subharmonic_freq_hz: f64,
            subharmonic_attack_ms: f64, subharmonic_release_ms: f64,
            stereo_width: f64, center_spread: f64, bandpass_hz: f64,
            enable_hr_direct: bool, hr_sharpen: f64, ambient_boost: f64,
            decorrelation_mode: usize, decorrelation_lfo_rate_hz: f64,
            velvet_noise_duration_ms: f64, velvet_noise_density: f64,
            height_hf_cap_hz: f64, height_transient_reduction: f64, height_direct_leak: f64,
            surround_direct_bleed: f64, rear_ambient_boost: f64, rear_late_reflection: f64,
            dialogue_weight: f64, voice_freq_min_hz: f64, voice_freq_max_hz: f64,
            dialogue_centroid_weight: f64, dialogue_variance_weight: f64, dialogue_coherence_weight: f64,
            safety_cap_db: f64,
            bypass_decorrelation: bool, bypass_transient_detection: bool,
            bypass_all_processing: bool, enable_ml_detection: bool,
        ]
    },
    Convolution {
        params: param_specs::convolution::PARAMS,
        layout: Some(&param_specs::convolution::LAYOUT),
        fields: [ir_file: skip, mix: f64, gain_db: f64]
    },
    BinauralDecoder {
        params: param_specs::binaural::PARAMS,
        layout: Some(&param_specs::binaural::LAYOUT),
        fields: [
            sofa_file: skip, input_channels: usize,
            enable_optimization: bool, externalization: f64, near_field_strength: f64,
        ]
    },
    XTC {
        params: param_specs::xtc::PARAMS,
        layout: Some(&param_specs::xtc::LAYOUT),
        fields: [
            distance_m: f64, speaker_angle_deg: f64, head_radius_m: f64,
            head_offset_x: f64, head_offset_z: f64, head_yaw_deg: f64,
            head_tracking_smooth_s: f64,
            beta_base: f64, beta_low_freq_boost: f64, beta_high_freq_boost: f64,
            head_shadow_cutoff_hz: f64, head_shadow_slope_db_per_octave: f64,
            max_gain_db: f64, spectral_normalization: bool,
            pinna_model_enabled: bool, room_reflections_enabled: bool,
            room_width_m: f64, room_depth_m: f64, wall_absorption: f64,
            reflection_beta_boost: f64,
            bypass_xtc_filters: bool, bypass_spectral_normalization: bool,
            bypass_neumann_refinement: bool,
            auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
        ]
    },
    Denoiser {
        params: param_specs::denoiser::PARAMS,
        layout: Some(&param_specs::denoiser::LAYOUT),
        fields: [
            reduction_db: f64, floor_db: f64, smoothing: f64, attack_ms: f64, release_ms: f64,
            low_latency: bool, polyphonic_detection: bool, crack_sensitivity: f64,
            mcra_alpha_s: f64, mcra_alpha_p: f64, mcra_l: usize, mcra_delta: f64,
            transparency: f64, dd_enabled: bool, dd_alpha: f64,
            psychoacoustic_masking: bool, transient_enabled: bool,
            spectral_smoothing_enabled: bool, temporal_smoothing_enabled: bool,
            hiss_enabled: bool, hiss_threshold_db: f64, hiss_frequency_hz: f64, hiss_strength: f64,
            spectral_sub_enabled: bool, spectral_sub_alpha: f64, spectral_sub_beta: f64,
            learn_noise: bool, use_captured_profile: bool, clear_profile: bool,
        ]
    },
    Pnd {
        params: param_specs::pnd::PARAMS,
        layout: Some(&param_specs::pnd::LAYOUT),
        fields: [correction_strength: f64, analysis_window_ms: f64, drift_smoothing: f64]
    },
    ABCompare {
        params: param_specs::ab_compare::PARAMS,
        layout: Some(&param_specs::ab_compare::LAYOUT),
        fields: [
            mix: f64, mix_mode: i32, selected_path: i32, bypass: bool,
            auto_gain_enabled: bool, loudness_type: i32,
            max_auto_gain_db: f64, gain_smoothing_ms: f64, mix_transition_ms: f64,
        ]
    },
    BandSplit {
        params: param_specs::band_split::PARAMS,
        layout: Some(&param_specs::band_split::LAYOUT),
        fields: [frequency: f64, crossover_type: [str crossover_type_to_index, index_to_crossover_type]]
    },
    BandMerge {
        params: param_specs::band_merge::PARAMS,
        layout: Some(&param_specs::band_merge::LAYOUT),
        fields: [bands: usize]
    },
    Downmix {
        params: param_specs::downmix::PARAMS,
        layout: Some(&param_specs::downmix::LAYOUT),
        fields: [
            center_gain_db: f64, surround_gain_db: f64, height_gain_db: f64, lfe_gain_db: f64,
            phase_coherence: bool, phase_blend_low_hz: f64, phase_blend_high_hz: f64,
        ]
    },
    MonoToStereo {
        params: param_specs::mono_to_stereo::PARAMS,
        layout: Some(&param_specs::mono_to_stereo::LAYOUT),
        fields: [
            stereo_width: f64, haas_delay_ms: f64, enable_comp_eq: bool,
            comp_eq_depth_db: f64, decor_low_hz: f64, decor_high_hz: f64,
        ]
    },
    Crossfeed {
        params: param_specs::crossfeed::PARAMS,
        layout: Some(&param_specs::crossfeed::LAYOUT),
        fields: [
            mode: [enum crossfeed_mode_to_index, index_to_crossfeed_mode],
            preset: [enum crossfeed_preset_to_index, index_to_crossfeed_preset],
            enabled: bool, mix: f64,
            bauer_fcut_hz: f64, bauer_feed_db: f64, meier_level: f64,
            mb_low_freq_hz: f64, mb_mid_high_freq_hz: f64,
            mb_low_feed_db: f64, mb_mid_feed_db: f64, mb_high_feed_db: f64,
            autogain_enabled: bool, autogain_target_lufs: f64,
            autogain_max_gain_db: f64, autogain_smoothing_ms: f64,
        ]
    },
    Delay {
        params: param_specs::delay::PARAMS,
        layout: Some(&param_specs::delay::LAYOUT),
        fields: [delay_ms: f64, feedback: f64, mix: f64]
    },
    EQ {
        params: param_specs::eq::GLOBAL_PARAMS,
        layout: None,
        fields: [max_filters: usize]
    },
    MultibandCompressor {
        params: param_specs::multiband_compressor::GLOBAL_PARAMS,
        layout: Some(&param_specs::multiband_compressor::LAYOUT),
        fields: [
            num_bands: usize, crossover_preset: i32,
            crossover_freq_1: f64, crossover_freq_2: f64,
            crossover_freq_3: f64, crossover_freq_4: f64,
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            knee_db: f64, mix: f64, link_channels: bool,
        ]
    },
    MultibandExpander {
        params: param_specs::multiband_expander::GLOBAL_PARAMS,
        layout: Some(&param_specs::multiband_expander::LAYOUT),
        fields: [
            num_bands: usize, crossover_preset: i32,
            crossover_freq_1: f64, crossover_freq_2: f64,
            crossover_freq_3: f64, crossover_freq_4: f64,
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            range_db: f64, knee_db: f64, hysteresis_db: f64, hold_ms: f64,
            mix: f64, link_channels: bool,
        ]
    }
    ;
    no_params_unit: [LoudnessMonitor];
    no_params_struct: [SpectrumAnalyzer, ChannelMuteSolo, Matrix]
}

// ============================================================================
// Manual accessor methods (kept outside the macro)
// ============================================================================

impl PluginSettings {
    /// Get the engine parameter key and value string for zero-dropout updates.
    ///
    /// Returns `None` for structural params, file paths, out-of-range indices,
    /// and plugins with no editable params. For plugins where PARAMS ordering
    /// matches the GPUI param index, this replaces the manual per-plugin mapping.
    pub fn engine_param_at(&self, idx: usize) -> Option<(String, String)> {
        let specs = self.param_specs();
        let spec = specs.get(idx)?;
        if spec.update_mode == param_specs::UpdateMode::Structural {
            return None;
        }
        if matches!(spec.param_type, param_specs::ParamType::FilePath) {
            return None;
        }
        let value = self.param_value(idx)?;
        Some((spec.engine_key.to_string(), spec.engine_value_string(value)))
    }

    /// Format the current value of parameter at `index` as a string for engine communication.
    ///
    /// Unlike `param_value()` which returns f64, this returns the raw string value
    /// suitable for JSON serialization to the plugin engine. String-typed choices
    /// (speaker_config, crossover_type) are returned as their string values.
    pub fn param_value_string(&self, index: usize) -> Option<String> {
        let specs = self.param_specs();
        let spec = specs.get(index)?;

        match spec.param_type {
            param_specs::ParamType::FilePath => {
                // Return the file path string directly
                match self {
                    Self::Convolution { ir_file, .. } if index == 0 => Some(ir_file.clone()),
                    Self::BinauralDecoder { sofa_file, .. } if index == 0 => {
                        Some(sofa_file.clone())
                    }
                    Self::ABCompare { path_a_file, .. } if index == 9 => Some(path_a_file.clone()),
                    Self::ABCompare { path_b_file, .. } if index == 10 => {
                        Some(path_b_file.clone())
                    }
                    _ => None,
                }
            }
            param_specs::ParamType::Bool { .. } => {
                self.param_value(index).map(|v| format!("{}", f2b(v)))
            }
            param_specs::ParamType::Choice { .. } => {
                // String-typed choices need special handling
                match self {
                    Self::Upmixer { speaker_config, .. } if index == 0 => {
                        Some(speaker_config.clone())
                    }
                    Self::BandSplit { crossover_type, .. } if index == 1 => {
                        Some(crossover_type.clone())
                    }
                    Self::Crossfeed { mode, .. } if index == 0 => Some(format!(
                        "{}",
                        serde_json::to_value(mode).unwrap_or_default()
                    )),
                    Self::Crossfeed { preset, .. } if index == 1 => Some(format!(
                        "{}",
                        serde_json::to_value(preset).unwrap_or_default()
                    )),
                    _ => {
                        // Numeric choice: format as integer
                        self.param_value(index).map(|v| format!("{}", v as i64))
                    }
                }
            }
            param_specs::ParamType::Int { .. } => {
                self.param_value(index).map(|v| format!("{}", v as i64))
            }
            param_specs::ParamType::Float { .. } => {
                self.param_value(index).map(|v| spec.format_value(v))
            }
        }
    }
}
