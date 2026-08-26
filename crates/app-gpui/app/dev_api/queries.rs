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
        "playback.shuffle" => json!(app.ui_state.phone_shuffle_enabled),
        "playback.repeat" => json!(app.ui_state.phone_repeat_enabled),
        "playback.seekable" => json!(app.playback.duration_secs > 0.0),
        "spectrum.hold" => json!(app.ui_state.phone_spectrum_hold),
        "spectrum.smoothing" => json!(app.ui_state.phone_spectrum_smoothed),
        "spectrum.has_data" => json!(app.playback.spectrum_info.is_some()),
        "listening.guide_open" => json!(
            app.tutorial.listening_guide_open
                || !app
                    .plugin_state
                    .listening_test_state
                    .eq_progress
                    .how_to_listen_completed
        ),
        "listening.guide_completed" => json!(
            app.plugin_state
                .listening_test_state
                .eq_progress
                .how_to_listen_completed
        ),
        "listening.surface" => json!(format!(
            "{:?}",
            app.plugin_state.listening_test_state.surface
        )),
        "screen.focused" => json!(format!("{:?}", app.ui_state.current_screen)),
        "input_mode" => json!(format!("{:?}", app.ui_state.input_mode)),
        "onboarding.completed" => json!(app.tutorial.completed),
        "queue.length" => json!(app.queue_state.len()),
        "queue.current_index" => match app.playback.current_queue_index {
            Some(i) => json!(i),
            None => Value::Null,
        },
        "queue.first_title" => json!(app.queue_state.get(0).map(|item| item.album.title.as_str())),
        "queue.second_title" => json!(app.queue_state.get(1).map(|item| item.album.title.as_str())),
        "queue.can_undo_clear" => json!(app.queue_state.can_undo_clear()),
        "queue.can_undo_remove" => json!(app.queue_state.can_undo_remove()),
        "streams.count" => json!(app.stream_state.store.streams.len()),
        "streams.error" => json!(app.stream_state.last_error),
        "streams.status" => json!(app.stream_state.last_status),
        "streams.name" => json!(app.stream_state.name_input),
        "streams.url" => json!(app.stream_state.url_input),
        "streams.seekable" => json!(app.stream_state.seekable_input),
        "playlists.count" => json!(app.playlist.controller.playlists().len()),
        "playlists.first_name" => json!(
            app.playlist
                .controller
                .playlists()
                .first()
                .map(|playlist| playlist.name.as_str())
        ),
        "playlists.dialog" => json!(format!("{:?}", app.playlist.dialog)),
        "playlists.active_track_count" => json!(
            app.playlist
                .controller
                .active_playlist()
                .map(|playlist| playlist.entries.len())
                .unwrap_or(0)
        ),
        "playlists.undo_available" => json!(app.playlist.deleted_playlist.is_some()),
        "library.album_count" => json!(app.library_state.library.albums.len()),
        "home.favorite_expanded" => json!(app.ui_state.expanded_home_sections.contains("favorite")),
        "library.filtered_album_count" => json!(app.filtered_albums().len()),
        "library.search_query" => json!(app.library_state.search_query),
        "library.sort_order" => json!(format!("{:?}", app.library_state.sort_order)),
        "library.channel_filter" => json!(format!("{:?}", app.library_state.filter)),
        "library.track_count" => json!(
            app.library_state
                .library
                .albums
                .iter()
                .map(|album| album.tracks.len())
                .sum::<usize>()
        ),

        // Metadata editor
        "metadata.editor_open" => json!(app.modal.metadata_editor.is_some()),
        "metadata.target" => json!(
            app.modal
                .metadata_editor
                .as_ref()
                .map(|editor| editor.target_label.clone())
        ),
        "metadata.title" => json!(
            app.modal
                .metadata_editor
                .as_ref()
                .map(|editor| editor.fields.title.clone())
        ),
        "metadata.year" => json!(
            app.modal
                .metadata_editor
                .as_ref()
                .map(|editor| editor.fields.year.clone())
        ),
        "metadata.preview_files" => json!(
            app.modal
                .metadata_editor
                .as_ref()
                .and_then(|editor| editor.preview.as_ref())
                .map(|preview| preview.affected_files.len())
        ),
        "metadata.unsupported_count" => json!(
            app.modal
                .metadata_editor
                .as_ref()
                .and_then(|editor| editor.preview.as_ref())
                .map(|preview| preview.unsupported_writes.len())
        ),
        "metadata.candidate_count" => json!(
            app.modal
                .metadata_editor
                .as_ref()
                .map(|editor| editor.search_results.len())
        ),
        "recording.all_done" => json!(
            app.measurement_state
                .recording_state
                .all_channels_recorded()
        ),
        "recording.channel_count" => json!(
            app.measurement_state
                .recording_state
                .channel_recordings
                .len()
        ),
        "recording.done_count" => json!(
            app.measurement_state
                .recording_state
                .channel_recordings
                .iter()
                .filter(|recording| {
                    recording.state == crate::app::types::ChannelRecordingState::Done
                })
                .count()
        ),
        "recording.error_count" => json!(
            app.measurement_state
                .recording_state
                .channel_recordings
                .iter()
                .filter(|recording| {
                    recording.state == crate::app::types::ChannelRecordingState::Error
                })
                .count()
        ),
        "recording.status_severity" => json!(format!(
            "{:?}",
            app.measurement_state.recording_state.status_severity
        )),
        "recording.step" => json!(format!("{:?}", app.measurement_state.recording_state.step)),
        "recording.probe_complete" => json!(matches!(
            app.measurement_state.recording_state.probe_capture.status,
            crate::app::types::recording::ProbeCaptureStatus::Complete
        )),
        "recording.bass_anchor_complete" => json!(matches!(
            app.measurement_state
                .recording_state
                .bass_anchor_capture
                .status,
            crate::app::types::recording::BassAnchorCaptureStatus::Complete
        )),
        "recording.spl_ready" => json!(
            app.measurement_state
                .recording_state
                .spl_calibration_capture
                .is_ready()
        ),
        "headphone.catalog_count" => json!(
            app.measurement_state
                .headphone_eq_state
                .available_headphones
                .len()
        ),
        "headphone.suggestion_count" => json!(
            app.measurement_state
                .headphone_eq_state
                .headphone_suggestions
                .len()
        ),
        "headphone.selected" => json!(app.measurement_state.headphone_eq_state.selected_headphone),
        "headphone.error" => json!(app.measurement_state.headphone_eq_state.error_message),
        "headphone.easy_applied" => json!(
            app.measurement_state
                .headphone_eq_state
                .easy_mode_last_apply
                .is_some()
        ),
        "headphone.export_path" => {
            json!(app.measurement_state.headphone_eq_state.qa_last_export_path)
        }
        "headphone.export_exists" => json!(
            app.measurement_state
                .headphone_eq_state
                .qa_last_export_path
                .as_ref()
                .is_some_and(|path| path.is_file())
        ),
        "headphone.export_json_reloadable" => json!(
            app.measurement_state
                .headphone_eq_state
                .qa_last_export_path
                .as_ref()
                .is_some_and(|path| {
                    std::fs::read_to_string(path)
                        .ok()
                        .and_then(|content| {
                            serde_json::from_str::<Vec<math_audio_iir_fir::Biquad>>(&content).ok()
                        })
                        .is_some_and(|filters| !filters.is_empty())
                })
        ),
        "headphone.step" => json!(format!(
            "{:?}",
            app.measurement_state.headphone_eq_state.step
        )),
        "headphone.optimization_status" => json!(format!(
            "{:?}",
            app.measurement_state.headphone_eq_state.optimization_status
        )),
        "headphone.measurement_path" => {
            json!(app.measurement_state.headphone_eq_state.measurement_path)
        }
        "headphone.curve_point_count" => json!(
            app.measurement_state
                .headphone_eq_state
                .downloaded_curve
                .as_ref()
                .map_or(0, Vec::len)
        ),
        "headphone.loading" => json!(
            app.measurement_state.headphone_eq_state.loading_headphones
                || app.measurement_state.headphone_eq_state.loading_download
        ),
        "spinorama.catalog_count" => json!(
            app.measurement_state
                .spinorama_eq_state
                .available_speakers
                .len()
        ),
        "spinorama.suggestion_count" => json!(
            app.measurement_state
                .spinorama_eq_state
                .speaker_suggestions
                .len()
        ),
        "spinorama.selected_speaker" => {
            json!(app.measurement_state.spinorama_eq_state.selected_speaker)
        }
        "spinorama.selected_version" => {
            json!(app.measurement_state.spinorama_eq_state.selected_version)
        }
        "spinorama.selected_measurement" => json!(
            app.measurement_state
                .spinorama_eq_state
                .selected_measurement
        ),
        "spinorama.measurement_count" => json!(
            app.measurement_state
                .spinorama_eq_state
                .available_measurements
                .len()
        ),
        "spinorama.loading" => json!(
            app.measurement_state.spinorama_eq_state.loading_speakers
                || app.measurement_state.spinorama_eq_state.loading_versions
                || app
                    .measurement_state
                    .spinorama_eq_state
                    .loading_measurements
        ),
        "spinorama.error" => json!(app.measurement_state.spinorama_eq_state.error_message),
        "spinorama.step" => json!(format!(
            "{:?}",
            app.measurement_state.spinorama_eq_state.step
        )),
        "spinorama.optimization_status" => json!(format!(
            "{:?}",
            app.measurement_state.spinorama_eq_state.optimization_status
        )),
        "spinorama.result_count" => json!(
            app.measurement_state
                .spinorama_eq_state
                .result
                .as_ref()
                .map_or(0, |result| result.biquads.len())
        ),
        "roomeq.step" => json!(format!("{:?}", app.measurement_state.room_eq_state.step)),
        "roomeq.measurement_count" => json!(
            app.measurement_state
                .room_eq_state
                .channel_measurements
                .len()
        ),
        "roomeq.frequency_grid_consistent" => json!(
            app.measurement_state
                .room_eq_state
                .channel_measurements
                .first()
                .is_none_or(|first| {
                    app.measurement_state
                        .room_eq_state
                        .channel_measurements
                        .iter()
                        .all(|measurement| {
                            measurement.measurement.frequencies == first.measurement.frequencies
                        })
                })
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
        "roomeq.wizard_mode" => json!(format!(
            "{:?}",
            app.measurement_state.room_eq_state.wizard_mode
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
        "settings.remote_server_count" => json!(app.remote.server_store.servers.len()),
        "settings.remote_token_revealed" => json!(app.settings.show_manual_remote_token),
        "settings.remote_manual_token_configured" => {
            json!(!app.remote.manual_auth_token.trim().is_empty())
        }

        // Playback preferences
        "playback.replay_gain_enabled" => json!(app.playback.replay_gain_enabled),
        "playback.replay_gain_mode" => json!(format!("{:?}", app.playback.replay_gain_mode)),

        // Audio devices
        "audio.output_device" => json!(app.audio_device_state.current_output_device_name),
        "audio.output_device_count" => json!(app.audio_device_state.output_devices.len()),
        "audio.input_device_count" => json!(app.audio_device_state.input_devices.len()),

        // Plugin chain state
        "plugins.graph.selection_count" => json!(
            app.plugin_state
                .graph_state
                .graph_selection
                .selected_nodes
                .len()
        ),
        "plugins.graph.connection_count" => {
            json!(app.plugin_state.graph.connections.len())
        }
        "plugins.graph.connecting" => json!(
            app.plugin_state
                .graph_state
                .keyboard_connect_source
                .is_some()
        ),
        "plugins.user_count" => json!(
            app.plugin_state
                .graph
                .nodes
                .values()
                .filter(|node| !node.plugin.permanent)
                .count()
        ),
        "plugins.first_user_type" => json!(
            app.plugin_state
                .graph
                .nodes
                .values()
                .find(|node| !node.plugin.permanent)
                .map(|node| node.plugin.plugin_type().name())
        ),
        "plugins.first_user_enabled" => json!(
            app.plugin_state
                .graph
                .nodes
                .values()
                .find(|node| !node.plugin.permanent)
                .map(|node| node.plugin.enabled)
        ),
        "toast.message" => json!(
            app.ui_state
                .toast_message
                .as_ref()
                .map(|toast| toast.message.as_str())
        ),
        "toast.type" => json!(
            app.ui_state
                .toast_message
                .as_ref()
                .map(|toast| format!("{:?}", toast.toast_type))
        ),

        other if other.starts_with("plugins.") => {
            sotf_audio_player::controllers::plugin::dev_api::queries::plugin_query(
                &app.plugin_state.graph,
                other,
            )?
        }

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
