use super::Params;
use super::consts::PARAMS;
use sotf_host::param_specs::find_by_key as pk;

use super::*;

#[test]
fn param_index_coverage() {
    let p = Params::default();
    for (i, spec) in PARAMS.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            assert!(
                p.param_value(i).is_none(),
                "param_value({}) should return None for FilePath",
                i
            );
        } else {
            assert!(
                p.param_value(i).is_some(),
                "param_value({}) returned None",
                i
            );
        }
    }
    assert!(
        p.param_value(PARAMS.len()).is_none(),
        "param_value beyond PARAMS.len() should return None"
    );
}

#[test]
fn roundtrip_serde() {
    let original = Params::default();
    let json = serde_json::to_value(&original).unwrap();
    let restored: Params = serde_json::from_value(json).unwrap();
    assert_eq!(original.distance_m, restored.distance_m);
    assert_eq!(original.speaker_angle_deg, restored.speaker_angle_deg);
    assert_eq!(original.head_radius_m, restored.head_radius_m);
    assert_eq!(original.head_offset_x, restored.head_offset_x);
    assert_eq!(original.head_offset_z, restored.head_offset_z);
    assert_eq!(original.head_yaw_deg, restored.head_yaw_deg);
    assert_eq!(
        original.head_tracking_smooth_s,
        restored.head_tracking_smooth_s
    );
    assert_eq!(original.beta_base, restored.beta_base);
    assert_eq!(original.beta_low_freq_boost, restored.beta_low_freq_boost);
    assert_eq!(original.beta_high_freq_boost, restored.beta_high_freq_boost);
    assert_eq!(
        original.head_shadow_cutoff_hz,
        restored.head_shadow_cutoff_hz
    );
    assert_eq!(
        original.head_shadow_slope_db_per_octave,
        restored.head_shadow_slope_db_per_octave
    );
    assert_eq!(original.max_gain_db, restored.max_gain_db);
    assert_eq!(
        original.spectral_normalization,
        restored.spectral_normalization
    );
    assert_eq!(original.pinna_model_enabled, restored.pinna_model_enabled);
    assert_eq!(
        original.room_reflections_enabled,
        restored.room_reflections_enabled
    );
    assert_eq!(original.room_width_m, restored.room_width_m);
    assert_eq!(original.room_depth_m, restored.room_depth_m);
    assert_eq!(original.wall_absorption, restored.wall_absorption);
    assert_eq!(
        original.reflection_beta_boost,
        restored.reflection_beta_boost
    );
    assert_eq!(original.bypass_xtc_filters, restored.bypass_xtc_filters);
    assert_eq!(
        original.bypass_spectral_normalization,
        restored.bypass_spectral_normalization
    );
    assert_eq!(
        original.bypass_neumann_refinement,
        restored.bypass_neumann_refinement
    );
    assert_eq!(original.auto_gain_enabled, restored.auto_gain_enabled);
    assert_eq!(original.auto_gain_max_db, restored.auto_gain_max_db);
    assert_eq!(
        original.auto_gain_smoothing_ms,
        restored.auto_gain_smoothing_ms
    );
}

#[test]
fn deserialize_empty_json_uses_defaults() {
    let p: Params = serde_json::from_str("{}").unwrap();
    assert_eq!(p.distance_m, pk(PARAMS, "distance_m").default_f64());
    assert_eq!(
        p.speaker_angle_deg,
        pk(PARAMS, "speaker_angle_deg").default_f64()
    );
    assert_eq!(p.head_radius_m, pk(PARAMS, "head_radius_m").default_f64());
    assert_eq!(p.head_offset_x, pk(PARAMS, "head_offset_x").default_f64());
    assert_eq!(p.head_offset_z, pk(PARAMS, "head_offset_z").default_f64());
    assert_eq!(p.head_yaw_deg, pk(PARAMS, "head_yaw_deg").default_f64());
    assert_eq!(
        p.head_tracking_smooth_s,
        pk(PARAMS, "head_tracking_smooth_s").default_f64()
    );
    assert_eq!(p.beta_base, pk(PARAMS, "beta_base").default_f64());
    assert_eq!(
        p.beta_low_freq_boost,
        pk(PARAMS, "beta_low_freq_boost").default_f64()
    );
    assert_eq!(
        p.beta_high_freq_boost,
        pk(PARAMS, "beta_high_freq_boost").default_f64()
    );
    assert_eq!(
        p.head_shadow_cutoff_hz,
        pk(PARAMS, "head_shadow_cutoff_hz").default_f64()
    );
    assert_eq!(
        p.head_shadow_slope_db_per_octave,
        pk(PARAMS, "head_shadow_slope_db_per_octave").default_f64()
    );
    assert_eq!(p.max_gain_db, pk(PARAMS, "max_gain_db").default_f64());
    assert_eq!(
        p.spectral_normalization,
        pk(PARAMS, "spectral_normalization").default_bool()
    );
    assert_eq!(
        p.pinna_model_enabled,
        pk(PARAMS, "pinna_model_enabled").default_bool()
    );
    assert_eq!(
        p.room_reflections_enabled,
        pk(PARAMS, "room_reflections_enabled").default_bool()
    );
    assert_eq!(p.room_width_m, pk(PARAMS, "room_width_m").default_f64());
    assert_eq!(p.room_depth_m, pk(PARAMS, "room_depth_m").default_f64());
    assert_eq!(
        p.wall_absorption,
        pk(PARAMS, "wall_absorption").default_f64()
    );
    assert_eq!(
        p.reflection_beta_boost,
        pk(PARAMS, "reflection_beta_boost").default_f64()
    );
    assert_eq!(
        p.bypass_xtc_filters,
        pk(PARAMS, "bypass_xtc_filters").default_bool()
    );
    assert_eq!(
        p.bypass_spectral_normalization,
        pk(PARAMS, "bypass_spectral_normalization").default_bool()
    );
    assert_eq!(
        p.bypass_neumann_refinement,
        pk(PARAMS, "bypass_neumann_refinement").default_bool()
    );
    assert_eq!(
        p.auto_gain_enabled,
        pk(PARAMS, "auto_gain_enabled").default_bool()
    );
    assert_eq!(
        p.auto_gain_max_db,
        pk(PARAMS, "auto_gain_max_db").default_f64()
    );
    assert_eq!(
        p.auto_gain_smoothing_ms,
        pk(PARAMS, "auto_gain_smoothing_ms").default_f64()
    );
}

use sotf_host::param_specs::ParamType;

#[test]
fn test_param_value_set_param_value_roundtrip() {
    let mut p = Params::default();
    for i in 0..PARAMS.len() {
        let spec = &PARAMS[i];
        if matches!(spec.param_type, ParamType::FilePath) {
            assert!(
                p.param_value(i).is_none(),
                "param_value({}) should return None for FilePath",
                i
            );
            continue;
        }
        let new_val = match spec.param_type {
            ParamType::Bool { .. } => 1.0,
            _ => spec.min_f64() + 0.5 * (spec.max_f64() - spec.min_f64()),
        };
        p.set_param_value(i, new_val);
        let retrieved = p.param_value(i).unwrap();
        if let ParamType::Bool { .. } = spec.param_type {
            assert!(
                (retrieved - 1.0).abs() < 0.001,
                "bool param {} roundtrip failed",
                spec.engine_key
            );
        } else {
            assert!(
                (retrieved - new_val).abs() < 0.001,
                "param {} roundtrip failed",
                spec.engine_key
            );
        }
    }
}

#[test]
fn test_param_value_out_of_range() {
    let p = Params::default();
    assert!(p.param_value(PARAMS.len()).is_none());
    assert!(p.param_value(999).is_none());
}

#[test]
fn test_set_param_value_out_of_range() {
    let mut p = Params::default();
    // Should not panic
    p.set_param_value(999, 1.0);
    p.set_param_value(PARAMS.len(), 1.0);
}
