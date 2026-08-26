use crate::app::state::AppState;
use crate::ui::PlayerView;
use anyhow::{Result, anyhow};
use gpui::{AnyWindowHandle, App, Context};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Increment whenever the dev-driver HTTP contract changes incompatibly.
pub(super) const DEV_API_PROTOCOL_VERSION: u32 = 1;

static PROCESS_STARTED_AT: OnceLock<SystemTime> = OnceLock::new();

pub(super) fn mark_process_started() {
    let _ = PROCESS_STARTED_AT.set(SystemTime::now());
}

pub(super) fn with_player_view<F, R>(window: AnyWindowHandle, cx: &mut App, f: F) -> Result<R>
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

pub(super) fn with_app_state<F, R>(window: AnyWindowHandle, cx: &mut App, f: F) -> Result<R>
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

pub(super) fn health_payload(window: AnyWindowHandle, cx: &mut App) -> Result<serde_json::Value> {
    with_app_state(window, cx, |state| {
        let process_started_at = PROCESS_STARTED_AT.get_or_init(SystemTime::now);
        let process_started_at_unix_ms = process_started_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        Ok(serde_json::json!({
            "ok": true,
            "pid": std::process::id(),
            "protocol_version": DEV_API_PROTOCOL_VERSION,
            "dev_api_enabled": true,
            "binary": {
                "package": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION"),
                "build_id": option_env!("SOTF_BUILD_ID").unwrap_or(env!("CARGO_PKG_VERSION")),
                "git_commit": option_env!("SOTF_GIT_COMMIT").unwrap_or("unknown"),
                "features": ["dev-api"],
            },
            "run_id": std::env::var("SOTF_DEV_API_RUN_ID").ok(),
            "process_started_at_unix_ms": process_started_at_unix_ms,
            "qa_directory": std::env::var("SOTF_QA_DIR").ok(),
            "viewport": {
                "width": state.app.ui_state.window_width,
                "height": state.app.ui_state.window_height,
            },
            "theme": format!("{:?}", state.app.ui_state.theme_id),
            "locale": format!("{:?}", state.app.ui_state.language),
            "screen": format!("{:?}", state.app.ui_state.current_screen),
            "queue_length": state.app.queue_state.len(),
        }))
    })
}
