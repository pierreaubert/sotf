//! Snapshot tests for plugin racks/configs and parameter descriptors.

use serde_json::{Value, json};
use sotf_audio::PluginConfig;
use sotf_plugins::create_plugin;

/// Recursively sort object keys so snapshots are stable regardless of
/// serde_json's map backend (BTreeMap vs IndexMap/preserve_order).
fn sort_json_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, sort_json_keys(v)))
                    .collect(),
            )
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_json_keys).collect()),
        other => other,
    }
}

fn sorted_config(plugin_type: &str, parameters: Value) -> PluginConfig {
    PluginConfig::new(plugin_type, sort_json_keys(parameters))
}

#[test]
fn snapshot_eq_rack_plugin_config() {
    let config = sorted_config(
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
    let config = sorted_config(
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
    let config = sorted_config(
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
