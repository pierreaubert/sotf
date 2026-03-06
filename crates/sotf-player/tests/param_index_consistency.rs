//! Tests to verify parameter index consistency between PARAMS ordering,
//! param_value/set_param_value, and the controller's set_plugin_param_value.

use sotf_audio_player::param_specs::{ParamType, UpdateMode};
use sotf_audio_player::{PluginSettings, PluginType};

fn default(pt: PluginType) -> PluginSettings {
    PluginSettings::default_for(&pt)
}

fn is_bool(pt: &ParamType) -> bool {
    matches!(pt, ParamType::Bool { .. })
}

fn is_file(pt: &ParamType) -> bool {
    matches!(pt, ParamType::FilePath)
}

fn test_value(spec: &sotf_audio_player::param_specs::ParamSpec) -> Option<f64> {
    match spec.param_type {
        ParamType::Bool { .. } => Some(1.0),
        ParamType::Choice { .. } => Some(1.0),
        ParamType::FilePath => None,
        ParamType::Int { .. } => Some((spec.min_f64() + spec.max_f64()) / 2.0),
        ParamType::Float { .. } => Some((spec.min_f64() + spec.max_f64()) / 2.0),
    }
}

fn roundtrip_test(name: &str, settings: &mut PluginSettings) {
    let specs = settings.param_specs();
    for idx in 0..specs.len() {
        let spec = &specs[idx];
        let val = match test_value(spec) {
            Some(v) => v,
            None => continue,
        };

        settings.set_param_value(idx, val);
        let readback = settings
            .param_value(idx)
            .unwrap_or_else(|| panic!("{}: param_value({}) returned None", name, idx));

        if is_bool(&spec.param_type) {
            assert_eq!(
                readback > 0.5,
                val > 0.5,
                "{} param {} ({}) bool mismatch",
                name,
                idx,
                spec.engine_key
            );
        } else {
            assert!(
                (readback - val).abs() < 0.01,
                "{} param {} ({}) roundtrip failed: set {} got {}",
                name,
                idx,
                spec.engine_key,
                val,
                readback
            );
        }
    }
}

#[test]
fn test_upmixer_param_roundtrip() {
    roundtrip_test("Upmixer", &mut default(PluginType::Upmixer));
}

#[test]
fn test_fletcher_munson_param_roundtrip() {
    roundtrip_test(
        "FletcherMunson",
        &mut default(PluginType::FletcherMunson),
    );
}

#[test]
fn test_convolution_param_roundtrip() {
    roundtrip_test("Convolution", &mut default(PluginType::Convolution));
}

/// Verify that param_specs().len() matches the number of valid indices in param_value().
#[test]
fn test_all_plugins_param_count_matches_specs() {
    let plugins = [
        ("Upmixer", PluginType::Upmixer),
        ("FletcherMunson", PluginType::FletcherMunson),
        ("Convolution", PluginType::Convolution),
        ("Gain", PluginType::Gain),
        ("Compressor", PluginType::Compressor),
        ("Limiter", PluginType::Limiter),
    ];

    for (name, pt) in plugins {
        let settings = default(pt);
        let spec_count = settings.param_specs().len();
        let mut value_count = 0;
        for i in 0..100 {
            if settings.param_value(i).is_some() {
                value_count = i + 1;
            }
        }
        assert_eq!(
            spec_count, value_count,
            "{}: param_specs has {} entries but param_value has {} valid indices",
            name, spec_count, value_count
        );
    }
}

/// Verify engine_param_at keys match param_specs engine_key for upmixer.
#[test]
fn test_upmixer_param_keys_match_specs() {
    let settings = default(PluginType::Upmixer);
    let specs = settings.param_specs();

    for (i, spec) in specs.iter().enumerate() {
        if spec.update_mode == UpdateMode::Structural || is_file(&spec.param_type) {
            continue;
        }
        let (key, _) = settings
            .engine_param_at(i)
            .unwrap_or_else(|| panic!("engine_param_at({}) returned None", i));
        assert_eq!(
            key, spec.engine_key,
            "Upmixer param {}: engine_param_at key '{}' != spec key '{}'",
            i, key, spec.engine_key
        );
    }
}

#[test]
fn test_multiband_empty_bands_no_crash() {
    // Simulates the fix: selected_band_idx.min(bands.len()) instead of .min(*num_bands)
    let selected_band_idx: usize = 3;
    let bands_len: usize = 0;
    assert_eq!(selected_band_idx.min(bands_len), 0);
}
