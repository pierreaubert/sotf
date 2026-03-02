//! Generic parameter accessor methods for `PluginSettings`.
//!
//! These methods map between parameter indices (as defined by the `PARAMS` arrays
//! in `param_specs`) and the actual fields of each `PluginSettings` variant.
//! Consumer code (TUI, GPUI, CLI) uses these instead of hardcoding per-plugin
//! field access.

use crate::plugins::PluginSettings;
use sotf_plugins::param_specs::{self, ParamSpec};
use sotf_plugins::{CrossfeedMode, CrossfeedPreset};

// ============================================================================
// Enum/String ↔ index helpers
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
// Bool ↔ f64 helpers
// ============================================================================

#[inline]
fn b2f(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

#[inline]
fn f2b(f: f64) -> bool {
    f > 0.5
}

// ============================================================================
// PluginSettings accessor methods
// ============================================================================

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
            Self::Gain { .. } => param_specs::gain::PARAMS,
            Self::Compressor { .. } => param_specs::compressor::PARAMS,
            Self::Gate { .. } => param_specs::gate::PARAMS,
            Self::Expander { .. } => param_specs::expander::PARAMS,
            Self::Limiter { .. } => param_specs::limiter::PARAMS,
            Self::LoudnessCompensation { .. } => param_specs::loudness_compensation::PARAMS,
            Self::FletcherMunson { .. } => param_specs::fletcher_munson::PARAMS,
            Self::Upmixer { .. } => param_specs::upmixer::PARAMS,
            Self::Convolution { .. } => param_specs::convolution::PARAMS,
            Self::BinauralDecoder { .. } => param_specs::binaural::PARAMS,
            Self::XTC { .. } => param_specs::xtc::PARAMS,
            Self::Denoiser { .. } => param_specs::denoiser::PARAMS,
            Self::Pnd { .. } => param_specs::pnd::PARAMS,
            Self::ABCompare { .. } => param_specs::ab_compare::PARAMS,
            Self::BandSplit { .. } => param_specs::band_split::PARAMS,
            Self::BandMerge { .. } => param_specs::band_merge::PARAMS,
            Self::Downmix { .. } => param_specs::downmix::PARAMS,
            Self::MonoToStereo { .. } => param_specs::mono_to_stereo::PARAMS,
            Self::Crossfeed { .. } => param_specs::crossfeed::PARAMS,
            // Dynamic-param plugins: return global params only
            Self::EQ { .. } => param_specs::eq::GLOBAL_PARAMS,
            Self::MultibandCompressor { .. } => param_specs::multiband_compressor::GLOBAL_PARAMS,
            Self::MultibandExpander { .. } => param_specs::multiband_expander::GLOBAL_PARAMS,
            // No user-editable params
            Self::LoudnessMonitor
            | Self::SpectrumAnalyzer { .. }
            | Self::ChannelMuteSolo { .. }
            | Self::Matrix { .. } => &[],
        }
    }

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

    /// Read the current value of parameter at `index` as f64.
    ///
    /// Returns `None` if the index is out of range or the parameter is a FilePath type.
    /// Bool parameters are returned as 1.0 (true) or 0.0 (false).
    /// Choice parameters are returned as their numeric index.
    pub fn param_value(&self, index: usize) -> Option<f64> {
        match self {
            // ----------------------------------------------------------------
            Self::Gain { gain_db, .. } => {
                if index == 0 {
                    Some(*gain_db)
                } else {
                    None
                }
            }
            // ----------------------------------------------------------------
            Self::Compressor {
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
                0 => Some(*threshold_db),
                1 => Some(*ratio),
                2 => Some(*attack_ms),
                3 => Some(*release_ms),
                4 => Some(*knee_db),
                5 => Some(*makeup_gain_db),
                6 => Some(*mix),
                7 => Some(b2f(*auto_makeup)),
                8 => Some(b2f(*link_channels)),
                9 => Some(*sidechain_hpf_hz),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Gate {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => match index {
                0 => Some(*threshold_db),
                1 => Some(*ratio),
                2 => Some(*attack_ms),
                3 => Some(*hold_ms),
                4 => Some(*release_ms),
                5 => Some(*mix),
                6 => Some(b2f(*link_channels)),
                7 => Some(*sidechain_hpf_hz),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Expander {
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
                0 => Some(*threshold_db),
                1 => Some(*ratio),
                2 => Some(*attack_ms),
                3 => Some(*release_ms),
                4 => Some(*range_db),
                5 => Some(*knee_db),
                6 => Some(*hysteresis_db),
                7 => Some(*hold_ms),
                8 => Some(*mix),
                9 => Some(b2f(*link_channels)),
                10 => Some(*sidechain_hpf_hz),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => match index {
                0 => Some(*threshold_db),
                1 => Some(*release_ms),
                2 => Some(*lookahead_ms),
                3 => Some(b2f(*soft)),
                4 => Some(*mix),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => match index {
                0 => Some(*low_freq),
                1 => Some(*low_gain),
                2 => Some(*high_freq),
                3 => Some(*high_gain),
                4 => Some(b2f(*auto_gain_enabled)),
                5 => Some(*auto_gain_max_db),
                6 => Some(*auto_gain_smoothing_ms),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::FletcherMunson {
                reference_level_db,
                enabled,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
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
                ..
            } => match index {
                0 => Some(*reference_level_db),
                1 => Some(b2f(*enabled)),
                2 => Some(*smoothing_ms),
                3 => Some(b2f(*auto_gain_enabled)),
                4 => Some(*auto_gain_max_db),
                5 => Some(*auto_gain_smoothing_ms),
                6 => Some(*band1_freq),
                7 => Some(*band1_q),
                8 => Some(*band1_max_gain),
                9 => Some(*band1_slope),
                10 => Some(*band2_freq),
                11 => Some(*band2_q),
                12 => Some(*band2_max_gain),
                13 => Some(*band2_slope),
                14 => Some(*band3_freq),
                15 => Some(*band3_q),
                16 => Some(*band3_max_gain),
                17 => Some(*band3_slope),
                18 => Some(*band4_freq),
                19 => Some(*band4_q),
                20 => Some(*band4_max_gain),
                21 => Some(*band4_slope),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                lfe_gain,
                lfe_cutoff_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                stereo_width,
                center_spread,
                bandpass_hz,
                enable_hr_direct,
                hr_sharpen,
                ambient_boost,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                surround_direct_bleed,
                rear_ambient_boost,
                rear_late_reflection,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
                safety_cap_db,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
                enable_ml_detection,
            } => match index {
                0 => Some(speaker_config_to_index(speaker_config)),
                1 => Some(*gain_front_direct),
                2 => Some(*gain_front_ambient),
                3 => Some(*gain_rear_ambient),
                4 => Some(*height_gain),
                5 => Some(*lfe_gain),
                6 => Some(*lfe_cutoff_hz),
                7 => Some(b2f(*enable_subharmonic_synth)),
                8 => Some(*subharmonic_gain),
                9 => Some(*subharmonic_freq_hz),
                10 => Some(*subharmonic_attack_ms),
                11 => Some(*subharmonic_release_ms),
                12 => Some(*stereo_width),
                13 => Some(*center_spread),
                14 => Some(*bandpass_hz),
                15 => Some(b2f(*enable_hr_direct)),
                16 => Some(*hr_sharpen),
                17 => Some(*ambient_boost),
                18 => Some(*decorrelation_mode as f64),
                19 => Some(*decorrelation_lfo_rate_hz),
                20 => Some(*velvet_noise_duration_ms),
                21 => Some(*velvet_noise_density),
                22 => Some(*height_hf_cap_hz),
                23 => Some(*height_transient_reduction),
                24 => Some(*height_direct_leak),
                25 => Some(*surround_direct_bleed),
                26 => Some(*rear_ambient_boost),
                27 => Some(*rear_late_reflection),
                28 => Some(*dialogue_weight),
                29 => Some(*voice_freq_min_hz),
                30 => Some(*voice_freq_max_hz),
                31 => Some(*dialogue_centroid_weight),
                32 => Some(*dialogue_variance_weight),
                33 => Some(*dialogue_coherence_weight),
                34 => Some(*safety_cap_db),
                35 => Some(b2f(*bypass_decorrelation)),
                36 => Some(b2f(*bypass_transient_detection)),
                37 => Some(b2f(*bypass_all_processing)),
                38 => Some(b2f(*enable_ml_detection)),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Convolution {
                ir_file: _,
                mix,
                gain_db,
            } => match index {
                0 => None, // FilePath
                1 => Some(*mix),
                2 => Some(*gain_db),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::BinauralDecoder {
                sofa_file: _,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => match index {
                0 => None, // FilePath
                1 => Some(*input_channels as f64),
                2 => Some(b2f(*enable_optimization)),
                3 => Some(*externalization),
                4 => Some(*near_field_strength),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                head_offset_x,
                head_offset_z,
                head_yaw_deg,
                head_tracking_smooth_s,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
                max_gain_db,
                spectral_normalization,
                pinna_model_enabled,
                room_reflections_enabled,
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
                ..
            } => match index {
                0 => Some(*distance_m),
                1 => Some(*speaker_angle_deg),
                2 => Some(*head_radius_m),
                3 => Some(*head_offset_x),
                4 => Some(*head_offset_z),
                5 => Some(*head_yaw_deg),
                6 => Some(*head_tracking_smooth_s),
                7 => Some(*beta_base),
                8 => Some(*beta_low_freq_boost),
                9 => Some(*beta_high_freq_boost),
                10 => Some(*head_shadow_cutoff_hz),
                11 => Some(*head_shadow_slope_db_per_octave),
                12 => Some(*max_gain_db),
                13 => Some(b2f(*spectral_normalization)),
                14 => Some(b2f(*pinna_model_enabled)),
                15 => Some(b2f(*room_reflections_enabled)),
                16 => Some(*room_width_m),
                17 => Some(*room_depth_m),
                18 => Some(*wall_absorption),
                19 => Some(*reflection_beta_boost),
                20 => Some(b2f(*bypass_xtc_filters)),
                21 => Some(b2f(*bypass_spectral_normalization)),
                22 => Some(b2f(*bypass_neumann_refinement)),
                23 => Some(b2f(*auto_gain_enabled)),
                24 => Some(*auto_gain_max_db),
                25 => Some(*auto_gain_smoothing_ms),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Denoiser {
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
                0 => Some(*reduction_db),
                1 => Some(*floor_db),
                2 => Some(*smoothing),
                3 => Some(*attack_ms),
                4 => Some(*release_ms),
                5 => Some(b2f(*low_latency)),
                6 => Some(b2f(*polyphonic_detection)),
                7 => Some(*crack_sensitivity),
                8 => Some(*mcra_alpha_s),
                9 => Some(*mcra_alpha_p),
                10 => Some(*mcra_l as f64),
                11 => Some(*mcra_delta),
                12 => Some(*transparency),
                13 => Some(b2f(*dd_enabled)),
                14 => Some(*dd_alpha),
                15 => Some(b2f(*psychoacoustic_masking)),
                16 => Some(b2f(*learn_noise)),
                17 => Some(b2f(*use_captured_profile)),
                18 => Some(b2f(*clear_profile)),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => match index {
                0 => Some(*correction_strength),
                1 => Some(*analysis_window_ms),
                2 => Some(*drift_smoothing),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::ABCompare {
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
                0 => Some(*mix),
                1 => Some(*mix_mode as f64),
                2 => Some(*selected_path as f64),
                3 => Some(b2f(*bypass)),
                4 => Some(b2f(*auto_gain_enabled)),
                5 => Some(*loudness_type as f64),
                6 => Some(*max_auto_gain_db),
                7 => Some(*gain_smoothing_ms),
                8 => Some(*mix_transition_ms),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::BandSplit {
                frequency,
                crossover_type,
                ..
            } => match index {
                0 => Some(*frequency),
                1 => Some(crossover_type_to_index(crossover_type)),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::BandMerge { bands, .. } => {
                if index == 0 {
                    Some(*bands as f64)
                } else {
                    None
                }
            }
            // ----------------------------------------------------------------
            Self::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => match index {
                0 => Some(*center_gain_db),
                1 => Some(*surround_gain_db),
                2 => Some(*height_gain_db),
                3 => Some(*lfe_gain_db),
                4 => Some(b2f(*phase_coherence)),
                5 => Some(*phase_blend_low_hz),
                6 => Some(*phase_blend_high_hz),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => match index {
                0 => Some(*stereo_width),
                1 => Some(*haas_delay_ms),
                2 => Some(b2f(*enable_comp_eq)),
                3 => Some(*comp_eq_depth_db),
                4 => Some(*decor_low_hz),
                5 => Some(*decor_high_hz),
                _ => None,
            },
            // ----------------------------------------------------------------
            Self::Crossfeed {
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
                0 => Some(crossfeed_mode_to_index(mode)),
                1 => Some(crossfeed_preset_to_index(preset)),
                2 => Some(b2f(*enabled)),
                3 => Some(*mix),
                4 => Some(*bauer_fcut_hz),
                5 => Some(*bauer_feed_db),
                6 => Some(*meier_level),
                7 => Some(*mb_low_freq_hz),
                8 => Some(*mb_mid_high_freq_hz),
                9 => Some(*mb_low_feed_db),
                10 => Some(*mb_mid_feed_db),
                11 => Some(*mb_high_feed_db),
                12 => Some(b2f(*autogain_enabled)),
                13 => Some(*autogain_target_lufs),
                14 => Some(*autogain_max_gain_db),
                15 => Some(*autogain_smoothing_ms),
                _ => None,
            },
            // ----------------------------------------------------------------
            // Dynamic-param plugins: global params only
            Self::EQ { max_filters, .. } => {
                if index == 0 {
                    Some(*max_filters as f64)
                } else {
                    None
                }
            }
            Self::MultibandCompressor {
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
                ..
            } => match index {
                0 => Some(*num_bands as f64),
                1 => Some(*crossover_preset as f64),
                2 => Some(*crossover_freq_1),
                3 => Some(*crossover_freq_2),
                4 => Some(*crossover_freq_3),
                5 => Some(*crossover_freq_4),
                6 => Some(*threshold_db),
                7 => Some(*ratio),
                8 => Some(*attack_ms),
                9 => Some(*release_ms),
                10 => Some(*knee_db),
                11 => Some(*mix),
                12 => Some(b2f(*link_channels)),
                _ => None,
            },
            Self::MultibandExpander {
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
                ..
            } => match index {
                0 => Some(*num_bands as f64),
                1 => Some(*crossover_preset as f64),
                2 => Some(*crossover_freq_1),
                3 => Some(*crossover_freq_2),
                4 => Some(*crossover_freq_3),
                5 => Some(*crossover_freq_4),
                6 => Some(*threshold_db),
                7 => Some(*ratio),
                8 => Some(*attack_ms),
                9 => Some(*release_ms),
                10 => Some(*range_db),
                11 => Some(*knee_db),
                12 => Some(*hysteresis_db),
                13 => Some(*hold_ms),
                14 => Some(*mix),
                15 => Some(b2f(*link_channels)),
                _ => None,
            },
            // ----------------------------------------------------------------
            // No editable params
            Self::LoudnessMonitor
            | Self::SpectrumAnalyzer { .. }
            | Self::ChannelMuteSolo { .. }
            | Self::Matrix { .. } => None,
        }
    }

    /// Set the value of parameter at `index` from an f64 value.
    ///
    /// Does nothing for FilePath params, out-of-range indices, or non-editable plugins.
    /// Bool parameters: values > 0.5 are treated as true.
    /// Choice parameters: value is cast to the appropriate integer/enum type.
    pub fn set_param_value(&mut self, index: usize, value: f64) {
        match self {
            // ----------------------------------------------------------------
            Self::Gain { gain_db, .. } => {
                if index == 0 {
                    *gain_db = value;
                }
            }
            // ----------------------------------------------------------------
            Self::Compressor {
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
                0 => *threshold_db = value,
                1 => *ratio = value,
                2 => *attack_ms = value,
                3 => *release_ms = value,
                4 => *knee_db = value,
                5 => *makeup_gain_db = value,
                6 => *mix = value,
                7 => *auto_makeup = f2b(value),
                8 => *link_channels = f2b(value),
                9 => *sidechain_hpf_hz = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Gate {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => match index {
                0 => *threshold_db = value,
                1 => *ratio = value,
                2 => *attack_ms = value,
                3 => *hold_ms = value,
                4 => *release_ms = value,
                5 => *mix = value,
                6 => *link_channels = f2b(value),
                7 => *sidechain_hpf_hz = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Expander {
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
                0 => *threshold_db = value,
                1 => *ratio = value,
                2 => *attack_ms = value,
                3 => *release_ms = value,
                4 => *range_db = value,
                5 => *knee_db = value,
                6 => *hysteresis_db = value,
                7 => *hold_ms = value,
                8 => *mix = value,
                9 => *link_channels = f2b(value),
                10 => *sidechain_hpf_hz = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => match index {
                0 => *threshold_db = value,
                1 => *release_ms = value,
                2 => *lookahead_ms = value,
                3 => *soft = f2b(value),
                4 => *mix = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => match index {
                0 => *low_freq = value,
                1 => *low_gain = value,
                2 => *high_freq = value,
                3 => *high_gain = value,
                4 => *auto_gain_enabled = f2b(value),
                5 => *auto_gain_max_db = value,
                6 => *auto_gain_smoothing_ms = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::FletcherMunson {
                reference_level_db,
                enabled,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
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
                ..
            } => match index {
                0 => *reference_level_db = value,
                1 => *enabled = f2b(value),
                2 => *smoothing_ms = value,
                3 => *auto_gain_enabled = f2b(value),
                4 => *auto_gain_max_db = value,
                5 => *auto_gain_smoothing_ms = value,
                6 => *band1_freq = value,
                7 => *band1_q = value,
                8 => *band1_max_gain = value,
                9 => *band1_slope = value,
                10 => *band2_freq = value,
                11 => *band2_q = value,
                12 => *band2_max_gain = value,
                13 => *band2_slope = value,
                14 => *band3_freq = value,
                15 => *band3_q = value,
                16 => *band3_max_gain = value,
                17 => *band3_slope = value,
                18 => *band4_freq = value,
                19 => *band4_q = value,
                20 => *band4_max_gain = value,
                21 => *band4_slope = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                lfe_gain,
                lfe_cutoff_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                stereo_width,
                center_spread,
                bandpass_hz,
                enable_hr_direct,
                hr_sharpen,
                ambient_boost,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                surround_direct_bleed,
                rear_ambient_boost,
                rear_late_reflection,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
                safety_cap_db,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
                enable_ml_detection,
            } => match index {
                0 => *speaker_config = index_to_speaker_config(value),
                1 => *gain_front_direct = value,
                2 => *gain_front_ambient = value,
                3 => *gain_rear_ambient = value,
                4 => *height_gain = value,
                5 => *lfe_gain = value,
                6 => *lfe_cutoff_hz = value,
                7 => *enable_subharmonic_synth = f2b(value),
                8 => *subharmonic_gain = value,
                9 => *subharmonic_freq_hz = value,
                10 => *subharmonic_attack_ms = value,
                11 => *subharmonic_release_ms = value,
                12 => *stereo_width = value,
                13 => *center_spread = value,
                14 => *bandpass_hz = value,
                15 => *enable_hr_direct = f2b(value),
                16 => *hr_sharpen = value,
                17 => *ambient_boost = value,
                18 => *decorrelation_mode = value as usize,
                19 => *decorrelation_lfo_rate_hz = value,
                20 => *velvet_noise_duration_ms = value,
                21 => *velvet_noise_density = value,
                22 => *height_hf_cap_hz = value,
                23 => *height_transient_reduction = value,
                24 => *height_direct_leak = value,
                25 => *surround_direct_bleed = value,
                26 => *rear_ambient_boost = value,
                27 => *rear_late_reflection = value,
                28 => *dialogue_weight = value,
                29 => *voice_freq_min_hz = value,
                30 => *voice_freq_max_hz = value,
                31 => *dialogue_centroid_weight = value,
                32 => *dialogue_variance_weight = value,
                33 => *dialogue_coherence_weight = value,
                34 => *safety_cap_db = value,
                35 => *bypass_decorrelation = f2b(value),
                36 => *bypass_transient_detection = f2b(value),
                37 => *bypass_all_processing = f2b(value),
                38 => *enable_ml_detection = f2b(value),
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Convolution {
                ir_file: _,
                mix,
                gain_db,
            } => match index {
                0 => {} // FilePath — not settable via f64
                1 => *mix = value,
                2 => *gain_db = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::BinauralDecoder {
                sofa_file: _,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => match index {
                0 => {} // FilePath
                1 => *input_channels = value as usize,
                2 => *enable_optimization = f2b(value),
                3 => *externalization = value,
                4 => *near_field_strength = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                head_offset_x,
                head_offset_z,
                head_yaw_deg,
                head_tracking_smooth_s,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
                max_gain_db,
                spectral_normalization,
                pinna_model_enabled,
                room_reflections_enabled,
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
                ..
            } => match index {
                0 => *distance_m = value,
                1 => *speaker_angle_deg = value,
                2 => *head_radius_m = value,
                3 => *head_offset_x = value,
                4 => *head_offset_z = value,
                5 => *head_yaw_deg = value,
                6 => *head_tracking_smooth_s = value,
                7 => *beta_base = value,
                8 => *beta_low_freq_boost = value,
                9 => *beta_high_freq_boost = value,
                10 => *head_shadow_cutoff_hz = value,
                11 => *head_shadow_slope_db_per_octave = value,
                12 => *max_gain_db = value,
                13 => *spectral_normalization = f2b(value),
                14 => *pinna_model_enabled = f2b(value),
                15 => *room_reflections_enabled = f2b(value),
                16 => *room_width_m = value,
                17 => *room_depth_m = value,
                18 => *wall_absorption = value,
                19 => *reflection_beta_boost = value,
                20 => *bypass_xtc_filters = f2b(value),
                21 => *bypass_spectral_normalization = f2b(value),
                22 => *bypass_neumann_refinement = f2b(value),
                23 => *auto_gain_enabled = f2b(value),
                24 => *auto_gain_max_db = value,
                25 => *auto_gain_smoothing_ms = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Denoiser {
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
                0 => *reduction_db = value,
                1 => *floor_db = value,
                2 => *smoothing = value,
                3 => *attack_ms = value,
                4 => *release_ms = value,
                5 => *low_latency = f2b(value),
                6 => *polyphonic_detection = f2b(value),
                7 => *crack_sensitivity = value,
                8 => *mcra_alpha_s = value,
                9 => *mcra_alpha_p = value,
                10 => *mcra_l = value as usize,
                11 => *mcra_delta = value,
                12 => *transparency = value,
                13 => *dd_enabled = f2b(value),
                14 => *dd_alpha = value,
                15 => *psychoacoustic_masking = f2b(value),
                16 => *learn_noise = f2b(value),
                17 => *use_captured_profile = f2b(value),
                18 => *clear_profile = f2b(value),
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => match index {
                0 => *correction_strength = value,
                1 => *analysis_window_ms = value,
                2 => *drift_smoothing = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::ABCompare {
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
                0 => *mix = value,
                1 => *mix_mode = value as i32,
                2 => *selected_path = value as i32,
                3 => *bypass = f2b(value),
                4 => *auto_gain_enabled = f2b(value),
                5 => *loudness_type = value as i32,
                6 => *max_auto_gain_db = value,
                7 => *gain_smoothing_ms = value,
                8 => *mix_transition_ms = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::BandSplit {
                frequency,
                crossover_type,
                ..
            } => match index {
                0 => *frequency = value,
                1 => *crossover_type = index_to_crossover_type(value),
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::BandMerge { bands, .. } => {
                if index == 0 {
                    *bands = value as usize;
                }
            }
            // ----------------------------------------------------------------
            Self::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => match index {
                0 => *center_gain_db = value,
                1 => *surround_gain_db = value,
                2 => *height_gain_db = value,
                3 => *lfe_gain_db = value,
                4 => *phase_coherence = f2b(value),
                5 => *phase_blend_low_hz = value,
                6 => *phase_blend_high_hz = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => match index {
                0 => *stereo_width = value,
                1 => *haas_delay_ms = value,
                2 => *enable_comp_eq = f2b(value),
                3 => *comp_eq_depth_db = value,
                4 => *decor_low_hz = value,
                5 => *decor_high_hz = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            Self::Crossfeed {
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
                0 => *mode = index_to_crossfeed_mode(value),
                1 => *preset = index_to_crossfeed_preset(value),
                2 => *enabled = f2b(value),
                3 => *mix = value,
                4 => *bauer_fcut_hz = value,
                5 => *bauer_feed_db = value,
                6 => *meier_level = value,
                7 => *mb_low_freq_hz = value,
                8 => *mb_mid_high_freq_hz = value,
                9 => *mb_low_feed_db = value,
                10 => *mb_mid_feed_db = value,
                11 => *mb_high_feed_db = value,
                12 => *autogain_enabled = f2b(value),
                13 => *autogain_target_lufs = value,
                14 => *autogain_max_gain_db = value,
                15 => *autogain_smoothing_ms = value,
                _ => {}
            },
            // ----------------------------------------------------------------
            // Dynamic-param plugins: global params only
            Self::EQ { max_filters, .. } => {
                if index == 0 {
                    *max_filters = value as usize;
                }
            }
            Self::MultibandCompressor {
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
                ..
            } => match index {
                0 => *num_bands = value as usize,
                1 => *crossover_preset = value as i32,
                2 => *crossover_freq_1 = value,
                3 => *crossover_freq_2 = value,
                4 => *crossover_freq_3 = value,
                5 => *crossover_freq_4 = value,
                6 => *threshold_db = value,
                7 => *ratio = value,
                8 => *attack_ms = value,
                9 => *release_ms = value,
                10 => *knee_db = value,
                11 => *mix = value,
                12 => *link_channels = f2b(value),
                _ => {}
            },
            Self::MultibandExpander {
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
                ..
            } => match index {
                0 => *num_bands = value as usize,
                1 => *crossover_preset = value as i32,
                2 => *crossover_freq_1 = value,
                3 => *crossover_freq_2 = value,
                4 => *crossover_freq_3 = value,
                5 => *crossover_freq_4 = value,
                6 => *threshold_db = value,
                7 => *ratio = value,
                8 => *attack_ms = value,
                9 => *release_ms = value,
                10 => *range_db = value,
                11 => *knee_db = value,
                12 => *hysteresis_db = value,
                13 => *hold_ms = value,
                14 => *mix = value,
                15 => *link_channels = f2b(value),
                _ => {}
            },
            // ----------------------------------------------------------------
            // No editable params
            Self::LoudnessMonitor
            | Self::SpectrumAnalyzer { .. }
            | Self::ChannelMuteSolo { .. }
            | Self::Matrix { .. } => {}
        }
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
