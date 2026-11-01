use crate::{BiquadFilterType, EQFilter, PluginSettings};

/// Explain a custom decoder choice that lacks valid speaker geometry.
pub fn ambisonics_layout_transition_error(
    settings: &PluginSettings,
    param_idx: usize,
    value: f64,
) -> Option<&'static str> {
    let PluginSettings::AmbisonicsDecoder { custom_layout, .. } = settings else {
        return None;
    };
    let param = settings.param_specs().get(param_idx)?;
    if param.engine_key != "target_layout" {
        return None;
    }
    let sotf_plugins::param_specs::ParamType::Choice { labels, .. } = param.param_type else {
        return None;
    };
    if labels.get((value as usize).min(labels.len().saturating_sub(1))) != Some(&"custom") {
        return None;
    }
    if custom_layout
        .as_ref()
        .is_some_and(sotf_audio::plugins::valid_ambisonics_custom_layout)
    {
        return None;
    }
    Some(
        "Custom layout needs valid speaker geometry. Load a preset with a custom speaker layout before selecting Custom.",
    )
}

/// Get parameter count for a plugin's settings.
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::EQ { filters, .. } => filters.len() * 4,
        PluginSettings::LinearPhaseEq { filters, .. } => {
            let specs = sotf_plugins::param_specs::linear_phase_eq::PARAMS;
            let bands = sotf_plugins::param_specs::linear_phase_eq::BAND_TEMPLATE;
            filters.len() * bands.len() + specs.len()
        }
        PluginSettings::DynamicEq { num_bands, .. } => 8 + (*num_bands as usize).clamp(1, 8) * 10,
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
/// Handles: output-layout choices set channel_count_changed,
/// MultibandCompressor/Expander num_bands (idx 0) resizes band arrays.
pub(super) fn apply_structural_side_effects(
    settings: &mut PluginSettings,
    param_idx: usize,
    channel_count_changed: &mut bool,
) {
    if matches!(
        settings,
        PluginSettings::Gate { .. } | PluginSettings::DeEsser { .. }
    ) && settings
        .param_specs()
        .get(param_idx)
        .is_some_and(|param| param.engine_key == "sidechain_external")
    {
        *channel_count_changed = true;
    }
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
        PluginSettings::AAE { .. } if param_idx == 0 => {
            *channel_count_changed = true;
        }
        PluginSettings::AmbisonicsDecoder { .. } if param_idx == 1 => {
            *channel_count_changed = true;
        }
        PluginSettings::Crossover { .. } if matches!(param_idx, 2 | 4 | 5 | 6 | 7) => {
            *channel_count_changed = true;
        }
        PluginSettings::BandSplit { .. } if param_idx == 3 => {
            *channel_count_changed = true;
        }
        PluginSettings::BandMerge { .. } if param_idx == 0 => {
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
            num_bands,
            bands,
            threshold,
            ratio,
            ..
        } if param_idx == 0 => {
            bands.resize_with((*num_bands as usize).clamp(1, 8), || {
                sotf_plugins::DynEqBandParams {
                    band_threshold: *threshold as f32,
                    band_ratio: *ratio as f32,
                    ..Default::default()
                }
            });
        }
        PluginSettings::LinearPhaseEq {
            num_filters,
            filters,
            ..
        } if param_idx == 100 => {
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
            if band_index >= *num_bands || !matches!(field, 6..=10 | 13..=19) {
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
                18 => band.range_db = None,
                19 => band.hold_ms = None,
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

/// Resolve the stable desktop ID of a Dynamic EQ band control.
pub(super) fn dynamic_eq_band_spec(
    index: usize,
) -> Option<&'static sotf_plugins::param_specs::ParamSpec> {
    use sotf_plugins::param_specs::dynamic_eq as specs;
    let local = index.checked_sub(100)? % 10;
    match local {
        0..=6 => specs::BAND_PARAMS.get(local),
        7 => specs::SHELF_PARAMS.first(),
        8 => specs::SHELF_PARAMS.get(1),
        9 => specs::ROUTING_PARAMS.first(),
        _ => None,
    }
}

/// Reset a band field, restoring global inheritance for threshold and ratio.
pub(super) fn reset_dynamic_eq_band(settings: &mut PluginSettings, index: usize) -> Option<bool> {
    let PluginSettings::DynamicEq {
        bands,
        threshold,
        ratio,
        ..
    } = settings
    else {
        return None;
    };
    if index < 100 {
        return None;
    }
    if bands.get((index - 100) / 10).is_none() {
        return Some(false);
    }
    let default = match (index - 100) % 10 {
        3 => *threshold,
        4 => *ratio,
        _ => dynamic_eq_band_spec(index)?.default_f64(),
    };
    Some(super::set::set_plugin_param_value(
        settings, index, default, &mut false,
    ))
}
