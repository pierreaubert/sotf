//! Integration tests for the engine's public types, exercising the API as a black box.
//!
//! These tests cover:
//! - Serialization/deserialization roundtrips for the public types
//! - Config validation happy paths and error paths
//! - Public type conversions and helper methods
//! - Simple state transitions

use serde::{Deserialize, Serialize};
use serde_json::json;
use sotf_audio::{
    AudioEngineState, AudioFrame, AudioSource, DsdOutputMode, DsdOutputStatus, EngineConfig,
    EngineOversamplingPolicy, IsolatedExternalPluginSandboxBackend,
    IsolatedExternalPluginSandboxStatus, IsolatedExternalPluginWorkerEvent,
    IsolatedExternalPluginWorkerStatus, LatencyCompensationMode, NetworkEndpointConfig,
    NetworkEndpointMode, NetworkEndpointStatus, OutputAccessMode, OutputAccessStatus,
    PlaybackState, PluginConfig, PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig,
    ServiceId, SinkConfig, SinkOpenResult, SinkType, StreamMetadata,
};
use std::path::{Path, PathBuf};

fn roundtrip<T: Serialize + for<'de> Deserialize<'de>>(value: T) -> T {
    let json = serde_json::to_string(&value).expect("serialization should succeed");
    serde_json::from_str(&json).expect("deserialization should succeed")
}

// ---------------------------------------------------------------------------
// AudioSource
// ---------------------------------------------------------------------------

#[test]
fn audio_source_file_roundtrip_and_accessors() {
    let original: AudioSource = PathBuf::from("/music/album/track.flac").into();
    let restored: AudioSource = roundtrip(original.clone());
    assert_eq!(restored, original);

    assert_eq!(
        restored.as_path(),
        Some(Path::new("/music/album/track.flac"))
    );
    assert!(restored.is_seekable());
    assert_eq!(restored.display_name(), "track.flac");
    assert_eq!(restored.to_string(), "/music/album/track.flac");
}

#[test]
fn audio_source_url_roundtrip_and_accessors() {
    let original = AudioSource::Url {
        url: "https://radio.example/stream.mp3".into(),
        format_hint: Some("mp3".into()),
        seekable: false,
    };
    let restored: AudioSource = roundtrip(original.clone());
    assert_eq!(restored, original);
    assert!(!restored.is_seekable());
    assert_eq!(restored.display_name(), "https://radio.example/stream.mp3");
    assert!(restored.as_path().is_none());
}

#[test]
fn audio_source_service_stream_roundtrip_and_accessors() {
    let original = AudioSource::ServiceStream {
        service: ServiceId::Tidal,
        track_id: "tid-12345".into(),
    };
    let restored: AudioSource = roundtrip(original.clone());
    assert_eq!(restored, original);
    assert!(!restored.is_seekable());
    assert_eq!(restored.display_name(), "Tidal:tid-12345");
    assert_eq!(restored.to_string(), "Tidal:tid-12345");
}

#[test]
fn audio_source_driver_roundtrip_and_accessors() {
    let original = AudioSource::Driver;
    let restored: AudioSource = roundtrip(original.clone());
    assert_eq!(restored, original);
    assert!(!restored.is_seekable());
    assert_eq!(restored.display_name(), "driver");
    assert!(restored.as_path().is_none());
}

#[test]
fn audio_source_from_path_ref() {
    let source: AudioSource = Path::new("/music/song.wav").into();
    assert!(matches!(source, AudioSource::File(_)));
    assert_eq!(source.as_path(), Some(Path::new("/music/song.wav")));
}

#[test]
fn service_id_roundtrip_and_display() {
    assert_eq!(ServiceId::Spotify.to_string(), "Spotify");
    assert_eq!(ServiceId::Tidal.to_string(), "Tidal");
    assert_eq!(roundtrip(ServiceId::Spotify), ServiceId::Spotify);
    assert_eq!(roundtrip(ServiceId::Tidal), ServiceId::Tidal);
}

// ---------------------------------------------------------------------------
// PluginConfig
// ---------------------------------------------------------------------------

#[test]
fn plugin_config_roundtrip_and_validation() {
    let params = json!({"freq": 1000.0, "gain_db": -3.0, "q": 1.4});
    let original = PluginConfig::try_new("eq", params.clone()).expect("valid plugin config");
    let restored: PluginConfig = roundtrip(original.clone());
    assert_eq!(restored.plugin_type, "eq");
    assert_eq!(restored.parameters, params);
    assert!(restored.validate().is_ok());
}

#[test]
fn plugin_config_rejects_empty_type() {
    let err = PluginConfig::try_new("   ", json!({})).expect_err("empty type should fail");
    assert!(err.contains("plugin_type"));
    let err = PluginConfig::new("\t\n", json!({})).validate().unwrap_err();
    assert!(err.contains("plugin_type"));
}

// ---------------------------------------------------------------------------
// PluginGraphConfig
// ---------------------------------------------------------------------------

fn gain_node(id: usize, channels: usize) -> PluginGraphNodeConfig {
    PluginGraphNodeConfig::try_new(id, "gain", json!({"gain_db": 0.0}), channels).unwrap()
}

#[test]
fn plugin_graph_valid_dag_roundtrip() {
    let original = PluginGraphConfig::try_new(
        vec![gain_node(0, 2), gain_node(1, 2), gain_node(2, 6)],
        vec![
            PluginGraphEdgeConfig::new(0, 2),
            PluginGraphEdgeConfig::new(1, 2),
        ],
    )
    .expect("valid DAG");
    let restored: PluginGraphConfig = roundtrip(original.clone());
    assert_eq!(restored.nodes.len(), 3);
    assert_eq!(restored.edges.len(), 2);
    assert!(restored.validate().is_ok());
}

#[test]
fn plugin_graph_rejects_cycle() {
    let err = PluginGraphConfig::try_new(
        vec![gain_node(0, 2), gain_node(1, 2)],
        vec![
            PluginGraphEdgeConfig::new(0, 1),
            PluginGraphEdgeConfig::new(1, 0),
        ],
    )
    .expect_err("cycle should fail");
    assert!(err.contains("acyclic"));
}

#[test]
fn plugin_graph_rejects_duplicate_node_id() {
    let err = PluginGraphConfig::try_new(vec![gain_node(0, 2), gain_node(0, 2)], vec![])
        .expect_err("duplicate id should fail");
    assert!(err.contains("duplicate"));
}

#[test]
fn plugin_graph_rejects_missing_edge_endpoint() {
    let err = PluginGraphConfig::try_new(
        vec![gain_node(0, 2)],
        vec![PluginGraphEdgeConfig::new(0, 99)],
    )
    .expect_err("missing endpoint should fail");
    assert!(err.contains("to_node"));
}

#[test]
fn plugin_graph_rejects_zero_input_channels() {
    let err = PluginGraphNodeConfig::try_new(0, "gain", json!({}), 0).unwrap_err();
    assert!(err.contains("input_channels"));
}

#[test]
fn plugin_graph_accepts_empty_graph() {
    let graph = PluginGraphConfig::try_new(vec![], vec![]).expect("empty graph is valid");
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
    let restored: PluginGraphConfig = roundtrip(graph.clone());
    assert!(restored.validate().is_ok());
}

// ---------------------------------------------------------------------------
// EngineConfig
// ---------------------------------------------------------------------------

#[test]
fn engine_config_default_validates_and_roundtrips() {
    let original = EngineConfig::default();
    assert!(original.validate().is_ok());
    let restored: EngineConfig = roundtrip(original.clone());
    assert_eq!(restored.version, original.version);
    assert_eq!(restored.frame_size, original.frame_size);
    assert_eq!(restored.output_sample_rate, original.output_sample_rate);
    assert_eq!(restored.volume, original.volume);
}

#[test]
fn engine_config_custom_values_roundtrip_and_validate() {
    let mut config = EngineConfig::default();
    config.frame_size = 512;
    config.buffer_ms = 100;
    config.output_sample_rate = 96000;
    config.input_channels = 2;
    config.output_channels = 6;
    config.volume = 0.75;
    config.plugins = vec![
        PluginConfig::try_new("eq", json!({"freq": 1000.0})).unwrap(),
        PluginConfig::try_new("compressor", json!({"threshold": -12.0})).unwrap(),
    ];
    config.oversampling_policy = EngineOversamplingPolicy::Force2x;
    config.output_access = OutputAccessMode::ExclusivePreferred;
    config.dsd_output = DsdOutputMode::PcmDecode;
    config.network_endpoint = NetworkEndpointConfig {
        mode: NetworkEndpointMode::HttpEndpoint,
        bind_addr: "127.0.0.1".into(),
        port: 12345,
    };

    config.validate().expect("custom config should be valid");
    let restored: EngineConfig = roundtrip(config.clone());
    assert_eq!(restored.frame_size, 512);
    assert_eq!(restored.output_sample_rate, 96000);
    assert_eq!(restored.volume, 0.75);
    assert_eq!(restored.plugins.len(), 2);
    assert_eq!(
        restored.oversampling_policy,
        EngineOversamplingPolicy::Force2x
    );
    assert_eq!(restored.output_access, OutputAccessMode::ExclusivePreferred);
    assert_eq!(restored.dsd_output, DsdOutputMode::PcmDecode);
    assert_eq!(restored.network_endpoint.port, 12345);
}

#[test]
fn engine_config_validation_errors() {
    let base = EngineConfig::default();
    assert!(base.validate().is_ok());

    let mut c = base.clone();
    c.frame_size = 0;
    assert!(c.validate().unwrap_err().contains("frame_size"));

    c = base.clone();
    c.buffer_ms = 0;
    assert!(c.validate().unwrap_err().contains("buffer_ms"));

    c = base.clone();
    c.output_sample_rate = 0;
    assert!(c.validate().unwrap_err().contains("output_sample_rate"));

    c = base.clone();
    c.input_channels = 0;
    assert!(c.validate().unwrap_err().contains("input_channels"));

    c = base.clone();
    c.output_channels = 0;
    assert!(c.validate().unwrap_err().contains("output_channels"));

    c = base.clone();
    c.volume = 1.5;
    assert!(c.validate().unwrap_err().contains("volume"));

    c = base.clone();
    c.volume = f32::NAN;
    assert!(c.validate().unwrap_err().contains("volume"));

    c = base.clone();
    c.version = 9999;
    assert!(c.validate().unwrap_err().contains("version"));

    c = base.clone();
    c.plugins = vec![PluginConfig::new("  ", json!({}))];
    assert!(c.validate().unwrap_err().contains("plugins[0]"));
}

#[test]
fn engine_config_sanitize_resets_invalid_defaults() {
    let mut config = EngineConfig::default();
    config.frame_size = 0;
    config.output_sample_rate = 0;
    config.sanitize();
    assert_eq!(config.frame_size, 1024);
    assert_eq!(config.output_sample_rate, 48000);
    assert!(config.validate().is_ok());
}

#[test]
fn engine_config_file_save_and_load_roundtrip() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("sotf-engine-config-{}.json", std::process::id()));

    let mut config = EngineConfig::default();
    config.volume = 0.5;
    config.plugins = vec![PluginConfig::try_new("eq", json!({"freq": 250.0})).unwrap()];

    config.save_to_file(&path).expect("save should succeed");
    let loaded = EngineConfig::load_from_file(&path).expect("load should succeed");

    // Skipped fields are lost on disk roundtrip.
    assert_eq!(loaded.version, config.version);
    assert_eq!(loaded.frame_size, config.frame_size);
    assert_eq!(loaded.volume, config.volume);
    assert_eq!(loaded.plugins.len(), config.plugins.len());

    std::fs::remove_file(&path).ok();
}

#[test]
fn engine_config_buffer_frame_calculations() {
    let mut config = EngineConfig::default();
    config.output_sample_rate = 48000;
    config.buffer_ms = 1000;
    config.frame_size = 1024;
    assert_eq!(config.total_buffer_frames(), 48000);
    assert_eq!(config.queue_capacity_frames(), 47);
}

// ---------------------------------------------------------------------------
// Engine feature enums and structs
// ---------------------------------------------------------------------------

#[test]
fn latency_compensation_mode_helpers_and_roundtrip() {
    assert!(LatencyCompensationMode::Enabled.is_enabled());
    assert!(!LatencyCompensationMode::Disabled.is_enabled());
    assert_eq!(
        roundtrip(LatencyCompensationMode::Enabled),
        LatencyCompensationMode::Enabled
    );
}

#[test]
fn output_access_mode_helpers_and_roundtrip() {
    assert!(!OutputAccessMode::Shared.prefers_exclusive());
    assert!(OutputAccessMode::ExclusivePreferred.prefers_exclusive());
    assert!(OutputAccessMode::ExclusiveRequired.requires_exclusive());
    assert!(!OutputAccessMode::ExclusivePreferred.requires_exclusive());
    assert_eq!(
        roundtrip(OutputAccessMode::ExclusiveRequired),
        OutputAccessMode::ExclusiveRequired
    );
}

#[test]
fn engine_oversampling_policy_helpers_and_roundtrip() {
    assert!(EngineOversamplingPolicy::PluginPreferred.plugin_preferred_enabled());
    assert!(!EngineOversamplingPolicy::Disabled.plugin_preferred_enabled());
    assert_eq!(EngineOversamplingPolicy::Force2x.forced_factor(), Some(2));
    assert_eq!(EngineOversamplingPolicy::Force4x.forced_factor(), Some(4));
    assert_eq!(EngineOversamplingPolicy::Disabled.forced_factor(), None);
    assert_eq!(
        roundtrip(EngineOversamplingPolicy::Force4x),
        EngineOversamplingPolicy::Force4x
    );
}

#[test]
fn dsd_output_mode_helpers_and_roundtrip() {
    assert!(!DsdOutputMode::Disabled.requires_bitstream_output());
    assert!(DsdOutputMode::DopRequired.requires_bitstream_output());
    assert!(DsdOutputMode::NativeRequired.requires_bitstream_output());
    assert!(!DsdOutputMode::DopPreferred.requires_bitstream_output());
    assert_eq!(
        roundtrip(DsdOutputMode::NativePreferred),
        DsdOutputMode::NativePreferred
    );
}

#[test]
fn network_endpoint_config_defaults_and_roundtrip() {
    let default = NetworkEndpointConfig::default();
    assert_eq!(default.mode, NetworkEndpointMode::Disabled);
    assert_eq!(default.bind_addr, "0.0.0.0");
    assert_eq!(default.port, 17890);

    let custom = NetworkEndpointConfig {
        mode: NetworkEndpointMode::InputClient,
        bind_addr: "127.0.0.1".into(),
        port: 8080,
    };
    let restored: NetworkEndpointConfig = roundtrip(custom.clone());
    assert_eq!(restored.mode, NetworkEndpointMode::InputClient);
    assert_eq!(restored.bind_addr, "127.0.0.1");
    assert_eq!(restored.port, 8080);
}

#[test]
fn status_enums_roundtrip_with_defaults() {
    assert_eq!(
        roundtrip(OutputAccessStatus::FallbackShared),
        OutputAccessStatus::FallbackShared
    );
    assert_eq!(
        roundtrip(DsdOutputStatus::DopFallbackPcm),
        DsdOutputStatus::DopFallbackPcm
    );
    assert_eq!(
        roundtrip(NetworkEndpointStatus::EndpointRunning),
        NetworkEndpointStatus::EndpointRunning
    );
}

// ---------------------------------------------------------------------------
// AudioFrame
// ---------------------------------------------------------------------------

#[test]
fn audio_frame_valid_construction_and_accessors() {
    let frame = AudioFrame::try_new(vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 3, 2, 48000).unwrap();
    assert_eq!(frame.num_frames, 3);
    assert_eq!(frame.num_channels, 2);
    assert_eq!(frame.sample_rate, 48000);
    assert_eq!(frame.num_samples(), 6);

    let frame = AudioFrame::new(vec![1.0; 8], 4, 2, 96000);
    assert_eq!(frame.num_samples(), 8);
}

#[test]
fn audio_frame_silent_and_clear() {
    let mut frame = AudioFrame::silent(5, 2, 48000);
    assert_eq!(frame.data.len(), 10);
    assert!(frame.data.iter().all(|&s| s == 0.0));

    frame.data.fill(1.0);
    frame.clear();
    assert!(frame.data.iter().all(|&s| s == 0.0));
    assert_eq!(frame.num_frames, 5);
}

#[test]
fn audio_frame_try_new_wrong_length() {
    let err = AudioFrame::try_new(vec![0.0; 3], 2, 2, 48000).unwrap_err();
    assert!(err.contains("data length"));
}

#[test]
fn audio_frame_try_new_overflow() {
    let err = AudioFrame::try_new(vec![], usize::MAX, 2, 48000).unwrap_err();
    assert!(err.contains("overflow"));
}

#[test]
fn audio_frame_new_panics_on_dimension_mismatch() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        AudioFrame::new(vec![0.0; 3], 2, 2, 48000);
    }));
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// PlaybackState and StreamMetadata
// ---------------------------------------------------------------------------

#[test]
fn playback_state_roundtrip() {
    for state in [
        PlaybackState::Stopped,
        PlaybackState::Playing,
        PlaybackState::Paused,
    ] {
        assert_eq!(roundtrip(state.clone()), state);
    }
}

#[test]
fn stream_metadata_default_and_roundtrip() {
    let default = StreamMetadata::default();
    assert!(default.stream_title.is_none());
    assert!(default.bitrate_kbps.is_none());

    let meta = StreamMetadata {
        stream_title: Some("Artist - Track".into()),
        stream_url: Some("https://example.com".into()),
        content_type: Some("audio/mpeg".into()),
        bitrate_kbps: Some(320),
    };
    let restored: StreamMetadata = roundtrip(meta.clone());
    assert_eq!(restored, meta);
}

// ---------------------------------------------------------------------------
// Sink types
// ---------------------------------------------------------------------------

#[test]
fn sink_config_construction_and_sink_type_default() {
    let config = SinkConfig {
        sample_rate: 48000,
        channels: 2,
        buffer_ms: 50,
        device: Some("BlackHole".into()),
        allow_virtual_output: true,
    };
    assert_eq!(config.sample_rate, 48000);

    let result = SinkOpenResult {
        channels: 2,
        buffer_capacity: 4096,
    };
    assert_eq!(result.buffer_capacity, 4096);

    assert_eq!(SinkType::default(), SinkType::Cpal);
}

// ---------------------------------------------------------------------------
// AudioEngineState
// ---------------------------------------------------------------------------

#[test]
fn audio_engine_state_defaults_and_roundtrip() {
    let state = AudioEngineState::default();
    assert_eq!(state.playback_state, PlaybackState::Stopped);
    assert_eq!(state.sample_rate, 48000);
    assert_eq!(state.num_channels, 2);
    assert_eq!(state.volume, 1.0);
    assert!(!state.muted);
    assert!(state.latency_compensation_enabled);

    let restored: AudioEngineState = roundtrip(state.clone());
    assert_eq!(restored.playback_state, state.playback_state);
    assert_eq!(restored.sample_rate, state.sample_rate);
    assert_eq!(restored.volume, state.volume);
}

#[test]
fn audio_engine_state_with_source_roundtrip() {
    let mut state = AudioEngineState::default();
    state.playback_state = PlaybackState::Playing;
    state.current_source = Some(AudioSource::File(PathBuf::from("/music/song.flac")));
    state.current_file = Some(PathBuf::from("/music/song.flac"));
    state.position = 12.5;
    state.duration = Some(180.0);
    state.volume = 0.8;
    state.muted = true;
    state.stream_metadata = Some(StreamMetadata {
        stream_title: Some("Live".into()),
        stream_url: None,
        content_type: Some("audio/flac".into()),
        bitrate_kbps: Some(1411),
    });

    let restored: AudioEngineState = roundtrip(state.clone());
    assert_eq!(restored.playback_state, PlaybackState::Playing);
    assert_eq!(restored.current_source, state.current_source);
    assert_eq!(restored.current_file, state.current_file);
    assert_eq!(restored.position, state.position);
    assert_eq!(restored.volume, state.volume);
    assert!(restored.muted);
    assert_eq!(restored.stream_metadata, state.stream_metadata);
}

#[test]
fn audio_engine_state_deserializes_missing_fields_with_defaults() {
    // Minimal JSON: only fields without serde defaults and a few core values.
    let json = json!({
        "playback_state": "Playing",
        "position": 5.0,
        "sample_rate": 44100,
        "num_channels": 2,
        "volume": 0.5,
        "muted": false,
        "processing_bypassed": false,
        "underruns": 0,
        "plugin_latency_samples": 0,
        "seeking": false,
    });
    let state: AudioEngineState = serde_json::from_value(json).expect("deserialize should succeed");
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(state.output_access_mode, OutputAccessMode::Shared);
    assert_eq!(state.dsd_output_mode, DsdOutputMode::Disabled);
    assert_eq!(
        state.oversampling_policy,
        EngineOversamplingPolicy::PluginPreferred
    );
    assert_eq!(state.network_endpoint.port, 17890);
    assert!(state.isolated_external_plugin_worker_statuses.is_empty());
}

// ---------------------------------------------------------------------------
// Isolated external plugin worker status
// ---------------------------------------------------------------------------

#[test]
fn isolated_worker_status_roundtrip_and_defaults() {
    let status = IsolatedExternalPluginWorkerStatus {
        plugin_index: 0,
        node_id: 7,
        event: Some(IsolatedExternalPluginWorkerEvent::Started { pid: 1234 }),
        error: None,
        worker_start_count: 1,
        worker_exit_count: 0,
        worker_launch_failure_count: 0,
        block_timeout_count: 0,
        block_worker_failure_count: 0,
        block_wrong_sequence_count: 0,
        sandbox_status: IsolatedExternalPluginSandboxStatus::Enforced,
        sandbox_backend: IsolatedExternalPluginSandboxBackend::MacosProcessIsolation,
        sandbox_reason: Some("ok".into()),
    };
    let restored: IsolatedExternalPluginWorkerStatus = roundtrip(status.clone());
    assert_eq!(restored.plugin_index, status.plugin_index);
    assert_eq!(restored.node_id, status.node_id);
    assert_eq!(restored.event, status.event);
    assert_eq!(
        restored.sandbox_status,
        IsolatedExternalPluginSandboxStatus::Enforced
    );
    assert_eq!(
        restored.sandbox_backend,
        IsolatedExternalPluginSandboxBackend::MacosProcessIsolation
    );
}

#[test]
fn isolated_worker_event_variants_roundtrip() {
    for event in [
        IsolatedExternalPluginWorkerEvent::AlreadyRunning,
        IsolatedExternalPluginWorkerEvent::Started { pid: 42 },
        IsolatedExternalPluginWorkerEvent::Exited { exit_code: Some(0) },
        IsolatedExternalPluginWorkerEvent::NotRunning,
    ] {
        assert_eq!(roundtrip(event.clone()), event);
    }
}
