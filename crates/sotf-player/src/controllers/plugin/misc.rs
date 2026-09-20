use crate::{BiquadFilterType, EQFilter, PluginSettings};

/// Get parameter count for a plugin's settings.
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::EQ { filters, .. } => filters.len() * 4,
        PluginSettings::LinearPhaseEq { filters, .. } => filters.len() * 5,
        PluginSettings::DynamicEq { num_bands, .. } => 8 + (*num_bands as usize).clamp(1, 8) * 7,
        _ => settings.param_specs().len(),
    }
}

pub(super) fn eq_band_types(allow_extended: bool) -> &'static [BiquadFilterType] {
    if allow_extended {
        &[
            BiquadFilterType::Peak,
            BiquadFilterType::Lowshelf,
            BiquadFilterType::Highshelf,
            BiquadFilterType::Lowpass,
            BiquadFilterType::Highpass,
            BiquadFilterType::Bandpass,
            BiquadFilterType::Notch,
        ]
    } else {
        &[
            BiquadFilterType::Peak,
            BiquadFilterType::Lowshelf,
            BiquadFilterType::Highshelf,
            BiquadFilterType::Lowpass,
            BiquadFilterType::Highpass,
        ]
    }
}

/// Apply structural side effects after a parameter update via the generic path.
///
/// Handles: Upmixer output topology params set channel_count_changed,
/// MultibandCompressor/Expander num_bands (idx 0) resizes band arrays.
pub(super) fn apply_structural_side_effects(
    settings: &mut PluginSettings,
    param_idx: usize,
    channel_count_changed: &mut bool,
) {
    let upmixer_binaural_preview_idx = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::upmixer::PARAMS,
        "binaural_preview",
    );

    match settings {
        PluginSettings::Upmixer { .. }
            if param_idx == 0 || param_idx == upmixer_binaural_preview_idx =>
        {
            *channel_count_changed = true;
        }
        PluginSettings::MultibandCompressor {
            num_bands, bands, ..
        } if param_idx == 0 => {
            let retained_bands = bands.len().min(*num_bands);
            bands.resize_with(*num_bands, Default::default);
            for (i, band) in bands.iter_mut().enumerate().skip(retained_bands) {
                band.active = match *num_bands {
                    4 => i < 3,
                    5 => i < 3,
                    _ => true,
                };
            }
            *channel_count_changed = true;
        }
        PluginSettings::MultibandExpander {
            num_bands, bands, ..
        } if param_idx == 0 => {
            let retained_bands = bands.len().min(*num_bands);
            bands.resize_with(*num_bands, Default::default);
            for (i, band) in bands.iter_mut().enumerate().skip(retained_bands) {
                band.active = match *num_bands {
                    4 => i < 3,
                    5 => i < 3,
                    _ => true,
                };
            }
            *channel_count_changed = true;
        }
        PluginSettings::DynamicEq {
            num_bands, bands, ..
        } if param_idx == 0 => {
            bands.resize_with((*num_bands as usize).clamp(1, 8), Default::default);
        }
        PluginSettings::LinearPhaseEq {
            num_filters,
            filters,
            ..
        } if param_idx == 0 => {
            let n = (*num_filters as usize).clamp(1, 10);
            filters.resize_with(n, || {
                EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0)
            });
        }
        _ => {}
    }
}

/// Reset a multiband override without changing other bands or global defaults.
/// None means this is not a multiband field; Some(false) means an invalid field.
pub(super) fn reset_multiband_override(
    settings: &mut PluginSettings,
    index: usize,
) -> Option<bool> {
    if index < 100 {
        return None;
    }
    let band_index = index / 100 - 1;
    let field = index % 100;
    match settings {
        PluginSettings::MultibandCompressor {
            num_bands, bands, ..
        } => {
            if band_index >= *num_bands || !matches!(field, 6..=10 | 13..=17) {
                return Some(false);
            }
            let Some(band) = bands.get_mut(band_index) else {
                return Some(true);
            };
            match field {
                6 => band.threshold_db = None,
                7 => band.ratio = None,
                8 => band.attack_ms = None,
                9 => band.release_ms = None,
                10 => band.knee_db = None,
                13 => band.makeup_gain_db = 0.0,
                14 => band.bypass = false,
                15 => band.solo = false,
                16 => band.auto_makeup = false,
                17 => band.active = band_index < 3 || *num_bands < 4,
                _ => return Some(false),
            }
            Some(true)
        }
        PluginSettings::MultibandExpander {
            num_bands, bands, ..
        } => {
            if band_index >= *num_bands || !matches!(field, 6..=17) {
                return Some(false);
            }
            let Some(band) = bands.get_mut(band_index) else {
                return Some(true);
            };
            match field {
                6 => band.threshold_db = None,
                7 => band.ratio = None,
                8 => band.attack_ms = None,
                9 => band.release_ms = None,
                10 => band.range_db = None,
                11 => band.knee_db = None,
                12 => band.hysteresis_db = None,
                13 => band.hold_ms = None,
                14 => band.bypass = false,
                15 => band.solo = false,
                16 => band.auto_makeup = false,
                17 => band.active = band_index < 3 || *num_bands < 4,
                _ => return Some(false),
            }
            Some(true)
        }
        _ => None,
    }
}
