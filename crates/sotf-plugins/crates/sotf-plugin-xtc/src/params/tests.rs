use super::*;
use sotf_host::param_specs::ParamType;
use sotf_host::plugin_params::PluginParamDef;

#[test]
fn canonical_runtime_defaults_match_every_schema_default() {
    let params = Params::default();
    for (index, spec) in PARAMS.iter().enumerate() {
        let Some(value) = params.param_value(index) else {
            assert!(matches!(spec.param_type, ParamType::FilePath));
            continue;
        };
        let expected = match spec.param_type {
            ParamType::Float { default, .. } => default,
            ParamType::Int { default, .. } => default as f64,
            ParamType::Bool { default, .. } => default as u8 as f64,
            ParamType::Choice { default_index, .. } => default_index as f64,
            ParamType::FilePath => unreachable!(),
        };
        assert!(
            (value - expected).abs() < 1e-6,
            "default drift for {}",
            spec.engine_key
        );
    }
}

#[test]
fn empty_json_and_default_are_identical() {
    let from_json: Params = serde_json::from_str("{}").unwrap();
    let default_json = serde_json::to_value(Params::default()).unwrap();
    assert_eq!(serde_json::to_value(from_json).unwrap(), default_json);
}

#[test]
fn preset_roundtrip_preserves_structural_and_runtime_fields() {
    let params = Params {
        fft_size: 4096,
        source_mode: "roomeq_recommended".into(),
        recommended_matrix_file: Some("matrix.json".into()),
        room_ir_file: Some("room.wav".into()),
        hrtf_file: None,
        head_shadow_cutoff_hz: 3200.0,
        auto_gain_max_db: 18.0,
        ..Default::default()
    };
    let encoded = serde_json::to_string(&params).unwrap();
    let decoded: Params = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(params).unwrap()
    );
}

#[test]
fn every_numeric_schema_entry_maps_both_directions() {
    let mut params = Params::default();
    for (index, spec) in PARAMS.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            continue;
        }
        let before = params.param_value(index).unwrap();
        let value = match spec.param_type {
            ParamType::Float { min, max, .. } => (min + max) * 0.5,
            ParamType::Int { min, max, .. } => ((min + max) / 2) as f64,
            ParamType::Bool { .. } => 1.0 - before,
            ParamType::Choice { labels, .. } => (labels.len() - 1) as f64,
            ParamType::FilePath => unreachable!(),
        };
        params.set_param_value(index, value);
        assert!(
            (params.param_value(index).unwrap() - value).abs() < 1e-6,
            "mapping drift for {}",
            spec.engine_key
        );
    }
}
