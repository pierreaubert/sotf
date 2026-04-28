//! Allow-listed property queries for the dev API.
//!
//! The match here is the entire surface of `/query`. Adding a new
//! property is two lines (one match arm + a comment). We deliberately
//! avoid reflective JSON serialisation of internal state — scripts
//! should depend on a small, stable subset.

use anyhow::{Result, anyhow};
use gpui::{AnyWindowHandle, App};
use serde_json::{Value, json};

use crate::app::state::AppState;
use crate::ui::PlayerView;

pub fn resolve(path: &str, window: AnyWindowHandle, cx: &mut App) -> Result<Value> {
    window
        .update(cx, |any_view, _window, cx| {
            let entity = any_view
                .downcast::<PlayerView>()
                .map_err(|_| anyhow!("root view is not PlayerView"))?;
            let view = entity.read(cx);
            let state: &AppState = view.state.read(cx);
            read_path(path, state)
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?
}

fn read_path(path: &str, state: &AppState) -> Result<Value> {
    let app = &state.app;
    Ok(match path {
        "playback.volume" => json!(app.playback.volume),
        "playback.is_playing" => json!(app.playback.is_playing),
        "playback.muted" => json!(app.playback.muted),
        "screen.focused" => json!(format!("{:?}", app.ui_state.current_screen)),
        "queue.length" => json!(app.queue_state.len()),
        "queue.current_index" => match app.playback.current_queue_index {
            Some(i) => json!(i),
            None => Value::Null,
        },
        other => return Err(anyhow!("unknown query path: `{other}`")),
    })
}
