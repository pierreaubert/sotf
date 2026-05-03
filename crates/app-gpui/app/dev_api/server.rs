//! Tiny HTTP/1.1 server for the dev API.
//!
//! Runs an OS thread bound to `127.0.0.1:<port>`. Each connection is
//! handled inline (one at a time is fine for testing), parsed into a
//! [`DevCommand`], and forwarded to the GPUI main thread via an mpsc
//! channel. The handler blocks on a synchronous reply channel before
//! writing the HTTP response and closing the connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

use anyhow::{Result, anyhow};
use gpui::{
    AnyWindowHandle, App, AsyncApp, Keystroke, MouseButton, MouseDownEvent, MouseUpEvent,
    PlatformInput, Point,
};

use super::commands::{DevCommand, DevQueryReply, DevReply};
use super::{queries, registry};

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
