//! Integration tests for the TUI dev API.
//!
//! These tests verify the HTTP server and command dispatch without
//! needing a full terminal UI.

use sotf_audio_player_tui::app::{App, InputMode, Screen};
use sotf_audio_player_tui::dev_api::commands::{DevCommand, DevQueryReply, DevReply};
use sotf_audio_player_tui::dev_api::queries::resolve;
use sotf_audio_player_tui::theme::Theme;
use std::sync::mpsc;

fn make_app() -> App {
    let theme = Theme::default();
    App::new(theme, true) // read_only = true avoids DB writes
}

#[test]
fn query_screen_focused() {
    let mut app = make_app();
    app.current_screen = Screen::Library;

    let value = resolve("screen.focused", &app).unwrap();
    assert_eq!(value, serde_json::json!("Library"));
}

#[test]
fn query_input_mode() {
    let mut app = make_app();
    app.input_mode = InputMode::Configure;

    let value = resolve("input_mode", &app).unwrap();
    assert_eq!(value, serde_json::json!("Configure"));
}

#[test]
fn query_playback_state() {
    let mut app = make_app();
    app.is_playing = true;
    app.volume = 0.75;
    app.muted = true;

    assert_eq!(resolve("playback.is_playing", &app).unwrap(), true);
    assert_eq!(resolve("playback.volume", &app).unwrap(), 0.75);
    assert_eq!(resolve("playback.muted", &app).unwrap(), true);
}

#[test]
fn query_queue() {
    let app = make_app();
    assert_eq!(resolve("queue.length", &app).unwrap(), 0);
    assert!(resolve("queue.current_index", &app).unwrap().is_null());
}

#[test]
fn query_library_counts() {
    let app = make_app();
    // Empty library in read-only mode
    let dirs: usize =
        serde_json::from_value(resolve("library.directory_count", &app).unwrap()).unwrap();
    let albums: usize =
        serde_json::from_value(resolve("library.album_count", &app).unwrap()).unwrap();
    assert_eq!(dirs, 0);
    assert_eq!(albums, 0);
}

#[test]
fn query_configure_sub_screen() {
    let mut app = make_app();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = sotf_audio_player_tui::app::ConfigureSubScreen::RoomEq;

    let value = resolve("configure.sub_screen", &app).unwrap();
    assert_eq!(value, serde_json::json!("RoomEq"));
}

#[test]
fn query_unknown_path_fails() {
    let app = make_app();
    let err = resolve("unknown.path", &app).unwrap_err();
    assert!(err.to_string().contains("unknown query path"));
}

#[test]
fn dev_reply_serialization() {
    let ok = DevReply::ok();
    assert_eq!(ok.to_json(), r#"{"ok":true}"#);

    let err = DevReply::err("something went wrong");
    assert!(err.to_json().contains("\"ok\":false"));
    assert!(err.to_json().contains("something went wrong"));
}

#[test]
fn dev_query_reply_serialization() {
    let ok = DevQueryReply::ok(serde_json::json!({"screen": "Library"}));
    assert!(ok.to_json().contains("\"ok\":true"));
    assert!(ok.to_json().contains("\"screen\":\"Library\""));

    let err = DevQueryReply::err("bad path");
    assert!(err.to_json().contains("\"ok\":false"));
    assert!(err.to_json().contains("bad path"));
}

#[test]
fn query_library_track_count() {
    let app = make_app();
    assert_eq!(resolve("library.track_count", &app).unwrap(), 0);
}

#[test]
fn query_metadata_editor() {
    use sotf_audio_player::{
        MetadataAffectedFile, MetadataEditPreview, MetadataImportCandidate, MetadataTarget,
    };
    use sotf_audio_player_tui::app::{
        MetadataEditorFields, MetadataEditorScope, MetadataEditorState,
    };

    let mut app = make_app();
    assert_eq!(resolve("metadata.editor_open", &app).unwrap(), false);
    assert!(resolve("metadata.target", &app).unwrap().is_null());

    app.metadata_editor = Some(MetadataEditorState {
        scope: MetadataEditorScope::Album,
        target: MetadataTarget::AlbumId(1),
        target_label: "Test Album".to_string(),
        fields: MetadataEditorFields {
            title: "Test Title".to_string(),
            year: "2024".to_string(),
            ..Default::default()
        },
        selected_field: 0,
        editing: false,
        edit_buffer: String::new(),
        preview: Some(MetadataEditPreview {
            target: None,
            affected_files: vec![MetadataAffectedFile {
                path: std::path::PathBuf::from("/tmp/a.flac"),
                backup_path: std::path::PathBuf::from("/tmp/a.flac.bak"),
                writable: true,
                reason: None,
            }],
            sidecar_path: None,
            sidecar_backup_path: None,
            affected_album_ids: vec![],
            affected_track_paths: vec![],
            unsupported_writes: vec![MetadataAffectedFile {
                path: std::path::PathBuf::from("/tmp/a.flac"),
                backup_path: std::path::PathBuf::from("/tmp/a.flac.bak"),
                writable: false,
                reason: Some("composer".to_string()),
            }],
        }),
        error: None,
        search_query: String::new(),
        search_results: vec![MetadataImportCandidate {
            provider_id: "test".to_string(),
            provider_entity_id: "1".to_string(),
            title: Some("Candidate".to_string()),
            artist: None,
            album_artist: None,
            album_title: None,
            year: None,
            track_number: None,
            disc_number: None,
            isrc: None,
            score: 0,
        }],
        selected_result: 0,
        search_error: None,
    });

    assert_eq!(resolve("metadata.editor_open", &app).unwrap(), true);
    assert_eq!(
        resolve("metadata.target", &app).unwrap(),
        serde_json::json!("Test Album")
    );
    assert_eq!(
        resolve("metadata.title", &app).unwrap(),
        serde_json::json!("Test Title")
    );
    assert_eq!(resolve("metadata.year", &app).unwrap(), serde_json::json!("2024"));
    assert_eq!(resolve("metadata.preview_files", &app).unwrap(), 1);
    assert_eq!(resolve("metadata.unsupported_count", &app).unwrap(), 1);
    assert_eq!(resolve("metadata.candidate_count", &app).unwrap(), 1);
}

#[test]
fn query_recording_state() {
    let mut app = make_app();
    app.recording = sotf_audio_player_tui::app::RecordingTuiState::default();
    app.recording.status_message = "ready".to_string();

    resolve("recording.step", &app).unwrap();
    // Empty channel recordings means all (zero) channels are done.
    assert_eq!(resolve("recording.all_done", &app).unwrap(), true);
    assert_eq!(resolve("recording.done_count", &app).unwrap(), 0);
    assert_eq!(resolve("recording.channel_count", &app).unwrap(), 0);
    assert_eq!(
        resolve("recording.status", &app).unwrap(),
        serde_json::json!("ready")
    );
}

#[test]
fn query_room_eq_state() {
    let mut app = make_app();
    app.room_eq = sotf_audio_player_tui::app::RoomEqTuiState::default();
    app.room_eq.opt_status_message = Some("running".to_string());
    app.room_eq.opt_error = Some("boom".to_string());

    resolve("roomeq.step", &app).unwrap();
    assert_eq!(resolve("roomeq.measurement_count", &app).unwrap(), 0);
    assert_eq!(resolve("roomeq.speaker_config_count", &app).unwrap(), 0);
    resolve("roomeq.optimization_status", &app).unwrap();
    assert_eq!(resolve("roomeq.result_count", &app).unwrap(), 0);
    assert_eq!(resolve("roomeq.has_dsp_output", &app).unwrap(), false);
    assert!(resolve("roomeq.dsp_channel_count", &app).unwrap().is_null());
    assert_eq!(resolve("roomeq.filter_count", &app).unwrap(), 0);
    assert!(resolve("roomeq.average_pre_score", &app).unwrap().is_null());
    assert!(resolve("roomeq.average_post_score", &app).unwrap().is_null());
    assert_eq!(
        resolve("roomeq.status", &app).unwrap(),
        serde_json::json!("running")
    );
    assert_eq!(resolve("roomeq.error", &app).unwrap(), serde_json::json!("boom"));
}

#[test]
fn query_headphone_eq_and_spinorama_steps() {
    let mut app = make_app();
    app.headphone_eq = sotf_audio_player_tui::app::HeadphoneEqTuiState::default();
    app.spinorama_eq = sotf_audio_player_tui::app::SpinoramaEqTuiState::default();

    resolve("headphoneeq.step", &app).unwrap();
    resolve("spinorama.step", &app).unwrap();
}

#[test]
fn query_settings_theme() {
    let app = make_app();
    assert_eq!(resolve("settings.theme", &app).unwrap(), serde_json::json!("dark"));
}

#[test]
fn query_audio_devices() {
    let mut app = make_app();
    app.output_devices = vec![];
    app.current_output_device_name = Some("BlackHole".to_string());

    assert_eq!(
        resolve("audio.output_device", &app).unwrap(),
        serde_json::json!("BlackHole")
    );
    assert_eq!(resolve("audio.output_device_count", &app).unwrap(), 0);
}

#[test]
fn query_plugins_and_playlists_and_level_meters_and_cast() {
    use sotf_audio_player::{ChannelGroup, ChannelInfo};

    let mut app = make_app();
    app.level_meter_groups = vec![ChannelGroup {
        name: "stereo".to_string(),
        channels: vec![
            ChannelInfo {
                index: 0,
                name: "L".to_string(),
                display_name: vec!["L".to_string()],
            },
            ChannelInfo {
                index: 1,
                name: "R".to_string(),
                display_name: vec!["R".to_string()],
            },
        ],
        muted: false,
        soloed: false,
        dimmed: false,
    }];
    app.cast_devices = vec![sotf_audio_player_tui::app::CastDeviceInfo {
        name: "Kitchen".to_string(),
        device_type: "Chromecast".to_string(),
        address: "192.168.1.10".to_string(),
        port: 8009,
    }];

    assert_eq!(
        resolve("plugins.count", &app).unwrap(),
        app.plugin_graph.plugin_count()
    );
    assert_eq!(
        resolve("playlists.count", &app).unwrap(),
        app.playlist_controller.playlists().len()
    );
    assert_eq!(resolve("level_meters.channel_count", &app).unwrap(), 2);
    assert_eq!(resolve("cast.device_count", &app).unwrap(), 1);
}

#[test]
fn query_room_eq_with_results() {
    use sotf_audio_player::room_eq_types::{ChannelOptResult, EqFilterConfig, OptimizationStatus};

    let mut app = make_app();
    app.room_eq = sotf_audio_player_tui::app::RoomEqTuiState::default();
    app.room_eq.opt_status = OptimizationStatus::Completed;
    app.room_eq.channel_results = vec![
        ChannelOptResult {
            channel_name: "L".to_string(),
            pre_score: 3.0,
            post_score: 1.0,
            eq_filters: vec![EqFilterConfig {
                filter_type: "peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 2.0,
            }],
            broadband_filters: vec![],
            preamp_gain_db: 0.0,
            crossover_freqs: None,
            driver_gains: None,
            original_response: None,
            corrected_response: None,
            normalized_response: None,
            target_curve: None,
            group_delay_before: None,
            group_delay_after: None,
            phase_response_before: None,
            phase_response_after: None,
            impulse_response: None,
        },
        ChannelOptResult {
            channel_name: "R".to_string(),
            pre_score: 5.0,
            post_score: 2.0,
            eq_filters: vec![],
            broadband_filters: vec![],
            preamp_gain_db: 0.0,
            crossover_freqs: None,
            driver_gains: None,
            original_response: None,
            corrected_response: None,
            normalized_response: None,
            target_curve: None,
            group_delay_before: None,
            group_delay_after: None,
            phase_response_before: None,
            phase_response_after: None,
            impulse_response: None,
        },
    ];

    assert_eq!(resolve("roomeq.result_count", &app).unwrap(), 2);
    assert_eq!(resolve("roomeq.filter_count", &app).unwrap(), 1);
    assert_eq!(resolve("roomeq.average_pre_score", &app).unwrap(), 4.0);
    assert_eq!(resolve("roomeq.average_post_score", &app).unwrap(), 1.5);
}

#[test]
fn dev_command_channel_roundtrip() {
    let (tx, rx) = mpsc::channel::<DevCommand>();
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);

    tx.send(DevCommand::Health { reply: reply_tx }).unwrap();

    match rx.recv().unwrap() {
        DevCommand::Health { reply } => {
            let _ = reply.send(DevQueryReply::ok(serde_json::json!({"ok": true})));
        }
        other => panic!("expected Health, got {:?}", other),
    }

    let reply = reply_rx.recv().unwrap();
    assert!(reply.value.is_ok());
}
