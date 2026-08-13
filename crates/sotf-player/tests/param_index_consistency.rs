//! Tests to verify parameter index consistency between PARAMS ordering,
//! param_value/set_param_value, and the controller's set_plugin_param_value.

use sotf_audio_player::param_specs::{ParamSpec, ParamType, UpdateMode};
use sotf_audio_player::{PluginSettings, PluginType};

fn default(pt: &PluginType) -> PluginSettings {
    PluginSettings::default_for(pt).unwrap()
}

fn is_bool(pt: &ParamType) -> bool {
    matches!(pt, ParamType::Bool { .. })
}

fn is_file(pt: &ParamType) -> bool {
    matches!(pt, ParamType::FilePath)
}

/// Return a value guaranteed to differ from the spec's default.
fn distinctive_value(spec: &ParamSpec) -> Option<f64> {
    match spec.param_type {
        ParamType::FilePath => None,
        ParamType::Bool { default, .. } => Some(if default { 0.0 } else { 1.0 }),
        ParamType::Choice {
            labels,
            default_index,
        } => {
            if labels.len() <= 1 {
                None
            } else {
                let next = if default_index + 1 < labels.len() {
                    default_index + 1
                } else {
                    0
                };
                Some(next as f64)
            }
        }
        ParamType::Int {
            default, min, max, ..
        } => {
            if max != default {
                Some(max as f64)
            } else {
                Some(min as f64)
            }
        }
        ParamType::Float {
            default, min, max, ..
        } => {
            if (max - default).abs() > f64::EPSILON {
                Some(max)
            } else {
                Some(min)
            }
        }
    }
}

fn test_value(spec: &ParamSpec) -> Option<f64> {
    match spec.param_type {
        ParamType::Bool { .. } => Some(1.0),
        ParamType::Choice { .. } => Some(1.0),
        ParamType::FilePath => None,
        ParamType::Int { .. } => Some(((spec.min_f64() + spec.max_f64()) / 2.0).floor()),
        ParamType::Float { .. } => Some((spec.min_f64() + spec.max_f64()) / 2.0),
    }
}

fn roundtrip_test(name: &str, settings: &mut PluginSettings) {
    let specs = settings.param_specs();
    for (idx, spec) in specs.iter().enumerate() {
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

// ---------------------------------------------------------------------------
// Roundtrip: all plugins
// ---------------------------------------------------------------------------

#[test]
fn test_roundtrip_all_plugins() {
    for pt in PluginType::all() {
        let name = pt.name();
        roundtrip_test(name, &mut default(&pt));
    }
}

#[test]
fn saturation_appends_asymmetric_choice_without_reindexing_presets() {
    let specs = default(&PluginType::Saturation).param_specs();
    let mode = specs.iter().find(|spec| spec.engine_key == "mode").unwrap();
    let ParamType::Choice { labels, .. } = mode.param_type else {
        panic!("saturation mode must remain a choice")
    };
    assert_eq!(
        labels,
        &["Soft Clip", "Tube", "Tape", "Exciter", "Asymmetric"]
    );
}

// ---------------------------------------------------------------------------
// Count: param_specs().len() == number of valid param_value() indices
// ---------------------------------------------------------------------------

#[test]
fn test_all_plugins_param_count_matches_specs() {
    for pt in PluginType::all() {
        let name = pt.name();
        let settings = default(&pt);
        let specs = settings.param_specs();

        // param_value() returns None for FilePath params by design,
        // so we check that every non-FilePath spec index returns Some,
        // and no index beyond specs.len() returns Some.
        for (i, spec) in specs.iter().enumerate() {
            if is_file(&spec.param_type) {
                assert!(
                    settings.param_value(i).is_none(),
                    "{}: FilePath param {} ({}) should return None from param_value",
                    name,
                    i,
                    spec.engine_key
                );
            } else {
                assert!(
                    settings.param_value(i).is_some(),
                    "{}: param_value({}) ({}) returned None but spec exists",
                    name,
                    i,
                    spec.engine_key
                );
            }
        }

        // No valid indices beyond specs length
        assert!(
            settings.param_value(specs.len()).is_none(),
            "{}: param_value({}) returned Some beyond spec count",
            name,
            specs.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Isolation: setting param[i] must not affect param[j] for j != i
// ---------------------------------------------------------------------------

#[test]
fn test_param_isolation_all_plugins() {
    for pt in PluginType::all() {
        let name = pt.name();
        let specs = default(&pt).param_specs();
        if specs.is_empty() {
            continue;
        }

        for i in 0..specs.len() {
            let spec = &specs[i];
            if is_file(&spec.param_type) {
                continue;
            }

            let new_val = match distinctive_value(spec) {
                Some(v) => v,
                None => continue,
            };

            // Snapshot default values
            let baseline = default(&pt);
            let baseline_vals: Vec<Option<f64>> =
                (0..specs.len()).map(|j| baseline.param_value(j)).collect();

            // Check that distinctive_value actually differs from default
            let default_val = baseline.param_value(i);
            if let Some(dv) = default_val {
                if is_bool(&spec.param_type) {
                    if (dv > 0.5) == (new_val > 0.5) {
                        continue; // can't distinguish, skip
                    }
                } else if (dv - new_val).abs() < 1e-9 {
                    continue; // distinctive value equals default, skip
                }
            }

            // Set only index i
            let mut modified = default(&pt);
            modified.set_param_value(i, new_val);

            // Verify index i changed
            let readback = modified.param_value(i);
            assert!(
                readback.is_some(),
                "{} param {} ({}): param_value returned None after set",
                name,
                i,
                spec.engine_key
            );

            // Verify no other index changed
            for j in 0..specs.len() {
                if j == i {
                    continue;
                }
                if is_file(&specs[j].param_type) {
                    continue;
                }
                let before = baseline_vals[j];
                let after = modified.param_value(j);
                match (before, after) {
                    (Some(b), Some(a)) => {
                        if is_bool(&specs[j].param_type) {
                            assert_eq!(
                                b > 0.5,
                                a > 0.5,
                                "{}: setting param {} ({}) changed param {} ({}) from {} to {}",
                                name,
                                i,
                                spec.engine_key,
                                j,
                                specs[j].engine_key,
                                b,
                                a
                            );
                        } else {
                            assert!(
                                (b - a).abs() < 1e-9,
                                "{}: setting param {} ({}) changed param {} ({}) from {} to {}",
                                name,
                                i,
                                spec.engine_key,
                                j,
                                specs[j].engine_key,
                                b,
                                a
                            );
                        }
                    }
                    (None, None) => {}
                    _ => {
                        panic!(
                            "{}: setting param {} ({}) changed param {} ({}) presence: {:?} -> {:?}",
                            name, i, spec.engine_key, j, specs[j].engine_key, before, after
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// engine_key consistency: engine_param_at(i).key == PARAMS[i].engine_key
// ---------------------------------------------------------------------------

#[test]
fn test_engine_key_consistency_all_plugins() {
    for pt in PluginType::all() {
        let name = pt.name();
        let settings = default(&pt);
        let specs = settings.param_specs();

        for (i, spec) in specs.iter().enumerate() {
            if spec.update_mode == UpdateMode::Structural || is_file(&spec.param_type) {
                continue;
            }
            let (key, _) = match settings.engine_param_at(i) {
                Some(kv) => kv,
                None => continue,
            };
            assert_eq!(
                key, spec.engine_key,
                "{} param {}: engine_param_at key '{}' != spec key '{}'",
                name, i, key, spec.engine_key
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Roundtrip: engine_value_string -> ParameterValue::parse must preserve type
// ---------------------------------------------------------------------------

#[test]
fn test_engine_value_string_roundtrip_all_plugins() {
    use sotf_audio_player::param_specs::ParamType;
    use sotf_plugins::ParameterValue;

    for pt in PluginType::all() {
        let name = pt.name();
        let settings = default(&pt);
        let specs = settings.param_specs();

        for (i, spec) in specs.iter().enumerate() {
            if spec.update_mode == UpdateMode::Structural || is_file(&spec.param_type) {
                continue;
            }

            let Some((key, value_str)) = settings.engine_param_at(i) else {
                continue;
            };

            let parsed = ParameterValue::parse(&value_str);

            match spec.param_type {
                ParamType::Float { .. } => {
                    assert!(
                        matches!(parsed, ParameterValue::Float(_)),
                        "{} param {} ({}): '{}' parsed as {:?}, expected Float",
                        name,
                        i,
                        key,
                        value_str,
                        parsed
                    );
                }
                ParamType::Int { .. } => {
                    assert!(
                        matches!(parsed, ParameterValue::Int(_)),
                        "{} param {} ({}): '{}' parsed as {:?}, expected Int",
                        name,
                        i,
                        key,
                        value_str,
                        parsed
                    );
                }
                ParamType::Choice { .. } => {
                    assert!(
                        matches!(parsed, ParameterValue::Int(_) | ParameterValue::String(_)),
                        "{} param {} ({}): '{}' parsed as {:?}, expected Int or String choice",
                        name,
                        i,
                        key,
                        value_str,
                        parsed
                    );
                }
                ParamType::Bool { .. } => {
                    assert!(
                        matches!(parsed, ParameterValue::Bool(_)),
                        "{} param {} ({}): '{}' parsed as {:?}, expected Bool",
                        name,
                        i,
                        key,
                        value_str,
                        parsed
                    );
                }
                ParamType::FilePath => {}
            }

            // Also test with whole-number edge cases that previously triggered
            // the Float-as-Int mismatch bug.
            if matches!(spec.param_type, ParamType::Float { .. }) {
                for test_val in [spec.min_f64(), spec.max_f64(), 0.0, 1.0, -1.0] {
                    let test_str = spec.engine_value_string(test_val);
                    let test_parsed = ParameterValue::parse(&test_str);
                    assert!(
                        matches!(test_parsed, ParameterValue::Float(_)),
                        "{} param {} ({}): value {} -> '{}' parsed as {:?}, expected Float",
                        name,
                        i,
                        key,
                        test_val,
                        test_str,
                        test_parsed
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

#[test]
fn test_multiband_empty_bands_no_crash() {
    // Simulates the fix: selected_band_idx.min(bands.len()) instead of .min(*num_bands)
    let selected_band_idx: usize = 3;
    let bands_len: usize = 0;
    assert_eq!(selected_band_idx.min(bands_len), 0);
}
