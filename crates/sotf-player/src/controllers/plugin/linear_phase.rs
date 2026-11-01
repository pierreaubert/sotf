//! Desktop FIR EQ parameter addresses preserve the original five-field bands.
//! Globals use 100..106 and placements use 200..210 to avoid aliasing bands.

// Rust guideline compliant 2026-02-21
use crate::PluginSettings;
use sotf_plugins::param_specs::{ParamSpec, linear_phase_eq as specs};
use sotf_plugins::plugin_eq::EqBandPlacement;

pub(super) fn spec(index: usize) -> Option<&'static ParamSpec> {
    match index {
        0..50 => specs::BAND_TEMPLATE.get(index % 5),
        100..106 => specs::PARAMS.get(index - 100),
        200..210 => specs::BAND_TEMPLATE.get(5),
        _ => None,
    }
}

pub(super) fn placement_value(placement: Option<EqBandPlacement>) -> f64 {
    match placement {
        None => 0.0,
        Some(EqBandPlacement::Stereo) => 1.0,
        Some(EqBandPlacement::Left) => 2.0,
        Some(EqBandPlacement::Right) => 3.0,
        Some(EqBandPlacement::Mid) => 4.0,
        Some(EqBandPlacement::Side) => 5.0,
    }
}

pub(super) fn value(settings: &PluginSettings, index: usize) -> Option<f64> {
    let PluginSettings::LinearPhaseEq {
        filters,
        num_filters,
        fir_length,
        phase_mode,
        auto_gain,
        accuracy_mode,
        mix,
        ..
    } = settings
    else {
        return None;
    };
    match index {
        100 => Some(*num_filters),
        101 => Some(*fir_length),
        102 => Some(*phase_mode),
        103 => Some(if *auto_gain { 1.0 } else { 0.0 }),
        104 => Some(*mix),
        105 => Some(f64::from(*accuracy_mode)),
        200..210 => Some(placement_value(filters.get(index - 200)?.placement)),
        0..50 => {
            let filter = filters.get(index / 5)?;
            match index % 5 {
                0 => super::misc::eq_band_types(false)
                    .iter()
                    .position(|kind| *kind == filter.filter_type)
                    .map(|v| v as f64),
                1 => Some(filter.frequency),
                2 => Some(filter.q),
                3 => Some(filter.gain_db),
                4 => Some(if filter.muted { 0.0 } else { 1.0 }),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn set_extra(settings: &mut PluginSettings, index: usize, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    let Some(spec) = spec(index) else {
        return false;
    };
    let value = spec.clamp_f64(value / spec.display_scale);
    let PluginSettings::LinearPhaseEq {
        filters,
        num_filters,
        fir_length,
        phase_mode,
        auto_gain,
        accuracy_mode,
        mix,
        ..
    } = settings
    else {
        return false;
    };
    match index {
        100 => {
            *num_filters = value;
            filters.resize_with(value as usize, || {
                crate::EQFilter::new(crate::BiquadFilterType::Peak, 1000.0, 1.0, 0.0)
            });
        }
        101 => *fir_length = value,
        102 => *phase_mode = value,
        103 => *auto_gain = value > 0.5,
        104 => *mix = value,
        105 => *accuracy_mode = value > 0.5,
        200..210 => {
            let Some(filter) = filters.get_mut(index - 200) else {
                return false;
            };
            filter.placement = match value as usize {
                1 => Some(EqBandPlacement::Stereo),
                2 => Some(EqBandPlacement::Left),
                3 => Some(EqBandPlacement::Right),
                4 => Some(EqBandPlacement::Mid),
                5 => Some(EqBandPlacement::Side),
                _ => None,
            };
        }
        _ => return false,
    }
    true
}

pub(super) fn reset(settings: &mut PluginSettings, index: usize) -> Option<bool> {
    if !matches!(settings, PluginSettings::LinearPhaseEq { .. }) {
        return None;
    }
    if value(settings, index).is_none() {
        return Some(false);
    }
    Some(super::set::set_plugin_param_value(
        settings,
        index,
        spec(index)?.default_f64() * spec(index)?.display_scale,
        &mut false,
    ))
}

#[cfg(test)]
mod accuracy_integration_tests {
    use super::super::parameter_metadata::{desktop_param_indices, desktop_param_value};
    use super::*;
    use crate::ui_params::TuiEditablePlugin;
    use crate::{PluginType, get_param_count};

    #[test]
    fn accuracy_mode_keeps_desktop_addresses_and_saved_manual_length() {
        let mut settings = PluginSettings::default_for(&PluginType::LinearPhaseEq).unwrap();
        let before = settings.clone();
        let addresses = desktop_param_indices(&settings);
        assert!(addresses.contains(&105));
        for (index, key) in [
            (100, "num_filters"),
            (101, "fir_length_index"),
            (102, "phase_mode_index"),
            (103, "auto_gain"),
            (104, "mix"),
            (105, "accuracy_mode"),
        ] {
            assert_eq!(spec(index).unwrap().engine_key, key);
        }
        assert_eq!(desktop_param_value(&settings, 105), Some(0.0));
        assert!(set_extra(&mut settings, 105, 1.0));
        assert_eq!(desktop_param_value(&settings, 105), Some(1.0));
        for index in addresses.into_iter().filter(|index| *index != 105) {
            assert_eq!(
                desktop_param_value(&settings, index),
                desktop_param_value(&before, index)
            );
        }
        let config = settings.to_plugin_config(96_000.0);
        assert_eq!(config.parameters["accuracy_mode"], true);
        assert_eq!(
            config.parameters["fir_length_index"],
            before.to_plugin_config(96_000.0).parameters["fir_length_index"]
        );
        let restored: PluginSettings =
            serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
        assert_eq!(desktop_param_value(&restored, 105), Some(1.0));
        assert_eq!(reset(&mut settings, 105), Some(true));
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::to_value(before).unwrap()
        );
    }

    #[test]
    fn accuracy_mode_tui_rows_follow_descriptors_without_aliasing_bands() {
        let mut settings = PluginSettings::default_for(&PluginType::LinearPhaseEq).unwrap();
        let before = settings.clone();
        let globals = specs::PARAMS.len();
        assert_eq!(get_param_count(&settings), settings.get_descriptors().len());
        assert_eq!(
            settings.get_value_as_string(5),
            specs::PARAMS[5].format_value(0.0)
        );
        assert!(settings.set_param(5, 1.0));
        assert_eq!(
            settings.get_value_as_string(5),
            specs::PARAMS[5].format_value(1.0)
        );
        assert_eq!(
            settings.get_value_as_string(globals + 1),
            before.get_value_as_string(globals + 1)
        );
        assert!(settings.adjust_param(globals + 1, 1.0));
        let PluginSettings::LinearPhaseEq {
            accuracy_mode,
            filters,
            fir_length,
            ..
        } = &settings
        else {
            panic!("FIR EQ")
        };
        let PluginSettings::LinearPhaseEq {
            filters: old_filters,
            fir_length: old_length,
            ..
        } = before
        else {
            panic!("FIR EQ")
        };
        assert!(*accuracy_mode);
        assert!(filters[0].frequency > old_filters[0].frequency);
        assert_eq!(fir_length, &old_length);
        assert!(settings.set_param(5, 0.0));
        assert_eq!(
            settings.get_value_as_string(5),
            specs::PARAMS[5].format_value(0.0)
        );
    }
}
