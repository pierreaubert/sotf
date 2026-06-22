//! Allow-listed property queries for the dev API.
//!
//! The match here is the entire surface of `/query`. Adding a new
//! property is two lines (one match arm + a comment). We deliberately
//! avoid reflective JSON serialisation of internal state — scripts
//! should depend on a small, stable subset.

use crate::app::App;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};

pub fn resolve(path: &str, app: &App) -> Result<Value> {
    read_path(path, app)
}

fn read_path(path: &str, app: &App) -> Result<Value> {
    Ok(match path {
        // Playback
        "playback.volume" => json!(app.playback.volume),
        "playback.is_playing" => json!(app.playback.is_playing),
        "playback.muted" => json!(app.playback.muted),

        // Screen / navigation
        "screen.focused" => json!(format!("{:?}", app.current_screen)),
        "input_mode" => json!(format!("{:?}", app.input_mode)),
        "configure.sub_screen" => json!(format!("{:?}", app.configure_sub_screen)),

        // Queue
        "queue.length" => json!(app.queue.len()),
        "queue.current_index" => match app.playback.current_queue_index {
            Some(i) => json!(i),
            None => Value::Null,
        },

        // Library
        "library.directory_count" => json!(app.library.directories.len()),
        "library.album_count" => json!(app.library.albums.len()),
        "library.track_count" => json!(
            app.library
                .albums
                .iter()
                .map(|a| a.tracks.len())
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

        // Recording
        "recording.step" => json!(format!("{:?}", app.recording.model.step)),
        "recording.all_done" => {
            json!(
                app.recording
                    .model
                    .channel_recordings
                    .iter()
                    .all(|c| c.state
                        == sotf_audio_player::recording_types::ChannelRecordingState::Done)
            )
        }
        "recording.done_count" => json!(
            app.recording
                .model
                .channel_recordings
                .iter()
                .filter(
                    |c| c.state == sotf_audio_player::recording_types::ChannelRecordingState::Done
                )
                .count()
        ),
        "recording.channel_count" => json!(app.recording.model.channel_recordings.len()),
        "recording.status" => json!(app.recording.model.status_message),

        // Room EQ
        "roomeq.step" => json!(format!("{:?}", app.room_eq.model.step)),
        "roomeq.measurement_count" => json!(app.room_eq.model.channel_measurements.len()),
        "roomeq.speaker_config_count" => json!(app.room_eq.model.channel_measurements.len()),
        "roomeq.optimization_status" => {
            json!(format!("{:?}", app.room_eq.model.optimization_status))
        }
        "roomeq.result_count" => json!(app.room_eq.model.channel_results.len()),
        "roomeq.has_dsp_output" => json!(app.room_eq.model.dsp_output.is_some()),
        "roomeq.dsp_channel_count" => {
            json!(
                app.room_eq
                    .model
                    .dsp_output
                    .as_ref()
                    .map(|d| d.channels.len())
            )
        }
        "roomeq.filter_count" => json!(
            app.room_eq
                .model
                .channel_results
                .iter()
                .map(|r| r.eq_filters.len())
                .sum::<usize>()
        ),
        "roomeq.average_pre_score" => {
            json!(average_room_eq_score(
                &app.room_eq.model.channel_results,
                |r| r.pre_score
            ))
        }
        "roomeq.average_post_score" => {
            json!(average_room_eq_score(
                &app.room_eq.model.channel_results,
                |r| r.post_score
            ))
        }
        "roomeq.status" => json!(app.room_eq.model.status_message.as_str()),
        "roomeq.error" => json!(app.room_eq.model.error_message.as_deref().unwrap_or("")),

        // Headphone EQ
        "headphoneeq.step" => json!(format!("{:?}", app.headphone_eq.step)),

        // Spinorama EQ
        "spinorama.step" => json!(format!("{:?}", app.spinorama_eq.step)),

        // Settings
        "settings.theme" => json!("dark"),

        // Audio devices
        "audio.output_device" => json!(app.audio_devices.current_output_name),
        "audio.output_device_count" => json!(app.audio_devices.outputs.len()),

        // Plugins
        "plugins.count" => json!(app.plugin_rack.graph.plugin_count()),

        // Playlists
        "playlists.count" => json!(app.playlists.controller.playlists().len()),

        // Level meters
        "level_meters.channel_count" => json!(
            app.level_meters
                .groups
                .iter()
                .map(|g| g.channels.len())
                .sum::<usize>()
        ),

        // Cast
        "cast.device_count" => json!(app.audio_devices.cast.len()),

        other => return Err(anyhow!("unknown query path: `{other}`")),
    })
}

fn average_room_eq_score(
    results: &[sotf_audio_player::room_eq_types::ChannelOptResult],
    score: impl Fn(&sotf_audio_player::room_eq_types::ChannelOptResult) -> f64,
) -> Option<f64> {
    if results.is_empty() {
        None
    } else {
        Some(results.iter().map(score).sum::<f64>() / results.len() as f64)
    }
}
