//! Integration tests for `sotf-player` covering public API workflows.
//!
//! Scenarios:
//! - Player state transitions and volume/mute roundtrips.
//! - Plugin graph construction from presets and serialization to engine configs.
//! - Federation source/server config loading and round-trips.
//! - Library metadata patch cleaning (trimming whitespace and dropping blanks).

use std::path::PathBuf;

use sotf_audio_player::NodePosition;
use sotf_audio_player::federation_config::{
    FederationSourceEntry, MpdClientAuthMode, ServerConfig, SourceConnectionConfig,
};
use sotf_audio_player::{
    MetadataImportCandidate, MetadataPatch, Player, PluginGraph, PluginType, SpecialNodeType,
};

// ---------------------------------------------------------------------------
// Player state transitions
// ---------------------------------------------------------------------------

#[test]
fn player_starts_in_idle_state() {
    let mut player = Player::new();
    let state = player.get_playback_state();

    assert_eq!(state.position_secs, 0.0);
    assert!(!state.is_playing);
    assert!(state.sample_rate.is_none());
    assert!(state.last_error.is_none());
    assert!(!state.engine_restarted);
    assert!(!state.engine_fatal);
    assert!(!player.is_playing());
}

#[test]
fn update_plugins_is_ok_when_idle() {
    let mut player = Player::new();
    player.update_plugins(Vec::new()).unwrap();
}

#[test]
fn volume_and_mute_roundtrip() {
    let player = Player::new();

    player.set_volume(0.75).unwrap();
    assert!((player.get_volume() - 0.75).abs() < f32::EPSILON);

    player.set_mute(true).unwrap();
    assert!(player.is_muted());

    player.set_mute(false).unwrap();
    assert!(!player.is_muted());
}

#[test]
fn stop_is_allowed_when_idle() {
    let mut player = Player::new();
    player.stop().unwrap();

    let state = player.get_playback_state();
    assert!(!state.is_playing);
    assert_eq!(state.position_secs, 0.0);
}

#[test]
fn load_and_play_missing_file_returns_error() {
    let mut player = Player::new();
    let result = player.load_and_play(
        PathBuf::from("/this/path/does/not/exist.flac"),
        Vec::new(),
        2,
        None,
    );
    assert!(result.is_err(), "loading a missing file should fail");
}

// ---------------------------------------------------------------------------
// Plugin graph construction from config
// ---------------------------------------------------------------------------

#[test]
fn default_rack_has_input_output_and_engine_configs() {
    let graph = PluginGraph::with_default_rack();

    assert!(!graph.is_empty());
    assert!(graph.input_node().is_some(), "default rack must have Input");
    assert!(
        graph.output_node().is_some(),
        "default rack must have Output"
    );

    // The default rack produces a non-empty linear chain config.
    let chain_configs = graph.to_plugin_configs(48_000.0);
    assert!(!chain_configs.is_empty());

    // The routed graph config preserves at least the processing plugins.
    let routed_configs = graph.to_plugin_graph_config(48_000.0);
    assert!(!routed_configs.nodes.is_empty());
}

#[test]
fn graph_manual_construction_and_topological_sort() {
    let mut graph = PluginGraph::new();

    let input_id = graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let eq_id = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0))
        .unwrap();
    let output_id =
        graph.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 0.0), 2);

    graph.add_connection(input_id, 0, eq_id, 0).unwrap();
    graph.add_connection(eq_id, 0, output_id, 0).unwrap();

    let sorted = graph.topological_sort().unwrap();
    assert_eq!(
        sorted.len(),
        3,
        "all nodes should appear in topological order"
    );

    // Engine serialization drops special I/O nodes, leaving only the EQ plugin.
    let routed = graph.to_plugin_graph_config(48_000.0);
    assert_eq!(routed.nodes.len(), 1);
    assert!(routed.edges.is_empty());
    assert_eq!(routed.nodes[0].plugin_type, "eq");
}

#[test]
fn graph_rejects_cycles_and_self_loops() {
    let mut graph = PluginGraph::new();
    let a = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(0.0, 0.0))
        .unwrap();
    let b = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 0.0))
        .unwrap();

    graph.add_connection(a, 0, b, 0).unwrap();

    let cycle = graph.add_connection(b, 0, a, 0);
    assert!(
        cycle.is_err(),
        "adding an edge that creates a cycle should fail"
    );

    let self_loop = graph.add_connection(a, 0, a, 0);
    assert!(
        self_loop.is_err(),
        "connecting a node to itself should be rejected"
    );
}

#[test]
fn graph_rejects_duplicate_connection() {
    let mut graph = PluginGraph::new();
    let input = graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let gain = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 0.0))
        .unwrap();

    graph.add_connection(input, 0, gain, 0).unwrap();
    let duplicate = graph.add_connection(input, 0, gain, 0);
    assert!(
        duplicate.is_err(),
        "duplicate connection should be rejected"
    );
}

#[test]
fn load_empty_preset_rebuilds_default_rack() {
    let dir = tempfile::tempdir().unwrap();
    let preset_path = dir.path().join("empty.json");
    std::fs::write(&preset_path, r#"{"plugins": []}"#).unwrap();

    let mut graph = PluginGraph::new();
    let warnings = graph.load_from_file(dir.path(), "empty").unwrap();

    assert!(
        warnings.is_empty(),
        "empty preset should produce no warnings"
    );
    assert!(graph.input_node().is_some());
    assert!(graph.output_node().is_some());
}

#[test]
fn load_invalid_preset_json_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let preset_path = dir.path().join("bad.json");
    std::fs::write(&preset_path, "this is not json").unwrap();

    let mut graph = PluginGraph::new();
    let result = graph.load_from_file(dir.path(), "bad");
    assert!(result.is_err(), "invalid JSON should fail to load");
}

#[test]
fn load_preset_with_broken_plugin_produces_warning() {
    let dir = tempfile::tempdir().unwrap();
    let preset_path = dir.path().join("partial.json");
    std::fs::write(
        &preset_path,
        r#"{"plugins": [{"settings": "DefinitelyNotARealPlugin"}]}"#,
    )
    .unwrap();

    let mut graph = PluginGraph::new();
    let warnings = graph.load_from_file(dir.path(), "partial").unwrap();
    assert!(
        !warnings.is_empty(),
        "unrecognised plugin should be skipped with a warning"
    );
    // The default rack is still rebuilt.
    assert!(graph.input_node().is_some());
}

// ---------------------------------------------------------------------------
// Federation config loading
// ---------------------------------------------------------------------------

#[test]
fn source_connection_default_for_known_types() {
    assert_eq!(
        SourceConnectionConfig::default_for_type("mpd").type_name(),
        "MPD"
    );
    assert_eq!(
        SourceConnectionConfig::default_for_type("subsonic").type_name(),
        "Subsonic"
    );
    assert_eq!(
        SourceConnectionConfig::default_for_type("dlna").type_name(),
        "DLNA"
    );
    assert_eq!(
        SourceConnectionConfig::default_for_type("unknown").type_name(),
        "MPD",
        "unknown type should fall back to MPD default"
    );
}

#[test]
fn source_connection_serde_roundtrip() {
    let original = SourceConnectionConfig::Mpd {
        host: "mpd.example.com".into(),
        port: 6600,
        auth_mode: MpdClientAuthMode::Password,
        password: Some("secret".into()),
        httpd_port: 6601,
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: SourceConnectionConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn federation_source_entry_serde_roundtrip() {
    let entry = FederationSourceEntry {
        source_id: "home".into(),
        display_name: "Home MPD".into(),
        priority: 10,
        is_enabled: true,
        connection: SourceConnectionConfig::Mpd {
            host: "localhost".into(),
            port: 6600,
            auth_mode: MpdClientAuthMode::None,
            password: None,
            httpd_port: 6601,
        },
        is_available: Some(true),
    };

    let json = serde_json::to_string(&entry).unwrap();
    let decoded: FederationSourceEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.source_id, "home");
    assert_eq!(decoded.priority, 10);
    assert_eq!(decoded.is_available, Some(true));
    assert_eq!(decoded.connection.type_name(), "MPD");
}

#[test]
fn server_config_defaults_and_serde_roundtrip() {
    let config = ServerConfig::default();
    assert_eq!(config.mpd.port, 6600);

    let json = serde_json::to_string(&config).unwrap();
    let decoded: ServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.mpd.port, config.mpd.port);
    assert_eq!(decoded.mpd.bind_address, config.mpd.bind_address);
}

// ---------------------------------------------------------------------------
// Library metadata cleaning
// ---------------------------------------------------------------------------

#[test]
fn metadata_patch_sanitized_trims_and_drops_blanks() {
    let patch = MetadataPatch {
        title: Some("  Track Title  ".into()),
        artist: Some("   ".into()),
        album_title: Some("Album".into()),
        genre: Some("\t\n".into()),
        year: Some(2024),
        ..Default::default()
    };

    let clean = patch.sanitized();
    assert_eq!(clean.title, Some("Track Title".into()));
    assert_eq!(clean.artist, None);
    assert_eq!(clean.album_title, Some("Album".into()));
    assert_eq!(clean.genre, None);
    assert_eq!(clean.year, Some(2024));
}

#[test]
fn metadata_patch_empty_detection() {
    let empty = MetadataPatch::default();
    assert!(empty.is_empty());

    let non_empty = MetadataPatch {
        title: Some("x".into()),
        ..Default::default()
    };
    assert!(!non_empty.is_empty());
}

#[test]
fn metadata_candidate_into_patch() {
    let candidate = MetadataImportCandidate {
        provider_id: "musicbrainz".into(),
        provider_entity_id: "abc-123".into(),
        title: Some("Title".into()),
        artist: Some("Artist".into()),
        album_artist: Some("Album Artist".into()),
        album_title: Some("Album".into()),
        year: Some(2020),
        track_number: Some(3),
        disc_number: Some(1),
        isrc: Some("USABC2000001".into()),
        score: 95,
    };

    let patch = candidate.into_patch();
    assert_eq!(patch.title, Some("Title".into()));
    assert_eq!(patch.artist, Some("Artist".into()));
    assert_eq!(patch.album_artist, Some("Album Artist".into()));
    assert_eq!(patch.year, Some(2020));
    assert_eq!(patch.track_number, Some(3));
    assert!(!patch.is_empty());
}

#[test]
fn metadata_patch_serde_roundtrip() {
    let patch = MetadataPatch {
        title: Some("Title".into()),
        artist: Some("Artist".into()),
        year: Some(2020),
        ..Default::default()
    };

    let json = serde_json::to_string(&patch).unwrap();
    let decoded: MetadataPatch = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, patch);
}
