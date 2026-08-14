//! Maps UI parameter indices to engine parameter IDs and formatted value strings.
//!
//! This enables zero-dropout parameter updates for plugins that support it.
//! Most plugins use the generic `engine_param_at()` path which derives the mapping
//! from `ParamSpec` arrays. Only plugins with special band-level encoding
//! (idx >= 100) need manual handling.

use crate::PluginSettings;

/// Map a UI parameter index to the engine parameter ID and formatted value string.
/// Returns None if this parameter requires a Structural update (e.g., EQ, channel count changes).
pub fn param_index_to_engine_param(
    settings: &PluginSettings,
    param_idx: usize,
) -> Option<(String, String)> {
    match settings {
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

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_plugins::{BandCompressorParams, BandExpanderParams, DynEqBandParams};

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
            },
        ]);

        // Band 1 (idx 100): threshold=6, ratio=7, attack=8, release=9, knee=10, makeup=13, bypass=14, solo=15
        assert_eq!(
            param_index_to_engine_param(&settings, 106),
            Some(("band_0_threshold".to_string(), "-18".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 107),
            Some(("band_0_ratio".to_string(), "3".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 113),
            Some(("band_0_makeup_gain".to_string(), "1.5".to_string()))
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
            Some(("band_0_threshold".to_string(), "-25".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 112),
            Some(("band_0_hysteresis".to_string(), "2".to_string()))
        );
        assert_eq!(
            param_index_to_engine_param(&settings, 115),
            Some(("band_0_solo".to_string(), "true".to_string()))
        );
    }

    #[test]
    fn param_index_to_engine_param_unknown_band_returns_placeholder() {
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
        }]);

        // Band 1 does not exist → id is still generated but value is a placeholder
        assert_eq!(
            param_index_to_engine_param(&settings, 206),
            Some(("band_1_threshold".to_string(), "?".to_string()))
        );
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
                frequency: 1000.0,
                q: 1.0,
                gain: 0.0,
                band_threshold: -12.0,
                band_ratio: 2.0,
                active: true,
                solo: false,
            }],
        };

        assert!(param_index_to_engine_param(&settings, 0).is_none());
        assert!(param_index_to_engine_param(&settings, 100).is_none());
    }
}
