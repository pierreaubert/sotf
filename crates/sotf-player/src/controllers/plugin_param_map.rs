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
