//! Snapshot tests for plugin racks/configs and parameter descriptors.

use serde_json::json;
use sotf_plugins::create_plugin;
use sotf_types::PluginConfig;

#[test]
fn snapshot_eq_rack_plugin_config() {
    let config = PluginConfig::new(
        "eq",
        json!({
            "filters": [
                { "filter_type": "peak", "frequency": 1000.0, "q": 1.5, "gain_db": 3.0 },
                { "filter_type": "lowshelf", "frequency": 80.0, "q": 0.7, "gain_db": -2.0 }
            ]
        }),
    );
    insta::assert_json_snapshot!(config);
}

#[test]
fn snapshot_compressor_plugin_config() {
    let config = PluginConfig::new(
        "compressor",
        json!({
            "threshold_db": -12.0,
            "ratio": 4.0,
            "attack_ms": 10.0,
            "release_ms": 100.0,
            "makeup_db": 0.0
        }),
    );
    insta::assert_json_snapshot!(config);
}

#[test]
fn snapshot_upmixer_plugin_config() {
    let config = PluginConfig::new(
        "upmixer",
        json!({
            "mode": "5.1",
            "width": 1.0,
            "center_focus": 0.5
        }),
    );
    insta::assert_json_snapshot!(config);
}

#[test]
fn snapshot_gain_parameter_descriptors() {
    let plugin = create_plugin("gain", &json!({}), 2, 48_000).expect("gain plugin creates");
    let params = plugin.parameters();
    insta::assert_json_snapshot!(params);
}
