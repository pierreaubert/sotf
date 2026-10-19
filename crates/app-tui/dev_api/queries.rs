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
        "playback.has_current_track" => json!(app.current_track_path().is_some()),

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
        "library.visible_album_count" => json!(app.library_view.cached_filtered_albums.len()),
        "library.selected_album_index" => json!(app.library_view.selected_album_index),

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
        "recording.step_tab_focused" => json!(app.recording.step_tab_focused),
        "recording.editing_path" => json!(
            app.recording.editing_output_dir || app.recording.editing_mic_cal_channel.is_some()
        ),
        "recording.focus_context" => json!(if app.recording.editing_output_dir
            || app.recording.editing_mic_cal_channel.is_some()
        {
            "PathEdit"
        } else if app.recording.step_tab_focused {
            "StepTabs"
        } else {
            "Content"
        }),
        "recording.selected_field" => json!(app.recording.selected_field),
        "recording.signal_duration_secs" => json!(app.recording.model.signal_duration_secs),
        "recording.bass_octave_duration_s" => {
            json!(app.recording.model.bass_octave_duration_s)
        }
        "recording.duration_control_kind" => json!(if app.recording.model.signal_type
            == sotf_audio_player::recording_types::RecordingSignalType::Sweep
        {
            "BassOctaveDuration"
        } else if app.recording.model.signal_type
            == sotf_audio_player::recording_types::RecordingSignalType::Mls
        {
            "SequenceDefined"
        } else {
            "SignalDuration"
        }),
        "recording.signal_type" => json!(app.recording.model.signal_type.as_str()),
        "recording.effective_acquisition" => {
            json!(crate::app::recording_acquisition_config(&app.recording.model).ok())
        }
        "recording.effective_playback_duration_secs" => json!(
            crate::app::recording_acquisition_config(&app.recording.model)
                .ok()
                .map(|config| config.playback_duration_secs)
        ),
        "recording.duration_control_applies" => json!(
            crate::app::recording_acquisition_config(&app.recording.model)
                .ok()
                .map(|config| config.duration_control_applies)
        ),
        "recording.stimulus_configs" => {
            let speaker_names = app
                .recording
                .model
                .playback_config
                .channel_mappings
                .iter()
                .map(|mapping| mapping.group_name.clone())
                .collect::<Vec<_>>();
            json!(
                sotf_audio_player::recording_helpers::recording_stimulus_entries_from_sources(
                    &app.recording.model.channel_recordings,
                    &speaker_names
                )
            )
        }
        "recording.stimulus_config_count" => {
            let speaker_names = app
                .recording
                .model
                .playback_config
                .channel_mappings
                .iter()
                .map(|mapping| mapping.group_name.clone())
                .collect::<Vec<_>>();
            json!(
                sotf_audio_player::recording_helpers::recording_stimulus_entries_from_sources(
                    &app.recording.model.channel_recordings,
                    &speaker_names
                )
                .len()
            )
        }
        "recording.stimulus_levels_vary" => {
            let mut levels = app
                .recording
                .model
                .channel_recordings
                .iter()
                .filter_map(|channel| {
                    channel
                        .result
                        .as_ref()?
                        .stimulus_config
                        .as_ref()?
                        .level_dbfs
                })
                .collect::<Vec<_>>();
            levels.sort_by(f32::total_cmp);
            json!(
                matches!((levels.first(), levels.last()), (Some(min), Some(max)) if *max - *min > 0.01)
            )
        }
        "recording.stimulus_config_matches_current" => {
            let mut saw_completed = false;
            let matches = app
                .recording
                .model
                .channel_recordings
                .iter()
                .filter(|channel| channel.result.is_some())
                .all(|channel| {
                    let Some(actual) = channel
                        .result
                        .as_ref()
                        .and_then(|result| result.stimulus_config.as_ref())
                    else {
                        return false;
                    };
                    saw_completed = true;
                    crate::app::recording_acquisition_config_for_range(
                        &app.recording.model,
                        channel.sweep_start_freq,
                        channel.sweep_end_freq,
                    )
                    .is_ok_and(|expected| &expected == actual)
                });
            json!(saw_completed && matches)
        }
        "recording.signal_level_db" => json!(app.recording.model.signal_level_db),
        "recording.sweep_start_hz" => json!(app.recording.model.sweep_start_freq),
        "recording.sweep_end_hz" => json!(app.recording.model.sweep_end_freq),
        "recording.num_sweeps" => json!(app.recording.model.num_sweeps),
        "recording.selected_sweep_start_hz" => json!(
            app.recording
                .model
                .current_recording_channel
                .and_then(|index| app.recording.model.channel_recordings.get(index))
                .map(|channel| channel.sweep_start_freq)
        ),
        "recording.selected_sweep_end_hz" => json!(
            app.recording
                .model
                .current_recording_channel
                .and_then(|index| app.recording.model.channel_recordings.get(index))
                .map(|channel| channel.sweep_end_freq)
        ),
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
        "recording.virtual_capture_profile" => json!(
            app.recording
                .virtual_capture_profile
                .as_deref()
                .unwrap_or("")
        ),
        "recording.current_state" => json!(
            app.recording
                .model
                .current_recording_channel
                .and_then(|index| app.recording.model.channel_recordings.get(index))
                .map(|channel| format!("{:?}", channel.state))
        ),
        "recording.review_needed_count" => json!(
            app.recording
                .model
                .channel_recordings
                .iter()
                .filter(|channel| {
                    channel.state
                        == sotf_audio_player::recording_types::ChannelRecordingState::ReviewNeeded
                })
                .count()
        ),
        "recording.quality_results" => json!(
            app.recording
                .model
                .channel_recordings
                .iter()
                .filter_map(|channel| channel
                    .result
                    .as_ref()
                    .and_then(|result| result.quality.as_ref()))
                .collect::<Vec<_>>()
        ),
        "recording.thd_warning_count" => json!(
            recording_quality_findings(app)
                .filter(|finding| {
                    finding.metric == sotf_capture::quality::QualityMetric::TotalHarmonicDistortion
                        && finding.status == sotf_capture::quality::QualityStatus::Warning
                })
                .count()
        ),
        "recording.max_thd_percent" => json!(
            recording_quality_findings(app)
                .filter(|finding| {
                    finding.metric == sotf_capture::quality::QualityMetric::TotalHarmonicDistortion
                })
                .filter_map(|finding| finding.measured)
                .reduce(f64::max)
        ),
        "recording.unknown_finding_count" => json!(
            recording_quality_findings(app)
                .filter(|finding| finding.status == sotf_capture::quality::QualityStatus::Unknown)
                .count()
        ),
        "recording.unknown_review_ack_required" => json!(
            app.recording
                .model
                .channel_recordings
                .iter()
                .any(|channel| {
                    channel.state
                        == sotf_audio_player::recording_types::ChannelRecordingState::ReviewNeeded
                        && channel.result.as_ref().is_some_and(|result| {
                            result.quality.as_ref().is_none_or(|quality| {
                                quality.findings.iter().any(|finding| {
                                    finding.status == sotf_capture::quality::QualityStatus::Unknown
                                })
                            })
                        })
                })
        ),
        "recording.unknown_review_explained" => json!(
            app.recording
                .model
                .status_message
                .to_ascii_lowercase()
                .contains("unknown")
                || app
                    .recording
                    .model
                    .status_message
                    .to_ascii_lowercase()
                    .contains("unavailable")
        ),
        "recording.quality_sidecar_count" => json!(
            app.recording_quality_sidecar_entries()
                .as_ref()
                .map_or(0, Vec::len)
        ),
        "recording.quality_sidecar_results" => {
            json!(app.recording_quality_sidecar_entries().unwrap_or_default())
        }
        "recording.quality_sidecar_unknown_count" => json!(
            app.recording_quality_sidecar_entries()
                .unwrap_or_default()
                .iter()
                .flat_map(|entry| entry.quality.findings.iter())
                .filter(|finding| {
                    finding.status == sotf_capture::quality::QualityStatus::Unknown
                })
                .count()
        ),
        "recording.save_success" => json!(app.recording.save.success),
        "recording.save_error" => {
            json!(app.recording.save.error.as_deref().unwrap_or(""))
        }
        "recording.persisted" => {
            let directory = if app.recording.output_directory.is_empty() {
                std::path::PathBuf::from(".")
            } else {
                std::path::PathBuf::from(&app.recording.output_directory)
            };
            json!(
                directory
                    .join(sotf_audio_player::recording_helpers::RECORDINGS_FILENAME)
                    .is_file()
            )
        }

        // Listening workflows use the shared session controllers. These
        // read-only counters let scenarios assert completed domain work.
        "learning.session_active" => json!(app.ui.ear_training.session.is_some()),
        "learning.config_trial_count" => json!(app.ui.ear_training.config.trial_count),
        "learning.session_current_answered" => json!(
            app.ui
                .ear_training
                .session
                .as_ref()
                .is_some_and(|session| session.current_is_answered())
        ),
        "learning.session_trial_count" => json!(
            app.ui
                .ear_training
                .session
                .as_ref()
                .map_or(0, |session| session.trials.len())
        ),
        "learning.session_correct_count" => json!(
            app.ui
                .ear_training
                .session
                .as_ref()
                .map_or(0, |session| session.correct_count())
        ),
        "learning.session_answered" => json!(
            app.ui
                .ear_training
                .session
                .as_ref()
                .is_some_and(|session| session.current_is_answered())
        ),
        "learning.progress_total_attempts" => {
            json!(app.ui.ear_training.progress.total_attempts())
        }
        "learning.progress_persisted" => json!(
            sotf_audio_player::config::get_ear_training_progress_path()
                .is_some_and(|path| path.is_file())
        ),
        "learning.progress_accuracy" => json!(app.ui.ear_training.progress.accuracy()),
        "learning.filtered_audition" => json!(app.ui.ear_training.filtered),
        "learning.selected_path" => {
            json!(app.plugin_rack.graph.nodes.values().find_map(
                |node| match &node.plugin.settings {
                    sotf_audio_player::PluginSettings::ABCompare { selected_path, .. } => {
                        Some(*selected_path)
                    }
                    _ => None,
                }
            ))
        }
        "ab_testing.step" => json!(format!("{:?}", app.ui.ab_testing.step)),
        "ab_testing.path_a_captured" => json!(app.ui.ab_testing.path_a.is_some()),
        "ab_testing.path_b_captured" => json!(app.ui.ab_testing.path_b.is_some()),
        "ab_testing.status" => json!(app.ui.ab_testing.status.as_str()),
        "ab_testing.session_active" => {
            json!(app.ui.ab_testing.controller.session().is_some())
        }
        "ab_testing.completed_trials" => {
            json!(app.ui.ab_testing.controller.view().completed_trials)
        }
        "ab_testing.abx_score" => json!(app.ui.ab_testing.controller.view().abx_score),
        "ab_testing.prepared_duration_ms" => json!(
            app.ui
                .ab_testing
                .controller
                .session()
                .map(|session| session.setup.media.duration_ms)
        ),
        "ab_testing.level_match_correction_db" => json!(
            app.ui
                .ab_testing
                .controller
                .session()
                .map(|session| session.setup.level_match.correction_b_db)
        ),
        "ab_testing.session_persisted" => json!(
            sotf_audio_player::config::get_app_config_dir()
                .is_some_and(|directory| directory.join("ab-testing-session.json").is_file())
        ),
        "ab_testing.runtime_active" => {
            json!(app.ui.ab_testing.controller.view().runtime_active)
        }
        "ab_testing.trial_mode" => json!(format!("{:?}", app.ui.ab_testing.trial_mode)),
        "ab_testing.blind_mode" => {
            json!(app.ui.ab_testing.trial_mode == sotf_audio_player::TrialMode::BlindAb)
        }
        "ab_testing.phase" => {
            json!(format!("{:?}", app.ui.ab_testing.controller.view().phase))
        }
        "ab_testing.available_cue_count" => {
            json!(app.ui.ab_testing.controller.view().available_cues.len())
        }
        "ab_testing.available_answer_count" => {
            json!(app.ui.ab_testing.controller.view().available_answers.len())
        }
        "ab_testing.assignment_seed" => json!(
            app.ui
                .ab_testing
                .controller
                .session()
                .map(|session| session.assignment_seed)
        ),
        "ab_testing.committed_x_path" => json!(
            app.ui
                .ab_testing
                .controller
                .session()
                .and_then(|session| session.trials.last())
                .and_then(|trial| {
                    trial
                        .path_for_cue(sotf_audio_player::TrialCue::Unknown)
                        .ok()
                })
                .map(|path| match path {
                    sotf_audio_player::controllers::ab_test_session::PathSelection::A => "A",
                    sotf_audio_player::controllers::ab_test_session::PathSelection::B => "B",
                })
        ),

        // Room EQ
        "roomeq.step" => json!(format!("{:?}", app.room_eq.model.step)),
        "roomeq.step_tab_focused" => json!(app.room_eq.step_tab_focused),
        "roomeq.editing_file_path" => json!(app.room_eq.editing_file_path),
        "roomeq.focus_context" => json!(if app.room_eq.editing_file_path
            && app.room_eq.model.step == sotf_audio_player::room_eq_types::RoomEqStep::LoadData
        {
            "FilePathEdit"
        } else if app.room_eq.step_tab_focused {
            "StepTabs"
        } else {
            "Content"
        }),
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
        "roomeq.error" => json!(app.room_eq.load_error.as_deref().unwrap_or("")),
        "roomeq.file_path" => json!(app.room_eq.file_path.as_str()),
        "roomeq.export_path" => json!(app.room_eq.export_path.as_str()),
        "roomeq.export_exists" => json!(std::path::Path::new(&app.room_eq.export_path).is_file()),
        // The JSON export is the full DspChainOutput: count EQ filters
        // across every channel chain in the exported file.
        "roomeq.export_filter_count" => json!(
            std::fs::read_to_string(&app.room_eq.export_path)
                .ok()
                .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
                .map(|value| chain_filter_count(&value))
                .unwrap_or(0)
        ),
        "roomeq.export_filter_count_matches_result" => {
            let exported_count = std::fs::read_to_string(&app.room_eq.export_path)
                .ok()
                .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
                .map(|value| chain_filter_count(&value));
            let result_count = app
                .room_eq
                .model
                .dsp_output
                .as_ref()
                .and_then(|output| serde_json::to_value(output).ok())
                .map(|value| chain_filter_count(&value));
            json!(
                exported_count.is_some()
                    && result_count.is_some()
                    && exported_count == result_count
            )
        }
        "roomeq.result_filter_count" => json!(
            app.room_eq
                .model
                .channel_results
                .iter()
                .map(|result| result.eq_filters.len())
                .sum::<usize>()
        ),
        "roomeq.result_nonzero_gain_count" => json!(
            app.room_eq
                .model
                .channel_results
                .iter()
                .flat_map(|result| result.eq_filters.iter())
                .filter(|filter| filter.gain_db.abs() > 1e-6)
                .count()
        ),
        "roomeq.applied_eq_filter_count" => json!(
            applied_room_eq_filters(app)
                .as_array()
                .into_iter()
                .flatten()
                .map(|plugin| plugin["filter_count"].as_u64().unwrap_or(0) as usize)
                .sum::<usize>()
        ),
        "roomeq.applied_eq_nonzero_gain_count" => json!(
            applied_room_eq_filters(app)
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|plugin| plugin.get("filters").and_then(serde_json::Value::as_array))
                .flatten()
                .filter(|filter| {
                    filter
                        .get("gain_db")
                        .and_then(serde_json::Value::as_f64)
                        .is_some_and(|gain| gain.abs() > 1e-6)
                })
                .count()
        ),
        "roomeq.applied_eq_filters" => applied_room_eq_filters(app),
        "roomeq.applied_eq_parameters_match_results" => {
            json!(applied_eq_parameters_match_results(app))
        }
        "roomeq.applied_eq_parameters_match_dsp_output" => {
            json!(applied_eq_parameters_match_dsp_output(app))
        }
        "roomeq.apply_status" => {
            json!(app.room_eq.apply_status.as_deref().unwrap_or(""))
        }
        "roomeq.apply_error" => {
            json!(app.room_eq.apply_error.as_deref().unwrap_or(""))
        }
        "roomeq.quality_sidecar_count" => json!(
            saved_quality_sidecar_entries(std::path::Path::new(&app.room_eq.file_path))
                .as_ref()
                .map_or(0, Vec::len)
        ),
        "roomeq.quality_sidecar_unknown_count" => json!(
            saved_quality_sidecar_entries(std::path::Path::new(&app.room_eq.file_path))
                .unwrap_or_default()
                .iter()
                .flat_map(|entry| entry.quality.findings.iter())
                .filter(|finding| {
                    finding.status == sotf_capture::quality::QualityStatus::Unknown
                })
                .count()
        ),
        "roomeq.quality_review_entry_count" => {
            json!(app.room_eq.quality_review_entries.len())
        }
        "roomeq.stimulus_configs" => json!(
            app.room_eq
                .quality_review_entries
                .iter()
                .filter_map(|entry| entry
                    .stimulus_config
                    .as_ref()
                    .map(|config| (&entry.source, config)))
                .collect::<Vec<_>>()
        ),
        "roomeq.stimulus_config_count" => json!(
            app.room_eq
                .quality_review_entries
                .iter()
                .filter(|entry| entry.stimulus_config.is_some())
                .count()
        ),
        "roomeq.capture_level_review_note" => {
            let levels = app
                .room_eq
                .quality_review_entries
                .iter()
                .filter_map(|entry| {
                    let level = entry.stimulus_config.as_ref()?.level_dbfs?;
                    Some((
                        entry.source.speaker.clone(),
                        entry.source.measurement_index + 1,
                        level,
                    ))
                })
                .collect::<Vec<_>>();
            json!(crate::ui::capture_level_review_notes(&levels).join("\n"))
        }
        "roomeq.stimulus_configs_match_recording" => {
            let speaker_names = app
                .recording
                .model
                .playback_config
                .channel_mappings
                .iter()
                .map(|mapping| mapping.group_name.clone())
                .collect::<Vec<_>>();
            let recorded =
                sotf_audio_player::recording_helpers::recording_stimulus_entries_from_sources(
                    &app.recording.model.channel_recordings,
                    &speaker_names,
                );
            let mut matched = 0usize;
            let all_match = app.room_eq.quality_review_entries.iter().all(|entry| {
                let Some(config) = entry.stimulus_config.as_ref() else {
                    return false;
                };
                let same_source = recorded
                    .iter()
                    .find(|recorded| recorded.source == entry.source);
                let is_match = same_source.is_some_and(|recorded| &recorded.config == config);
                matched += usize::from(is_match);
                is_match
            });
            json!(matched > 0 && all_match)
        }
        "roomeq.quality_unknown_count" => json!(
            app.room_eq
                .quality_review_entries
                .iter()
                .filter(|entry| {
                    entry.quality.as_ref().is_none_or(|quality| {
                        !quality.trustworthy
                            && sotf_audio_player::recording_helpers::take_verdict_text(quality)
                                .to_ascii_lowercase()
                                .contains("unknown")
                    })
                })
                .count()
        ),
        "roomeq.thd_warning_count" => json!(
            room_eq_quality_findings(app)
                .filter(|finding| {
                    finding.metric == sotf_capture::quality::QualityMetric::TotalHarmonicDistortion
                        && finding.status == sotf_capture::quality::QualityStatus::Warning
                })
                .count()
        ),
        "roomeq.max_thd_percent" => json!(
            room_eq_quality_findings(app)
                .filter(|finding| {
                    finding.metric == sotf_capture::quality::QualityMetric::TotalHarmonicDistortion
                })
                .filter_map(|finding| finding.measured)
                .reduce(f64::max)
        ),
        "roomeq.unknown_finding_count" => json!(
            room_eq_quality_findings(app)
                .filter(|finding| finding.status == sotf_capture::quality::QualityStatus::Unknown)
                .count()
        ),
        "roomeq.quality_review_entries" => json!(
            app.room_eq
                .quality_review_entries
                .iter()
                .map(|entry| {
                    json!({
                        "source": entry.source,
                        "source_label": entry.source_label,
                        "quality": entry.quality,
                        "stimulus_config": entry.stimulus_config,
                        "verdict": entry.quality.as_ref().map(
                            sotf_audio_player::recording_helpers::take_verdict_text
                        ),
                    })
                })
                .collect::<Vec<_>>()
        ),
        "roomeq.quality_sidecar_warning" => {
            json!(app.room_eq.quality_sidecar_warning.as_deref().unwrap_or(""))
        }

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

        other if other.starts_with("plugins.") => {
            sotf_audio_player::controllers::plugin::dev_api::queries::plugin_query(
                &app.plugin_rack.graph,
                other,
            )?
        }

        other => return Err(anyhow!("unknown query path: `{other}`")),
    })
}

fn saved_quality_sidecar_entries(
    recordings_json_path: &std::path::Path,
) -> Option<Vec<sotf_audio_player::recording_helpers::RecordingQualityEntry>> {
    let recordings_json = std::fs::read(recordings_json_path).ok()?;
    let sidecar = sotf_audio_player::recording_helpers::load_recording_quality_sidecar(
        recordings_json_path,
        &recordings_json,
    )
    .ok()??;
    Some(sidecar.entries)
}

fn recording_quality_findings(
    app: &App,
) -> impl Iterator<Item = &sotf_capture::quality::QualityFinding> {
    app.recording
        .model
        .channel_recordings
        .iter()
        .filter_map(|channel| channel.result.as_ref()?.quality.as_ref())
        .flat_map(|quality| quality.findings.iter())
}

fn room_eq_quality_findings(
    app: &App,
) -> impl Iterator<Item = &sotf_capture::quality::QualityFinding> {
    app.room_eq
        .quality_review_entries
        .iter()
        .filter_map(|entry| entry.quality.as_ref())
        .flat_map(|quality| quality.findings.iter())
}

fn applied_room_eq_filters(app: &App) -> serde_json::Value {
    use sotf_audio_player::PluginSettings;

    let plugins = (0..app.plugin_rack.graph.len())
        .filter_map(|index| {
            let plugin = app.plugin_rack.graph.get_plugin(index)?;
            let PluginSettings::EQ {
                filters,
                channel_filters,
                ..
            } = &plugin.settings
            else {
                return None;
            };
            let channel_filters = channel_filters.as_ref();
            let effective_filters = channel_filters
                .filter(|channels| !channels.is_empty())
                .map(|channels| channels.iter().flatten().collect::<Vec<_>>())
                .unwrap_or_else(|| filters.iter().collect());
            Some(serde_json::json!({
                "index": index,
                "name": plugin.name.as_deref().unwrap_or(""),
                "filter_count": effective_filters.len(),
                "filters": effective_filters.iter().map(|filter| serde_json::json!({
                    "frequency": filter.frequency,
                    "gain_db": filter.gain_db,
                    "q": filter.q,
                })).collect::<Vec<_>>(),
            }))
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(plugins)
}

fn applied_eq_parameters_match_results(app: &App) -> bool {
    let mut expected = app
        .room_eq
        .model
        .channel_results
        .iter()
        .flat_map(|result| result.eq_filters.iter())
        .map(|filter| (filter.frequency, filter.q, filter.gain_db))
        .collect::<Vec<_>>();
    let mut actual = applied_room_eq_filters(app)
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|plugin| plugin.get("filters").and_then(serde_json::Value::as_array))
        .flatten()
        .filter_map(|filter| {
            Some((
                filter.get("frequency")?.as_f64()?,
                filter.get("q")?.as_f64()?,
                filter.get("gain_db")?.as_f64()?,
            ))
        })
        .collect::<Vec<_>>();
    let sort = |filters: &mut Vec<(f64, f64, f64)>| {
        filters.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| left.2.total_cmp(&right.2))
        });
    };
    sort(&mut expected);
    sort(&mut actual);
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual.iter())
            .all(|(expected, actual)| {
                (expected.0 - actual.0).abs() < 1e-6
                    && (expected.1 - actual.1).abs() < 1e-6
                    && (expected.2 - actual.2).abs() < 1e-6
            })
}

fn applied_eq_parameters_match_dsp_output(app: &App) -> bool {
    let Some(dsp_output) = app.room_eq.model.dsp_output.as_ref() else {
        return false;
    };
    let channel_names = app
        .room_eq
        .model
        .channel_results
        .iter()
        .map(|result| result.channel_name.as_str())
        .collect::<Vec<_>>();
    let graph_apply =
        !sotf_audio_player::room_eq_types::DspChainOutputExt::is_rack_compatible(dsp_output);
    let mut expected = channel_names
        .iter()
        .filter_map(|name| dsp_output.channels.get(*name))
        .flat_map(|channel| {
            if graph_apply {
                channel
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.plugin_type.eq_ignore_ascii_case("eq"))
                    .filter_map(|plugin| {
                        plugin
                            .parameters
                            .get("filters")
                            .and_then(serde_json::Value::as_array)
                    })
                    .flat_map(|filters| {
                        sotf_audio_player::room_eq_types::parse_eq_filters_from_json(filters)
                    })
                    .collect::<Vec<_>>()
            } else {
                let (main, broadband) =
                    sotf_audio_player::autoeq::classify_channel_eq_filters(channel);
                main.into_iter().chain(broadband).collect()
            }
        })
        .map(|filter| (filter.frequency, filter.q, filter.gain_db))
        .collect::<Vec<_>>();
    let mut actual = applied_room_eq_filters(app)
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|plugin| plugin.get("filters").and_then(serde_json::Value::as_array))
        .flatten()
        .filter_map(|filter| {
            Some((
                filter.get("frequency")?.as_f64()?,
                filter.get("q")?.as_f64()?,
                filter.get("gain_db")?.as_f64()?,
            ))
        })
        .collect::<Vec<_>>();
    let sort = |filters: &mut Vec<(f64, f64, f64)>| {
        filters.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| left.2.total_cmp(&right.2))
        });
    };
    sort(&mut expected);
    sort(&mut actual);
    !expected.is_empty()
        && expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual.iter())
            .all(|(expected, actual)| {
                (expected.0 - actual.0).abs() < 1e-6
                    && (expected.1 - actual.1).abs() < 1e-6
                    && (expected.2 - actual.2).abs() < 1e-6
            })
}

impl App {
    fn recording_quality_sidecar_entries(
        &self,
    ) -> Option<Vec<sotf_audio_player::recording_helpers::RecordingQualityEntry>> {
        let directory = if self.recording.output_directory.is_empty() {
            std::path::PathBuf::from(".")
        } else {
            std::path::PathBuf::from(&self.recording.output_directory)
        };
        saved_quality_sidecar_entries(
            &directory.join(sotf_audio_player::recording_helpers::RECORDINGS_FILENAME),
        )
    }
}

/// Count EQ filters in a serialized full-chain output
/// (`channels -> plugins -> parameters.filters`).
fn chain_filter_count(value: &Value) -> usize {
    let mut total = 0usize;
    if let Some(channels) = value.get("channels").and_then(Value::as_object) {
        for channel in channels.values() {
            for collection in ["plugins", "global_plugins"] {
                if let Some(plugins) = channel.get(collection).and_then(Value::as_array) {
                    for plugin in plugins {
                        if let Some(filters) = plugin
                            .get("parameters")
                            .and_then(|p| p.get("filters"))
                            .and_then(Value::as_array)
                        {
                            total += filters.len();
                        }
                    }
                }
            }
            if let Some(drivers) = channel.get("drivers").and_then(Value::as_array) {
                for driver in drivers {
                    if let Some(plugins) = driver.get("plugins").and_then(Value::as_array) {
                        for plugin in plugins {
                            if let Some(filters) = plugin
                                .get("parameters")
                                .and_then(|p| p.get("filters"))
                                .and_then(Value::as_array)
                            {
                                total += filters.len();
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(plugins) = value.get("global_plugins").and_then(Value::as_array) {
        for plugin in plugins {
            if let Some(filters) = plugin
                .get("parameters")
                .and_then(|p| p.get("filters"))
                .and_then(Value::as_array)
            {
                total += filters.len();
            }
        }
    }
    total
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
