//! Loudness Monitor plugin surface, including transient I/LRA lifecycle controls.

use super::level_meters::render_lufs_with_true_peak;
use crate::app::AppState;
use crate::app::i18n::LevelMeterTranslations;
use crate::app::state::plugin::LoudnessControlUiRequest;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};

#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};

pub fn render_loudness_monitor_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    graph_plugin_id: Option<usize>,
    engine_index: Option<usize>,
    control_available: bool,
    loudness: Option<std::sync::Arc<sotf_audio_player::LoudnessData>>,
    layout_scale: f32,
    text: LevelMeterTranslations,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let runtime_id = loudness
        .as_ref()
        .map(|snapshot| snapshot.integrated_control_instance_id)
        .filter(|id| *id != 0);
    let request = graph_plugin_id.and_then(|id| {
        entity
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .loudness_control_requests
            .get(&id)
            .cloned()
    });
    let status = loudness_control_status(
        request.as_ref(),
        runtime_id,
        loudness.as_deref(),
        control_available,
        text,
    );
    let status_view = div().child(status.clone());
    #[cfg(feature = "dev-api")]
    let status_view = status_view.dev_track_with_state(
        format!(
            "plugin.loudness.control-status.{}",
            graph_plugin_id.unwrap_or(0)
        ),
        DevElementState::default().text(status),
    );

    let mut controls = div().flex().flex_wrap().gap(d.grid).w_full();
    let enabled = control_available
        && graph_plugin_id.is_some()
        && engine_index.is_some()
        && runtime_id.is_some();
    for (operation, label) in [
        ("start", text.loudness_start),
        ("pause", text.loudness_pause),
        ("continue", text.loudness_continue),
        ("reset", text.loudness_reset),
    ] {
        let graph_id = graph_plugin_id;
        let engine = engine_index;
        let runtime = runtime_id;
        let entity_for_click = entity.clone();
        let label_text = label.to_string();
        #[cfg(feature = "dev-api")]
        let selector = graph_id.map(|id| format!("plugin.loudness.control.{id}.{operation}"));
        let button = Button::new(
            SharedString::from(format!(
                "loudness-control-{operation}-{}",
                graph_id.unwrap_or(0)
            )),
            label_text.clone(),
        )
        .size(ButtonSize::Xs)
        .variant(if operation == "reset" {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        })
        .disabled(!enabled)
        .aria_label(label_text)
        .theme(theme.to_button_theme())
        .on_click(move |_window, app_cx| {
            if let (Some(graph_id), Some(engine), Some(runtime)) = (graph_id, engine, runtime) {
                submit_loudness_control(
                    &entity_for_click,
                    graph_id,
                    engine,
                    runtime,
                    operation,
                    text,
                    app_cx,
                );
            }
        });
        #[cfg(feature = "dev-api")]
        let button = if let Some(selector) = selector {
            button.dev_track_with_state(
                selector,
                DevElementState::default().enabled(enabled).text(label),
            )
        } else {
            button.dev_track_with_state(
                format!("plugin.loudness.control.unavailable.{operation}"),
                DevElementState::default().enabled(false).text(label),
            )
        };
        controls = controls.child(button);
    }
    let retry = request.clone();
    let entity_for_retry = entity.clone();
    let retry_enabled =
        loudness_control_retry_enabled(enabled, retry.as_ref(), runtime_id, loudness.as_deref());
    let retry_button = Button::new(
        SharedString::from(format!(
            "loudness-control-retry-{}",
            graph_plugin_id.unwrap_or(0)
        )),
        text.loudness_retry,
    )
    .size(ButtonSize::Xs)
    .variant(ButtonVariant::Ghost)
    .disabled(!retry_enabled)
    .theme(theme.to_button_theme())
    .on_click(move |_window, app_cx| {
        if let (Some(graph_id), Some(engine), Some(runtime), Some(request)) =
            (graph_plugin_id, engine_index, runtime_id, retry.as_ref())
        {
            submit_loudness_control(
                &entity_for_retry,
                graph_id,
                engine,
                runtime,
                request.operation,
                text,
                app_cx,
            );
        }
    });
    #[cfg(feature = "dev-api")]
    let retry_button = retry_button.dev_track_with_state(
        format!(
            "plugin.loudness.control.retry.{}",
            graph_plugin_id.unwrap_or(0)
        ),
        DevElementState::default()
            .enabled(retry_enabled)
            .text(text.loudness_retry),
    );
    controls = controls.child(retry_button);

    let panel = div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(render_lufs_with_true_peak(
            d,
            loudness.as_deref(),
            layout_scale,
            text,
            theme,
        ))
        .child(status_view)
        .child(controls);
    #[cfg(feature = "dev-api")]
    let panel = panel.dev_track("plugin.loudness-monitor.panel");
    panel
}

fn loudness_control_status(
    request: Option<&LoudnessControlUiRequest>,
    runtime_id: Option<u64>,
    loudness: Option<&sotf_audio_player::LoudnessData>,
    control_available: bool,
    text: LevelMeterTranslations,
) -> String {
    if let Some(request) = request {
        if runtime_id != Some(request.runtime_instance_id) {
            return text.control_monitor_changed.to_string();
        }
        if let Some(loudness) = loudness {
            if loudness.integrated_control_request_id == request.request_id {
                return if loudness.integrated_measurement_running {
                    text.integrated_running.to_string()
                } else {
                    text.integrated_paused.to_string()
                };
            }
            if loudness.integrated_control_request_id > request.request_id {
                return text.control_superseded.to_string();
            }
        }
        if let Some(error) = request.error.as_ref() {
            return format!("{}: {error}", text.control_submission_failed);
        }
    }
    if !control_available {
        return text.control_unavailable.to_string();
    }
    if runtime_id.is_none() {
        return text.control_unavailable.to_string();
    }
    if let Some(request) = request {
        let operation = match request.operation {
            "start" => text.loudness_start,
            "pause" => text.loudness_pause,
            "continue" => text.loudness_continue,
            "reset" => text.loudness_reset,
            _ => text.loudness_start,
        };
        return format!("{operation} · {}", text.control_waiting);
    }
    if loudness.is_some_and(|data| data.integrated_measurement_running) {
        text.integrated_running.to_string()
    } else {
        text.integrated_paused.to_string()
    }
}

fn loudness_control_retry_enabled(
    controls_enabled: bool,
    request: Option<&LoudnessControlUiRequest>,
    runtime_id: Option<u64>,
    loudness: Option<&sotf_audio_player::LoudnessData>,
) -> bool {
    if !controls_enabled {
        return false;
    }
    let (Some(request), Some(runtime_id), Some(loudness)) = (request, runtime_id, loudness) else {
        return false;
    };
    !request.command.is_empty()
        && request.runtime_instance_id == runtime_id
        && loudness.integrated_control_request_id < request.request_id
}

fn submit_loudness_control(
    entity: &Entity<AppState>,
    graph_plugin_id: usize,
    engine_index: usize,
    runtime_instance_id: u64,
    operation: &'static str,
    text: LevelMeterTranslations,
    cx: &mut App,
) {
    entity.update(cx, |state, cx| {
        let ui = &mut state.app.plugin_state.plugin_ui_state;
        let Some(request_id) = ui.next_loudness_control_request_id.checked_add(1) else {
            ui.loudness_control_requests.insert(
                graph_plugin_id,
                LoudnessControlUiRequest {
                    runtime_instance_id,
                    request_id: ui.next_loudness_control_request_id,
                    engine_index,
                    operation,
                    command: String::new(),
                    error: Some(text.control_id_exhausted.to_string()),
                },
            );
            cx.notify();
            return;
        };
        ui.next_loudness_control_request_id = request_id;
        let command = format!("{runtime_instance_id}:{request_id}:{operation}");
        let error = state
            .player
            .set_plugin_parameter(
                engine_index,
                "integrated_control_command".to_string(),
                command.clone(),
            )
            .err()
            .map(|error| error.to_string());
        ui.loudness_control_requests.insert(
            graph_plugin_id,
            LoudnessControlUiRequest {
                runtime_instance_id,
                request_id,
                engine_index,
                operation,
                command,
                error,
            },
        );
        cx.notify();
    });
}
