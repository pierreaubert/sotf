//! Common utilities for plugin UI components

use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Potentiometer, PotentiometerScale, PotentiometerSize, Toggle, ToggleStyle, VerticalSlider,
    VerticalSliderTheme,
};
use sotf_audio_player::PluginSettings;

/// Map a UI parameter index to the engine parameter ID and formatted value string.
/// Returns None if this parameter requires a Structural update (e.g., EQ, channel count changes).
///
/// This enables zero-dropout parameter updates for plugins that support it.
pub fn param_index_to_engine_param(
    settings: &PluginSettings,
    param_idx: usize,
) -> Option<(String, String)> {
    match settings {
        PluginSettings::Gain { gain_db, .. } => match param_idx {
            0 => Some(("gain_db".to_string(), format!("{}", gain_db))),
            _ => None,
        },
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
        } => match param_idx {
            0 => Some(("threshold".to_string(), format!("{}", threshold_db))),
            1 => Some(("ratio".to_string(), format!("{}", ratio))),
            2 => Some(("attack".to_string(), format!("{}", attack_ms))),
            3 => Some(("release".to_string(), format!("{}", release_ms))),
            4 => Some(("knee".to_string(), format!("{}", knee_db))),
            5 => Some(("makeup_gain".to_string(), format!("{}", makeup_gain_db))),
            6 => Some(("mix".to_string(), format!("{}", mix))),
            7 => Some(("auto_makeup".to_string(), auto_makeup.to_string())),
            8 => Some(("link_channels".to_string(), link_channels.to_string())),
            9 => Some((
                "sidechain_hpf_hz".to_string(),
                format!("{}", sidechain_hpf_hz),
            )),
            _ => None,
        },
        PluginSettings::Upmixer {
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            lfe_gain,
            lfe_cutoff_hz,
            stereo_width,
            center_spread,
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
            surround_direct_bleed,
            safety_cap_db,
            rear_ambient_boost,
            rear_late_reflection,
            ambient_boost,
            dialogue_weight,
            voice_freq_min_hz,
            voice_freq_max_hz,
            ..
        } => match param_idx {
            // param 0 = speaker_config: requires Structural (changes channel count)
            0 => None,
            1 => Some((
                "gain_front_direct".to_string(),
                format!("{}", gain_front_direct),
            )),
            2 => Some((
                "gain_front_ambient".to_string(),
                format!("{}", gain_front_ambient),
            )),
            3 => Some((
                "gain_rear_ambient".to_string(),
                format!("{}", gain_rear_ambient),
            )),
            4 => Some(("height_gain".to_string(), format!("{}", height_gain))),
            5 => Some(("lfe_gain".to_string(), format!("{}", lfe_gain))),
            6 => Some(("lfe_cutoff_hz".to_string(), format!("{}", lfe_cutoff_hz))),
            7 => Some(("stereo_width".to_string(), format!("{}", stereo_width))),
            8 => Some(("center_spread".to_string(), format!("{}", center_spread))),
            9 => Some(("bandpass_hz".to_string(), format!("{}", bandpass_hz))),
            10 => Some((
                "enable_subharmonic_synth".to_string(),
                enable_subharmonic_synth.to_string(),
            )),
            11 => Some((
                "subharmonic_gain".to_string(),
                format!("{}", subharmonic_gain),
            )),
            12 => Some(("enable_hr_direct".to_string(), enable_hr_direct.to_string())),
            13 => Some(("hr_sharpen".to_string(), format!("{}", hr_sharpen))),
            14 => Some(("safety_cap_db".to_string(), format!("{}", safety_cap_db))),
            15 => Some((
                "decorrelation_mode".to_string(),
                format!("{}", decorrelation_mode),
            )),
            16 => Some((
                "subharmonic_freq_hz".to_string(),
                format!("{}", subharmonic_freq_hz),
            )),
            17 => Some((
                "subharmonic_attack_ms".to_string(),
                format!("{}", subharmonic_attack_ms),
            )),
            18 => Some((
                "subharmonic_release_ms".to_string(),
                format!("{}", subharmonic_release_ms),
            )),
            19 => Some((
                "decorrelation_lfo_rate_hz".to_string(),
                format!("{}", decorrelation_lfo_rate_hz),
            )),
            20 => Some((
                "velvet_noise_duration_ms".to_string(),
                format!("{}", velvet_noise_duration_ms),
            )),
            21 => Some((
                "velvet_noise_density".to_string(),
                format!("{}", velvet_noise_density),
            )),
            22 => Some((
                "height_hf_cap_hz".to_string(),
                format!("{}", height_hf_cap_hz),
            )),
            23 => Some((
                "height_transient_reduction".to_string(),
                format!("{}", height_transient_reduction),
            )),
            24 => Some((
                "height_direct_leak".to_string(),
                format!("{}", height_direct_leak),
            )),
            25 => Some((
                "surround_direct_bleed".to_string(),
                format!("{}", surround_direct_bleed),
            )),
            26 => Some((
                "rear_ambient_boost".to_string(),
                format!("{}", rear_ambient_boost),
            )),
            27 => Some((
                "rear_late_reflection".to_string(),
                format!("{}", rear_late_reflection),
            )),
            28 => Some((
                "ambient_boost".to_string(),
                format!("{}", ambient_boost),
            )),
            29 => Some((
                "dialogue_weight".to_string(),
                format!("{}", dialogue_weight),
            )),
            30 => Some((
                "voice_freq_min_hz".to_string(),
                format!("{}", voice_freq_min_hz),
            )),
            31 => Some((
                "voice_freq_max_hz".to_string(),
                format!("{}", voice_freq_max_hz),
            )),
            _ => None,
        },
        PluginSettings::Convolution { mix, gain_db, .. } => match param_idx {
            // param 0 = ir_file: requires Structural (file path change)
            0 => None,
            1 => Some(("mix".to_string(), format!("{}", mix))),
            2 => Some(("gain_db".to_string(), format!("{}", gain_db))),
            _ => None,
        },
        // EQ doesn't support individual parameter updates
        PluginSettings::EQ { .. } => None,
        PluginSettings::Denoiser {
            reduction_db,
            floor_db,
            smoothing,
            attack_ms,
            release_ms,
            polyphonic_detection,
            transparency,
            dd_enabled,
            dd_alpha,
            psychoacoustic_masking,
            use_captured_profile,
            ..
        } => match param_idx {
            0 => Some(("reduction_db".to_string(), format!("{}", reduction_db))),
            1 => Some(("floor_db".to_string(), format!("{}", floor_db))),
            2 => Some(("smoothing".to_string(), format!("{}", smoothing))),
            3 => Some(("attack_ms".to_string(), format!("{}", attack_ms))),
            4 => Some(("release_ms".to_string(), format!("{}", release_ms))),
            // low_latency (5) requires structural update (FFT resize), so return None
            5 => None,
            6 => Some((
                "polyphonic_detection".to_string(),
                polyphonic_detection.to_string(),
            )),
            7 => Some(("transparency".to_string(), format!("{}", transparency))),
            8 => Some(("dd_enabled".to_string(), dd_enabled.to_string())),
            9 => Some(("dd_alpha".to_string(), format!("{}", dd_alpha))),
            10 => Some((
                "psychoacoustic_masking".to_string(),
                psychoacoustic_masking.to_string(),
            )),
            11 => None, // learn_noise trigger — handled by set_parameter_value
            12 => Some((
                "use_captured_profile".to_string(),
                use_captured_profile.to_string(),
            )),
            13 => None, // clear_profile trigger — handled by set_parameter_value
            _ => None,
        },
        PluginSettings::Pnd {
            correction_strength,
            analysis_window_ms,
            drift_smoothing,
        } => match param_idx {
            0 => Some((
                "correction_strength".to_string(),
                format!("{}", correction_strength),
            )),
            1 => Some((
                "analysis_window_ms".to_string(),
                format!("{}", analysis_window_ms),
            )),
            2 => Some((
                "drift_smoothing".to_string(),
                format!("{}", drift_smoothing),
            )),
            _ => None,
        },
        PluginSettings::FletcherMunson {
            playback_volume_db,
            reference_level_db,
            enabled,
            smoothing_ms,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            auto_gain_loudness_type,
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
        } => {
            // Check global params
            match param_idx {
                0 => Some((
                    "playback_volume_db".to_string(),
                    format!("{}", playback_volume_db),
                )),
                1 => Some((
                    "reference_level_db".to_string(),
                    format!("{}", reference_level_db),
                )),
                2 => Some(("enabled".to_string(), enabled.to_string())),
                3 => Some(("smoothing_ms".to_string(), format!("{}", smoothing_ms))),
                4 => Some((
                    "auto_gain_enabled".to_string(),
                    auto_gain_enabled.to_string(),
                )),
                5 => Some((
                    "auto_gain_max_db".to_string(),
                    format!("{}", auto_gain_max_db),
                )),
                6 => Some((
                    "auto_gain_smoothing_ms".to_string(),
                    format!("{}", auto_gain_smoothing_ms),
                )),
                7 => Some((
                    "auto_gain_loudness_type".to_string(),
                    format!("{}", auto_gain_loudness_type),
                )),
                _ => {
                    // Band params logic
                    if (8..24).contains(&param_idx) {
                        let rel_idx = param_idx - 8;
                        let band_idx = (rel_idx / 4) + 1;
                        let field_idx = rel_idx % 4;

                        let (freq, q, max_gain, slope) = match band_idx {
                            1 => (band1_freq, band1_q, band1_max_gain, band1_slope),
                            2 => (band2_freq, band2_q, band2_max_gain, band2_slope),
                            3 => (band3_freq, band3_q, band3_max_gain, band3_slope),
                            4 => (band4_freq, band4_q, band4_max_gain, band4_slope),
                            _ => return None,
                        };

                        match field_idx {
                            0 => Some((format!("band{}_freq", band_idx), format!("{}", freq))),
                            1 => Some((format!("band{}_q", band_idx), format!("{}", q))),
                            2 => Some((
                                format!("band{}_max_gain", band_idx),
                                format!("{}", max_gain),
                            )),
                            3 => Some((format!("band{}_slope", band_idx), format!("{}", slope))),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
            }
        }
        PluginSettings::MultibandCompressor {
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
            if param_idx < 100 {
                match param_idx {
                    6 => Some(("threshold".to_string(), format!("{}", threshold_db))),
                    7 => Some(("ratio".to_string(), format!("{}", ratio))),
                    8 => Some(("attack".to_string(), format!("{}", attack_ms))),
                    9 => Some(("release".to_string(), format!("{}", release_ms))),
                    10 => Some(("knee".to_string(), format!("{}", knee_db))),
                    11 => Some(("mix".to_string(), format!("{}", mix))),
                    12 => Some(("link_channels".to_string(), link_channels.to_string())),
                    _ => None,
                }
            } else {
                let band_idx = param_idx / 100;
                let local_idx = param_idx % 100;
                let band_zero_based = band_idx - 1;

                let param_name = match local_idx {
                    6 => "threshold",
                    7 => "ratio",
                    8 => "attack",
                    9 => "release",
                    10 => "knee",
                    13 => "makeup_gain",
                    14 => "bypass",
                    15 => "solo",
                    _ => return None,
                };

                let id = format!("band_{}_{}", band_zero_based, param_name);

                let val_str = if let Some(band) = bands.get(band_zero_based) {
                    match local_idx {
                        6 => format!("{}", band.threshold_db.unwrap_or(*threshold_db as f32)),
                        7 => format!("{}", band.ratio.unwrap_or(*ratio as f32)),
                        8 => format!("{}", band.attack_ms.unwrap_or(*attack_ms as f32)),
                        9 => format!("{}", band.release_ms.unwrap_or(*release_ms as f32)),
                        10 => format!("{}", band.knee_db.unwrap_or(*knee_db as f32)),
                        13 => format!("{}", band.makeup_gain_db),
                        14 => format!("{}", band.bypass),
                        15 => format!("{}", band.solo),
                        _ => "?".to_string(),
                    }
                } else {
                    "?".to_string()
                };

                Some((id, val_str))
            }
        }
        PluginSettings::MultibandExpander {
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
            if param_idx < 100 {
                match param_idx {
                    6 => Some(("threshold".to_string(), format!("{}", threshold_db))),
                    7 => Some(("ratio".to_string(), format!("{}", ratio))),
                    8 => Some(("attack".to_string(), format!("{}", attack_ms))),
                    9 => Some(("release".to_string(), format!("{}", release_ms))),
                    10 => Some(("range".to_string(), format!("{}", range_db))),
                    11 => Some(("knee".to_string(), format!("{}", knee_db))),
                    12 => Some(("hysteresis".to_string(), format!("{}", hysteresis_db))),
                    13 => Some(("hold".to_string(), format!("{}", hold_ms))),
                    14 => Some(("mix".to_string(), format!("{}", mix))),
                    15 => Some(("link_channels".to_string(), link_channels.to_string())),
                    _ => None,
                }
            } else {
                let band_idx = param_idx / 100;
                let local_idx = param_idx % 100;
                let band_zero_based = band_idx - 1;

                let param_name = match local_idx {
                    6 => "threshold",
                    7 => "ratio",
                    8 => "attack",
                    9 => "release",
                    10 => "range",
                    11 => "knee",
                    12 => "hysteresis",
                    13 => "hold",
                    14 => "bypass",
                    15 => "solo",
                    _ => return None,
                };

                let id = format!("band_{}_{}", band_zero_based, param_name);

                let val_str = if let Some(band) = bands.get(band_zero_based) {
                    match local_idx {
                        6 => format!("{}", band.threshold_db.unwrap_or(*threshold_db as f32)),
                        7 => format!("{}", band.ratio.unwrap_or(*ratio as f32)),
                        8 => format!("{}", band.attack_ms.unwrap_or(*attack_ms as f32)),
                        9 => format!("{}", band.release_ms.unwrap_or(*release_ms as f32)),
                        10 => format!("{}", band.range_db.unwrap_or(*range_db as f32)),
                        11 => format!("{}", band.knee_db.unwrap_or(*knee_db as f32)),
                        12 => format!("{}", band.hysteresis_db.unwrap_or(*hysteresis_db as f32)),
                        13 => format!("{}", band.hold_ms.unwrap_or(*hold_ms as f32)),
                        14 => format!("{}", band.bypass),
                        15 => format!("{}", band.solo),
                        _ => "?".to_string(),
                    }
                } else {
                    "?".to_string()
                };

                Some((id, val_str))
            }
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
        } => match param_idx {
            0 => Some(("center_gain_db".to_string(), format!("{}", center_gain_db))),
            1 => Some((
                "surround_gain_db".to_string(),
                format!("{}", surround_gain_db),
            )),
            2 => Some(("height_gain_db".to_string(), format!("{}", height_gain_db))),
            3 => Some(("lfe_gain_db".to_string(), format!("{}", lfe_gain_db))),
            4 => Some(("phase_coherence".to_string(), phase_coherence.to_string())),
            5 => Some((
                "phase_blend_low_hz".to_string(),
                format!("{}", phase_blend_low_hz),
            )),
            6 => Some((
                "phase_blend_high_hz".to_string(),
                format!("{}", phase_blend_high_hz),
            )),
            _ => None,
        },
        PluginSettings::MonoToStereo {
            stereo_width,
            haas_delay_ms,
            enable_comp_eq,
            comp_eq_depth_db,
            decor_low_hz,
            decor_high_hz,
        } => match param_idx {
            0 => Some(("stereo_width".to_string(), format!("{}", stereo_width))),
            1 => Some(("haas_delay_ms".to_string(), format!("{}", haas_delay_ms))),
            2 => Some(("enable_comp_eq".to_string(), enable_comp_eq.to_string())),
            3 => Some((
                "comp_eq_depth_db".to_string(),
                format!("{}", comp_eq_depth_db),
            )),
            4 => Some(("decor_low_hz".to_string(), format!("{}", decor_low_hz))),
            5 => Some(("decor_high_hz".to_string(), format!("{}", decor_high_hz))),
            _ => None,
        },
        PluginSettings::BandSplit {
            frequency,
            crossover_type,
            ..
        } => match param_idx {
            0 => Some(("frequency".to_string(), format!("{}", frequency))),
            1 => Some(("type".to_string(), crossover_type.clone())),
            _ => None,
        },
        PluginSettings::BandMerge { .. } => {
            // BandMerge changes input channels when 'bands' changes, so return None
            // to force a structural update (rebuild the plugin chain).
            None
        }
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            lookahead_ms,
            soft,
            mix,
        } => match param_idx {
            0 => Some(("threshold".to_string(), format!("{}", threshold_db))),
            1 => Some(("release".to_string(), format!("{}", release_ms))),
            2 => Some(("lookahead".to_string(), format!("{}", lookahead_ms))),
            3 => Some(("soft".to_string(), soft.to_string())),
            4 => Some(("mix".to_string(), format!("{}", mix))),
            _ => None,
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
        } => match param_idx {
            0 => Some(("threshold".to_string(), format!("{}", threshold_db))),
            1 => Some(("ratio".to_string(), format!("{}", ratio))),
            2 => Some(("attack".to_string(), format!("{}", attack_ms))),
            3 => Some(("hold".to_string(), format!("{}", hold_ms))),
            4 => Some(("release".to_string(), format!("{}", release_ms))),
            5 => Some(("mix".to_string(), format!("{}", mix))),
            6 => Some(("link_channels".to_string(), link_channels.to_string())),
            7 => Some((
                "sidechain_hpf_hz".to_string(),
                format!("{}", sidechain_hpf_hz),
            )),
            _ => None,
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
        } => match param_idx {
            0 => Some(("threshold".to_string(), format!("{}", threshold_db))),
            1 => Some(("ratio".to_string(), format!("{}", ratio))),
            2 => Some(("attack".to_string(), format!("{}", attack_ms))),
            3 => Some(("release".to_string(), format!("{}", release_ms))),
            4 => Some(("range".to_string(), format!("{}", range_db))),
            5 => Some(("knee".to_string(), format!("{}", knee_db))),
            6 => Some(("hysteresis".to_string(), format!("{}", hysteresis_db))),
            7 => Some(("hold".to_string(), format!("{}", hold_ms))),
            8 => Some(("mix".to_string(), format!("{}", mix))),
            9 => Some(("link_channels".to_string(), link_channels.to_string())),
            10 => Some((
                "sidechain_hpf_hz".to_string(),
                format!("{}", sidechain_hpf_hz),
            )),
            _ => None,
        },
        PluginSettings::LoudnessCompensation {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            ..
        } => match param_idx {
            0 => Some(("low_freq".to_string(), format!("{}", low_freq))),
            1 => Some(("low_gain".to_string(), format!("{}", low_gain))),
            2 => Some(("high_freq".to_string(), format!("{}", high_freq))),
            3 => Some(("high_gain".to_string(), format!("{}", high_gain))),
            // auto_gain params require structural update (not supported by plugin set_parameter)
            4..=6 => None,
            _ => None,
        },
        PluginSettings::BinauralDecoder {
            externalization,
            ..
        } => match param_idx {
            // sofa_file (0) and input_channels (1) require structural update
            0 | 1 => None,
            // enable_optimization (2) requires structural update (changes processing mode)
            2 => None,
            3 => Some((
                "externalization".to_string(),
                format!("{}", externalization),
            )),
            // near_field_strength (4) not supported by plugin set_parameter
            4 => None,
            _ => None,
        },
        PluginSettings::SpectrumAnalyzer { .. } => {
            // SpectrumAnalyzer has no set_parameter support; all changes require structural update
            None
        }
        PluginSettings::XTC {
            distance_m,
            speaker_angle_deg,
            bypass_xtc_filters,
            bypass_spectral_normalization,
            bypass_neumann_refinement,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            ..
        } => match param_idx {
            0 => Some(("distance_m".to_string(), format!("{}", distance_m))),
            1 => Some((
                "speaker_angle_deg".to_string(),
                format!("{}", speaker_angle_deg),
            )),
            // head_radius, beta params not supported by plugin set_parameter — structural
            9 => Some(("bypass_xtc_filters".to_string(), format!("{}", bypass_xtc_filters))),
            10 => Some(("bypass_spectral_normalization".to_string(), format!("{}", bypass_spectral_normalization))),
            11 => Some(("bypass_neumann_refinement".to_string(), format!("{}", bypass_neumann_refinement))),
            12 => Some(("auto_gain_enabled".to_string(), format!("{}", auto_gain_enabled))),
            13 => Some(("auto_gain_max_db".to_string(), format!("{}", auto_gain_max_db))),
            14 => Some(("auto_gain_smoothing_ms".to_string(), format!("{}", auto_gain_smoothing_ms))),
            _ => None,
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
        } => match param_idx {
            0 => Some(("mix".to_string(), format!("{}", mix))),
            1 => Some(("mix_mode".to_string(), format!("{}", mix_mode))),
            2 => Some(("selected_path".to_string(), format!("{}", selected_path))),
            3 => Some(("bypass".to_string(), bypass.to_string())),
            4 => Some((
                "auto_gain_enabled".to_string(),
                auto_gain_enabled.to_string(),
            )),
            5 => Some(("loudness_type".to_string(), format!("{}", loudness_type))),
            6 => Some((
                "max_auto_gain_db".to_string(),
                format!("{}", max_auto_gain_db),
            )),
            7 => Some((
                "gain_smoothing_ms".to_string(),
                format!("{}", gain_smoothing_ms),
            )),
            8 => Some((
                "mix_transition_ms".to_string(),
                format!("{}", mix_transition_ms),
            )),
            _ => None,
        },
        // ChannelMuteSolo and Matrix: structural (grid-based editing)
        PluginSettings::ChannelMuteSolo { .. } | PluginSettings::Matrix { .. } => None,
        // LoudnessMonitor: no parameters
        PluginSettings::LoudnessMonitor => None,
        PluginSettings::Crossfeed {
            mode: _,
            preset: _,
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
        } => match param_idx {
            // mode (0) and preset (1) require structural update (changes processing mode)
            0 | 1 => None,
            2 => Some(("enabled".to_string(), format!("{}", enabled))),
            3 => Some(("mix".to_string(), format!("{}", mix))),
            4 => Some(("bauer_fcut_hz".to_string(), format!("{}", bauer_fcut_hz))),
            5 => Some(("bauer_feed_db".to_string(), format!("{}", bauer_feed_db))),
            6 => Some(("meier_level".to_string(), format!("{}", meier_level))),
            7 => Some(("mb_low_freq_hz".to_string(), format!("{}", mb_low_freq_hz))),
            8 => Some((
                "mb_mid_high_freq_hz".to_string(),
                format!("{}", mb_mid_high_freq_hz),
            )),
            9 => Some(("mb_low_feed_db".to_string(), format!("{}", mb_low_feed_db))),
            10 => Some(("mb_mid_feed_db".to_string(), format!("{}", mb_mid_feed_db))),
            11 => Some((
                "mb_high_feed_db".to_string(),
                format!("{}", mb_high_feed_db),
            )),
            12 => Some((
                "autogain_enabled".to_string(),
                format!("{}", autogain_enabled),
            )),
            13 => Some((
                "autogain_target_lufs".to_string(),
                format!("{}", autogain_target_lufs),
            )),
            14 => Some((
                "autogain_max_gain_db".to_string(),
                format!("{}", autogain_max_gain_db),
            )),
            15 => Some((
                "autogain_smoothing_ms".to_string(),
                format!("{}", autogain_smoothing_ms),
            )),
            _ => None,
        },
    }
}

/// Render a parameter row with name and value
pub fn render_param_row(
    name: &str,
    value: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(if is_selected {
            theme.accent_muted
        } else {
            theme.surface
        })
        .border_l_4()
        .border_color(if is_selected {
            theme.accent
        } else {
            theme.surface
        })
        // Parameter name
        .child(
            div()
                .text_sm()
                .text_color(if is_selected {
                    theme.text_primary
                } else {
                    theme.text_secondary
                })
                .font_weight(if is_selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .child(name.to_string()),
        )
        // Value
        .child(
            div()
                .min_w(px(80.0))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if is_selected {
                    theme.background
                } else {
                    theme.background_secondary
                })
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(value.to_string()),
        )
}

/// Render a section header (with bottom margin - use for bordered sections)
pub fn render_section_header(title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_primary)
        .mb_2()
        .child(title.to_string())
}

/// Render a compact section title (no margin - use for borderless layouts)
pub fn render_section_title(title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_secondary)
        .child(title.to_string())
}

/// Trait extension for applying parameter section styling to any Div
pub trait ParamSectionStyle {
    /// Apply base param section styling (rounded, background, border) without padding
    fn param_section_base(self, theme: &Theme) -> Self;
    /// Apply param section styling with standard p_3 padding
    fn param_section_style(self, theme: &Theme) -> Self;
    /// Apply param section styling with larger p_4 padding
    fn param_section_style_lg(self, theme: &Theme) -> Self;
}

impl ParamSectionStyle for Div {
    fn param_section_base(self, theme: &Theme) -> Self {
        self.rounded_xl()
            .bg(theme.background_secondary)
            .border_1()
            .border_color(theme.border)
    }

    fn param_section_style(self, theme: &Theme) -> Self {
        self.param_section_base(theme).p_3()
    }

    fn param_section_style_lg(self, theme: &Theme) -> Self {
        self.param_section_base(theme).p_4()
    }
}

/// Create a new parameter section container with flex column layout
pub fn render_param_section(theme: &Theme) -> Div {
    div().flex().flex_col().gap_2().param_section_style(theme)
}

/// Create a new parameter section container with flex column layout and larger padding
pub fn render_param_section_lg(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .param_section_style_lg(theme)
}

/// Render keyboard hints for edit mode
pub fn render_edit_hints(theme: &Theme) -> impl IntoElement {
    div()
        .mt_4()
        .p_3()
        .rounded_lg()
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .flex()
        .gap_4()
        .text_xs()
        .text_color(theme.text_muted)
        .child("↑/↓: Select")
        .child("←/→: Adjust")
        .child("[/]: Large step")
        .child("Enter: Done")
}

/// Render a toggle button using gpui-ui-kit Toggle component
/// Uses Entity<AppState> for direct state updates
pub fn render_toggle(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    Toggle::new(("toggle", plugin_idx * 1000 + idx))
        .checked(enabled)
        .label(label.to_string())
        .style(ToggleStyle::Segmented)
        .selected(is_selected)
        .theme(theme.to_toggle_theme())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, if new_value { 1.0 } else { 0.0 });
                });
            }
        })
}

/// Render a toggle button without label (just the switch)
/// Uses Entity<AppState> for direct state updates
pub fn render_toggle_button(
    entity: Entity<AppState>,
    plugin_idx: usize,
    enabled: bool,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    Toggle::new(("toggle-btn", plugin_idx * 1000 + idx))
        .checked(enabled)
        .style(ToggleStyle::Segmented)
        .selected(is_selected)
        .theme(theme.to_toggle_theme())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state
                        .app
                        .set_plugin_param(plugin_idx, idx, if new_value { 1.0 } else { 0.0 });
                });
            }
        })
}

/// Render a value with unit and color coding
pub fn render_colored_value(
    value: f64,
    unit: &str,
    zero_is_neutral: bool,
    theme: &Theme,
) -> impl IntoElement {
    let color = if zero_is_neutral {
        if value > 0.5 {
            theme.success // Green for positive
        } else if value < -0.5 {
            theme.error // Red for negative
        } else {
            theme.text_muted
        }
    } else {
        theme.text_primary
    };

    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(format!("{:+.1}{}", value, unit))
}

/// Format a label with keyboard shortcut indicator
/// e.g., "Threshold" with key 't' -> "[T]hreshold"
pub fn format_shortcut_label(label: &str, shortcut_key: Option<char>) -> String {
    match shortcut_key {
        Some(key) => {
            let key_lower = key.to_ascii_lowercase();
            let label_lower = label.to_lowercase();
            if let Some(pos) = label_lower.find(key_lower) {
                format!(
                    "{}[{}]{}",
                    &label[..pos],
                    label.chars().nth(pos).unwrap().to_ascii_uppercase(),
                    &label[pos + 1..]
                )
            } else {
                format!("[{}] {}", key.to_ascii_uppercase(), label)
            }
        }
        None => label.to_string(),
    }
}

/// Convert Theme to VerticalSliderTheme for gpui-ui-kit VerticalSlider
fn theme_to_vertical_slider_theme(theme: &Theme) -> VerticalSliderTheme {
    VerticalSliderTheme {
        surface: theme.surface,
        surface_hover: theme.surface_hover,
        track_bg: theme.background,
        accent: theme.accent,
        accent_muted: theme.accent_muted,
        border: theme.border,
        text_secondary: theme.text_secondary,
        text_primary: theme.text_primary,
        text_muted: theme.text_muted,
        text_on_accent: theme.text_on_accent,
        background_secondary: theme.background_secondary,
        peak_marker: theme.meter_colors.peak,
    }
}

/// Render a vertical slider with label, value, drag support and enhanced visual feedback
/// Uses Entity<AppState> for direct state updates
pub fn render_vertical_slider(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    shortcut_key: Option<char>,
    theme: &Theme,
) -> impl IntoElement {
    render_vertical_slider_sized(
        entity,
        plugin_idx,
        label,
        value,
        min,
        max,
        unit,
        idx,
        selected_param,
        is_editing,
        shortcut_key,
        None,
        theme,
    )
}

/// Render a vertical slider with custom height
/// Uses Entity<AppState> for direct state updates
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_slider_sized(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    shortcut_key: Option<char>,
    height: Option<f32>,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    let mut slider = VerticalSlider::new(("slider", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .selected(is_selected)
        .theme(theme_to_vertical_slider_theme(theme));

    if let Some(key) = shortcut_key {
        slider = slider.shortcut_key(key);
    }

    div().key_context("plugin-control").child(slider)
}

/// Render a vertical slider with tick marks, custom height, and enhanced visual feedback
/// Uses Entity<AppState> for direct state updates
#[allow(clippy::too_many_arguments)]
pub fn render_vertical_slider_with_ticks(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    shortcut_key: Option<char>,
    height: f32,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    let mut slider = VerticalSlider::new(("slider-ticks", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .height(height)
        .with_ticks()
        .selected(is_selected)
        .theme(theme_to_vertical_slider_theme(theme))
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.is_dragging_knob = true;
                    state.app.knob_drag_plugin_idx = plugin_idx;
                    state.app.knob_drag_param_idx = idx;
                    state.app.knob_drag_start_y = Some(start_y);
                    state.app.knob_drag_start_value = start_value;
                    state.app.knob_drag_min = min;
                    state.app.knob_drag_max = max;
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                });
            }
        });

    if let Some(key) = shortcut_key {
        slider = slider.shortcut_key(key);
    }

    div().key_context("plugin-control").child(slider)
}

/// Render a simple transfer curve visualization (input vs output)
pub fn render_transfer_curve(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    theme: &Theme,
) -> impl IntoElement {
    render_transfer_curve_sized(threshold_db, ratio, knee_db, is_limiter, 100.0, theme)
}

/// Render a transfer curve visualization with custom width
pub fn render_transfer_curve_sized(
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
    width: f32,
    theme: &Theme,
) -> impl IntoElement {
    let curve_width = width;
    let num_points = 20;

    // Generate curve points
    let mut bars: Vec<(f32, Rgba)> = Vec::new();

    for i in 0..num_points {
        let input_db = -60.0 + (i as f64 / num_points as f64) * 60.0; // -60 to 0 dB
        let output_db = if is_limiter {
            // Limiter: hard clip at threshold
            input_db.min(threshold_db)
        } else {
            // Compressor: soft knee compression
            if input_db < threshold_db - knee_db / 2.0 {
                input_db
            } else if input_db > threshold_db + knee_db / 2.0 {
                threshold_db + (input_db - threshold_db) / ratio
            } else {
                // Knee region
                let knee_input = input_db - (threshold_db - knee_db / 2.0);
                let knee_ratio = knee_input / knee_db;
                input_db + (knee_ratio * knee_ratio / 2.0) * (1.0 / ratio - 1.0) * knee_db
            }
        };

        let height = ((output_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
        let is_compressed = output_db < input_db - 0.5;
        let color = if is_compressed {
            theme.meter_clip // Red for compressed region
        } else {
            theme.accent // Blue for linear region
        };
        bars.push((height, color));
    }

    div()
        .flex()
        .flex_col()
        .flex_1() // Grow to fill available space
        .gap_2()
        // Title
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .text_center()
                .child("Transfer Curve"),
        )
        // Curve visualization - grows to fill space
        .child(
            div()
                .w(px(curve_width))
                .flex_1() // Grow to fill available space
                .min_h(px(60.0)) // Minimum height
                .bg(theme.background)
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .p_1()
                .flex()
                .items_end()
                .gap_px()
                .children(bars.into_iter().map(|(height, color)| {
                    div().flex_1().h(relative(height)).bg(color).rounded_t_sm()
                })),
        )
        // Axis labels
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .child("-60")
                .child("Input (dB)")
                .child("0"),
        )
}

/// Render a rotary knob control using gpui-ui-kit Potentiometer
/// Uses Entity<AppState> for direct state updates
pub fn render_knob(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    shortcut_key: Option<char>,
    theme: &Theme,
) -> impl IntoElement {
    render_knob_sized(
        entity,
        plugin_idx,
        label,
        value,
        min,
        max,
        unit,
        idx,
        selected_param,
        is_editing,
        shortcut_key,
        PotentiometerSize::Md,
        theme,
    )
}

/// Render a rotary knob control with custom size
pub fn render_knob_sized(
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    idx: usize,
    selected_param: usize,
    is_editing: bool,
    shortcut_key: Option<char>,
    size: PotentiometerSize,
    theme: &Theme,
) -> impl IntoElement {
    let is_selected = selected_param == idx && is_editing;

    // Determine scale type based on unit (Hz parameters use logarithmic scale)
    let scale = if unit == "Hz" {
        PotentiometerScale::Logarithmic
    } else {
        PotentiometerScale::Linear
    };

    let mut knob = Potentiometer::new(("knob", plugin_idx * 1000 + idx))
        .value(value)
        .min(min)
        .max(max)
        .unit(unit.to_string())
        .label(label.to_string())
        .size(size)
        .scale(scale)
        .selected(is_selected)
        .theme(theme.to_potentiometer_theme())
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.set_plugin_param(plugin_idx, idx, new_value);
                });
            }
        })
        .on_drag_start({
            let entity = entity.clone();
            move |start_y, start_value, _, cx| {
                entity.update(cx, |state, _| {
                    state.app.is_dragging_knob = true;
                    state.app.knob_drag_plugin_idx = plugin_idx;
                    state.app.knob_drag_param_idx = idx;
                    state.app.knob_drag_start_y = Some(start_y);
                    state.app.knob_drag_start_value = start_value;
                    state.app.knob_drag_min = min;
                    state.app.knob_drag_max = max;
                });
            }
        })
        .on_select({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = idx;
                });
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |state, _| {
                    state.app.reset_plugin_param(plugin_idx, idx);
                });
            }
        });

    if let Some(key) = shortcut_key {
        knob = knob.shortcut_key(key);
    }

    div().key_context("plugin-control").child(knob)
}
