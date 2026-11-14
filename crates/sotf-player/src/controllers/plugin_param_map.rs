//! Maps UI parameter indices to engine parameter IDs and formatted value strings.
//!
//! This enables zero-dropout parameter updates for plugins that support it.
//! Most plugins use the generic `engine_param_at()` path which derives the mapping
//! from `ParamSpec` arrays. Sparse multiband IDs need manual handling, and
//! EQ/FIR band-local IDs must bypass the overlapping static global specs.

use crate::PluginSettings;

/// Momentary profile controls are named host commands, not persisted toggles.
/// Their indexed DSP setters deliberately do not fire the action.
#[must_use]
pub fn momentary_action_param(settings: &PluginSettings, param_idx: usize) -> Option<&'static str> {
    match settings {
        PluginSettings::Denoiser { .. } => match param_idx {
            20 => Some("learn_noise"),
            22 => Some("clear_profile"),
            _ => None,
        },
        PluginSettings::HissReducer { .. } => match param_idx {
            5 => Some("learn_noise"),
            7 => Some("clear_profile"),
            _ => None,
        },
        _ => None,
    }
}

/// Map a UI parameter index to the engine parameter ID and formatted value string.
/// Returns None if this parameter requires a Structural update (e.g., EQ, channel count changes).
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "Parameter indices map bounded values to the engine single-precision interface"
)]
#[expect(
    clippy::too_many_lines,
    reason = "Exhaustive plugin parameter dispatch stays together for schema review"
)]
pub fn param_index_to_engine_param(
    settings: &PluginSettings,
    param_idx: usize,
) -> Option<(String, String)> {
    if momentary_action_param(settings, param_idx).is_some() {
        return None;
    }
    match settings {
        // Independent channel edits reconstruct the crossover's filter bank.
        PluginSettings::Crossover { .. }
            if param_idx >= sotf_plugins::param_specs::crossover::CHANNEL_PARAM_START =>
        {
            None
        }
        // The single-band Expander's host parameter is numeric, while its UI
        // settings and generic choice mapping use "Peak"/"RMS" strings.
        // Rebuild until that host parameter contract is unified, otherwise a
        // realtime command fails with Parameter type mismatch after the UI
        // already saved the new choice.
        PluginSettings::Expander { .. }
            if param_idx
                == sotf_plugins::param_specs::index_of(
                    sotf_plugins::param_specs::expander::PARAMS,
                    "detection_mode",
                ) =>
        {
            None
        }
        // EQ editors use band-local numeric IDs. Those IDs overlap unrelated
        // entries in the static DSP ParamSpec arrays (for example FIR band 0
        // active is index 4, while the engine spec at 4 is global mix).
        // Rebuild the exact edited graph until a dedicated band ID protocol
        // exists; never dispatch the coincidentally indexed global parameter.
        PluginSettings::EQ { .. } | PluginSettings::LinearPhaseEq { .. } => None,
        // MultibandCompressor: band-level params (idx >= 100) need manual handling
        PluginSettings::MultibandCompressor {
            num_bands,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            range_db,
            hold_ms,
            bands,
            ..
        } if param_idx >= 100 => {
            let band_idx = param_idx / 100;
            let local_idx = param_idx % 100;
            let band_zero_based = band_idx - 1;

            if band_zero_based >= *num_bands {
                return None;
            }
            let param_name = match local_idx {
                6 => "threshold",
                7 => "ratio",
                8 => "attack",
                9 => "release",
                10 => "knee",
                13 => "makeup",
                14 => "bypass",
                15 => "solo",
                18 => "range_db",
                19 => "hold_ms",
                _ => return None,
            };

            let id = format!("band_{band_zero_based}_{param_name}");

            let inherited = sotf_plugins::BandCompressorParams::default();
            let band = bands.get(band_zero_based).unwrap_or(&inherited);
            let val_str = {
                match local_idx {
                    6 => format!("{:?}", band.threshold_db.unwrap_or(*threshold_db as f32)),
                    7 => format!("{:?}", band.ratio.unwrap_or(*ratio as f32)),
                    8 => format!("{:?}", band.attack_ms.unwrap_or(*attack_ms as f32)),
                    9 => format!("{:?}", band.release_ms.unwrap_or(*release_ms as f32)),
                    10 => format!("{:?}", band.knee_db.unwrap_or(*knee_db as f32)),
                    13 => format!("{:?}", band.makeup_gain_db),
                    14 => format!("{}", band.bypass),
                    15 => format!("{}", band.solo),
                    18 => format!("{:?}", band.range_db.unwrap_or(*range_db as f32)),
                    19 => format!("{:?}", band.hold_ms.unwrap_or(*hold_ms as f32)),
                    _ => return None,
                }
            };

            Some((id, val_str))
        }
        // MultibandExpander: band-level params (idx >= 100) need manual handling
        PluginSettings::MultibandExpander {
            num_bands,
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

            if band_zero_based >= *num_bands {
                return None;
            }
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

            let id = format!("band_{band_zero_based}_{param_name}");

            let inherited = sotf_plugins::BandExpanderParams::default();
            let band = bands.get(band_zero_based).unwrap_or(&inherited);
            let val_str = {
                match local_idx {
                    6 => format!("{:?}", band.threshold_db.unwrap_or(*threshold_db as f32)),
                    7 => format!("{:?}", band.ratio.unwrap_or(*ratio as f32)),
                    8 => format!("{:?}", band.attack_ms.unwrap_or(*attack_ms as f32)),
                    9 => format!("{:?}", band.release_ms.unwrap_or(*release_ms as f32)),
                    10 => format!("{:?}", band.range_db.unwrap_or(*range_db as f32)),
                    11 => format!("{:?}", band.knee_db.unwrap_or(*knee_db as f32)),
                    12 => format!("{:?}", band.hysteresis_db.unwrap_or(*hysteresis_db as f32)),
                    13 => format!("{:?}", band.hold_ms.unwrap_or(*hold_ms as f32)),
                    14 => format!("{}", band.bypass),
                    15 => format!("{}", band.solo),
                    _ => return None,
                }
            };

            Some((id, val_str))
        }
        // Generic: all other plugins derive mapping from ParamSpec arrays.
        _ => settings.engine_param_at(param_idx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_plugins::{
        BandCompressorParams, BandExpanderParams, DynEqBandParams, MultibandCompressorPlugin,
        ParameterId, ParameterValue,
    };

    #[test]
    fn spectrum_frequency_grid_edits_rebuild_while_smoothing_stays_live() {
        let mut settings =
            PluginSettings::default_for(&crate::PluginType::SpectrumAnalyzer).unwrap();
        for (index, value) in [(0, 64.0), (1, 80.0), (2, 12000.0)] {
            settings.set_param_value(index, value);
            assert!(settings.engine_param_at(index).is_none());
            assert!(param_index_to_engine_param(&settings, index).is_none());
        }
        let (key, _) = param_index_to_engine_param(&settings, 3).unwrap();
        assert_eq!(key, "smoothing");
    }

    #[test]
    fn expander_detection_choice_uses_structural_route_until_host_types_match() {
        let mut settings = PluginSettings::default_for(&crate::PluginType::Expander).unwrap();
        let idx = sotf_plugins::param_specs::index_of(
            sotf_plugins::param_specs::expander::PARAMS,
            "detection_mode",
        );
        let PluginSettings::Expander { detection_mode, .. } = &mut settings else {
            unreachable!()
        };
        *detection_mode = "RMS".to_string();
        assert!(param_index_to_engine_param(&settings, idx).is_none());
        assert!(param_index_to_engine_param(&settings, 0).is_some());
    }

    #[test]
    fn linear_phase_eq_band_active_does_not_alias_global_mix() {
        let settings = PluginSettings::default_for(&crate::PluginType::LinearPhaseEq).unwrap();
        assert!(settings.engine_param_at(4).is_some());
        assert!(param_index_to_engine_param(&settings, 4).is_none());
        assert!(param_index_to_engine_param(&settings, 9).is_none());
    }

    fn mb_compressor_settings(bands: Vec<BandCompressorParams>) -> PluginSettings {
        PluginSettings::MultibandCompressor {
            num_bands: bands.len(),
            crossover_preset: 0,
            crossover_freq_1: 200.0,
            crossover_freq_2: 1000.0,
            crossover_freq_3: 4000.0,
            crossover_freq_4: 8000.0,
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            mix: 1.0,
            link_channels: false,
            per_band_lookahead_ms: 0.0,
            ms_mode: false,
            bands,
            sidechain_tilt_db: 0.0,
            link_amount: 0.0,
            range_db: 120.0,
            hold_ms: 0.0,
            sidechain_hpf_hz: 80.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "Peak".to_string(),
            sidechain_hpf_enabled: false,
        }
    }

    fn mb_expander_settings(bands: Vec<BandExpanderParams>) -> PluginSettings {
        PluginSettings::MultibandExpander {
            num_bands: bands.len(),
            crossover_preset: 0,
            crossover_freq_1: 200.0,
            crossover_freq_2: 1000.0,
            crossover_freq_3: 4000.0,
            crossover_freq_4: 8000.0,
            threshold_db: -30.0,
            ratio: 2.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            range_db: 40.0,
            knee_db: 6.0,
            hysteresis_db: 3.0,
            hold_ms: 20.0,
            mix: 1.0,
            link_channels: false,
            detection_mode: "rms".to_string(),
            lookahead_ms: 0.0,
            bands,
        }
    }

    #[test]
    fn param_index_to_engine_param_round_trips_multiband_compressor_band() {
        let settings = mb_compressor_settings(vec![
            BandCompressorParams {
                threshold_db: Some(-18.0),
                ratio: Some(3.0),
                attack_ms: Some(8.0),
                release_ms: Some(80.0),
                knee_db: Some(5.0),
                makeup_gain_db: 1.5,
                auto_makeup: false,
                measured_auto_makeup: false,
                active: true,
                solo: false,
                bypass: false,
                range_db: None,
                hold_ms: None,
            },
            BandCompressorParams {
                threshold_db: Some(-22.0),
                ratio: Some(5.0),
                attack_ms: Some(12.0),
                release_ms: Some(120.0),
                knee_db: Some(7.0),
                makeup_gain_db: 2.0,
                auto_makeup: false,
                measured_auto_makeup: false,
                active: true,
                solo: false,
                bypass: true,
                range_db: None,
                hold_ms: None,
            },
        ]);

        // Band 1 (idx 100): threshold=6, ratio=7, attack=8, release=9, knee=10, makeup=13, bypass=14, solo=15
        assert_eq!(
            param_index_to_engine_param(&settings, 106),
            Some(("band_0_threshold".to_string(), "-18.0".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 107),
            Some(("band_0_ratio".to_string(), "3.0".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 113),
            Some(("band_0_makeup".to_string(), "1.5".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 114),
            Some(("band_0_bypass".to_string(), "false".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 215),
            Some(("band_1_solo".to_string(), "false".to_string()))
        );
    }

    #[test]
    fn multiband_makeup_mapping_is_accepted_by_the_live_plugin() {
        let settings = mb_compressor_settings(vec![BandCompressorParams {
            makeup_gain_db: 3.0,
            ..BandCompressorParams::default()
        }]);
        let (key, text) = param_index_to_engine_param(&settings, 113).unwrap();
        let value = ParameterValue::parse(&text);
        assert_eq!(value, ParameterValue::Float(3.0));

        let mut plugin = MultibandCompressorPlugin::new(2);
        let id = ParameterId::from(key.as_str());
        plugin.set_parameter(id.clone(), value.clone()).unwrap();
        assert_eq!(plugin.get_parameter(&id).unwrap(), value);
    }

    #[test]
    fn multiband_range_and_hold_mapping_reaches_the_live_plugin() {
        let settings = mb_compressor_settings(vec![
            BandCompressorParams {
                range_db: Some(80.0),
                hold_ms: Some(25.0),
                ..BandCompressorParams::default()
            },
            BandCompressorParams::default(),
        ]);
        let mut plugin = MultibandCompressorPlugin::new(2);
        for (index, expected_key, expected) in [
            (118, "band_0_range_db", 80.0),
            (119, "band_0_hold_ms", 25.0),
            (218, "band_1_range_db", 120.0),
            (219, "band_1_hold_ms", 0.0),
        ] {
            let (key, text) = param_index_to_engine_param(&settings, index).unwrap();
            assert_eq!(key, expected_key);
            let value = ParameterValue::parse(&text);
            assert_eq!(value, ParameterValue::Float(expected));
            let id = ParameterId::from(key.as_str());
            plugin.set_parameter(id.clone(), value.clone()).unwrap();
            assert_eq!(plugin.get_parameter(&id).unwrap(), value);
        }
    }

    #[test]
    fn param_index_to_engine_param_round_trips_multiband_expander_band() {
        let settings = mb_expander_settings(vec![BandExpanderParams {
            threshold_db: Some(-25.0),
            ratio: Some(2.5),
            attack_ms: Some(6.0),
            release_ms: Some(60.0),
            range_db: Some(35.0),
            knee_db: Some(5.0),
            hysteresis_db: Some(2.0),
            hold_ms: Some(15.0),
            auto_makeup: false,
            measured_auto_makeup: false,
            active: true,
            solo: true,
            bypass: false,
        }]);

        assert_eq!(
            param_index_to_engine_param(&settings, 106),
            Some(("band_0_threshold".to_string(), "-25.0".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 112),
            Some(("band_0_hysteresis".to_string(), "2.0".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 115),
            Some(("band_0_solo".to_string(), "true".to_string()))
        );
    }

    #[test]
    fn param_index_to_engine_param_unknown_band_is_rejected() {
        let settings = mb_compressor_settings(vec![BandCompressorParams {
            threshold_db: None,
            ratio: None,
            attack_ms: None,
            release_ms: None,
            knee_db: None,
            makeup_gain_db: 0.0,
            auto_makeup: false,
            measured_auto_makeup: false,
            active: true,
            solo: false,
            bypass: false,
            range_db: None,
            hold_ms: None,
        }]);

        // Band 1 does not exist → id is still generated but value is a placeholder
        assert_eq!(param_index_to_engine_param(&settings, 206), None);
    }

    #[test]
    fn sparse_multiband_parameters_send_inherited_values() {
        for kind in [
            crate::PluginType::MultibandCompressor,
            crate::PluginType::MultibandExpander,
        ] {
            let mut settings = PluginSettings::default_for(&kind).unwrap();
            settings.set_param_value(6, -25.0);
            for band in 1..=3 {
                assert_eq!(
                    param_index_to_engine_param(&settings, band * 100 + 6),
                    Some((format!("band_{}_threshold", band - 1), "-25.0".into()))
                );
                assert_eq!(
                    param_index_to_engine_param(&settings, band * 100 + 14),
                    Some((format!("band_{}_bypass", band - 1), "false".into()))
                );
            }
            assert_eq!(param_index_to_engine_param(&settings, 406), None);
            assert_eq!(param_index_to_engine_param(&settings, 199), None);
        }
    }

    #[test]
    fn multiband_float_values_parse_as_host_floats() {
        use sotf_plugins::ParameterValue;

        for kind in [
            crate::PluginType::MultibandCompressor,
            crate::PluginType::MultibandExpander,
        ] {
            let mut inherited = PluginSettings::default_for(&kind).unwrap();
            inherited.set_param_value(7, 3.0);
            let (_, value) = param_index_to_engine_param(&inherited, 107).unwrap();
            assert_eq!(ParameterValue::parse(&value), ParameterValue::Float(3.0));
            let (_, bypass) = param_index_to_engine_param(&inherited, 114).unwrap();
            assert_eq!(ParameterValue::parse(&bypass), ParameterValue::Bool(false));
        }

        let compressor = mb_compressor_settings(vec![BandCompressorParams {
            ratio: Some(3.0),
            attack_ms: Some(15.0),
            ..Default::default()
        }]);
        let expander = mb_expander_settings(vec![BandExpanderParams {
            ratio: Some(3.0),
            hold_ms: Some(20.0),
            ..Default::default()
        }]);
        for (settings, indices) in [(compressor, [107, 108]), (expander, [107, 113])] {
            for index in indices {
                let (_, value) = param_index_to_engine_param(&settings, index).unwrap();
                assert!(matches!(
                    ParameterValue::parse(&value),
                    ParameterValue::Float(_)
                ));
            }
        }
    }

    #[test]
    fn param_index_to_engine_param_falls_back_to_engine_param_for_generic_plugins() {
        let settings = PluginSettings::Gain {
            channels: 2,
            gain_db: -6.0,
            smoothing_ms: 20.0,
        };

        // Gain plugin has a generic param spec; param 0 is gain_db
        let result = param_index_to_engine_param(&settings, 0);
        assert!(result.is_some());
        let (id, value) = result.unwrap();
        assert_eq!(id, "gain_db");
        assert!((value.parse::<f64>().unwrap() - -6.0).abs() < 0.01);
    }

    #[test]
    fn param_index_to_engine_param_dynamic_eq_band_level_returns_none() {
        // DynamicEq band-level params are handled by adjust/set, not this mapper
        let settings = PluginSettings::DynamicEq {
            num_bands: 1.0,
            threshold: -20.0,
            ratio: 2.0,
            attack: 10.0,
            release: 100.0,
            knee: 6.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                shape: Default::default(),
                placement: Default::default(),
                shelf_slope: 1.0,
                frequency: 1000.0,
                q: 1.0,
                gain: 0.0,
                band_threshold: -12.0,
                band_ratio: 2.0,
                active: true,
                solo: false,
            }],
            stereo_pairs: None,
        };

        assert!(param_index_to_engine_param(&settings, 0).is_none());
        assert!(param_index_to_engine_param(&settings, 100).is_none());
    }
}
