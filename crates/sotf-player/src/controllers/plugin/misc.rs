use crate::{BiquadFilterType, EQFilter, PluginSettings};

/// Get parameter count for a plugin's settings.
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::EQ { filters, .. } => filters.len() * 4,
        PluginSettings::LinearPhaseEq { filters, .. } => filters.len() * 4,
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
            bands.resize_with(*num_bands, Default::default);
            for (i, band) in bands.iter_mut().enumerate() {
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
            bands.resize_with(*num_bands, Default::default);
            for (i, band) in bands.iter_mut().enumerate() {
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
