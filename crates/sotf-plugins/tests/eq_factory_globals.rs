use sotf_host::{ParameterId, ParameterValue};

#[test]
fn eq_factory_applies_global_construction_controls() {
    let plugin = sotf_plugins::create_plugin(
        "eq",
        &serde_json::json!({
            "filters": [{
                "filter_type": "peak",
                "freq": 1000.0,
                "q": 1.0,
                "db_gain": 3.0
            }],
            "tdf2": true,
            "auto_gain": {"enabled": false},
            "oversampling": 4.0
        }),
        2,
        48_000,
    )
    .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("tdf2")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("auto_gain_enabled")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("oversampling")),
        Some(ParameterValue::Int(4))
    );
}
