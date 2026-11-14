//! Tiny HTTP/1.1 server for the dev API.
//!
//! Runs an OS thread bound to `127.0.0.1:<port>`. Each connection is
//! handled inline (one at a time is fine for testing), parsed into a
//! [`DevCommand`], and forwarded to the TUI main thread via an mpsc
//! channel. The handler blocks on a synchronous reply channel before
//! writing the HTTP response and closing the connection.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use sotf_dev_api::server::{ServerConfig, start_server};
use sotf_dev_api::{
    Capabilities, FixtureCapability, HttpResponse, InputCapabilities, Method, NamedCapability,
    RunId,
};

use super::commands::{DevCommand, DevQueryReply, DevReply};

const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Spawn the dev-api server. Listens on `127.0.0.1:<port>`.
///
/// Returns an mpsc receiver that the TUI main loop should poll for
/// incoming commands.
pub fn start(port: u16) -> mpsc::Receiver<DevCommand> {
    let (tx, rx) = mpsc::sync_channel::<DevCommand>(64);
    let run_id = std::env::var("SOTF_DEV_API_RUN_ID")
        .map_err(anyhow::Error::from)
        .and_then(|value| RunId::parse(value).map_err(anyhow::Error::from));
    match run_id {
        Ok(run_id) => {
            let mut config = ServerConfig::loopback(run_id, capabilities());
            config.bind = ([127, 0, 0, 1], port).into();
            let dispatcher_tx = tx.clone();
            match start_server(
                config,
                move |request: sotf_dev_api::HttpRequest, _context| {
                    let request = HttpRequest {
                        method: match request.method {
                            Method::Get => "GET".into(),
                            Method::Post => "POST".into(),
                        },
                        path: request.path,
                        body: request.body,
                    };
                    legacy_response(&dispatch_request(&request, &dispatcher_tx))
                },
            ) {
                Ok(handle) => {
                    let endpoint = handle.endpoint();
                    let owner = std::thread::Builder::new()
                        .name("sotf-tui-dev-api-owner".into())
                        .spawn(move || {
                            loop {
                                std::hint::black_box(&handle);
                                std::thread::park_timeout(Duration::from_secs(3600));
                            }
                        });
                    match owner {
                        Ok(_) => {
                            log::info!("TUI dev-api protocol v2 listening on http://{endpoint}");
                        }
                        Err(error) => {
                            log::error!("Failed to retain TUI dev-api server handle: {error}");
                        }
                    }
                }
                Err(error) => log::error!("TUI dev-api listener failed: {error}"),
            }
        }
        Err(error) => {
            log::error!(
                "refusing to start TUI dev-api without a valid SOTF_DEV_API_RUN_ID: {error:#}"
            );
        }
    }

    rx
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the supported API routes and their response mappings together"
)]
fn capabilities() -> Capabilities {
    let mut capabilities = Capabilities::new("tui", "sotf-tui");
    capabilities.build_version = env!("CARGO_PKG_VERSION").into();
    capabilities.build_id = option_env!("SOTF_BUILD_ID").unwrap_or("development").into();
    capabilities.debug_features = vec!["dev-api".into(), "qa".into(), "headless".into()];
    capabilities.actions = [
        "PlayPause",
        "Stop",
        "VolumeUp",
        "VolumeDown",
        "Mute",
        "SwitchToLibrary",
        "SwitchToQueue",
        "SwitchToConfigure",
        "SwitchToPlugins",
        "SwitchToDevices",
        "SwitchToPlaylists",
        "PluginClear",
        "PluginAdd",
        "PluginRemove",
        "PluginToggle",
        "PluginMoveUp",
        "PluginMoveDown",
        "PluginSetParam",
        "PluginSetParamString",
        "PluginChainSave",
        "PluginChainLoad",
        "MetadataSeedAlbum",
        "MetadataOpenAlbumEditor",
        "MetadataSetField",
        "MetadataPreview",
    ]
    .into_iter()
    .map(|name| NamedCapability {
        name: name.into(),
        family: if name.starts_with("SwitchTo") {
            "navigation".into()
        } else {
            "tui_action".into()
        },
        payload_schema: None,
    })
    .collect();
    capabilities.queries = vec![
        "screen.focused".into(),
        "input_mode".into(),
        "configure.sub_screen".into(),
        "playback.volume".into(),
        "playback.is_playing".into(),
        "playback.muted".into(),
        "playback.has_current_track".into(),
        "queue.length".into(),
        "queue.current_index".into(),
        "library.album_count".into(),
        "library.track_count".into(),
        "library.visible_album_count".into(),
        "library.selected_album_index".into(),
        "metadata.editor_open".into(),
        "recording.review_needed_count".into(),
        "recording.quality_results".into(),
        "recording.thd_warning_count".into(),
        "recording.max_thd_percent".into(),
        "recording.unknown_finding_count".into(),
        "recording.unknown_review_ack_required".into(),
        "recording.unknown_review_explained".into(),
        "recording.unknown_finding_count".into(),
        "recording.quality_sidecar_count".into(),
        "recording.quality_sidecar_results".into(),
        "recording.quality_sidecar_unknown_count".into(),
        "recording.save_success".into(),
        "recording.save_error".into(),
        "recording.persisted".into(),
        "learning.session_active".into(),
        "learning.config_trial_count".into(),
        "learning.session_current_answered".into(),
        "learning.session_trial_count".into(),
        "learning.session_correct_count".into(),
        "learning.session_answered".into(),
        "learning.progress_total_attempts".into(),
        "learning.progress_persisted".into(),
        "learning.progress_accuracy".into(),
        "learning.filtered_audition".into(),
        "learning.selected_path".into(),
        "ab_testing.step".into(),
        "ab_testing.path_a_captured".into(),
        "ab_testing.path_b_captured".into(),
        "ab_testing.status".into(),
        "ab_testing.session_active".into(),
        "ab_testing.completed_trials".into(),
        "ab_testing.abx_score".into(),
        "ab_testing.prepared_duration_ms".into(),
        "ab_testing.level_match_correction_db".into(),
        "ab_testing.session_persisted".into(),
        "ab_testing.runtime_active".into(),
        "ab_testing.trial_mode".into(),
        "ab_testing.blind_mode".into(),
        "ab_testing.phase".into(),
        "ab_testing.available_cue_count".into(),
        "ab_testing.available_answer_count".into(),
        "ab_testing.assignment_seed".into(),
        "ab_testing.committed_x_path".into(),
        "recording.step".into(),
        "recording.step_tab_focused".into(),
        "recording.editing_path".into(),
        "recording.focus_context".into(),
        "recording.selected_field".into(),
        "recording.signal_duration_secs".into(),
        "recording.bass_octave_duration_s".into(),
        "recording.duration_control_kind".into(),
        "recording.signal_type".into(),
        "recording.effective_acquisition".into(),
        "recording.effective_playback_duration_secs".into(),
        "recording.duration_control_applies".into(),
        "recording.stimulus_configs".into(),
        "recording.stimulus_config_count".into(),
        "recording.stimulus_levels_vary".into(),
        "recording.stimulus_config_matches_current".into(),
        "recording.signal_level_db".into(),
        "recording.sweep_start_hz".into(),
        "recording.sweep_end_hz".into(),
        "recording.num_sweeps".into(),
        "recording.selected_sweep_start_hz".into(),
        "recording.selected_sweep_end_hz".into(),
        "recording.all_done".into(),
        "recording.done_count".into(),
        "recording.channel_count".into(),
        "recording.status".into(),
        "recording.virtual_capture_profile".into(),
        "recording.current_state".into(),
        "roomeq.step".into(),
        "roomeq.step_tab_focused".into(),
        "roomeq.editing_file_path".into(),
        "roomeq.focus_context".into(),
        "roomeq.measurement_count".into(),
        "roomeq.optimization_status".into(),
        "roomeq.result_count".into(),
        "roomeq.has_dsp_output".into(),
        "roomeq.dsp_channel_count".into(),
        "roomeq.filter_count".into(),
        "roomeq.average_pre_score".into(),
        "roomeq.average_post_score".into(),
        "roomeq.status".into(),
        "roomeq.error".into(),
        "roomeq.file_path".into(),
        "roomeq.export_path".into(),
        "roomeq.export_exists".into(),
        "roomeq.export_filter_count".into(),
        "roomeq.export_filter_count_matches_result".into(),
        "roomeq.result_filter_count".into(),
        "roomeq.result_nonzero_gain_count".into(),
        "roomeq.applied_eq_filter_count".into(),
        "roomeq.applied_eq_nonzero_gain_count".into(),
        "roomeq.applied_eq_filters".into(),
        "roomeq.applied_eq_parameters_match_results".into(),
        "roomeq.apply_status".into(),
        "roomeq.apply_error".into(),
        "roomeq.quality_sidecar_count".into(),
        "roomeq.quality_sidecar_unknown_count".into(),
        "roomeq.quality_review_entry_count".into(),
        "roomeq.stimulus_configs".into(),
        "roomeq.stimulus_config_count".into(),
        "roomeq.capture_level_review_note".into(),
        "roomeq.stimulus_configs_match_recording".into(),
        "roomeq.quality_unknown_count".into(),
        "roomeq.thd_warning_count".into(),
        "roomeq.max_thd_percent".into(),
        "roomeq.unknown_finding_count".into(),
        "roomeq.quality_review_entries".into(),
        "roomeq.quality_sidecar_warning".into(),
    ];
    capabilities.inputs = InputCapabilities {
        key: true,
        text: true,
        selector: false,
        pointer: false,
        touch: false,
        scroll: false,
        resize: true,
        remote: vec![],
    };
    capabilities.fixtures = ["seed", "virtual-capture", "room-eq-ui", "playback-output"]
        .into_iter()
        .map(|name| FixtureCapability {
            name: name.into(),
            max_body_bytes: capabilities.limits.fixture_body_bytes,
        })
        .collect();
    capabilities
}

fn legacy_response(response: &str) -> HttpResponse {
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((response, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_ascii_whitespace().nth(1))
        .and_then(|status| status.parse().ok())
        .unwrap_or(500);
    HttpResponse::json(status, body.as_bytes().to_vec())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn dispatch_request(req: &HttpRequest, tx: &mpsc::SyncSender<DevCommand>) -> String {
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
        ("POST", "/text") => post_text(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/health") => get_health(tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/snapshot") => get_snapshot(tx).map(|r| {
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
        ("POST", "/qa/capture/virtual") => post_qa_virtual_capture(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/room-eq/ui-fixture") => post_qa_room_eq_fixture(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/playback/output") => post_qa_playback_output(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        _ => Err(anyhow!("unknown route: {} {}", req.method, req.path)),
    };
    match result {
        Ok((status, body)) => http_response(status, &body),
        Err(e) => http_response(400, &DevReply::err(format!("{e:#}")).to_json()),
    }
}

fn split_path_query(full: &str) -> (&str, &str) {
    match full.find('?') {
        Some(i) => (&full[..i], &full[i + 1..]),
        None => (full, ""),
    }
}

fn get_query(raw_query: &str, tx: &mpsc::SyncSender<DevCommand>) -> Result<DevQueryReply> {
    let mut path: Option<String> = None;
    for kv in raw_query.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        if k == "path" {
            path = Some(percent_decode(v));
        }
    }
    let path = path.ok_or_else(|| anyhow!("missing `path` query parameter"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Query {
        path,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn percent_decode(s: &str) -> String {
    // Minimal: handle %XX and `+` → space. Fine for ASCII paths.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn post_action(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        name: String,
        #[serde(default)]
        payload: Option<serde_json::Value>,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx): (mpsc::SyncSender<DevReply>, _) = mpsc::sync_channel(1);
    tx.send(DevCommand::Action {
        name: payload.name,
        payload: payload.payload,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;

    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_key(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        keystroke: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Key {
        keystroke: payload.keystroke,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_text(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        text: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    if payload.text.chars().count() > 4096 {
        return Err(anyhow!("text input exceeds 4096 characters"));
    }
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Text {
        text: payload.text,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))
}

fn get_health(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevQueryReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Health { reply: reply_tx })
        .map_err(|_error| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn get_snapshot(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevQueryReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Snapshot { reply: reply_tx })
        .map_err(|_error| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api snapshot reply timeout"))
}

fn post_quit(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Quit { reply: reply_tx })
        .map_err(|_error| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_qa_seed(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        #[serde(default)]
        library_dirs: Vec<std::path::PathBuf>,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaSeed {
        library_dirs: payload.library_dirs,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_qa_virtual_capture(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        profile: String,
        #[serde(default)]
        output_directory: Option<std::path::PathBuf>,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaVirtualCapture {
        profile: payload.profile,
        output_directory: payload.output_directory,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))
}

fn post_qa_room_eq_fixture(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        fixture_dir: std::path::PathBuf,
        #[serde(default)]
        invalid: Option<String>,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    if payload.invalid.is_some() {
        return Ok(DevReply::err(
            "TUI RoomEQ fixture accepts measurement files only; invalid state fixtures are unsupported",
        ));
    }
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRoomEqFixture {
        fixture_dir: payload.fixture_dir,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))
}

fn post_qa_playback_output(body: &[u8], tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        enabled: bool,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaPlaybackOutput {
        enabled: payload.enabled,
        reply: reply_tx,
    })
    .map_err(|_error| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_error| anyhow!("dev-api reply timeout"))
}

fn http_response(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body,
    )
}
