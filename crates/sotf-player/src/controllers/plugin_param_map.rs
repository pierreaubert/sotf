//! Maps UI parameter indices to engine parameter IDs and formatted value strings.
//!
//! This enables zero-dropout parameter updates for plugins that support it.
//! Most plugins use the generic `engine_param_at()` path which derives the mapping
//! from `ParamSpec` arrays. Only plugins with PARAMS ordering mismatches or special
//! band-level encoding (idx >= 100) need manual handling.

use crate::PluginSettings;

/// Map a UI parameter index to the engine parameter ID and formatted value string.
/// Returns None if this parameter requires a Structural update (e.g., EQ, channel count changes).
pub fn param_index_to_engine_param(
    settings: &PluginSettings,
    param_idx: usize,
) -> Option<(String, String)> {
    match settings {
        // Upmixer: PARAMS ordering differs from GPUI ordering — keep manual mapping
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
            dialogue_centroid_weight,
            dialogue_variance_weight,
            dialogue_coherence_weight,
            bypass_decorrelation,
            bypass_transient_detection,
            bypass_all_processing,
            enable_ml_detection,
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
            28 => Some(("ambient_boost".to_string(), format!("{}", ambient_boost))),
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
            32 => Some((
                "dialogue_centroid_weight".to_string(),
                format!("{}", dialogue_centroid_weight),
            )),
            33 => Some((
                "dialogue_variance_weight".to_string(),
                format!("{}", dialogue_variance_weight),
            )),
            34 => Some((
                "dialogue_coherence_weight".to_string(),
                format!("{}", dialogue_coherence_weight),
            )),
            35 => Some((
                "bypass_decorrelation".to_string(),
                bypass_decorrelation.to_string(),
            )),
            36 => Some((
                "bypass_transient_detection".to_string(),
                bypass_transient_detection.to_string(),
            )),
            37 => Some((
                "bypass_all_processing".to_string(),
                bypass_all_processing.to_string(),
            )),
            38 => Some((
                "enable_ml_detection".to_string(),
                enable_ml_detection.to_string(),
            )),
            _ => None,
        },
        // FletcherMunson: PARAMS ordering differs from GPUI ordering — keep manual mapping
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
        // MultibandCompressor: band-level params (idx >= 100) need manual handling
        PluginSettings::MultibandCompressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            bands,
            ..
        } if param_idx >= 100 => {
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
        // MultibandExpander: band-level params (idx >= 100) need manual handling
        PluginSettings::MultibandExpander {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            range_db,
            knee_db,
            hysteresis_db,
            hold_ms,
            bands,
            ..
        } if param_idx >= 100 => {
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
        // Generic: all other plugins derive mapping from ParamSpec arrays.
        _ => settings.engine_param_at(param_idx),
    }
}
