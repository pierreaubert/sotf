use super::super::{
    AudioEngineState, DsdOutputMode, EngineOversamplingPolicy, LatencyCompensationMode,
    NetworkEndpointConfig, OutputAccessMode, PlaybackState, PluginConfig, PluginGraphConfig,
    PluginGraphEdgeConfig, PluginGraphNodeConfig,
};
use super::engine_config::EngineConfig;
use super::misc::default_engine_config_version;
use std::path::PathBuf;

fn temp_config_path(test_name: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sotf-engine-types-{test_name}-{}-{unique}.json",
        std::process::id()
    ))
}

#[test]
fn test_queue_capacity_calculation() {
    let config = EngineConfig {
        frame_size: 1024,
        buffer_ms: 200,
        output_sample_rate: 48000,
        ..Default::default()
    };

    // 200ms at 48kHz = 9600 frames
    // 9600 / 1024 = ~9.375, rounds up to 10 chunks
    let capacity = config.queue_capacity_frames();
    assert_eq!(capacity, 10);

    let total_frames = config.total_buffer_frames();
    assert_eq!(total_frames, 9600);
}

#[test]
fn buffer_frame_calculations_round_up_fractional_milliseconds() {
    let config = EngineConfig {
        frame_size: 661,
        buffer_ms: 15,
        output_sample_rate: 44100,
        ..Default::default()
    };

    // 15ms at 44.1kHz = 661.5 frames, so both total frames and queue
    // capacity must reserve the fractional frame.
    assert_eq!(config.total_buffer_frames(), 662);
    assert_eq!(config.queue_capacity_frames(), 2);
}

#[test]
fn test_queue_capacity_different_rates() {
    let config = EngineConfig {
        frame_size: 512,
        buffer_ms: 100,
        output_sample_rate: 44100,
        ..Default::default()
    };

    // 100ms at 44.1kHz = 4410 frames
    // 4410 / 512 = ~8.6, rounds up to 9 chunks
    let capacity = config.queue_capacity_frames();
    assert_eq!(capacity, 9);
}

#[test]
fn test_frame_size_zero_does_not_panic() {
    let config = EngineConfig {
        frame_size: 0,
        buffer_ms: 200,
        output_sample_rate: 48000,
        ..Default::default()
    };
    // Should not panic — treats frame_size 0 as 1
    let capacity = config.queue_capacity_frames();
    assert_eq!(capacity, 9600);
}

#[test]
fn test_sanitize_fixes_zero_frame_size() {
    let mut config = EngineConfig {
        frame_size: 0,
        output_sample_rate: 0,
        ..Default::default()
    };
    config.sanitize();
    assert_eq!(config.frame_size, 1024);
    assert_eq!(config.output_sample_rate, 48000);
}

#[test]
fn validate_rejects_invalid_engine_invariants() {
    let mut config = EngineConfig {
        buffer_ms: 0,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("buffer_ms"));

    config = EngineConfig {
        input_channels: 0,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("input_channels"));

    config = EngineConfig {
        output_channels: 0,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("output_channels"));

    config = EngineConfig {
        volume: 1.5,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("volume"));

    config = EngineConfig {
        volume: f32::NAN,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("volume"));

    config = EngineConfig {
        plugins: vec![PluginConfig::new("", serde_json::json!({}))],
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("plugins[0]"));
}

#[test]
fn load_from_file_rejects_invalid_values_after_deserialize() {
    let path = temp_config_path("invalid-values");
    let config = EngineConfig {
        volume: 2.0,
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

    let error = EngineConfig::load_from_file(&path).unwrap_err();
    std::fs::remove_file(&path).unwrap();

    assert!(error.to_string().contains("volume"));
}

#[test]
fn save_to_file_rejects_invalid_values() {
    let path = temp_config_path("invalid-save");
    let config = EngineConfig {
        output_channels: 0,
        ..Default::default()
    };

    let error = config.save_to_file(&path).unwrap_err();

    assert!(error.to_string().contains("output_channels"));
    assert!(!path.exists());
}

#[test]
fn default_config_exposes_product_review_feature_policies() {
    let config = EngineConfig::default();
    assert_eq!(
        config.latency_compensation,
        LatencyCompensationMode::Enabled
    );
    assert_eq!(config.output_access, OutputAccessMode::Shared);
    assert_eq!(config.dsd_output, DsdOutputMode::Disabled);
    assert_eq!(
        config.oversampling_policy,
        EngineOversamplingPolicy::PluginPreferred
    );
    assert_eq!(config.network_endpoint, NetworkEndpointConfig::default());
}

#[test]
fn migrate_accepts_legacy_versions() {
    for version in [0, 1] {
        let config = EngineConfig {
            version,
            ..Default::default()
        };

        let migrated = EngineConfig::migrate(config).unwrap();
        assert_eq!(migrated.version, default_engine_config_version());
    }
}

#[test]
fn load_from_file_migrates_legacy_config() {
    let path = temp_config_path("legacy");
    let config = EngineConfig {
        version: 0,
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

    let loaded = EngineConfig::load_from_file(&path).unwrap();
    let persisted_json = std::fs::read_to_string(&path).unwrap();
    let persisted: EngineConfig = serde_json::from_str(&persisted_json).unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(loaded.version, default_engine_config_version());
    assert_eq!(persisted.version, default_engine_config_version());
}

#[test]
fn load_from_file_rejects_future_versions() {
    let path = temp_config_path("future");
    let config = EngineConfig {
        version: default_engine_config_version() + 1,
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

    let error = EngineConfig::load_from_file(&path).unwrap_err();
    std::fs::remove_file(&path).unwrap();

    assert!(error.to_string().contains("Unknown EngineConfig version"));
}

#[test]
fn deserializes_legacy_config_with_feature_policy_defaults() {
    let json = r#"{
            "version": 1,
            "frame_size": 512,
            "buffer_ms": 100,
            "output_sample_rate": 48000,
            "input_channels": 2,
            "output_channels": 2,
            "plugins": [],
            "volume": 1.0,
            "muted": false,
            "driver_mode": false,
            "allow_virtual_output": false
        }"#;

    let config: EngineConfig = serde_json::from_str(json).unwrap();
    assert_eq!(
        config.latency_compensation,
        LatencyCompensationMode::Enabled
    );
    assert_eq!(
        config.oversampling_policy,
        EngineOversamplingPolicy::PluginPreferred
    );
    assert_eq!(
        config.network_endpoint.mode,
        crate::NetworkEndpointMode::Disabled
    );
}

#[test]
fn validate_rejects_frame_size_zero() {
    let config = EngineConfig {
        frame_size: 0,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("frame_size"));
}

#[test]
fn validate_rejects_sizes_outside_allocation_free_contract() {
    let config = EngineConfig {
        frame_size: EngineConfig::MAX_FRAME_SIZE + 1,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("frame_size"));

    let config = EngineConfig {
        input_channels: EngineConfig::MAX_CHANNELS + 1,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("input_channels"));

    let config = EngineConfig {
        output_channels: EngineConfig::MAX_CHANNELS + 1,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("output_channels"));
}

#[test]
fn validate_rejects_output_sample_rate_zero() {
    let config = EngineConfig {
        output_sample_rate: 0,
        ..Default::default()
    };
    assert!(
        config
            .validate()
            .unwrap_err()
            .contains("output_sample_rate")
    );
}

#[test]
fn validate_rejects_version_too_new() {
    let config = EngineConfig {
        version: default_engine_config_version() + 1,
        ..Default::default()
    };
    assert!(
        config
            .validate()
            .unwrap_err()
            .contains("Unknown EngineConfig version")
    );
}

#[test]
fn validate_rejects_negative_volume() {
    let config = EngineConfig {
        volume: -0.1,
        ..Default::default()
    };
    assert!(config.validate().unwrap_err().contains("volume"));
}

#[test]
fn validate_rejects_infinite_volume() {
    for volume in [f32::INFINITY, f32::NEG_INFINITY] {
        let config = EngineConfig {
            volume,
            ..Default::default()
        };
        assert!(config.validate().unwrap_err().contains("volume"));
    }
}

#[test]
fn validate_accepts_default_config() {
    assert!(EngineConfig::default().validate().is_ok());
}

#[test]
fn try_new_returns_ok_for_valid_config() {
    let config = EngineConfig::default();
    let result = EngineConfig::try_new(config.clone()).unwrap();
    assert_eq!(result.frame_size, config.frame_size);
}

#[test]
fn try_new_returns_err_for_invalid_config() {
    let config = EngineConfig {
        buffer_ms: 0,
        ..Default::default()
    };
    assert!(
        EngineConfig::try_new(config)
            .unwrap_err()
            .contains("buffer_ms")
    );
}

// =========================================================================
// Schema / version compatibility tests (QA-CORE-001)
// =========================================================================

#[test]
fn engine_config_ignores_unknown_fields() {
    let json = r#"{
        "version": 2,
        "frame_size": 512,
        "buffer_ms": 100,
        "output_sample_rate": 48000,
        "input_channels": 2,
        "output_channels": 2,
        "plugins": [],
        "volume": 1.0,
        "muted": false,
        "driver_mode": false,
        "allow_virtual_output": false,
        "future_field": [1, 2, 3],
        "another_unknown": "value"
    }"#;

    let config: EngineConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.frame_size, 512);
    assert_eq!(config.output_sample_rate, 48000);
}

#[test]
fn engine_config_missing_optional_fields_use_defaults() {
    let json = r#"{
        "version": 1,
        "frame_size": 512,
        "buffer_ms": 100,
        "output_sample_rate": 48000,
        "input_channels": 2,
        "output_channels": 2,
        "plugins": [],
        "volume": 1.0,
        "muted": false
    }"#;

    let config: EngineConfig = serde_json::from_str(json).unwrap();
    assert!(!config.driver_mode);
    assert!(!config.allow_virtual_output);
    assert_eq!(
        config.latency_compensation,
        LatencyCompensationMode::Enabled
    );
    assert_eq!(config.output_access, OutputAccessMode::Shared);
    assert_eq!(config.dsd_output, DsdOutputMode::Disabled);
    assert_eq!(
        config.oversampling_policy,
        EngineOversamplingPolicy::PluginPreferred
    );
    assert_eq!(config.network_endpoint, NetworkEndpointConfig::default());
}

#[test]
fn engine_config_serde_roundtrip() {
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
        plugins: vec![PluginConfig::new("eq", serde_json::json!({"filters": []}))],
        oversampling_policy: EngineOversamplingPolicy::Force2x,
        output_access: OutputAccessMode::ExclusivePreferred,
        network_endpoint: NetworkEndpointConfig {
            mode: crate::NetworkEndpointMode::HttpEndpoint,
            bind_addr: "127.0.0.1".into(),
            port: 12345,
        },
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: EngineConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.frame_size, config.frame_size);
    assert_eq!(decoded.output_sample_rate, config.output_sample_rate);
    assert_eq!(decoded.volume, config.volume);
    assert_eq!(decoded.plugins.len(), config.plugins.len());
}

#[test]
fn audio_engine_state_ignores_unknown_fields() {
    let json = r#"{
        "playback_state": "Playing",
        "current_source": null,
        "current_file": null,
        "position": 12.5,
        "duration": 180.0,
        "sample_rate": 48000,
        "num_channels": 2,
        "volume": 0.8,
        "muted": false,
        "processing_bypassed": false,
        "underruns": 0,
        "plugin_latency_samples": 0,
        "last_error": null,
        "seeking": false,
        "future_state_field": true,
        "unknown_nested": {"x": 1}
    }"#;

    let state: AudioEngineState = serde_json::from_str(json).unwrap();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(state.position, 12.5);
    assert_eq!(state.sample_rate, 48000);
}

#[test]
fn audio_engine_state_missing_optional_fields_use_defaults() {
    let json = r#"{
        "playback_state": "Stopped",
        "current_source": null,
        "current_file": null,
        "position": 0.0,
        "duration": null,
        "sample_rate": 48000,
        "num_channels": 2,
        "volume": 1.0,
        "muted": false,
        "processing_bypassed": false,
        "underruns": 0,
        "plugin_latency_samples": 0,
        "last_error": null,
        "seeking": false
    }"#;

    let state: AudioEngineState = serde_json::from_str(json).unwrap();
    assert_eq!(state.playback_output_device, None);
    assert_eq!(state.playback_callback_count, 0);
    assert_eq!(state.playback_buffer_fill_percent, 0);
    assert_eq!(state.output_peak_linear, 0.0);
    assert!(!state.output_clipping_detected);
    assert!(state.latency_compensation_enabled);
    assert_eq!(state.output_access_mode, OutputAccessMode::Shared);
    assert!(state.plugin_build_diagnostics.is_empty());
    assert!(state.isolated_external_plugin_worker_statuses.is_empty());
}

#[test]
fn audio_engine_state_serde_roundtrip() {
    let state = AudioEngineState {
        playback_state: PlaybackState::Paused,
        current_source: Some(crate::AudioSource::File(PathBuf::from("/tmp/test.flac"))),
        current_file: Some(PathBuf::from("/tmp/test.flac")),
        position: 30.0,
        duration: Some(240.0),
        sample_rate: 96000,
        num_channels: 2,
        playback_channels: 2,
        volume: 0.5,
        muted: true,
        processing_bypassed: false,
        underruns: 2,
        playback_output_device: Some("default".into()),
        playback_callback_count: 100,
        playback_buffer_fill_percent: 75,
        playback_stream_error_count: 0,
        playback_frames_received: 1000,
        playback_frames_written: 999,
        playback_frames_dropped: 1,
        playback_effective_sample_rate: 96000,
        output_peak_linear: 0.75,
        output_clipping_detected: true,
        plugin_latency_samples: 512,
        latency_compensation_enabled: false,
        output_access_mode: OutputAccessMode::ExclusivePreferred,
        output_access_status: crate::OutputAccessStatus::ExclusiveActive,
        dsd_output_mode: DsdOutputMode::Disabled,
        dsd_output_status: crate::DsdOutputStatus::Disabled,
        oversampling_policy: EngineOversamplingPolicy::Force2x,
        network_endpoint: NetworkEndpointConfig::default(),
        network_endpoint_status: crate::NetworkEndpointStatus::Disabled,
        stream_metadata: Some(crate::StreamMetadata {
            stream_title: Some("Test".into()),
            stream_url: None,
            content_type: Some("audio/flac".into()),
            bitrate_kbps: Some(1411),
        }),
        last_error: None,
        plugin_build_diagnostics: vec![crate::PluginBuildDiagnostic::graph_node(
            7,
            Some(42),
            "external",
            "worker could not load plugin",
        )],
        seeking: false,
        isolated_external_plugin_worker_statuses: Vec::new(),
    };

    let json = serde_json::to_string(&state).unwrap();
    let decoded: AudioEngineState = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.playback_state, state.playback_state);
    assert_eq!(decoded.position, state.position);
    assert_eq!(
        decoded.plugin_build_diagnostics,
        state.plugin_build_diagnostics
    );
    assert_eq!(decoded.volume, state.volume);
    assert_eq!(decoded.muted, state.muted);
    assert_eq!(decoded.playback_output_device, state.playback_output_device);
}

#[test]
fn plugin_config_serde_roundtrip() {
    let config = PluginConfig::new(
        "eq",
        serde_json::json!({
            "filters": [
                {"filter_type": "peak", "frequency": 1000.0, "q": 1.5, "gain_db": 3.0}
            ]
        }),
    );

    let json = serde_json::to_string(&config).unwrap();
    let decoded: PluginConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.plugin_type, config.plugin_type);
    assert_eq!(decoded.parameters, config.parameters);
}

#[test]
fn plugin_graph_config_ignores_unknown_fields() {
    let json = r#"{
        "nodes": [
            {"id": 0, "plugin_type": "eq", "parameters": {"filters": []}, "input_channels": 2}
        ],
        "edges": [],
        "future_graph_field": "ignored"
    }"#;

    let graph: PluginGraphConfig = serde_json::from_str(json).unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.edges.len(), 0);
}

#[test]
fn plugin_graph_config_serde_roundtrip() {
    let graph = PluginGraphConfig::try_new(
        vec![
            PluginGraphNodeConfig::try_new(0, "eq", serde_json::json!({"filters": []}), 2).unwrap(),
            PluginGraphNodeConfig::try_new(1, "gain", serde_json::json!({"gain_db": -6.0}), 2)
                .unwrap(),
        ],
        vec![PluginGraphEdgeConfig::new(0, 1)],
    )
    .unwrap();

    let json = serde_json::to_string(&graph).unwrap();
    let decoded: PluginGraphConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.nodes.len(), graph.nodes.len());
    assert_eq!(decoded.edges.len(), graph.edges.len());
    assert_eq!(decoded.nodes[0].plugin_type, "eq");
    assert_eq!(decoded.nodes[1].plugin_type, "gain");
}
