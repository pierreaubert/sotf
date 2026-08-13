use super::misc::apply_structural_side_effects;
use super::misc::eq_band_types;
use super::types::EqEditTarget;
use crate::{EQFilter, PluginSettings};
use sotf_plugins::param_specs::eq::{clamp_q, q_max_for};

pub(super) fn set_eq_band_field_for_plugin(
    filter_idx: usize,
    field_idx: usize,
    filters: &mut [EQFilter],
    value: f64,
    allow_extended_types: bool,
) -> bool {
    let Some(filter) = filters.get_mut(filter_idx) else {
        return false;
    };
    match field_idx {
        0 => {
            filter.frequency = value.clamp(20.0, 20_000.0);
            true
        }
        1 => {
            filter.q = clamp_q(filter.filter_type, value);
            true
        }
        2 => {
            filter.gain_db = value.clamp(-24.0, 24.0);
            true
        }
        3 => {
            let types = eq_band_types(allow_extended_types);
            let type_idx = (value as usize).clamp(0, types.len() - 1);
            filter.filter_type = types[type_idx];
            // Keep Q within the new type's accepted range.
            filter.q = filter.q.clamp(0.1, q_max_for(filter.filter_type));
            true
        }
        _ => false,
    }
}

pub(super) fn set_eq_param_value_for_target(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    param_idx: usize,
    value: f64,
) -> bool {
    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = settings
    else {
        return false;
    };
    let target_filters = match target {
        EqEditTarget::Global => filters,
        EqEditTarget::Channel(channel) => {
            let Some(filters) = channel_filters
                .as_mut()
                .and_then(|channels| channels.get_mut(channel))
            else {
                return false;
            };
            filters
        }
    };
    set_eq_band_field_for_plugin(param_idx / 4, param_idx % 4, target_filters, value, true)
}

fn set_fir_eq_band_field_for_plugin(
    filter_idx: usize,
    field_idx: usize,
    filters: &mut [EQFilter],
    value: f64,
) -> bool {
    let Some(filter) = filters.get_mut(filter_idx) else {
        return false;
    };
    match field_idx {
        0 => {
            let types = eq_band_types(false);
            let type_idx = (value as usize).clamp(0, types.len() - 1);
            filter.filter_type = types[type_idx];
            true
        }
        1 => {
            filter.frequency = value.clamp(20.0, 20_000.0);
            true
        }
        2 => {
            filter.q = value.clamp(0.1, 10.0);
            true
        }
        3 => {
            filter.gain_db = value.clamp(-24.0, 24.0);
            true
        }
        4 => {
            filter.muted = value <= 0.5;
            true
        }
        _ => false,
    }
}

/// Set a specific parameter value. Returns true if the parameter was set.
///
/// Most plugins delegate to `PluginSettings::set_param_value()` (generic path).
/// Only plugins with side effects beyond simple field updates have manual arms.
pub fn set_plugin_param_value(
    settings: &mut PluginSettings,
    param_idx: usize,
    value: f64,
    channel_count_changed: &mut bool,
) -> bool {
    match settings {
        // === EQ: dynamic filter array, param indices map to band/field ===
        PluginSettings::EQ { filters, .. } => {
            let filter_idx = param_idx / 4;
            let field_idx = param_idx % 4;

            set_eq_band_field_for_plugin(filter_idx, field_idx, filters, value, true)
        }
        PluginSettings::LinearPhaseEq { filters, .. } => {
            let filter_idx = param_idx / 5;
            let field_idx = param_idx % 5;

            set_fir_eq_band_field_for_plugin(filter_idx, field_idx, filters, value)
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
                use sotf_plugins::param_specs::{find_by_key as pk, spectrum::PARAMS as SP};
                *num_bins = (value as usize).clamp(
                    pk(SP, "num_bins").min_f64() as usize,
                    pk(SP, "num_bins").max_f64() as usize,
                );
                true
            }
            1 => {
                use sotf_plugins::param_specs::{find_by_key as pk, spectrum::PARAMS as SP};
                *min_freq = (value as f32).clamp(
                    pk(SP, "min_freq").min_f64() as f32,
                    pk(SP, "min_freq").max_f64() as f32,
                );
                true
            }
            2 => {
                use sotf_plugins::param_specs::{find_by_key as pk, spectrum::PARAMS as SP};
                *max_freq = (value as f32).clamp(
                    pk(SP, "max_freq").min_f64() as f32,
                    pk(SP, "max_freq").max_f64() as f32,
                );
                true
            }
            3 => {
                *smoothing = (value as f32).clamp(0.0, 1.0);
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
                *tilt_correction = modes[(value as usize).clamp(0, modes.len() - 1)];
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
                *tilt_reference = modes[(value as usize).clamp(0, modes.len() - 1)];
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
                        band.frequency = p(BT, "frequency").clamp_f64(value) as f32;
                        true
                    }
                    1 => {
                        band.q = p(BT, "q").clamp_f64(value) as f32;
                        true
                    }
                    2 => {
                        band.gain = p(BT, "gain").clamp_f64(value) as f32;
                        true
                    }
                    3 => {
                        band.band_threshold = p(BT, "band_threshold").clamp_f64(value) as f32;
                        true
                    }
                    4 => {
                        band.band_ratio = p(BT, "band_ratio").clamp_f64(value) as f32;
                        true
                    }
                    5 => {
                        band.active = value > 0.5;
                        true
                    }
                    6 => {
                        band.solo = value > 0.5;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === MultibandCompressor band-level params (idx >= 100) ===
        PluginSettings::MultibandCompressor { bands, .. } if param_idx >= 100 => {
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => {
                        band.threshold_db = Some(value as f32);
                        true
                    }
                    7 => {
                        band.ratio = Some(value as f32);
                        true
                    }
                    8 => {
                        band.attack_ms = Some(value as f32);
                        true
                    }
                    9 => {
                        band.release_ms = Some(value as f32);
                        true
                    }
                    10 => {
                        band.knee_db = Some(value as f32);
                        true
                    }
                    13 => {
                        band.makeup_gain_db = value as f32;
                        true
                    }
                    14 => {
                        band.bypass = value > 0.5;
                        true
                    }
                    15 => {
                        band.solo = value > 0.5;
                        true
                    }
                    16 => {
                        band.auto_makeup = value > 0.5;
                        true
                    }
                    17 => {
                        band.active = value > 0.5;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        // === MultibandExpander band-level params (idx >= 100) ===
        PluginSettings::MultibandExpander { bands, .. } if param_idx >= 100 => {
            let band_idx = (param_idx / 100) - 1;
            let local_idx = param_idx % 100;
            if let Some(band) = bands.get_mut(band_idx) {
                match local_idx {
                    6 => {
                        band.threshold_db = Some(value as f32);
                        true
                    }
                    7 => {
                        band.ratio = Some(value as f32);
                        true
                    }
                    8 => {
                        band.attack_ms = Some(value as f32);
                        true
                    }
                    9 => {
                        band.release_ms = Some(value as f32);
                        true
                    }
                    10 => {
                        band.range_db = Some(value as f32);
                        true
                    }
                    11 => {
                        band.knee_db = Some(value as f32);
                        true
                    }
                    12 => {
                        band.hysteresis_db = Some(value as f32);
                        true
                    }
                    13 => {
                        band.hold_ms = Some(value as f32);
                        true
                    }
                    14 => {
                        band.bypass = value > 0.5;
                        true
                    }
                    15 => {
                        band.solo = value > 0.5;
                        true
                    }
                    16 => {
                        band.auto_makeup = value > 0.5;
                        true
                    }
                    17 => {
                        band.active = value > 0.5;
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
                CrossfeedPreset::Hrtf,
            ];
            let idx = (value as usize).min(presets.len() - 1);
            *preset = presets[idx];
            let p_params = sotf_plugins::CrossfeedPluginParams::from_preset(*preset);
            *mode = p_params.mode;
            *bauer_fcut_hz = p_params.bauer_fcut_hz as f64;
            *bauer_feed_db = p_params.bauer_feed_db as f64;
            *meier_level = p_params.meier_level as f64;
            *mb_low_freq_hz = p_params.mb_low_freq_hz as f64;
            *mb_mid_high_freq_hz = p_params.mb_mid_high_freq_hz as f64;
            *mb_low_feed_db = p_params.mb_low_feed_db as f64;
            *mb_mid_feed_db = p_params.mb_mid_feed_db as f64;
            *mb_high_feed_db = p_params.mb_high_feed_db as f64;
            true
        }
        // === Generic path: all other plugins use set_param_value() ===
        other => {
            let specs = other.param_specs();
            if let Some(spec) = specs.get(param_idx) {
                let raw = value / spec.display_scale;
                other.set_param_value(param_idx, spec.clamp_f64(raw));
                apply_structural_side_effects(other, param_idx, channel_count_changed);
                true
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;
    use sotf_plugins::BandCompressorParams;

    #[test]
    fn set_plugin_param_value_eq_frequency_field() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
            auto_gain_enabled: false,
            oversampling: 1.0,
        };
        let mut changed = false;

        // Param index 0 = band 0 frequency
        assert!(set_plugin_param_value(
            &mut settings,
            0,
            2500.0,
            &mut changed
        ));
        match settings {
            PluginSettings::EQ { filters, .. } => assert_eq!(filters[0].frequency, 2500.0),
            _ => panic!("expected EQ"),
        }
        assert!(!changed);
    }

    #[test]
    fn set_plugin_param_value_eq_gain_clamped() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
            auto_gain_enabled: false,
            oversampling: 1.0,
        };
        let mut changed = false;

        // Param index 2 = band 0 gain_db; should clamp to [-24, 24]
        assert!(set_plugin_param_value(&mut settings, 2, 50.0, &mut changed));
        match settings {
            PluginSettings::EQ { filters, .. } => assert_eq!(filters[0].gain_db, 24.0),
            _ => panic!("expected EQ"),
        }
    }

    #[test]
    fn set_plugin_param_value_eq_q_notch_allows_up_to_40() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Notch, 1000.0, 1.0, 0.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
            auto_gain_enabled: false,
            oversampling: 1.0,
        };
        let mut changed = false;

        // Param index 1 = band 0 q; notch accepts up to 40
        assert!(set_plugin_param_value(&mut settings, 1, 25.0, &mut changed));
        match &settings {
            PluginSettings::EQ { filters, .. } => assert_eq!(filters[0].q, 25.0),
            _ => panic!("expected EQ"),
        }

        // Above 40 clamps to 40
        assert!(set_plugin_param_value(&mut settings, 1, 99.0, &mut changed));
        match settings {
            PluginSettings::EQ { filters, .. } => assert_eq!(filters[0].q, 40.0),
            _ => panic!("expected EQ"),
        }
    }

    #[test]
    fn set_plugin_param_value_eq_q_peak_clamps_to_20() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
            auto_gain_enabled: false,
            oversampling: 1.0,
        };
        let mut changed = false;

        assert!(set_plugin_param_value(&mut settings, 1, 25.0, &mut changed));
        match settings {
            PluginSettings::EQ { filters, .. } => assert_eq!(filters[0].q, 20.0),
            _ => panic!("expected EQ"),
        }
    }

    #[test]
    fn set_plugin_param_value_eq_type_switch_clamps_q() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Notch, 1000.0, 25.0, 0.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
            auto_gain_enabled: false,
            oversampling: 1.0,
        };
        let mut changed = false;

        // Switch band 0 (q=25 notch) to Peak (index 0): q must clamp to 20
        assert!(set_plugin_param_value(&mut settings, 3, 0.0, &mut changed));
        match settings {
            PluginSettings::EQ { filters, .. } => {
                assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
                assert_eq!(filters[0].q, 20.0);
            }
            _ => panic!("expected EQ"),
        }
    }

    #[test]
    fn set_plugin_param_value_eq_type_field() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0)],
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
            auto_gain_enabled: false,
            oversampling: 1.0,
        };
        let mut changed = false;

        // Param index 3 = band 0 filter type; 1 = Lowshelf
        assert!(set_plugin_param_value(&mut settings, 3, 1.0, &mut changed));
        match settings {
            PluginSettings::EQ { filters, .. } => {
                assert_eq!(filters[0].filter_type, BiquadFilterType::Lowshelf)
            }
            _ => panic!("expected EQ"),
        }
    }

    #[test]
    fn set_plugin_param_value_linear_phase_eq_uses_five_param_band_stride() {
        let mut settings = PluginSettings::LinearPhaseEq {
            filters: vec![
                EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0),
                EQFilter::new(BiquadFilterType::Peak, 2000.0, 1.0, 0.0),
            ],
            num_filters: 2.0,
            fir_length: 1024.0,
            phase_mode: 0.0,
            auto_gain: true,
            mix: 1.0,
        };
        let mut changed = false;

        // Band 1 frequency is index 6: 5 params for band 0, then local field 1.
        assert!(set_plugin_param_value(
            &mut settings,
            6,
            3200.0,
            &mut changed
        ));
        match settings {
            PluginSettings::LinearPhaseEq { filters, .. } => {
                assert_eq!(filters[0].frequency, 1000.0);
                assert_eq!(filters[1].frequency, 3200.0);
            }
            _ => panic!("expected LinearPhaseEq"),
        }
    }

    #[test]
    fn set_plugin_param_value_fir_eq_active_field_controls_mute_state() {
        let mut settings = PluginSettings::LinearPhaseEq {
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0)],
            num_filters: 1.0,
            fir_length: 2048.0,
            phase_mode: 0.0,
            auto_gain: true,
            mix: 1.0,
        };
        let mut changed = false;

        assert!(set_plugin_param_value(&mut settings, 4, 0.0, &mut changed));
        match &settings {
            PluginSettings::LinearPhaseEq { filters, .. } => assert!(filters[0].muted),
            _ => panic!("expected LinearPhaseEq"),
        }

        assert!(set_plugin_param_value(&mut settings, 4, 1.0, &mut changed));
        match settings {
            PluginSettings::LinearPhaseEq { filters, .. } => assert!(!filters[0].muted),
            _ => panic!("expected LinearPhaseEq"),
        }
    }

    #[test]
    fn set_plugin_param_value_gain() {
        let mut settings = PluginSettings::Gain {
            channels: 2,
            gain_db: 0.0,
            smoothing_ms: 20.0,
        };
        let mut changed = false;

        assert!(set_plugin_param_value(
            &mut settings,
            0,
            -12.0,
            &mut changed
        ));
        match settings {
            PluginSettings::Gain { gain_db, .. } => assert!((gain_db - -12.0).abs() < 0.01),
            _ => panic!("expected Gain"),
        }
    }

    #[test]
    fn set_plugin_param_value_multiband_compressor_band_threshold() {
        let mut settings = PluginSettings::MultibandCompressor {
            num_bands: 1,
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
            bands: vec![BandCompressorParams {
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
            }],
            sidechain_tilt_db: 0.0,
            link_amount: 0.0,
        };
        let mut changed = false;

        // Band-level threshold at index 106
        assert!(set_plugin_param_value(
            &mut settings,
            106,
            -15.0,
            &mut changed
        ));
        match settings {
            PluginSettings::MultibandCompressor { bands, .. } => {
                assert!((bands[0].threshold_db.unwrap() - -15.0).abs() < 0.01);
            }
            _ => panic!("expected MultibandCompressor"),
        }
    }

    #[test]
    fn set_plugin_param_value_returns_false_for_out_of_range_index() {
        let mut settings = PluginSettings::Gain {
            channels: 2,
            gain_db: 0.0,
            smoothing_ms: 20.0,
        };
        let mut changed = false;

        assert!(!set_plugin_param_value(
            &mut settings,
            999,
            0.0,
            &mut changed
        ));
    }
}
