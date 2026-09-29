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

use super::performance;

pub fn resolve(path: &str, window: AnyWindowHandle, cx: &mut App) -> Result<Value> {
    window
        .update(cx, |any_view, window, cx| {
            let viewport = window.viewport_size();
            if path == "window.width" {
                return Ok(json!(f32::from(viewport.width)));
            }
            if path == "window.height" {
                return Ok(json!(f32::from(viewport.height)));
            }
            if path.starts_with("performance.") {
                return performance::resolve(path);
            }
            let entity = any_view
                .downcast::<PlayerView>()
                .map_err(|_| anyhow!("root view is not PlayerView"))?;
            let view = entity.read(cx);
            let state: &AppState = view.state.read(cx);
            read_path(path, state)
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?
}

fn maximum_true_peak_query(loudness: Option<&sotf_audio_player::LoudnessData>) -> Value {
    json!(loudness.and_then(|data| data.maximum_true_peak_dbtp))
}

fn maximum_momentary_lufs_query(loudness: Option<&sotf_audio_player::LoudnessData>) -> Value {
    json!(loudness.and_then(|data| {
        data.maximum_momentary_lufs
            .filter(|value| value.is_finite())
    }))
}

fn maximum_shortterm_lufs_query(loudness: Option<&sotf_audio_player::LoudnessData>) -> Value {
    json!(loudness.and_then(|data| {
        data.maximum_shortterm_lufs
            .filter(|value| value.is_finite())
    }))
}

fn read_path(path: &str, state: &AppState) -> Result<Value> {
    let app = &state.app;
    Ok(match path {
        "navigation.compact_open" => json!(app.ui_state.show_studio_menu),
        "navigation.compact_highlight" => json!(app.ui_state.navigation.compact_highlight),
        "navigation.studio_picker_open" => json!(app.ui_state.navigation.studio_picker_open),
        "playback.transport_diagnostics" => state.player.transport_diagnostics(),
        "playback.volume" => json!(app.playback.volume),
        "playback.is_playing" => json!(app.playback.is_playing),
        "playback.muted" => json!(app.playback.muted),
        "playback.position_secs" => json!(app.playback.position_secs),
        "playback.duration_secs" => json!(app.playback.display_duration_secs()),
        "playback.current_title" => json!(
            app.playback
                .current_queue_index
                .and_then(|queue_index| app.queue_state.get(queue_index))
                .map(|item| {
                    item.current_track()
                        .and_then(|track| track.title.clone())
                        .unwrap_or_else(|| item.album.title.clone())
                })
        ),
        "playback.current_track_index" => json!(
            app.playback
                .current_queue_index
                .and_then(|queue_index| app.queue_state.get(queue_index))
                .map(|item| item.current_track_index)
        ),
        "meters.has_data" => json!(app.playback.loudness_info.is_some()),
        "meters.integrated_control_requests" => json!(
            app.plugin_state
                .plugin_ui_state
                .loudness_control_requests
                .iter()
                .map(|(plugin_id, request)| json!({
                    "plugin_id": plugin_id,
                    "runtime_instance_id": request.runtime_instance_id,
                    "request_id": request.request_id,
                    "engine_index": request.engine_index,
                    "operation": request.operation,
                    "command": request.command,
                    "error": request.error,
                }))
                .collect::<Vec<_>>()
        ),
        "meters.maximum_true_peak_dbtp" => {
            maximum_true_peak_query(app.playback.loudness_info.as_deref())
        }
        "meters.maximum_momentary_lufs" => {
            maximum_momentary_lufs_query(app.playback.loudness_info.as_deref())
        }
        "meters.maximum_shortterm_lufs" => {
            maximum_shortterm_lufs_query(app.playback.loudness_info.as_deref())
        }
        "meters.channel_count" => json!(
            app.playback
                .loudness_info
                .as_ref()
                .map(|data| data.channel_peaks.len())
                .unwrap_or(0)
        ),
        "meters.group_count" => json!(app.level_meters.groups.len()),
        "meters.selected_group" => json!(app.level_meters.selected_group),
        "playback.shuffle" => json!(app.ui_state.phone_shuffle_enabled),
        "playback.repeat" => json!(app.ui_state.phone_repeat_enabled),
        "playback.seekable" => json!(
            app.playback.display_duration_secs().is_finite()
                && app.playback.display_duration_secs() > 0.0
        ),
        "spectrum.hold" => json!(app.ui_state.spectrum_view.hold),
        "spectrum.smoothing" => json!(app.ui_state.spectrum_view.smoothed),
        "spectrum.details" => json!(app.ui_state.spectrum_view.details_open),
        "spectrum.cursor_fraction" => json!(app.ui_state.spectrum_view.cursor_fraction),
        "spectrum.output_rate" => json!(if app.ui_state.spectrum_view.hold {
            app.ui_state
                .spectrum_view
                .held_frame
                .as_ref()
                .and_then(|frame| frame.sample_rate)
        } else {
            app.playback.spectrum_output_sample_rate()
        }),
        "spectrum.cursor_level" => {
            let spectrum = &app.ui_state.spectrum_view;
            let frame = if spectrum.hold {
                spectrum.held_frame.as_ref().map(|frame| &frame.data)
            } else {
                app.playback.spectrum_info.as_ref()
            };
            json!(frame.and_then(|frame| {
                let index = spectrum.inspected_band(frame.magnitudes.len())?;
                let values =
                    crate::app::state::ui::SpectrumViewState::magnitudes(frame, spectrum.smoothed);
                values.get(index).copied().filter(|value| value.is_finite())
            }))
        }
        "spectrum.has_data" => json!(app.playback.spectrum_info.is_some()),
        "library.detail.open" => json!(app.library_state.album_detail.is_some()),
        "library.detail.title" => json!(
            app.library_state
                .album_detail
                .as_ref()
                .map(|album| &album.title)
        ),
        "library.detail.track_count" => json!(
            app.library_state
                .album_detail
                .as_ref()
                .map_or(0, |album| album.tracks.len())
        ),
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
        "listening.break_open" => json!(app.tutorial.listening_break_prompt_open),
        "listening.break_interval" => json!(app.tutorial.listening_break_interval),
        "listening.eq.source.loading" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .source_request
                .is_some()
        ),
        "listening.eq.source.count" => {
            json!(app.plugin_state.listening_test_state.eq_sources.len())
        }
        "listening.eq.source.index" => json!(app.plugin_state.listening_test_state.eq_source_index),
        "listening.eq.source.filename" => json!(
            app.plugin_state
                .listening_test_state
                .eq_sources
                .get(app.plugin_state.listening_test_state.eq_source_index)
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy())
        ),
        "playback.current_filename" => json!(app.get_current_track_path().and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })),
        "playback.current_channels" => json!(
            app.playback
                .current_queue_index
                .and_then(|index| app.queue_state.get(index))
                .and_then(|item| item.current_track())
                .map(|track| track.channels)
        ),
        "listening.eq.config.bands" => {
            json!(app.plugin_state.listening_test_state.eq_config.band_count)
        }
        "listening.eq.config.gain" => {
            json!(app.plugin_state.listening_test_state.eq_config.gain_db)
        }
        "listening.eq.config.q" => json!(app.plugin_state.listening_test_state.eq_config.q),
        "listening.eq.config.trials" => {
            json!(app.plugin_state.listening_test_state.eq_config.trial_count)
        }
        "listening.eq.config.min-frequency" => json!(
            app.plugin_state
                .listening_test_state
                .eq_config
                .min_frequency_hz
        ),
        "listening.eq.config.max-frequency" => json!(
            app.plugin_state
                .listening_test_state
                .eq_config
                .max_frequency_hz
        ),
        "listening.eq.frequency_details_open" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .practice
                .frequency_details_open
        ),
        "listening.eq.session_gain" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .map(|session| session.config.gain_db)
        ),
        "listening.eq.session_q" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .map(|session| session.config.q)
        ),
        "listening.eq.session_trials" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .map(|session| session.config.trial_count)
        ),
        "listening.eq.session_min_frequency" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .map(|session| session.config.min_frequency_hz)
        ),
        "listening.eq.session_max_frequency" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .map(|session| session.config.max_frequency_hz)
        ),
        "listening.eq.paused" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .practice
                .paused
        ),
        "listening.eq.confirm_end" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .practice
                .confirm_end
        ),
        "listening.eq.ended_early" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .is_some_and(|session| session.ended_early)
        ),
        "listening.eq.submitted" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .map_or(0, |session| session.trials.len())
        ),
        "listening.eq.has_question" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .is_some_and(|session| session.current_question.is_some())
        ),
        "listening.eq.has_session" => {
            json!(app.plugin_state.listening_test_state.eq_session.is_some())
        }
        "listening.eq.answered" => json!(
            app.plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .is_some_and(sotf_audio_player::EqTrainingSession::current_is_answered)
        ),
        "listening.eq.selected_answer" => {
            json!(app.plugin_state.listening_test_state.eq_selected_band)
        }
        "listening.eq.filtered" => json!(app.plugin_state.listening_test_state.eq_filtered),
        "listening.eq.progress_sessions" => json!(
            app.plugin_state
                .listening_test_state
                .eq_progress
                .sessions
                .len()
        ),
        "listening.paths_ready" => json!(
            app.plugin_state.listening_test_state.path_a.is_some()
                && app.plugin_state.listening_test_state.path_b.is_some()
        ),
        "listening.metadata_open" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .metadata_open
        ),
        "listening.trials_open" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .trials_open
        ),
        "listening.confirm_end" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .confirm_end
        ),
        "listening.results_open" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .results_open
        ),
        "listening.session_exists" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .session()
                .is_some()
        ),
        "listening.session_ready" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .session()
                .is_some_and(|session| session.setup.level_match.within_tolerance())
        ),
        "listening.pending_mode" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .session()
                .and_then(|session| session.pending_mode())
                .map(|mode| format!("{mode:?}"))
        ),
        "listening.completed_trials" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .view()
                .completed_trials
        ),
        "listening.planned_trials" => json!(
            app.plugin_state
                .plugin_ui_state
                .listening_workspace
                .planned_trials
        ),
        "listening.abx_correct" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .view()
                .abx_score
                .0
        ),
        "listening.abx_total" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .view()
                .abx_score
                .1
        ),
        "listening.runtime_active" => json!(
            app.plugin_state
                .listening_test_state
                .ab_test
                .view()
                .runtime_active
        ),
        "listening.status" => json!(app.plugin_state.listening_test_state.status),
        "listening.session_file_exists" => json!(
            std::env::var_os("SOTF_QA_DIR")
                .map(std::path::PathBuf::from)
                .map(|path| path.join("listening-session.json").is_file())
                .unwrap_or(false)
        ),
        "screen.focused" => json!(format!("{:?}", app.ui_state.current_screen)),
        "input_mode" => json!(format!("{:?}", app.ui_state.input_mode)),
        "onboarding.completed" => json!(app.tutorial.completed),
        "queue.length" => json!(app.queue_state.len()),
        "queue.upcoming_count" => json!(app.queue_state.upcoming_track_positions().len()),
        "queue.upcoming_titles" => json!(
            app.queue_state
                .upcoming_track_positions()
                .iter()
                .filter_map(|position| {
                    app.queue_state
                        .get(position.item)?
                        .album
                        .tracks
                        .get(position.track)
                        .map(|track| track.title.clone().unwrap_or_default())
                })
                .collect::<Vec<_>>()
        ),
        "queue.current_index" => match app.playback.current_queue_index {
            Some(i) => json!(i),
            None => Value::Null,
        },
        "queue.first_title" => json!(
            app.queue_state
                .first()
                .map(|item| item.album.title.as_str())
        ),
        "queue.second_title" => json!(app.queue_state.get(1).map(|item| item.album.title.as_str())),
        "queue.can_undo_clear" => json!(app.queue_state.can_undo_clear()),
        "queue.can_undo_remove" => json!(app.queue_state.can_undo_remove()),
        "streams.count" => json!(app.stream_state.store.streams.len()),
        "now_playing.details_open" => json!(app.playback.track_information_open),
        "now_playing.signal_open" => json!(app.playback.signal_path_open),
        "now_playing.signal_ready" => json!(
            app.playback
                .signal_path
                .as_ref()
                .is_some_and(|path| path.source.is_some())
        ),
        "now_playing.signal_plugins" => json!(app.playback.signal_path.as_ref().map(|path| {
            path.plugin_chain
                .iter()
                .map(|plugin| &plugin.plugin_type)
                .collect::<Vec<_>>()
        })),
        "home.recent_count" => json!(app.library_state.library.recently_played_albums().len()),
        "home.recent_first_title" => json!(
            app.library_state
                .library
                .recently_played_albums()
                .first()
                .map(|album| &album.title)
        ),
        "home.recent_titles" => json!(
            app.library_state
                .library
                .recently_played_albums()
                .iter()
                .map(|album| &album.title)
                .collect::<Vec<_>>()
        ),
        "home.favorite_count" => json!(
            app.library_state
                .library
                .albums
                .iter()
                .filter(|album| album.is_favorite)
                .count()
        ),
        "streams.editor_open" => json!(app.stream_state.editor_open),
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
        "playlists.names" => json!(
            app.playlist
                .controller
                .playlists()
                .iter()
                .map(|playlist| playlist.name.as_str())
                .collect::<Vec<_>>()
        ),
        "playlists.dialog" => json!(format!("{:?}", app.playlist.dialog)),
        "playlists.active_name" => json!(
            app.playlist
                .controller
                .active_playlist()
                .map(|playlist| playlist.name.as_str())
        ),
        "playlists.active_track_count" => json!(
            app.playlist
                .controller
                .active_playlist()
                .map(|playlist| playlist.entries.len())
                .unwrap_or(0)
        ),
        "playlists.active_first_track" => json!(
            app.playlist
                .controller
                .active_playlist()
                .and_then(|playlist| playlist.entries.first())
                .and_then(|entry| entry.track_path.file_name())
                .and_then(|name| name.to_str())
        ),
        "playlists.active_second_track" => json!(
            app.playlist
                .controller
                .active_playlist()
                .and_then(|playlist| playlist.entries.get(1))
                .and_then(|entry| entry.track_path.file_name())
                .and_then(|name| name.to_str())
        ),
        "playlists.error" => json!(app.playlist.error),
        "playlists.undo_available" => json!(app.playlist.deleted_playlist.is_some()),
        "library.album_count" => json!(app.library_state.library.albums.len()),
        "home.favorite_expanded" => json!(app.ui_state.expanded_home_sections.contains("favorite")),
        "library.filtered_album_count" => json!(app.filtered_albums().len()),
        "library.filtered_track_count" => {
            json!(app.library_state.selection_filtered_tracks().len())
        }
        "library.search_query" => json!(app.library_state.search_query),
        "library.list_view" => json!(app.library_state.album_list_view),
        "library.result_order" => json!(format!(
            "{:?}",
            app.library_state.result_order.unwrap_or_default()
        )),
        "library.sort_menu_open" => json!(app.library_state.sort_menu_open),
        "library.sort_highlight" => json!(app.library_state.sort_highlighted_index),
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
        "recording.save_status" => json!(format!(
            "{:?}",
            app.measurement_state.recording_state.save_status()
        )),
        "recording.room_width_m" => {
            let rec = &app.measurement_state.recording_state;
            json!(rec.room_dimension_unit.to_meters(rec.room_width_input))
        }
        "recording.room_unit" => json!(format!(
            "{:?}",
            app.measurement_state.recording_state.room_dimension_unit
        )),
        "recording.metadata_valid" => json!(
            app.measurement_state
                .recording_state
                .session_metadata_is_valid()
        ),
        "recording.setup_description" => {
            json!(app.measurement_state.recording_state.setup_description)
        }
        "recording.save_name" => json!(app.measurement_state.recording_state.save_name),
        "recording.saved_path" => json!(app.measurement_state.recording_state.take_review.saved_to),
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
        "headphone.autoeq_stage" => json!(format!(
            "{:?}",
            app.measurement_state.headphone_eq_state.autoeq_stage
        )),
        "headphone.autoeq_detail" => json!(format!(
            "{:?}",
            app.measurement_state.headphone_eq_state.detail_level
        )),
        "headphone.audition_active" => {
            json!(app.measurement_state.headphone_eq_state.audition.is_some())
        }
        "headphone.audition_corrected" => json!(
            app.measurement_state
                .headphone_eq_state
                .audition
                .as_ref()
                .is_some_and(|audition| audition.corrected)
        ),
        "headphone.audition_pending" => json!(
            app.measurement_state
                .headphone_eq_state
                .audition
                .as_ref()
                .is_some_and(|audition| audition.is_pending())
        ),
        "headphone.audition_preamp_db" => json!(
            app.measurement_state
                .headphone_eq_state
                .audition
                .as_ref()
                .map(|audition| audition.preamp_db)
        ),
        "headphone.error" => json!(app.measurement_state.headphone_eq_state.error_message),
        "roomeq.application_status" => {
            let correction = &app.measurement_state.room_eq_state;
            json!(format!(
                "{:?}",
                app.correction_application_status(
                    &correction.delivery,
                    correction.result_is_current()
                )
            ))
        }
        "spinorama.export_current" => {
            let speaker = &app.measurement_state.spinorama_eq_state;
            json!(
                speaker
                    .delivery
                    .current_result_exported(speaker.result_is_current())
            )
        }
        "spinorama.export_filter_summary" => {
            let speaker = &app.measurement_state.spinorama_eq_state;
            speaker
                .delivery
                .last_export()
                .and_then(|export| std::fs::read_to_string(&export.path).ok())
                .and_then(|content| serde_json::from_str::<Vec<serde_json::Value>>(&content).ok())
                .map(|filters| {
                    json!(
                        filters
                            .iter()
                            .map(|filter| format!(
                                "{}@{}",
                                filter["filter_type"].as_str().unwrap_or("unknown"),
                                filter["srate"].as_f64().unwrap_or(0.0)
                            ))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .unwrap_or(serde_json::Value::Null)
        }
        "spinorama.application_status" => {
            let correction = &app.measurement_state.spinorama_eq_state;
            json!(format!(
                "{:?}",
                app.correction_application_status(
                    &correction.delivery,
                    correction.result_is_current()
                )
            ))
        }
        "headphone.application_status" => {
            let headphone = &app.measurement_state.headphone_eq_state;
            json!(format!(
                "{:?}",
                app.correction_application_status(
                    &headphone.delivery,
                    headphone.result_is_current()
                )
            ))
        }
        "headphone.easy_applied" => {
            let headphone = &app.measurement_state.headphone_eq_state;
            json!(headphone.easy_mode_last_apply.is_some() && app.correction_application_status(&headphone.delivery, headphone.result_is_current())
                == sotf_audio_player::ui_models::correction_delivery::CorrectionApplicationStatus::Applied)
        }
        "headphone.export_current" => {
            let headphone = &app.measurement_state.headphone_eq_state;
            json!(
                headphone
                    .delivery
                    .current_result_exported(headphone.result_is_current())
            )
        }
        "headphone.result_revision" => {
            json!(app.measurement_state.headphone_eq_state.delivery.revision())
        }
        "headphone.export_revision" => json!(
            app.measurement_state
                .headphone_eq_state
                .delivery
                .last_export()
                .map(|export| export.revision)
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
        "headphone.identity_expanded" => json!(
            app.measurement_state
                .headphone_eq_state
                .expanded_sections
                .iter()
                .any(|section| section == "measurement-identity")
        ),
        "headphone.target_preset" => json!(app.measurement_state.headphone_eq_state.target_preset),
        "headphone.target_open" => json!(
            app.measurement_state
                .headphone_eq_state
                .dropdowns
                .target_open
        ),
        "headphone.can_advance" => json!(app.can_advance_workflow_step()),
        "headphone.identity.model"
        | "headphone.identity.rig"
        | "headphone.identity.sample"
        | "headphone.identity.compensation" => {
            let preview = app
                .measurement_state
                .headphone_eq_state
                .active_file_preview();
            let key = path.rsplit('.').next().unwrap_or_default();
            json!(preview.and_then(|value| value.identity_field(key)))
        }
        "headphone.file_preview_point_count" => json!(
            app.measurement_state
                .headphone_eq_state
                .active_file_preview()
                .map_or(0, |preview| preview.points().len())
        ),
        "headphone.measurement_bounds" => json!(
            app.measurement_state
                .headphone_eq_state
                .measurement_frequency_bounds()
        ),
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
        "roomeq.config.filter_count" => json!(
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .num_filters
        ),
        "roomeq.config.opt_mode" => json!(
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .mode
                .to_code()
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
        "roomeq.has_multi_driver" => json!(app.measurement_state.room_eq_state.has_multi_driver()),
        path if path.starts_with("roomeq.driver_count.") => json!(
            app.measurement_state
                .room_eq_state
                .channel_measurements
                .iter()
                .find(|channel| Some(channel.channel_name.as_str())
                    == path.strip_prefix("roomeq.driver_count."))
                .map(|channel| channel.driver_measurement_sets.len())
        ),
        path if path.starts_with("roomeq.position_count.") => {
            json!(
                app.measurement_state
                    .room_eq_state
                    .multi_position_counts
                    .iter()
                    .find(|(name, _)| Some(name.as_str())
                        == path.strip_prefix("roomeq.position_count."))
                    .map(|(_, count)| count)
            )
        }
        "roomeq.driver_counts" => json!(
            app.measurement_state
                .room_eq_state
                .channel_measurements
                .iter()
                .map(|channel| (&channel.channel_name, channel.driver_measurement_sets.len()))
                .collect::<std::collections::BTreeMap<_, _>>()
        ),
        "roomeq.position_counts" => json!(
            app.measurement_state
                .room_eq_state
                .multi_position_counts
                .iter()
                .map(|(name, count)| (name, count))
                .collect::<std::collections::BTreeMap<_, _>>()
        ),
        "roomeq.multi_measurement_enabled" => json!(
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .multi_measurement
                .enabled
        ),
        path if path.starts_with("roomeq.crossover_freq.") => {
            let suffix = path.strip_prefix("roomeq.crossover_freq.").unwrap_or("");
            let mut parts = suffix.split('.');
            let value = match (parts.next(), parts.next(), parts.next()) {
                (Some(channel), Some(index), None) => channel
                    .parse::<usize>()
                    .ok()
                    .zip(index.parse::<usize>().ok())
                    .and_then(|(channel, index)| {
                        app.measurement_state
                            .room_eq_state
                            .speaker_configs
                            .get(channel)
                            .and_then(|config| config.crossover_freq_hints.get(index).copied())
                    }),
                _ => None,
            };
            json!(value)
        }
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
        "preferences.close_pending" => json!(app.settings.navigation.close_pending),
        "preferences.category_open" => json!(app.settings.navigation.category_open),
        "preferences.maintenance_busy" => json!(app.settings.library.maintenance.busy),
        "preferences.maintenance_ready" => json!(app.settings.library.maintenance.review.is_some()),
        "preferences.maintenance_count" => json!(
            app.settings
                .library
                .maintenance
                .review
                .as_ref()
                .map(|review| review.entries().len())
        ),
        "preferences.maintenance_error" => json!(app.settings.library.maintenance.error),
        "preferences.maintenance_removed" => json!(app.settings.library.maintenance.removed),
        "preferences.setting" => json!(
            app.settings
                .navigation
                .setting
                .map(|setting| format!("{setting:?}"))
        ),
        "preferences.reveal_pending" => json!(app.settings.navigation.reveal_setting),
        "preferences.analysis_expanded" => json!(
            app.settings
                .expanded_sections
                .iter()
                .any(|id| id == "library-analysis")
        ),
        "preferences.sample_rate_highlight" => {
            json!(app.audio_device_state.hal_dropdowns.highlights[0])
        }
        "preferences.systemwide_input" => {
            json!(app.audio_device_state.output_draft.systemwide_input)
        }
        "preferences.sample_rate_hz" => json!(app.audio_device_state.output_draft.sample_rate_hz),
        "preferences.channel_count" => json!(app.audio_device_state.output_draft.channel_count),
        "preferences.buffer_frames" => json!(app.audio_device_state.output_draft.buffer_frames),
        "settings.playback_source" => {
            json!(format!("{:?}", app.audio_device_state.playback_source))
        }
        "settings.sample_rate_hz" => json!(app.audio_device_state.hal_config.sample_rate),
        "settings.channel_count" => json!(app.audio_device_state.hal_config.channel_count),
        "settings.buffer_frames" => json!(app.audio_device_state.hal_config.buffer_frames),
        "preferences.audio_dirty" => json!(app.audio_device_state.output_draft.is_dirty()),
        "preferences.audio_applying" => json!(app.audio_device_state.audio_apply.pending.is_some()),
        "preferences.output_dropdown_open" => json!(app.audio_device_state.output_ui.open),
        "preferences.device_details_expanded" => {
            json!(app.audio_device_state.output_ui.details_expanded)
        }
        "preferences.replay_gain_enabled" => {
            json!(app.audio_device_state.output_draft.replay_gain_enabled)
        }
        "preferences.replay_gain_mode" => json!(
            app.audio_device_state
                .output_draft
                .replay_gain_mode
                .map(|mode| format!("{mode:?}"))
        ),
        "settings.replay_gain_enabled" => json!(app.playback.replay_gain_enabled),
        "settings.replay_gain_mode" => json!(format!("{:?}", app.playback.replay_gain_mode)),
        "preferences.output_default_draft" => {
            json!(app.audio_device_state.output_draft.system_default)
        }
        "audio.follow_system_default" => json!(app.audio_device_state.follow_system_default),
        "preferences.output_draft" => json!(app.audio_device_state.output_draft.device_name),
        "preferences.output_error" => json!(app.audio_device_state.output_draft.error),
        "settings.theme" => json!(format!("{:?}", app.ui_state.theme_id)),
        "settings.theme_mode" => json!(match app.ui_state.theme_mode_preference {
            gpui_themes::ThemeModePreference::FollowSystem => "system",
            gpui_themes::ThemeModePreference::Light => "light",
            gpui_themes::ThemeModePreference::Dark => "dark",
            gpui_themes::ThemeModePreference::Scheduled { .. } => "scheduled",
        }),
        "settings.plugin_scan_in_progress" => {
            json!(app.plugin_state.external_plugin_ui.scan_in_progress)
        }
        "settings.plugin_scan_completed" => {
            json!(app.plugin_state.external_plugin_ui.scan_completed)
        }
        "settings.language" => json!(format!("{:?}", app.ui_state.language)),
        "settings.active_tab" => json!(format!("{:?}", app.ui_state.active_settings_tab)),
        "settings.font_scale" => json!(app.ui_state.font_scale),
        "settings.scanner_threads" => json!(app.ui_state.scanner_threads),
        "settings.max_cpu_cores" => json!(app.ui_state.max_cpu_cores),
        "settings.reduce_motion" => json!(app.ui_state.reduce_motion),
        "settings.accessibility_palette" => {
            json!(format!("{:?}", app.ui_state.accessibility_palette))
        }
        "settings.keymap_preset" => json!(format!("{:?}", app.ui_state.keymap_preset)),
        "settings.custom_keybinding_count" => {
            json!(app.settings.keybindings.overrides.len())
        }
        "settings.keybinding_pending_key" => {
            json!(app.settings.keybindings.pending_key_spec.as_deref())
        }
        "settings.keybinding_conflict_action" => json!(
            app.settings
                .keybindings
                .conflict
                .as_ref()
                .map(|conflict| conflict.existing_action_name.as_str())
        ),
        "settings.release_channel" => json!(format!("{:?}", app.ui_state.release_channel)),
        key if key.starts_with("settings.persisted_audio.") => crate::Config::load()
            .ok()
            .and_then(|config| serde_json::to_value(config.audio).ok())
            .and_then(|audio| {
                audio
                    .get(&key["settings.persisted_audio.".len()..])
                    .cloned()
            })
            .unwrap_or(serde_json::Value::Null),
        "settings.persisted_release_channel" => json!(
            crate::Config::load()
                .map(|config| format!("{:?}", config.release_channel))
                .ok()
        ),
        "settings.metadata_search_enabled" => json!(
            sotf_audio_player::config::load_metadata_services_config()
                .ok()
                .and_then(|config| config.providers.first().map(|provider| provider.enabled))
        ),
        "settings.metadata_error" => json!(app.settings.metadata_error),
        "settings.metadata_busy" => json!(app.settings.metadata_loading),
        "settings.metadata_cached_enabled" => json!(
            app.settings
                .metadata_config
                .as_ref()
                .and_then(|config| config.providers.first().map(|provider| provider.enabled))
        ),
        "settings.library_folder_count" => json!(app.library_state.library.directories.len()),
        "settings.persisted_library_folder_count" => json!(
            crate::app::config::Config::load()
                .ok()
                .map(|config| config.directories.len())
        ),
        "settings.library_folder_error" => json!(
            app.settings
                .library
                .directory_error
                .as_ref()
                .map(|error| match error {
                    sotf_audio_player::LibraryDirectoryAccessError::NotFound(_) => "NotFound",
                    sotf_audio_player::LibraryDirectoryAccessError::NotDirectory(_) => {
                        "NotDirectory"
                    }
                    sotf_audio_player::LibraryDirectoryAccessError::PermissionDenied(_) => {
                        "PermissionDenied"
                    }
                    sotf_audio_player::LibraryDirectoryAccessError::Unreadable { .. } => {
                        "Unreadable"
                    }
                })
        ),
        "settings.library_remove_pending" => {
            json!(app.settings.library.pending_remove_index)
        }
        "settings.library_scan_in_progress" => json!(app.library_state.scan_in_progress),
        "settings.library_scan_tracks" => json!(app.library_state.scan_progress_tracks),
        "settings.library_scan_albums" => json!(app.library_state.scan_progress_albums),
        "settings.library_scan_error" => json!(app.settings.library.scan_error),
        "settings.federation_source_count" => json!(app.federation.sources.len()),
        "settings.federation_tidal_login_active" => {
            #[cfg(feature = "tidal")]
            {
                json!(app.federation.tidal_login.is_some())
            }
            #[cfg(not(feature = "tidal"))]
            {
                json!(false)
            }
        }
        "settings.federation_tidal_logged_in" => {
            json!(app.federation.sources.iter().any(|source| matches!(
                &source.connection,
                sotf_audio_player::federation_config::SourceConnectionConfig::Tidal {
                    access_token,
                    ..
                } if !access_token.trim().is_empty()
            )))
        }
        "settings.federation_tidal_persisted_logged_in" => json!(
            app.library_state
                .library
                .get_database()
                .and_then(|database| database.load_federation_sources().ok())
                .is_some_and(|sources| sources.iter().any(|source| matches!(
                    &source.connection,
                    sotf_audio_player::federation_config::SourceConnectionConfig::Tidal {
                        access_token,
                        ..
                    } if !access_token.trim().is_empty()
                )))
        ),
        "settings.federation_login_error" => json!(app.federation.service_login_error),
        "settings.design_language" => {
            json!(app.ui_state.design_language.as_deref().unwrap_or("default"))
        }
        "settings.remote_server_count" => json!(app.remote.server_store.servers.len()),
        "settings.remote_manual_name" => json!(app.remote.manual_server_name),
        "settings.sotf_api_enabled" => json!(app.federation.server_config.api.enabled),
        "settings.mpd_password_revealed" => json!(app.settings.show_mpd_password),
        "settings.mpd_password_configured" => {
            json!(app.federation.server_config.mpd.password.is_some())
        }
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
        "routing.draft_connection_count" => json!(
            app.plugin_state
                .routing_controller()
                .graph
                .connections
                .len()
        ),
        "plugins.update_pending" => json!(
            app.plugin_state.update_state.pending_ack.is_some()
                || app
                    .plugin_state
                    .update_state
                    .pending_plugin_update
                    .is_some()
        ),
        "routing.applying" => json!(app.plugin_state.graph_state.applying_original.is_some()),
        "routing.header_details_open" => json!(app.plugin_state.graph_state.header_details_open),
        "routing.dirty" => json!(app.plugin_state.routing_draft_is_dirty()),
        "routing.inspector_close_pending" => {
            json!(app.plugin_state.graph_state.confirm_close_dirty)
        }
        "routing.active_matches_base" => json!(
            app.plugin_state.graph_state.draft_base
                == serde_json::to_value(&app.plugin_state.graph).ok()
        ),
        "routing.form_open" => json!(app.plugin_state.graph_state.connection_form.open),
        "routing.endpoints" => json!(app.plugin_state.graph_state.connection_form.endpoints),
        "plugins.graph.connecting" => json!(
            app.plugin_state
                .graph_state
                .keyboard_connect_source
                .is_some()
        ),
        "headphone.applied_filters" => json!(
            app.plugin_state
                .graph
                .nodes
                .values()
                .filter_map(|node| match &node.plugin.settings {
                    sotf_audio_player::PluginSettings::EQ { filters, .. }
                        if !node.plugin.permanent =>
                        Some(filters),
                    _ => None,
                })
                .flatten()
                .map(|filter| format!(
                    "{:?}:{}:{}:{}",
                    filter.filter_type, filter.frequency, filter.q, filter.gain_db
                ))
                .collect::<Vec<_>>()
                .join(";")
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

        other if other.starts_with("meters.peak.") => {
            let channel = other
                .trim_start_matches("meters.peak.")
                .parse::<usize>()
                .map_err(|_| anyhow!("invalid meter peak query path: `{other}`"))?;
            json!(
                app.playback
                    .loudness_info
                    .as_ref()
                    .and_then(|data| data.channel_peaks.get(channel).copied())
            )
        }
        other if other.starts_with("meters.group.") => {
            let parts: Vec<_> = other.split('.').collect();
            if parts.len() != 4 {
                return Err(anyhow!("invalid meter group query path: `{other}`"));
            }
            let group_idx = parts[2]
                .parse::<usize>()
                .map_err(|_| anyhow!("invalid meter group query path: `{other}`"))?;
            let group = app
                .level_meters
                .groups
                .get(group_idx)
                .ok_or_else(|| anyhow!("meter group {group_idx} does not exist"))?;
            match parts[3] {
                "muted" => json!(group.muted),
                "soloed" => json!(group.soloed),
                "dimmed" => json!(group.dimmed),
                state => return Err(anyhow!("unknown meter group state: `{state}`")),
            }
        }
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
