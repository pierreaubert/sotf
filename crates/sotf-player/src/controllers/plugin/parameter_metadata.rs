//! Desktop parameter addresses and readback shared by editors and introspection.
//!
//! Widgets and MIDI badges must retain these addresses instead of indexing presentation rows.
//! Values are raw settings values; display scaling belongs at the widget boundary.

// Rust guideline compliant 2026-02-21
/// Enumerate the actual desktop parameter addresses for the current plugin.
///
/// Dynamic bands and FIR globals have sparse addresses; callers must not use row positions.
/// Standard EQ globals have dedicated header operations and are not band parameter addresses.
pub fn desktop_param_indices(settings: &crate::PluginSettings) -> Vec<usize> {
    match settings {
        crate::PluginSettings::Crossover { .. } => {
            let mut indices: Vec<_> = (0..settings.param_specs().len()).collect();
            let start = sotf_plugins::param_specs::crossover::CHANNEL_PARAM_START;
            indices.extend(start..start + 2 * super::crossover::channel_count(settings));
            indices
        }
        crate::PluginSettings::LinearPhaseEq { filters, .. } => {
            let mut indices: Vec<usize> = (0..filters.len() * 5).collect();
            indices.extend(100..100 + sotf_plugins::param_specs::linear_phase_eq::PARAMS.len());
            indices.extend(200..200 + filters.len());
            indices
        }
        crate::PluginSettings::DynamicEq { num_bands, .. } => {
            let mut indices: Vec<usize> =
                (0..sotf_plugins::param_specs::dynamic_eq::NUM_GLOBAL_PARAMS).collect();
            for band in 0..(*num_bands as usize).clamp(1, 8) {
                indices.extend((0..10).map(|local| 100 + band * 10 + local));
            }
            indices
        }
        crate::PluginSettings::MultibandCompressor { num_bands, .. } => {
            let mut indices: Vec<usize> =
                (0..sotf_plugins::param_specs::multiband_compressor::GLOBAL_PARAMS.len()).collect();
            for band in 0..*num_bands {
                indices.extend(
                    [6, 7, 8, 9, 10, 13, 14, 15, 16, 17, 18, 19]
                        .map(|local| (band + 1) * 100 + local),
                );
            }
            indices
        }
        crate::PluginSettings::MultibandExpander { num_bands, .. } => {
            let mut indices: Vec<usize> =
                (0..sotf_plugins::param_specs::multiband_expander::GLOBAL_PARAMS.len()).collect();
            for band in 0..*num_bands {
                indices.extend((6..=17).map(|local| (band + 1) * 100 + local));
            }
            indices
        }
        _ => (0..crate::get_param_count(settings)).collect(),
    }
}

/// Resolve desktop parameter metadata, including sparse band and global addresses.
///
/// Returns None for addresses absent from desktop_param_indices.
pub fn desktop_param_spec(
    settings: &crate::PluginSettings,
    idx: usize,
) -> Option<&'static sotf_plugins::param_specs::ParamSpec> {
    if !desktop_param_indices(settings).contains(&idx) {
        return None;
    }
    match settings {
        crate::PluginSettings::Crossover { .. }
            if idx >= sotf_plugins::param_specs::crossover::CHANNEL_PARAM_START =>
        {
            super::crossover::spec(settings, idx)
        }
        crate::PluginSettings::EQ { .. } => {
            sotf_plugins::param_specs::eq::BAND_TEMPLATE.get(idx % 4)
        }
        crate::PluginSettings::LinearPhaseEq { .. } => super::linear_phase::spec(idx),
        crate::PluginSettings::DynamicEq { .. } => {
            if idx < sotf_plugins::param_specs::dynamic_eq::NUM_GLOBAL_PARAMS {
                sotf_plugins::param_specs::dynamic_eq::PARAMS.get(idx)
            } else if idx >= 100 {
                super::misc::dynamic_eq_band_spec(idx)
            } else {
                None
            }
        }
        crate::PluginSettings::MultibandCompressor { .. } if idx >= 100 => {
            let local = idx % 100;
            let template = match local {
                6..=10 => local - 4,
                13 => 7,
                14 => 1,
                15 => 0,
                16 => 8,
                17 => 9,
                18 => 10,
                19 => 11,
                _ => return None,
            };
            sotf_plugins::param_specs::multiband_compressor::BAND_TEMPLATE.get(template)
        }
        crate::PluginSettings::MultibandExpander { .. } if idx >= 100 => {
            let local = idx % 100;
            let template = match local {
                6..=13 => local - 4,
                14 => 1,
                15 => 0,
                16 => 10,
                17 => 11,
                _ => return None,
            };
            sotf_plugins::param_specs::multiband_expander::BAND_TEMPLATE.get(template)
        }
        _ => settings.param_specs().get(idx),
    }
}

/// Read a raw desktop parameter value without parsing formatted display text.
///
/// Returns None for invalid addresses and file parameters. Multiply by the spec display_scale
/// for widgets; the desktop setter consumes values in that display space.
pub fn desktop_param_value(settings: &crate::PluginSettings, idx: usize) -> Option<f64> {
    if !desktop_param_indices(settings).contains(&idx) {
        return None;
    }
    match settings {
        crate::PluginSettings::Crossover { .. }
            if idx >= sotf_plugins::param_specs::crossover::CHANNEL_PARAM_START =>
        {
            super::crossover::value(settings, idx)
        }
        crate::PluginSettings::EQ { .. } => {
            desktop_eq_param_value(settings, super::EqEditTarget::Global, idx)
        }
        settings @ crate::PluginSettings::LinearPhaseEq { .. } => {
            super::linear_phase::value(settings, idx)
        }
        crate::PluginSettings::DynamicEq { bands, .. } if idx >= 100 => {
            let band = bands.get((idx - 100) / 10)?;
            match (idx - 100) % 10 {
                0 => Some(f64::from(band.frequency)),
                1 => Some(f64::from(band.q)),
                2 => Some(f64::from(band.gain)),
                3 => Some(f64::from(band.band_threshold)),
                4 => Some(f64::from(band.band_ratio)),
                5 => Some(f64::from(band.active as u8)),
                6 => Some(f64::from(band.solo as u8)),
                7 => Some(f64::from(band.shape.choice_index())),
                8 => Some(f64::from(band.shelf_slope)),
                9 => Some(f64::from(band.placement.choice_index())),
                _ => None,
            }
        }
        crate::PluginSettings::MultibandCompressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            range_db,
            hold_ms,
            bands,
            ..
        } if idx >= 100 => {
            let band = bands.get(idx / 100 - 1);
            Some(match idx % 100 {
                6 => f64::from(
                    band.and_then(|b| b.threshold_db)
                        .unwrap_or(*threshold_db as f32),
                ),
                7 => f64::from(band.and_then(|b| b.ratio).unwrap_or(*ratio as f32)),
                8 => f64::from(band.and_then(|b| b.attack_ms).unwrap_or(*attack_ms as f32)),
                9 => f64::from(
                    band.and_then(|b| b.release_ms)
                        .unwrap_or(*release_ms as f32),
                ),
                10 => f64::from(band.and_then(|b| b.knee_db).unwrap_or(*knee_db as f32)),
                13 => f64::from(band.map_or(0.0, |b| b.makeup_gain_db)),
                14 => f64::from(band.is_some_and(|b| b.bypass) as u8),
                15 => f64::from(band.is_some_and(|b| b.solo) as u8),
                16 => f64::from(band.is_some_and(|b| b.auto_makeup) as u8),
                17 => f64::from(band.is_none_or(|b| b.active) as u8),
                18 => f64::from(band.and_then(|b| b.range_db).unwrap_or(*range_db as f32)),
                19 => f64::from(band.and_then(|b| b.hold_ms).unwrap_or(*hold_ms as f32)),
                _ => return None,
            })
        }
        crate::PluginSettings::MultibandExpander {
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
        } if idx >= 100 => {
            let band = bands.get(idx / 100 - 1);
            Some(match idx % 100 {
                6 => f64::from(
                    band.and_then(|b| b.threshold_db)
                        .unwrap_or(*threshold_db as f32),
                ),
                7 => f64::from(band.and_then(|b| b.ratio).unwrap_or(*ratio as f32)),
                8 => f64::from(band.and_then(|b| b.attack_ms).unwrap_or(*attack_ms as f32)),
                9 => f64::from(
                    band.and_then(|b| b.release_ms)
                        .unwrap_or(*release_ms as f32),
                ),
                10 => f64::from(band.and_then(|b| b.range_db).unwrap_or(*range_db as f32)),
                11 => f64::from(band.and_then(|b| b.knee_db).unwrap_or(*knee_db as f32)),
                12 => f64::from(
                    band.and_then(|b| b.hysteresis_db)
                        .unwrap_or(*hysteresis_db as f32),
                ),
                13 => f64::from(band.and_then(|b| b.hold_ms).unwrap_or(*hold_ms as f32)),
                14 => f64::from(band.is_some_and(|b| b.bypass) as u8),
                15 => f64::from(band.is_some_and(|b| b.solo) as u8),
                16 => f64::from(band.is_some_and(|b| b.auto_makeup) as u8),
                17 => f64::from(band.is_none_or(|b| b.active) as u8),
                _ => return None,
            })
        }
        _ => settings.param_value(idx),
    }
}

/// Read an EQ value from the exact bank selected by the desktop setter.
///
/// Returns None for non-EQ settings, missing channel banks, or absent filter fields.
pub fn desktop_eq_param_value(
    settings: &crate::PluginSettings,
    target: super::EqEditTarget,
    index: usize,
) -> Option<f64> {
    let crate::PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = settings
    else {
        return None;
    };
    let filters = match target {
        super::EqEditTarget::Global => filters,
        super::EqEditTarget::Channel(channel) => channel_filters.as_ref()?.get(channel)?,
    };
    let filter = filters.get(index / 4)?;
    match index % 4 {
        0 => Some(filter.frequency),
        1 => Some(filter.q),
        2 => Some(filter.gain_db),
        3 => super::eq_band_types(true)
            .iter()
            .position(|kind| *kind == filter.filter_type)
            .map(|index| index as f64),
        _ => None,
    }
}

/// Check declarative edit prerequisites without treating hidden placement as disabled.
///
/// Tables keep parameters discoverable but respect read-only controls, group conditions,
/// and each control's enabled_when condition. Custom controls without a declarative owner
/// require their specialized editor's constraints.
pub fn desktop_param_enabled(settings: &crate::PluginSettings, index: usize) -> bool {
    if !desktop_param_indices(settings).contains(&index) {
        return false;
    }
    let Some(layout) = settings.layout() else {
        return true;
    };
    let values: Vec<_> = (0..settings.param_specs().len())
        .map(|index| settings.param_value(index).unwrap_or(0.0))
        .collect();
    let standalone = layout
        .config
        .iter()
        .chain(layout.output.iter())
        .chain(layout.tabs.iter().flat_map(|tab| tab.controls.iter()));
    let standalone_enabled = standalone
        .filter(|control| control.param_index == index)
        .all(|control| control.is_enabled(&values));
    let grouped_enabled = layout.main.iter().all(|group| {
        group
            .controls
            .iter()
            .filter(|control| control.param_index == index)
            .all(|control| group.is_visible(&values) && control.is_enabled(&values))
    });
    standalone_enabled && grouped_enabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginType;
    use sotf_plugins::param_specs::ParamType;
    use std::collections::HashSet;

    #[test]
    fn dynamic_parameter_addresses_have_unique_metadata_and_real_readback() {
        for kind in [
            PluginType::EQ,
            PluginType::LinearPhaseEq,
            PluginType::DynamicEq,
            PluginType::MultibandCompressor,
            PluginType::MultibandExpander,
        ] {
            let settings = crate::PluginSettings::default_for(&kind).unwrap();
            let indices = desktop_param_indices(&settings);
            assert!(!indices.is_empty(), "{kind:?}");
            assert_eq!(
                indices.iter().copied().collect::<HashSet<_>>().len(),
                indices.len()
            );
            for index in indices {
                let spec = desktop_param_spec(&settings, index).unwrap();
                assert!(!spec.name.is_empty());
                if !matches!(spec.param_type, ParamType::FilePath) {
                    assert!(
                        desktop_param_value(&settings, index).is_some(),
                        "{kind:?} {index}"
                    );
                }
            }
            assert!(desktop_param_spec(&settings, usize::MAX).is_none());
            assert!(desktop_param_value(&settings, usize::MAX).is_none());
        }
    }

    #[test]
    fn eq_table_frequency_address_updates_frequency_instead_of_filter_count() {
        let mut settings = crate::PluginSettings::default_for(&PluginType::EQ).unwrap();
        let before = settings.clone();
        assert!(super::super::set::set_plugin_param_value(
            &mut settings,
            0,
            320.0,
            &mut false
        ));
        let mut expected = before;
        let crate::PluginSettings::EQ { filters, .. } = &mut expected else {
            unreachable!()
        };
        filters[0].frequency = 320.0;
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(desktop_param_value(&settings, 0), Some(320.0));
    }

    #[test]
    fn fir_sparse_mix_address_preserves_bands_and_stores_display_percent() {
        let mut settings = crate::PluginSettings::default_for(&PluginType::LinearPhaseEq).unwrap();
        let before = settings.clone();
        let spec = desktop_param_spec(&settings, 104).unwrap();
        assert_eq!(spec.engine_key, "mix");
        assert_eq!(spec.display_scale, 100.0);
        assert!(super::super::set::set_plugin_param_value(
            &mut settings,
            104,
            37.0,
            &mut false
        ));
        let mut expected = before;
        let crate::PluginSettings::LinearPhaseEq { mix, .. } = &mut expected else {
            unreachable!()
        };
        *mix = 0.37;
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(desktop_param_value(&settings, 104), Some(0.37));
    }
}

#[cfg(test)]
mod target_tests {
    use super::super::EqEditTarget;
    use super::*;
    use crate::{PluginSettings, PluginType};
    use sotf_plugins::param_specs::{index_of, upmixer::PARAMS as UP};

    #[test]
    fn table_prerequisites_follow_subharmonic_enable_without_disabling_the_switch() {
        let mut settings = PluginSettings::default_for(&PluginType::Upmixer).unwrap();
        let enable = index_of(UP, "enable_subharmonic_synth");
        let gain = index_of(UP, "subharmonic_gain");
        let attack = index_of(UP, "subharmonic_attack_ms");
        assert!(desktop_param_enabled(&settings, enable));
        assert!(!desktop_param_enabled(&settings, gain));
        assert!(!desktop_param_enabled(&settings, attack));
        assert!(super::super::set::set_plugin_param_value(
            &mut settings,
            enable,
            1.0,
            &mut false
        ));
        assert!(desktop_param_enabled(&settings, gain));
        assert!(desktop_param_enabled(&settings, attack));
        assert!(super::super::set::set_plugin_param_value(
            &mut settings,
            enable,
            0.0,
            &mut false
        ));
        assert!(!desktop_param_enabled(&settings, gain));
    }

    #[test]
    fn eq_target_readback_and_mutation_use_the_same_channel_bank() {
        let mut settings = PluginSettings::default_for(&PluginType::EQ).unwrap();
        let PluginSettings::EQ {
            filters,
            channel_filters,
            per_channel_mode,
            ..
        } = &mut settings
        else {
            unreachable!();
        };
        *per_channel_mode = true;
        let mut left = filters.clone();
        let mut right = filters.clone();
        left[0].frequency = 55.0;
        right[0].frequency = 440.0;
        *channel_filters = Some(vec![left, right]);
        let before = settings.clone();
        assert_eq!(
            desktop_eq_param_value(&settings, EqEditTarget::Channel(0), 0),
            Some(55.0)
        );
        assert_eq!(
            desktop_eq_param_value(&settings, EqEditTarget::Channel(1), 0),
            Some(440.0)
        );
        assert!(desktop_eq_param_value(&settings, EqEditTarget::Channel(2), 0).is_none());
        assert!(super::super::set::set_eq_param_value_for_target(
            &mut settings,
            EqEditTarget::Channel(1),
            0,
            700.0,
        ));
        let mut expected = before;
        let PluginSettings::EQ {
            channel_filters, ..
        } = &mut expected
        else {
            unreachable!()
        };
        channel_filters.as_mut().unwrap()[1][0].frequency = 700.0;
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(
            desktop_eq_param_value(&settings, EqEditTarget::Channel(1), 0),
            Some(700.0)
        );
        assert_eq!(
            desktop_eq_param_value(&settings, EqEditTarget::Channel(0), 0),
            Some(55.0)
        );
    }
}
