//! Command + reply types exchanged between the HTTP server thread
//! and the GPUI main thread.

use std::sync::mpsc;

/// Reply payload returned to the HTTP handler thread once a command
/// has been processed by the GPUI side.
#[derive(Debug, Clone)]
pub struct DevReply {
    pub ok: bool,
    pub error: Option<String>,
}

impl DevReply {
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

/// Reply for a /query request — same envelope as DevReply, plus an
/// optional `value` payload on success.
#[derive(Debug, Clone)]
pub struct DevQueryReply {
    pub value: Result<serde_json::Value, String>,
}

impl DevQueryReply {
    pub fn ok(value: serde_json::Value) -> Self {
        Self { value: Ok(value) }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            value: Err(msg.into()),
        }
    }

    pub fn to_json(&self) -> String {
        match &self.value {
            Ok(v) => format!(r#"{{"ok":true,"value":{}}}"#, v),
            Err(e) => format!(
                r#"{{"ok":false,"error":{}}}"#,
                serde_json::Value::String(e.clone())
            ),
        }
    }
}

/// Commands the HTTP server can send to the GPUI main thread.
pub enum DevCommand {
    /// Dispatch a named Action with optional JSON payload.
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
    /// Synthesise a keystroke (e.g. "cmd-shift-p", "enter", "a"). The
    /// string is parsed by `gpui::Keystroke::parse`.
    Key {
        keystroke: String,
        reply: mpsc::SyncSender<DevReply>,
    },
    /// Click a tracked element by selector. The element must have been
    /// registered via `dev_track(<selector>, ...)` in the UI tree on a
    /// recent frame.
    Click {
        selector: String,
        reply: mpsc::SyncSender<DevReply>,
    },
}
