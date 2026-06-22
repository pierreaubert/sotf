//! Snapshot tests for SOTF shared configuration types.

use serde_json::json;
use sotf_types::{
    EngineConfig, EngineOversamplingPolicy, NetworkEndpointConfig, NetworkEndpointMode,
    OutputAccessMode, PluginConfig, PluginGraphConfig, PluginGraphEdgeConfig,
    PluginGraphNodeConfig,
};

fn eq_plugin() -> PluginConfig {
    PluginConfig::new(
        "eq",
        json!({
            "filters": [
                {
                    "filter_type": "peak",
                    "frequency": 1000.0,
                    "q": 1.5,
                    "gain_db": 3.0
                }
            ]
        }),
    )
}

fn compressor_plugin() -> PluginConfig {
    PluginConfig::new(
        "compressor",
        json!({
            "threshold_db": -12.0,
            "ratio": 4.0,
            "attack_ms": 10.0,
            "release_ms": 100.0,
            "makeup_db": 0.0
        }),
    )
}

fn gain_plugin() -> PluginConfig {
    PluginConfig::new("gain", json!({ "gain_db": -6.0 }))
}

#[test]
fn snapshot_engine_config_default() {
    let config = EngineConfig::default();
    insta::assert_json_snapshot!(config);
}

#[test]
fn snapshot_engine_config_populated() {
    let config = EngineConfig {
        frame_size: 512,
        buffer_ms: 100,
        output_sample_rate: 96000,
        input_channels: 2,
        output_channels: 6,
        volume: 0.75,
        muted: true,
        driver_mode: true,
        allow_virtual_output: true,
        plugins: vec![eq_plugin(), compressor_plugin(), gain_plugin()],
        oversampling_policy: EngineOversamplingPolicy::Force2x,
        output_access: OutputAccessMode::ExclusivePreferred,
        network_endpoint: NetworkEndpointConfig {
            mode: NetworkEndpointMode::HttpEndpoint,
            bind_addr: "127.0.0.1".into(),
            port: 12345,
        },
        ..Default::default()
    };
    insta::assert_json_snapshot!(config);
}

#[test]
fn snapshot_plugin_graph_eq_compressor_gain() {
    let graph = PluginGraphConfig::try_new(
        vec![
            PluginGraphNodeConfig::try_new(0, "eq", eq_plugin().parameters, 2).unwrap(),
            PluginGraphNodeConfig::try_new(1, "compressor", compressor_plugin().parameters, 2)
                .unwrap(),
            PluginGraphNodeConfig::try_new(2, "gain", gain_plugin().parameters, 2).unwrap(),
        ],
        vec![
            PluginGraphEdgeConfig::new(0, 1),
            PluginGraphEdgeConfig::new(1, 2),
        ],
    )
    .expect("valid DAG");
    insta::assert_json_snapshot!(graph);
}

#[test]
fn snapshot_plugin_graph_validation_errors() {
    let cycle = PluginGraphConfig::try_new(
        vec![
            PluginGraphNodeConfig::try_new(0, "gain", json!({}), 2).unwrap(),
            PluginGraphNodeConfig::try_new(1, "gain", json!({}), 2).unwrap(),
        ],
        vec![
            PluginGraphEdgeConfig::new(0, 1),
            PluginGraphEdgeConfig::new(1, 0),
        ],
    );

    let duplicate_ids = PluginGraphConfig::try_new(
        vec![
            PluginGraphNodeConfig::try_new(0, "gain", json!({}), 2).unwrap(),
            PluginGraphNodeConfig::try_new(0, "eq", json!({}), 2).unwrap(),
        ],
        vec![],
    );

    let missing_endpoint = PluginGraphConfig::try_new(
        vec![PluginGraphNodeConfig::try_new(0, "gain", json!({}), 2).unwrap()],
        vec![PluginGraphEdgeConfig::new(0, 99)],
    );

    let zero_channels = PluginGraphNodeConfig::try_new(0, "gain", json!({}), 0);

    let empty_type = PluginGraphNodeConfig::try_new(0, "   ", json!({}), 2);

    insta::assert_debug_snapshot!(
        "validation_errors",
        [
            cycle.unwrap_err().to_string(),
            duplicate_ids.unwrap_err().to_string(),
            missing_endpoint.unwrap_err().to_string(),
            zero_channels.unwrap_err().to_string(),
            empty_type.unwrap_err().to_string(),
        ]
    );
}
