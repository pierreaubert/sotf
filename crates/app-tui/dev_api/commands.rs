//! Command + reply types exchanged between the HTTP server thread
//! and the TUI main thread.

use std::path::PathBuf;
use std::sync::mpsc;

/// Reply payload returned to the HTTP handler thread once a command
/// has been processed by the TUI side.
#[derive(Debug, Clone)]
pub struct DevReply {
    pub ok: bool,
    pub error: Option<String>,
}

impl DevReply {
    #[must_use]
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        match &self.error {
            None => r#"{"ok":true}"#.to_string(),
            Some(e) => format!(
                r#"{{"ok":false,"error":{}}}"#,
                serde_json::Value::String(e.clone())
            ),
        }
    }
}

/// Reply for a /query request — same envelope as `DevReply`, plus an
/// optional `value` payload on success.
#[derive(Debug, Clone)]
pub struct DevQueryReply {
    pub value: Result<serde_json::Value, String>,
}

impl DevQueryReply {
    #[must_use]
    pub fn ok(value: serde_json::Value) -> Self {
        Self { value: Ok(value) }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            value: Err(msg.into()),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        match &self.value {
            Ok(v) => format!(r#"{{"ok":true,"value":{v}}}"#),
            Err(e) => format!(
                r#"{{"ok":false,"error":{}}}"#,
                serde_json::Value::String(e.clone())
            ),
        }
    }
}

/// Commands the HTTP server can send to the TUI main thread.
#[derive(Debug)]
pub enum DevCommand {
    /// Dispatch a named action with optional JSON payload.
    Action {
        name: String,
        payload: Option<serde_json::Value>,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Read a property by string path.
    Query {
        path: String,
        reply: mpsc::SyncSender<DevQueryReply>,
    },
    /// Synthesise a keystroke (e.g. "enter", "a", "ctrl-c").
    /// The string is parsed into a `crossterm::event::KeyEvent`.
    Key {
        keystroke: String,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Dispatch bounded text as individual character key events.
    Text {
        text: String,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Return an allow-listed protocol-v2 state snapshot.
    Snapshot {
        reply: mpsc::SyncSender<DevQueryReply>,
    },
    /// Return a small readiness/status payload.
    Health {
        reply: mpsc::SyncSender<DevQueryReply>,
    },
    /// Gracefully quit the app.
    Quit { reply: mpsc::SyncSender<DevReply> },
    /// Add QA audio directories, then start the production library scanner.
    QaSeed {
        library_dirs: Vec<PathBuf>,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Install a virtual capture backend for a hardware-free recording flow.
    QaVirtualCapture {
        profile: String,
        output_directory: Option<PathBuf>,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Point the `RoomEQ` `LoadData` form at a copied measurement fixture.
    QaRoomEqFixture {
        fixture_dir: PathBuf,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Enable or disable the isolated test-only playback sink for this Player.
    QaPlaybackOutput {
        enabled: bool,
        reply: mpsc::SyncSender<DevReply>,
    },
}
