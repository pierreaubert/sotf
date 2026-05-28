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
        "recording.all_done" => json!(
            app.measurement_state
                .recording_state
                .all_channels_recorded()
        ),
        "roomeq.step" => json!(format!("{:?}", app.measurement_state.room_eq_state.step)),
        "roomeq.measurement_count" => json!(
            app.measurement_state
                .room_eq_state
                .channel_measurements
                .len()
        ),
        "roomeq.speaker_config_count" => {
            json!(app.measurement_state.room_eq_state.speaker_configs.len())
        }
        "roomeq.optimization_status" => json!(format!(
            "{:?}",
            app.measurement_state.room_eq_state.optimization_status
        )),
        "roomeq.result_count" => json!(app.measurement_state.room_eq_state.channel_results.len()),
        "roomeq.has_dsp_output" => json!(app.measurement_state.room_eq_state.dsp_output.is_some()),
        "roomeq.dsp_channel_count" => json!(
            app.measurement_state
                .room_eq_state
                .dsp_output
                .as_ref()
                .map(|dsp| dsp.channels.len())
        ),
        "roomeq.filter_count" => json!(
            app.measurement_state
                .room_eq_state
                .channel_results
                .iter()
                .map(|result| result.eq_filters.len())
                .sum::<usize>()
        ),
        "roomeq.average_pre_score" => json!(average_room_eq_score(
            &app.measurement_state.room_eq_state.channel_results,
            |result| result.pre_score,
        )),
        "roomeq.average_post_score" => json!(average_room_eq_score(
            &app.measurement_state.room_eq_state.channel_results,
            |result| result.post_score,
        )),
        "roomeq.wizard.target" => {
            json!(format!(
                "{:?}",
                app.measurement_state.room_eq_state.simple_preset.target
            ))
        }
        "roomeq.wizard.loss" => {
            json!(format!(
                "{:?}",
                app.measurement_state.room_eq_state.simple_preset.loss
            ))
        }
        "roomeq.wizard.processing" => json!(format!(
            "{:?}",
            app.measurement_state.room_eq_state.simple_preset.processing
        )),
        "roomeq.wizard.crossover" => json!(format!(
            "{:?}",
            app.measurement_state.room_eq_state.simple_preset.crossover
        )),
        "roomeq.status" => json!(app.measurement_state.room_eq_state.status_message),
        "roomeq.error" => json!(app.measurement_state.room_eq_state.error_message),
        "roomeq.export.path" => json!(default_room_eq_export_path()),
        "roomeq.export.exists" => json!(default_room_eq_export_path().is_file()),
        "roomeq.export.bytes" => json!(room_eq_export_summary().and_then(|s| s.bytes)),
        "roomeq.export.channel_count" => {
            json!(room_eq_export_summary().and_then(|s| s.channel_count))
        }
        "roomeq.export.plugin_count" => {
            json!(room_eq_export_summary().and_then(|s| s.plugin_count))
        }
        "roomeq.export.filter_count" => {
            json!(room_eq_export_summary().and_then(|s| s.filter_count))
        }
        "roomeq.export.version" => json!(room_eq_export_summary().and_then(|s| s.version)),

        // Settings / preferences
        "settings.theme" => json!(format!("{:?}", app.ui_state.theme_id)),
        "settings.language" => json!(format!("{:?}", app.ui_state.language)),
        "settings.active_tab" => json!(format!("{:?}", app.ui_state.active_settings_tab)),
        "settings.font_scale" => json!(app.ui_state.font_scale),
        "settings.design_language" => {
            json!(app.ui_state.design_language.as_deref().unwrap_or("default"))
        }

        // Playback preferences
        "playback.replay_gain_enabled" => json!(app.playback.replay_gain_enabled),
        "playback.replay_gain_mode" => json!(format!("{:?}", app.playback.replay_gain_mode)),

        // Audio devices
        "audio.output_device" => json!(app.audio_device_state.current_output_device_name),
        "audio.output_device_count" => json!(app.audio_device_state.output_devices.len()),
        "audio.input_device_count" => json!(app.audio_device_state.input_devices.len()),

        other => return Err(anyhow!("unknown query path: `{other}`")),
    })
}

fn average_room_eq_score(
    results: &[crate::app::types::ChannelOptResult],
    score: impl Fn(&crate::app::types::ChannelOptResult) -> f64,
) -> Option<f64> {
    if results.is_empty() {
        None
    } else {
        Some(results.iter().map(score).sum::<f64>() / results.len() as f64)
    }
}

struct RoomEqExportSummary {
    bytes: Option<u64>,
    version: Option<String>,
    channel_count: Option<usize>,
    plugin_count: Option<usize>,
    filter_count: Option<usize>,
}

fn default_room_eq_export_path() -> std::path::PathBuf {
    sotf_audio_player::config::get_app_config_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("sotf-qa"))
        .join("qa-room-eq-export.json")
}

fn room_eq_export_summary() -> Option<RoomEqExportSummary> {
    let path = default_room_eq_export_path();
    let bytes = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let channel_count = json
        .get("channels")
        .and_then(|v| v.as_object())
        .map(|channels| channels.len());
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

    Some(RoomEqExportSummary {
        bytes,
        version: json
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        channel_count,
        plugin_count: Some(global_plugin_count + channel_plugin_count),
        filter_count: Some(filter_count),
    })
}
