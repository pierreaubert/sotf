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
                    std::thread::Builder::new()
                        .name("sotf-tui-dev-api-owner".into())
                        .spawn(move || {
                            loop {
                                std::hint::black_box(&handle);
                                std::thread::park_timeout(Duration::from_secs(3600));
                            }
                        })
                        .expect("failed to retain TUI dev-api server handle");
                    log::info!("TUI dev-api protocol v2 listening on http://{endpoint}");
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
        "queue.length".into(),
        "queue.current_index".into(),
        "library.album_count".into(),
        "library.track_count".into(),
        "metadata.editor_open".into(),
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
    capabilities.fixtures = vec![FixtureCapability {
        name: "seed".into(),
        max_body_bytes: capabilities.limits.fixture_body_bytes,
    }];
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
        ("POST", "/qa/seed") => post_qa_seed(tx).map(|r| {
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
    .map_err(|_| anyhow!("dev-api queue closed"))?;

    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
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
    .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn get_health(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevQueryReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Health { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn get_snapshot(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevQueryReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Snapshot { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api snapshot reply timeout"))
}

fn post_quit(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::Quit { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
}

fn post_qa_seed(tx: &mpsc::SyncSender<DevCommand>) -> Result<DevReply> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(DevCommand::QaSeed { reply: reply_tx })
        .map_err(|_| anyhow!("dev-api queue closed"))?;
    let reply = reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| anyhow!("dev-api reply timeout"))?;
    Ok(reply)
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
