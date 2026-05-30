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
