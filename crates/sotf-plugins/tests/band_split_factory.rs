use sotf_plugins::{PluginLatencyModel, catalog_entry, create_plugin};

#[test]
fn band_split_catalog_and_factory_enforce_runtime_contracts() {
    let entry = catalog_entry("band_split").expect("Band Split catalog entry");
    assert_eq!(entry.metadata.latency_model, PluginLatencyModel::Zero);
    assert!(
        entry
            .metadata
            .channel_layout
            .supported_inputs
            .supports(12)
            .unwrap()
    );

    let valid = serde_json::json!({
        "frequencies": [200.0, 2_000.0, 8_000.0],
        "type": "LR48"
    });
    let plugin = create_plugin("band_split", &valid, 12, 48_000).unwrap();
    assert_eq!(plugin.input_channels(), 12);
    assert_eq!(plugin.output_channels(), 48);

    for invalid in [
        serde_json::json!({"frequencies": [2_000.0, 200.0]}),
        serde_json::json!({"frequency": 1_000.0, "type": "unknown"}),
        serde_json::json!({"frequency": 1_000.0, "unknown": true}),
    ] {
        assert!(create_plugin("band_split", &invalid, 2, 48_000).is_err());
    }
    assert!(create_plugin("band_split", &serde_json::json!({}), 0, 48_000).is_err());
}
