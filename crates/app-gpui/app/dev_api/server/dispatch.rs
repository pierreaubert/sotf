use super::super::commands::{DevCommand, DevQueryReply, DevReply};
use super::super::{queries, registry};
use super::get::get_health;
use super::get::get_query;
use super::misc::POLL_INTERVAL;
use super::misc::http_response;
use super::misc::list_elements_json;
use super::misc::resolve_action_name;
use super::misc::split_path_query;
use super::post::post_action;
use super::post::post_click;
use super::post::post_key;
use super::post::post_qa_room_eq;
use super::post::post_qa_room_eq_export_json;
use super::post::post_qa_seed;
use super::post::post_quit;
use super::qa::qa_room_eq;
use super::qa::qa_room_eq_export_json;
use super::qa::qa_seed;
use super::types::HttpRequest;
use super::with::health_payload;
use super::with::with_app_state;
use crate::app::{InputMode, MetadataEditorState, Screen, SettingsTab};
use anyhow::{Result, anyhow};
use gpui::{
    AnyWindowHandle, App, AsyncApp, Keystroke, MouseButton, MouseDownEvent, MouseUpEvent,
    PlatformInput, Point,
};
use serde_json::Value;
use std::sync::mpsc::{self, Receiver};

pub(super) async fn consume_commands(
    rx: Receiver<DevCommand>,
    window: AnyWindowHandle,
    cx: &mut AsyncApp,
) {
    loop {
        // Drain any commands the listener thread has queued.
        loop {
            match rx.try_recv() {
                Ok(cmd) => process_command(cmd, window, cx),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

pub(super) fn process_command(cmd: DevCommand, window: AnyWindowHandle, cx: &mut AsyncApp) {
    match cmd {
        DevCommand::Action {
            name,
            payload,
            reply,
        } => {
            let result = cx.update(|cx| dispatch_action(&name, payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            // Best effort — handler thread may have already given up.
            let _ = reply.send(dev_reply);
        }
        DevCommand::Query { path, reply } => {
            let result = cx.update(|cx| queries::resolve(&path, window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Key { keystroke, reply } => {
            let result = cx.update(|cx| dispatch_key(&keystroke, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Click { selector, reply } => {
            let result = cx.update(|cx| dispatch_click(&selector, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Health { reply } => {
            let result = cx.update(|cx| health_payload(window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Quit { reply } => {
            let result = cx.update(|cx| -> Result<()> {
                cx.quit();
                Ok(())
            });
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaSeed { payload, reply } => {
            let result = cx.update(|cx| qa_seed(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaRoomEq { payload, reply } => {
            let result = cx.update(|cx| qa_room_eq(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaRoomEqExportJson { payload, reply } => {
            let result = cx.update(|cx| qa_room_eq_export_json(payload, window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
    }
}

pub(super) fn dispatch_key(
    keystroke_str: &str,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let keystroke = Keystroke::parse(keystroke_str)
        .map_err(|e| anyhow!("invalid keystroke `{keystroke_str}`: {e:?}"))?;
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_click(selector: &str, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
    let bounds = registry::lookup(selector)
        .ok_or_else(|| anyhow!("no tracked element for selector `{selector}` (was it painted?)"))?;
    let position: Point<gpui::Pixels> = bounds.center();
    window
        .update(cx, |_view, window, cx| {
            let modifiers = Default::default();
            let down = MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
                first_mouse: false,
            };
            let up = MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
            };
            window.dispatch_event(PlatformInput::MouseDown(down), cx);
            window.dispatch_event(PlatformInput::MouseUp(up), cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_action(
    name: &str,
    payload: Option<Value>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    if dispatch_metadata_action(name, payload.clone(), window, cx)? {
        return Ok(());
    }

    // Resolve via the gpui action registry. Action names are namespaced
    // (e.g. `player_ui::PlayPause`); we accept either the full name or the
    // bare name and try to disambiguate by suffix match against registered
    // names. Bare names are convenient for scripts but ambiguous in theory —
    // we fail loudly when more than one action matches.
    let resolved = resolve_action_name(name, cx)?;
    let action = cx
        .build_action(&resolved, payload)
        .map_err(|e| anyhow!("build_action({resolved}) failed: {e}"))?;
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_action(action, cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

fn dispatch_metadata_action(
    name: &str,
    payload: Option<Value>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<bool> {
    match name {
        "SettingsSetTab" => {
            let tab = parse_settings_tab(payload_str(&payload, "tab")?)?;
            with_app_state(window, cx, |state| {
                state.app.ui_state.current_screen = Screen::Settings;
                state.app.ui_state.active_settings_tab = tab;
                state.app.ui_state.input_mode = InputMode::Normal;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataSeedAlbum" => {
            with_app_state(window, cx, |state| {
                state.app.library_state.library.albums =
                    vec![sotf_audio_player::dev_api_fixtures::metadata_fixture_album()];
                state.app.library_state.selected_index = 0;
                state.app.library_state.invalidate_cache();
                state.app.ui_state.current_screen = Screen::Library;
                state.app.ui_state.input_mode = InputMode::Normal;
                state.app.modal.metadata_editor = None;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataOpenAlbumEditor" => {
            with_app_state(window, cx, |state| {
                let album = state
                    .app
                    .library_state
                    .selected_album()
                    .cloned()
                    .or_else(|| state.app.library_state.library.albums.first().cloned())
                    .ok_or_else(|| anyhow!("no album available for metadata editor"))?;
                state.app.modal.metadata_editor = Some(
                    MetadataEditorState::for_album(&album)
                        .map_err(|err| anyhow!("metadata editor unavailable: {err}"))?,
                );
                state.app.ui_state.input_mode = InputMode::MetadataEditor;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataSetField" => {
            with_app_state(window, cx, |state| {
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                let field = payload_str(&payload, "field")?;
                let value = payload_str(&payload, "value")?;
                set_metadata_field(editor, field, value.to_string())?;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataPreview" => {
            with_app_state(window, cx, |state| {
                let (target, patch) = {
                    let editor = state
                        .app
                        .modal
                        .metadata_editor
                        .as_ref()
                        .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                    (
                        editor.target.clone(),
                        editor
                            .patch()
                            .map_err(|err| anyhow!("invalid metadata patch: {err}"))?,
                    )
                };
                let result = state.app.library_state.preview_metadata_edit(target, patch);
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                match result {
                    Ok(preview) => {
                        editor.preview = Some(preview);
                        editor.error = None;
                    }
                    Err(err) => {
                        editor.error = Some(err.to_string());
                        return Err(anyhow!(err.to_string()));
                    }
                }
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataInjectCandidate" => {
            with_app_state(window, cx, |state| {
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                editor
                    .search_results
                    .push(metadata_candidate_from_payload(payload.as_ref()));
                editor.selected_result = editor.search_results.len().saturating_sub(1);
                editor.search_error = None;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataImportCandidate" => {
            with_app_state(window, cx, |state| {
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                let candidate = editor
                    .search_results
                    .get(editor.selected_result)
                    .cloned()
                    .ok_or_else(|| anyhow!("no metadata candidate selected"))?;
                editor.apply_candidate(candidate);
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataClose" => {
            with_app_state(window, cx, |state| {
                state.app.modal.metadata_editor = None;
                state.app.ui_state.input_mode = InputMode::Normal;
                Ok(())
            })?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_settings_tab(value: &str) -> Result<SettingsTab> {
    match value {
        "Library" => Ok(SettingsTab::Library),
        "Theme" => Ok(SettingsTab::Theme),
        "Language" => Ok(SettingsTab::Language),
        "Keybindings" => Ok(SettingsTab::Keybindings),
        "AudioDevice" => Ok(SettingsTab::AudioDevice),
        "Misc" => Ok(SettingsTab::Misc),
        "Federation" => Ok(SettingsTab::Federation),
        "Servers" => Ok(SettingsTab::Servers),
        "Metadata" => Ok(SettingsTab::Metadata),
        "ReleaseChannel" => Ok(SettingsTab::ReleaseChannel),
        other => Err(anyhow!("unknown settings tab `{other}`")),
    }
}

fn payload_str<'a>(payload: &'a Option<Value>, key: &str) -> Result<&'a str> {
    payload
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("metadata action payload needs string `{key}`"))
}

fn payload_u32(payload: Option<&Value>, key: &str, default: u32) -> u32 {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(default)
}

fn payload_u8(payload: Option<&Value>, key: &str, default: u8) -> u8 {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(default)
}

fn set_metadata_field(editor: &mut MetadataEditorState, field: &str, value: String) -> Result<()> {
    match field {
        "title" => editor.fields.title = value,
        "artist" => editor.fields.artist = value,
        "album_artist" => editor.fields.album_artist = value,
        "year" => editor.fields.year = value,
        "genre" => editor.fields.genre = value,
        "composer" => editor.fields.composer = value,
        "disc" | "disc_number" => editor.fields.disc_number = value,
        "track" | "track_number" => editor.fields.track_number = value,
        "conductor" => editor.fields.conductor = value,
        "performer" => editor.fields.performer = value,
        "isrc" => editor.fields.isrc = value,
        "ensemble" => editor.fields.ensemble = value,
        "edition" => editor.fields.edition = value,
        other => return Err(anyhow!("unknown metadata field `{other}`")),
    }
    editor.preview = None;
    editor.error = None;
    Ok(())
}

fn payload_string(payload: Option<&Value>, key: &str, default: &str) -> Option<String> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| Some(default.to_string()))
}

fn metadata_candidate_from_payload(
    payload: Option<&Value>,
) -> sotf_audio_player::MetadataImportCandidate {
    sotf_audio_player::MetadataImportCandidate {
        provider_id: "musicbrainz".to_string(),
        provider_entity_id: payload
            .and_then(|value| value.get("provider_entity_id"))
            .and_then(Value::as_str)
            .unwrap_or("scenario-release")
            .to_string(),
        title: payload_string(payload, "title", "Imported Track"),
        artist: payload_string(payload, "artist", "Imported Artist"),
        album_artist: payload_string(payload, "album_artist", "Imported Artist"),
        album_title: payload_string(payload, "album_title", "Imported Album"),
        year: Some(payload_u32(payload, "year", 2024)),
        track_number: Some(payload_u32(payload, "track_number", 1)),
        disc_number: Some(payload_u32(payload, "disc_number", 1)),
        isrc: payload
            .and_then(|value| value.get("isrc"))
            .and_then(Value::as_str)
            .map(str::to_string),
        score: payload_u8(payload, "score", 96),
    }
}

pub(super) fn dispatch_request(req: &HttpRequest, tx: &mpsc::Sender<DevCommand>) -> String {
    let (path, query) = split_path_query(&req.path);
    let result: Result<(u16, String)> = match (req.method.as_str(), path) {
        ("POST", "/action") => post_action(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/query") => get_query(query, tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/key") => post_key(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/click") => post_click(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/health") => get_health(tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/quit") => post_quit(tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/seed") => post_qa_seed(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/room-eq") => post_qa_room_eq(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/room-eq/export-json") => {
            post_qa_room_eq_export_json(&req.body, tx).map(|r| {
                let status = if r.value.is_ok() { 200 } else { 500 };
                (status, r.to_json())
            })
        }
        ("GET", "/elements") => Ok((200, list_elements_json())),
        _ => Err(anyhow!("unknown route: {} {}", req.method, req.path)),
    };
    match result {
        Ok((status, body)) => http_response(status, &body),
        Err(e) => http_response(400, &DevReply::err(format!("{e:#}")).to_json()),
    }
}
