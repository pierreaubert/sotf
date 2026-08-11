use super::misc::apply_structural_side_effects;
use super::misc::eq_band_types;
use crate::{EQFilter, PluginSettings};

pub(super) fn adjust_eq_band_field_for_plugin(
    filter_idx: usize,
    field_idx: usize,
    filters: &mut [EQFilter],
    delta: f64,
    allow_extended_types: bool,
) -> bool {
    let Some(filter) = filters.get_mut(filter_idx) else {
        return false;
    };
    if field_idx != 3 {
        return crate::ui_params::apply_eq_band_field(filter, field_idx, delta);
    }

    let types = eq_band_types(allow_extended_types);
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
    // Keep Q within the new type's accepted range.
    filter.q = filter
        .q
        .clamp(0.1, sotf_plugins::param_specs::eq::q_max_for(filter.filter_type));
    true
}

/// Adjust a plugin parameter by delta. Returns true if the parameter was adjusted.
///
/// Most plugins delegate to `PluginSettings::adjust_param_value()` (generic path).
/// Only plugins with side effects beyond simple field updates have manual arms.
pub(super) fn adjust_plugin_param(
    settings: &mut PluginSettings,
    param_idx: usize,
    delta: f64,
    channel_count_changed: &mut bool,
) -> bool {
    match settings {
        // === EQ: dynamic filter array, param indices map to band/field ===
        //
        // Controller index space: idx 0 = band-0-frequency, idx 1 = band-0-q,
        // …, idx 4 = band-1-frequency, … (no `max_filters` slot — that lives
        // separately in the TUI index space, see `ui_params::adjust_param`).
        // Per-field math is shared with `ui_params::apply_eq_band_field` so the
        // two index spaces stay in lockstep when one of them is touched.
        PluginSettings::EQ { filters, .. } => {
            if filters.is_empty() {
                return false;
            }

            let total_params = filters.len() * 4;
            if param_idx >= total_params {
                return false;
            }

            let filter_idx = param_idx / 4;
            let field_idx = param_idx % 4;

            adjust_eq_band_field_for_plugin(filter_idx, field_idx, filters, delta, true)
        }
        PluginSettings::LinearPhaseEq { filters, .. } => {
            if filters.is_empty() {
                return false;
            }

            let total_params = filters.len() * 4;
            if param_idx >= total_params {
                return false;
            }

            let filter_idx = param_idx / 4;
            let field_idx = param_idx % 4;
            adjust_eq_band_field_for_plugin(filter_idx, field_idx, filters, delta, false)
        }
        // === SpectrumAnalyzer: no_params_struct — not in the macro, needs manual handling ===
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
            tilt_correction,
            tilt_reference,
            ..
        } => match param_idx {
            0 => {
                *num_bins = (*num_bins as i64 + delta as i64).clamp(10, 100) as usize;
                true
            }
            1 => {
                *min_freq = (*min_freq + delta as f32).clamp(10.0, 100.0);
                true
            }
            2 => {
                *max_freq = (*max_freq + delta as f32 * 100.0).clamp(1000.0, 24000.0);
                true
            }
            3 => {
                *smoothing = (*smoothing + delta as f32 * 0.01).clamp(0.0, 1.0);
                true
            }
            4 => {
                use sotf_plugins::SpectralTiltCorrection as STC;
                let modes = [
                    STC::None,
                    STC::ThreeDbPerOctave,
                    STC::SixDbPerOctave,
                    STC::Pink,
                ];
                let current = modes.iter().position(|m| m == tilt_correction).unwrap_or(0);
                let next = if delta > 0.0 {
                    (current + 1) % modes.len()
                } else if current == 0 {
                    modes.len() - 1
                } else {
                    current - 1
                };
                *tilt_correction = modes[next];
                true
            }
            5 => {
                use sotf_plugins::TiltReferenceFreq as TRF;
                let modes = [
                    TRF::Standard,
                    TRF::OneKilohertz,
                    TRF::TwoKilohertz,
                    TRF::MinFreq,
                ];
                let current = modes.iter().position(|m| m == tilt_reference).unwrap_or(0);
                let next = if delta > 0.0 {
                    (current + 1) % modes.len()
                } else if current == 0 {
                    modes.len() - 1
                } else {
                    current - 1
                };
                *tilt_reference = modes[next];
                true
            }
            _ => false,
        },
        // === DynamicEq band-level params (idx >= 100) ===
        PluginSettings::DynamicEq { bands, .. } if param_idx >= 100 => {
            let band_idx = (param_idx - 100) / 10;
            let local_idx = (param_idx - 100) % 10;
            if let Some(band) = bands.get_mut(band_idx) {
                use sotf_plugins::param_specs::{dynamic_eq::BAND_PARAMS as BT, find_by_key as p};
                match local_idx {
                    0 => {
                        let s = p(BT, "frequency");
                        band.frequency = s.adjust_f64(band.frequency as f64, delta) as f32;
                        true
                    }
                    1 => {
                        let s = p(BT, "q");
                        band.q = s.adjust_f64(band.q as f64, delta) as f32;
                        true
                    }
                    2 => {
                        let s = p(BT, "gain");
                        band.gain = s.adjust_f64(band.gain as f64, delta) as f32;
                        true
                    }
                    3 => {
                        let s = p(BT, "band_threshold");
                        band.band_threshold =
                            s.adjust_f64(band.band_threshold as f64, delta) as f32;
                        true
                    }
                    4 => {
                        let s = p(BT, "band_ratio");
                        band.band_ratio = s.adjust_f64(band.band_ratio as f64, delta) as f32;
                        true
                    }
                    5 => {
                        band.active = !band.active;
                        true
                    }
                    6 => {
                        band.solo = !band.solo;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === MultibandCompressor band-level params (idx >= 100) ===
        PluginSettings::MultibandCompressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            bands,
            ..
        } if param_idx >= 100 => {
            use sotf_plugins::param_specs::{
                find_by_key as p, multiband_compressor::BAND_TEMPLATE as BT,
            };
            macro_rules! band_adj {
                ($field:expr, $global:expr, $key:literal, $step:expr) => {{
                    let spec = p(BT, $key);
                    $field = match $field {
                        None => Some(*$global as f32),
                        Some(v) => {
                            let new_v = v + $step;
                            if new_v < spec.min_f64() as f32 {
                                None
                            } else {
                                Some(new_v.clamp(spec.min_f64() as f32, spec.max_f64() as f32))
                            }
                        }
                    };
                    true
                }};
            }
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => band_adj!(band.threshold_db, threshold_db, "threshold", delta as f32),
                    7 => band_adj!(band.ratio, ratio, "ratio", delta as f32 * 0.1),
                    8 => band_adj!(band.attack_ms, attack_ms, "attack", delta as f32 * 0.5),
                    9 => band_adj!(band.release_ms, release_ms, "release", delta as f32 * 5.0),
                    10 => band_adj!(band.knee_db, knee_db, "knee", delta as f32 * 0.1),
                    13 => {
                        let s = p(BT, "makeup_gain");
                        band.makeup_gain_db = (band.makeup_gain_db + delta as f32 * 0.5)
                            .clamp(s.min_f64() as f32, s.max_f64() as f32);
                        true
                    }
                    14 => {
                        band.bypass = !band.bypass;
                        true
                    }
                    15 => {
                        band.solo = !band.solo;
                        true
                    }
                    16 => {
                        band.auto_makeup = !band.auto_makeup;
                        true
                    }
                    17 => {
                        band.active = !band.active;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === MultibandExpander band-level params (idx >= 100) ===
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
            use sotf_plugins::param_specs::{
                find_by_key as p, multiband_expander::BAND_TEMPLATE as BT,
            };
            macro_rules! band_adj {
                ($field:expr, $global:expr, $key:literal, $step:expr) => {{
                    let spec = p(BT, $key);
                    $field = match $field {
                        None => Some(*$global as f32),
                        Some(v) => {
                            let new_v = v + $step;
                            if new_v < spec.min_f64() as f32 {
                                None
                            } else {
                                Some(new_v.clamp(spec.min_f64() as f32, spec.max_f64() as f32))
                            }
                        }
                    };
                    true
                }};
            }
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => band_adj!(band.threshold_db, threshold_db, "threshold", delta as f32),
                    7 => band_adj!(band.ratio, ratio, "ratio", delta as f32 * 0.1),
                    8 => band_adj!(band.attack_ms, attack_ms, "attack", delta as f32 * 0.1),
                    9 => band_adj!(band.release_ms, release_ms, "release", delta as f32 * 10.0),
                    10 => band_adj!(band.range_db, range_db, "range", delta as f32),
                    11 => band_adj!(band.knee_db, knee_db, "knee", delta as f32 * 0.1),
                    12 => band_adj!(
                        band.hysteresis_db,
                        hysteresis_db,
                        "hysteresis",
                        delta as f32 * 0.1
                    ),
                    13 => band_adj!(band.hold_ms, hold_ms, "hold", delta as f32 * 5.0),
                    14 => {
                        band.bypass = !band.bypass;
                        true
                    }
                    15 => {
                        band.solo = !band.solo;
                        true
                    }
                    16 => {
                        band.auto_makeup = !band.auto_makeup;
                        true
                    }
                    17 => {
                        band.active = !band.active;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === Crossfeed preset (idx 1): sets multiple fields atomically ===
        PluginSettings::Crossfeed {
            mode,
            preset,
            bauer_fcut_hz,
            bauer_feed_db,
            meier_level,
            mb_low_freq_hz,
            mb_mid_high_freq_hz,
            mb_low_feed_db,
            mb_mid_feed_db,
            mb_high_feed_db,
            ..
        } if param_idx == 1 => {
            use sotf_plugins::CrossfeedPreset;
            let presets = [
                CrossfeedPreset::Default,
                CrossfeedPreset::Cmoy,
                CrossfeedPreset::Meier,
                CrossfeedPreset::Mb,
                CrossfeedPreset::Off,
            ];
            let current = presets.iter().position(|pr| pr == preset).unwrap_or(0);
            let next = if delta > 0.0 {
                (current + 1) % presets.len()
            } else {
                (current + presets.len() - 1) % presets.len()
            };
            *preset = presets[next];
            let pp = sotf_plugins::CrossfeedPluginParams::from_preset(*preset);
            *mode = pp.mode;
            *bauer_fcut_hz = pp.bauer_fcut_hz as f64;
            *bauer_feed_db = pp.bauer_feed_db as f64;
            *meier_level = pp.meier_level as f64;
            *mb_low_freq_hz = pp.mb_low_freq_hz as f64;
            *mb_mid_high_freq_hz = pp.mb_mid_high_freq_hz as f64;
            *mb_low_feed_db = pp.mb_low_feed_db as f64;
            *mb_mid_feed_db = pp.mb_mid_feed_db as f64;
            *mb_high_feed_db = pp.mb_high_feed_db as f64;
            true
        }
        // === Generic path: all other plugins use adjust_param_value() ===
        other => {
            let adjusted = other.adjust_param_value(param_idx, delta);
            if adjusted {
                apply_structural_side_effects(other, param_idx, channel_count_changed);
            }
            adjusted
        }
    }
}
