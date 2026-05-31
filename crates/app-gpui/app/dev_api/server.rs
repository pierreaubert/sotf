//! Tiny HTTP/1.1 server for the dev API.
//!
//! Runs an OS thread bound to `127.0.0.1:<port>`. Each connection is
//! handled inline (one at a time is fine for testing), parsed into a
//! [`DevCommand`], and forwarded to the GPUI main thread via an mpsc
//! channel. The handler blocks on a synchronous reply channel before
//! writing the HTTP response and closing the connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

use anyhow::{Result, anyhow};
use gpui::{
    AnyWindowHandle, App, AsyncApp, Context, Keystroke, MouseButton, MouseDownEvent, MouseUpEvent,
    PlatformInput, Point,
};
use sotf_audio_player::room_eq_types::{
    RoomEqWizardMode, SimpleCrossoverChoice, SimpleLossChoice, SimpleProcessingChoice, SpeakerTier,
};

use super::commands::{DevCommand, DevQueryReply, DevReply};
use super::{queries, registry};
use crate::app::state::AppState;
use crate::app::types::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, RecordingResult, RecordingState,
    RecordingStep, RoomEqStep,
};
use crate::ui::PlayerView;

const REPLY_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Spawn the dev-api server. Listens on `127.0.0.1:<port>`.
///
/// Must be called from the GPUI main thread, after the window has been
/// created. The returned task is detached and runs for the lifetime of
/// the app.
pub fn start(cx: &mut App, port: u16, window: AnyWindowHandle) {
    let (tx, rx) = mpsc::channel::<DevCommand>();

    std::thread::Builder::new()
        .name("sotf-dev-api".into())
        .spawn(move || {
            if let Err(e) = run_listener(port, tx) {
                log::error!("dev-api listener exited: {e:#}");
            }
        })
        .expect("failed to spawn dev-api thread");

    log::info!("dev-api listening on http://127.0.0.1:{port}");

    cx.spawn(async move |cx: &mut AsyncApp| {
        consume_commands(rx, window, cx).await;
    })
    .detach();
}

async fn consume_commands(rx: Receiver<DevCommand>, window: AnyWindowHandle, cx: &mut AsyncApp) {
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

fn process_command(cmd: DevCommand, window: AnyWindowHandle, cx: &mut AsyncApp) {
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

fn with_player_view<F, R>(window: AnyWindowHandle, cx: &mut App, f: F) -> Result<R>
where
    F: FnOnce(&mut PlayerView, &mut Context<PlayerView>) -> Result<R>,
{
    window
        .update(cx, |any_view, _window, cx| {
            let entity = any_view
                .downcast::<PlayerView>()
                .map_err(|_| anyhow!("root view is not PlayerView"))?;
            entity.update(cx, |view, cx| f(view, cx))
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?
}

fn with_app_state<F, R>(window: AnyWindowHandle, cx: &mut App, f: F) -> Result<R>
where
    F: FnOnce(&mut AppState) -> Result<R>,
{
    window
        .update(cx, |any_view, _window, cx| {
            let entity = any_view
                .downcast::<PlayerView>()
                .map_err(|_| anyhow!("root view is not PlayerView"))?;
            let state_entity = {
                let view = entity.read(cx);
                view.state.clone()
            };
            state_entity.update(cx, |state, _cx| f(state))
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?
}

fn health_payload(window: AnyWindowHandle, cx: &mut App) -> Result<serde_json::Value> {
    with_app_state(window, cx, |state| {
        Ok(serde_json::json!({
            "ok": true,
            "pid": std::process::id(),
            "screen": format!("{:?}", state.app.ui_state.current_screen),
            "queue_length": state.app.queue_state.len(),
        }))
    })
}

fn qa_room_eq(payload: serde_json::Value, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
    let fixture_dir = payload
        .get("fixture_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("room-eq payload needs `fixture_dir` string"))?;
    let fixture_dir = PathBuf::from(fixture_dir);
    if !fixture_dir.is_dir() {
        return Err(anyhow!(
            "RoomEQ fixture directory does not exist: {}",
            fixture_dir.display()
        ));
    }

    let start = payload
        .get("start")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let num_filters = payload
        .get("num_filters")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 32) as usize);
    let max_iter = payload
        .get("max_iter")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 50_000) as usize);
    let population = payload
        .get("population")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 10_000) as usize);
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .map(parse_speaker_tier)
        .transpose()?;
    let loss = payload
        .get("loss")
        .and_then(|v| v.as_str())
        .map(parse_simple_loss)
        .transpose()?;
    let processing = payload
        .get("processing")
        .and_then(|v| v.as_str())
        .map(parse_simple_processing)
        .transpose()?;
    let crossover = payload
        .get("crossover")
        .and_then(|v| v.as_str())
        .map(parse_simple_crossover)
        .transpose()?;

    let recording_state = load_room_eq_recording_fixture(&fixture_dir)?;

    with_player_view(window, cx, |view, cx| {
        view.state.update(cx, |state, cx| {
            state.app.measurement_state.recording_state = recording_state.clone();
            cx.notify();
        });

        view.load_room_eq_from_recording(cx);

        view.state.update(cx, |state, cx| {
            let room_eq = &mut state.app.measurement_state.room_eq_state;
            room_eq.wizard_mode = RoomEqWizardMode::Simple;
            if let Some(target) = target {
                room_eq.simple_preset.target = target;
            }
            if let Some(loss) = loss {
                room_eq.simple_preset.loss = loss;
            }
            if let Some(processing) = processing {
                room_eq.simple_preset.processing = processing;
            }
            if let Some(crossover) = crossover {
                room_eq.simple_preset.crossover = crossover;
            }
            let preset = room_eq.simple_preset.clone();
            sotf_audio_player::room_eq_types::apply_simple_preset(
                &preset,
                &mut room_eq.optimizer_config,
            );
            if let Some(num_filters) = num_filters {
                room_eq.optimizer_config.num_filters = num_filters;
            }
            if let Some(max_iter) = max_iter {
                room_eq.optimizer_config.max_iter = max_iter;
            }
            if let Some(population) = population {
                room_eq.optimizer_config.population = population;
            }
            room_eq.wizard_mode = RoomEqWizardMode::Full;
            room_eq.step = RoomEqStep::Optimize;
            room_eq.optimization_status = crate::app::types::OptimizationStatus::Idle;
            room_eq.channel_results.clear();
            room_eq.dsp_output = None;
            room_eq.overall_progress = 0.0;
            room_eq.progress_history.clear();
            room_eq.status_message =
                "QA RoomEQ fixture loaded with default wizard preset".to_string();
            room_eq.error_message = None;
            cx.notify();
        });

        if start {
            view.start_room_eq_optimization(cx);
        }

        Ok(())
    })
}

fn qa_room_eq_export_json(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<serde_json::Value> {
    let path = payload
        .get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(default_room_eq_export_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let dsp_output = with_app_state(window, cx, |state| {
        let room_eq = &state.app.measurement_state.room_eq_state;
        if room_eq.optimization_status != crate::app::types::OptimizationStatus::Completed {
            return Err(anyhow!(
                "RoomEQ optimization is not completed: {:?}",
                room_eq.optimization_status
            ));
        }
        room_eq
            .dsp_output
            .clone()
            .ok_or_else(|| anyhow!("RoomEQ has no DSP output to export"))
    })?;

    let json = serde_json::to_string_pretty(&dsp_output)?;
    std::fs::write(&path, json)?;
    let summary = room_eq_export_summary_for_path(&path)?;

    with_app_state(window, cx, |state| {
        let room_eq = &mut state.app.measurement_state.room_eq_state;
        room_eq.step = RoomEqStep::Export;
        room_eq.status_message = format!("QA RoomEQ JSON exported: {}", path.display());
        room_eq.error_message = None;
        Ok(())
    })?;

    Ok(summary)
}

fn default_room_eq_export_path() -> PathBuf {
    sotf_audio_player::config::get_app_config_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("sotf-qa"))
        .join("qa-room-eq-export.json")
}

fn room_eq_export_summary_for_path(path: &Path) -> Result<serde_json::Value> {
    let bytes = std::fs::metadata(path)?.len();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let channel_count = json
        .get("channels")
        .and_then(|v| v.as_object())
        .map(|channels| channels.len())
        .unwrap_or(0);
    let global_plugin_count = json
        .get("global_plugins")
        .and_then(|v| v.as_array())
        .map(|plugins| plugins.len())
        .unwrap_or(0);
    let channel_plugins: Vec<&serde_json::Value> = json
        .get("channels")
        .and_then(|v| v.as_object())
        .map(|channels| {
            channels
                .values()
                .filter_map(|channel| channel.get("plugins").and_then(|v| v.as_array()))
                .flat_map(|plugins| plugins.iter())
                .collect()
        })
        .unwrap_or_default();
    let channel_plugin_count = channel_plugins.len();
    let filter_count = channel_plugins
        .iter()
        .filter_map(|plugin| plugin.get("parameters"))
        .filter_map(|params| params.get("filters"))
        .filter_map(|filters| filters.as_array())
        .map(|filters| filters.len())
        .sum::<usize>();

    Ok(serde_json::json!({
        "path": path,
        "exists": true,
        "bytes": bytes,
        "version": json.get("version").and_then(|v| v.as_str()),
        "channel_count": channel_count,
        "plugin_count": global_plugin_count + channel_plugin_count,
        "filter_count": filter_count,
    }))
}

fn load_room_eq_recording_fixture(fixture_dir: &Path) -> Result<RecordingState> {
    let recordings_path = fixture_dir.join("recordings.json");
    if !recordings_path.is_file() {
        return Err(anyhow!(
            "RoomEQ fixture is missing recordings.json: {}",
            recordings_path.display()
        ));
    }
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&recordings_path)?)
        .map_err(|e| anyhow!("invalid recordings.json: {e}"))?;
    let speakers = json
        .get("speakers")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("recordings.json needs `speakers` object"))?;

    let mut names: Vec<String> = speakers.keys().cloned().collect();
    names.sort_by_key(|name| channel_sort_key(name));
    if names.is_empty() {
        return Err(anyhow!("recordings.json contains no speakers"));
    }

    let mut recording = RecordingState {
        recording_directory: Some(fixture_dir.to_string_lossy().into_owned()),
        ..RecordingState::default()
    };
    recording.playback_config.num_channels = names.len();
    recording.playback_config.channel_mappings = names
        .iter()
        .enumerate()
        .map(|(idx, name)| ChannelMapping::single(idx + 1, name.clone()))
        .collect();
    recording.recording_config.num_channels = 1;
    recording.recording_config.channel_mappings = vec![0];

    recording.channel_recordings = names
        .iter()
        .enumerate()
        .map(|(idx, name)| -> Result<ChannelRecording> {
            let speaker = speakers
                .get(name)
                .ok_or_else(|| anyhow!("missing speaker `{name}`"))?;
            let frequencies = json_f32_array(speaker, "frequencies", name)?;
            let magnitude_db = json_f32_array(speaker, "magnitude_db", name)?;
            let phase_deg = json_f32_array(speaker, "phase_deg", name)?;
            if frequencies.len() != magnitude_db.len() || frequencies.len() != phase_deg.len() {
                return Err(anyhow!(
                    "speaker `{name}` has mismatched response lengths: frequencies={}, magnitude_db={}, phase_deg={}",
                    frequencies.len(),
                    magnitude_db.len(),
                    phase_deg.len()
                ));
            }

            let mut rec = ChannelRecording::new(idx, name.clone());
            rec.state = ChannelRecordingState::Done;
            rec.result = Some(RecordingResult {
                channel: idx,
                wav_path: fixture_child_path(fixture_dir, speaker.get("wav_path")),
                csv_path: fixture_child_path(fixture_dir, speaker.get("csv_path")),
                frequencies,
                magnitude_db,
                phase_deg,
                impulse_response: None,
                impulse_time_ms: None,
                thd_percent: None,
                harmonic_distortion_db: None,
                excess_group_delay_ms: None,
                rt60_ms: None,
                clarity_c50_db: None,
                clarity_c80_db: None,
                spectrogram_db: None,
            });
            Ok(rec)
        })
        .collect::<Result<Vec<_>>>()?;

    recording.step = RecordingStep::Evaluating;
    recording.recording_progress = 1.0;
    recording.current_recording_channel = None;
    recording.status_message = format!("QA RoomEQ fixture loaded: {} channels", names.len());

    Ok(recording)
}

fn json_f32_array(parent: &serde_json::Value, key: &str, speaker_name: &str) -> Result<Vec<f32>> {
    let array = parent
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("speaker `{speaker_name}` needs `{key}` array"))?;
    array
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            value
                .as_f64()
                .map(|v| v as f32)
                .ok_or_else(|| anyhow!("speaker `{speaker_name}` `{key}`[{idx}] is not numeric"))
        })
        .collect()
}

fn fixture_child_path(root: &Path, value: Option<&serde_json::Value>) -> Option<String> {
    let raw = value.and_then(|v| v.as_str())?;
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    Some(path.to_string_lossy().into_owned())
}

fn channel_sort_key(name: &str) -> (usize, String) {
    let rank = match name {
        "L" => 0,
        "R" => 1,
        "C" => 2,
        "LFE" | "Sub" | "SW" => 3,
        "SL" => 4,
        "SR" => 5,
        "BL" => 6,
        "BR" => 7,
        _ => 100,
    };
    (rank, name.to_string())
}

fn parse_speaker_tier(value: &str) -> Result<SpeakerTier> {
    match value {
        "NearField" | "near-field" | "nearfield" => Ok(SpeakerTier::NearField),
        "MidField" | "mid-field" | "midfield" => Ok(SpeakerTier::MidField),
        "FarField" | "far-field" | "farfield" => Ok(SpeakerTier::FarField),
        other => Err(anyhow!("unknown RoomEQ target `{other}`")),
    }
}

fn parse_simple_loss(value: &str) -> Result<SimpleLossChoice> {
    match value {
        "Flat" | "flat" => Ok(SimpleLossChoice::Flat),
        "Epa" | "EPA" | "epa" => Ok(SimpleLossChoice::Epa),
        other => Err(anyhow!("unknown RoomEQ loss `{other}`")),
    }
}

fn parse_simple_processing(value: &str) -> Result<SimpleProcessingChoice> {
    match value {
        "Iir" | "IIR" | "iir" => Ok(SimpleProcessingChoice::Iir),
        "MixedPhase" | "mixed-phase" | "mixed_phase" => Ok(SimpleProcessingChoice::MixedPhase),
        other => Err(anyhow!("unknown RoomEQ processing `{other}`")),
    }
}

fn parse_simple_crossover(value: &str) -> Result<SimpleCrossoverChoice> {
    match value {
        "Lr24" | "LR24" | "lr24" => Ok(SimpleCrossoverChoice::Lr24),
        "Lr48" | "LR48" | "lr48" => Ok(SimpleCrossoverChoice::Lr48),
        other => Err(anyhow!("unknown RoomEQ crossover `{other}`")),
    }
}

fn dispatch_key(keystroke_str: &str, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
    let keystroke = Keystroke::parse(keystroke_str)
        .map_err(|e| anyhow!("invalid keystroke `{keystroke_str}`: {e:?}"))?;
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

fn dispatch_click(selector: &str, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
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

fn dispatch_action(
    name: &str,
    payload: Option<serde_json::Value>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
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

fn resolve_action_name(name: &str, cx: &App) -> Result<String> {
    if name.contains("::") {
        return Ok(name.to_string());
    }
    let suffix = format!("::{name}");
    let matches: Vec<&str> = cx
        .all_action_names()
        .iter()
        .copied()
        .filter(|n| n.ends_with(&suffix) || *n == name)
        .collect();
    match matches.as_slice() {
        [] => Err(anyhow!("no action registered for `{name}`")),
        [only] => Ok((*only).to_string()),
        many => Err(anyhow!(
            "ambiguous action `{name}`: matched {} ({})",
            many.len(),
            many.join(", ")
        )),
    }
}

// ---------------------------------------------------------------------------
// HTTP listener (runs on a dedicated OS thread).
// ---------------------------------------------------------------------------

fn run_listener(port: u16, tx: mpsc::Sender<DevCommand>) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                log::warn!("dev-api accept failed: {e}");
                continue;
            }
        };
        if let Err(e) = handle_connection(stream, &tx) {
            log::warn!("dev-api connection error: {e:#}");
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, tx: &mpsc::Sender<DevCommand>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let req = parse_request(&mut stream)?;
    let response = dispatch_request(&req, tx);
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn parse_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut reader = BufReader::new(stream);

    // Request line.
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| anyhow!("missing method"))?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| anyhow!("missing path"))?
        .to_string();

    // Headers — we only care about Content-Length.
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        if header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some(value) = header
            .strip_prefix("Content-Length:")
            .or_else(|| header.strip_prefix("content-length:"))
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(HttpRequest { method, path, body })
}

fn dispatch_request(req: &HttpRequest, tx: &mpsc::Sender<DevCommand>) -> String {
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

fn split_path_query(full: &str) -> (&str, &str) {
    match full.find('?') {
        Some(i) => (&full[..i], &full[i + 1..]),
        None => (full, ""),
    }
}

fn get_query(raw_query: &str, tx: &mpsc::Sender<DevCommand>) -> Result<DevQueryReply> {
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
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
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

fn post_action(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        name: String,
        #[serde(default)]
        payload: Option<serde_json::Value>,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx): (SyncSender<DevReply>, _) = mpsc::sync_channel(1);
    tx.send(DevCommand::Action {
        name: payload.name,
        payload: payload.payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;

    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn list_elements_json() -> String {
    let entries = registry::snapshot();
    let mut items = Vec::with_capacity(entries.len());
    for (selector, bounds) in entries {
        let centre = bounds.center();
        items.push(serde_json::json!({
            "selector": selector,
            "x": f32::from(bounds.origin.x),
            "y": f32::from(bounds.origin.y),
            "w": f32::from(bounds.size.width),
            "h": f32::from(bounds.size.height),
            "cx": f32::from(centre.x),
            "cy": f32::from(centre.y),
        }));
    }
    serde_json::json!({ "ok": true, "elements": items }).to_string()
}

fn post_key(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
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
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_click(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
    #[derive(serde::Deserialize)]
    struct Payload {
        selector: String,
    }
    let payload: Payload =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))?;

    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Click {
        selector: payload.selector,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn get_health(tx: &mpsc::Sender<DevCommand>) -> Result<DevQueryReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Health { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_quit(tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Quit { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_qa_room_eq(body: &[u8], tx: &mpsc::Sender<DevCommand>) -> Result<DevReply> {
    let payload = parse_json_payload(body)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRoomEq {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_qa_room_eq_export_json(
    body: &[u8],
    tx: &mpsc::Sender<DevCommand>,
) -> Result<DevQueryReply> {
    let payload = if body.is_empty() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        parse_json_payload(body)?
    };
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaRoomEqExportJson {
        payload,
        reply: reply_tx,
    })
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn parse_json_payload(body: &[u8]) -> Result<serde_json::Value> {
    serde_json::from_slice(body).map_err(|e| anyhow!("invalid JSON: {e}"))
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
