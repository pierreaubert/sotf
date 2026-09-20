use super::*;

impl PlayerView {
    pub(super) fn set_ear_training_surface(
        &mut self,
        surface: EarTrainingSurface,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            activate_listening_surface(&mut state.app, surface);
        });
        cx.notify();
    }

    pub(super) fn set_eq_training_config(
        &mut self,
        field: EqConfigField,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        self.state.update(cx, |state, _| {
            let settings_changed = state
                .app
                .ui_state
                .translations
                .listening_test
                .eq
                .configure_start;
            let listening = &mut state.app.plugin_state.listening_test_state;
            if listening.eq_session.is_some() {
                return;
            }
            let mut config = listening.eq_config.clone();
            match field {
                EqConfigField::Bands => config.band_count = value.round() as usize,
                EqConfigField::Gain => config.gain_db = value,
                EqConfigField::Q => config.q = value,
                EqConfigField::Trials => config.trial_count = value.round() as usize,
                EqConfigField::MinFrequency => config.min_frequency_hz = value,
                EqConfigField::MaxFrequency => config.max_frequency_hz = value,
            }
            if config.validate().is_err() {
                return;
            }
            listening.eq_config = config;
            listening.eq_active_course = None;
            listening.eq_selected_band = 0;
            listening.eq_filtered = false;
            listening.status = settings_changed.into();
        });
        cx.notify();
    }

    pub(super) fn cycle_eq_training_change_mode(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let settings_changed = state
                .app
                .ui_state
                .translations
                .listening_test
                .eq
                .configure_start;
            let change_label = state.app.ui_state.translations.listening_test.eq.change;
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_config.change_mode = match listening.eq_config.change_mode {
                EqChangeMode::Boost => EqChangeMode::Cut,
                EqChangeMode::Cut => EqChangeMode::Mixed,
                EqChangeMode::Mixed => EqChangeMode::Boost,
            };
            listening.eq_session = None;
            listening.status = format!(
                "{}: {}. {settings_changed}",
                change_label,
                eq_change_mode_symbol(listening.eq_config.change_mode)
            );
        });
        cx.notify();
    }

    pub(super) fn start_eq_training_session(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let workspace = &mut state.app.plugin_state.plugin_ui_state.listening_workspace;
            // A source probe started before this session must never replace its audio,
            // including if the session ends before the probe finishes.
            workspace.source_request = None;
            workspace.practice = Default::default();
        });
        self.ensure_eq_audition_plugin(cx);
        let started = self.state.update(cx, |state, _| {
            let session_started = state
                .app
                .ui_state
                .translations
                .listening_test
                .eq
                .session_started;
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_config.seed = listening.eq_config.seed.wrapping_add(1);
            match EqTrainingSession::new(listening.eq_config.clone()).and_then(|mut session| {
                session.start()?;
                Ok(session)
            }) {
                Ok(session) => {
                    listening.eq_session = Some(session);
                    listening.eq_selected_band = 0;
                    listening.eq_filtered = false;
                    listening.status = session_started.into();
                    true
                }
                Err(error) => {
                    listening.status = error.to_string();
                    false
                }
            }
        });
        if started {
            self.activate_eq_training_path(false, cx);
        }
        cx.notify();
    }

    pub(super) fn start_eq_course(&mut self, course: EarTrainingCourse, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_config = course.config();
            listening.eq_active_course = Some(course);
            listening.surface = EarTrainingSurface::EqBands;
        });
        self.start_eq_training_session(cx);
    }

    pub(super) fn cycle_eq_training_exercise(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_config.exercise = match listening.eq_config.exercise {
                EqTrainingExercise::BandIdentification => {
                    EqTrainingExercise::BoostCutIdentification
                }
                EqTrainingExercise::BoostCutIdentification => {
                    EqTrainingExercise::GainIdentification
                }
                EqTrainingExercise::GainIdentification => EqTrainingExercise::BandIdentification,
            };
            listening.eq_session = None;
            listening.eq_active_course = None;
        });
        cx.notify();
    }

    pub(super) fn toggle_eq_training_adaptive(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_adaptive = !listening.eq_adaptive;
            if listening.eq_adaptive {
                let exercise = listening.eq_config.exercise;
                listening.eq_config = listening.eq_progress.adaptive_config();
                listening.eq_config.exercise = exercise;
                listening.eq_active_course = None;
            }
        });
        cx.notify();
    }

    pub(super) fn browse_eq_training_source(&mut self, cx: &mut Context<Self>) {
        self.browse_listening_source(EarTrainingSurface::EqBands, cx);
    }

    pub(super) fn browse_listening_source(
        &mut self,
        surface: EarTrainingSurface,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let title = self
                .state
                .read(cx)
                .app
                .ui_state
                .translations
                .listening_test
                .eq
                .choose_source();
            let owner = cx.entity().downgrade();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .set_title(title)
                    .add_filter(
                        "Audio",
                        &[
                            "wav", "flac", "mp3", "m4a", "ogg", "opus", "aiff", "aif", "aac",
                            "dsf", "dff", "wv", "caf",
                        ],
                    )
                    .pick_file()
                    .await;
                if let Some(file) = file {
                    let _ = owner.update(cx, |view, cx| {
                        view.load_listening_source(file.path().to_path_buf(), surface, cx)
                    });
                }
            })
            .detach();
        }
    }

    /// Native picker and QA both use the same bounded metadata-probe worker.
    pub(crate) fn load_eq_training_source(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.load_listening_source(path, EarTrainingSurface::EqBands, cx);
    }

    pub(crate) fn load_listening_source(
        &mut self,
        path: std::path::PathBuf,
        surface: EarTrainingSurface,
        cx: &mut Context<Self>,
    ) {
        let state = self.state.read(cx);
        if state.app.ui_state.current_screen != crate::app::Screen::ListeningTest
            || state.app.plugin_state.listening_test_state.surface != surface
            || (surface == EarTrainingSurface::BlindComparison && self.comparison_setup_locked(cx))
            || (surface == EarTrainingSurface::EqBands
                && state
                    .app
                    .plugin_state
                    .listening_test_state
                    .eq_session
                    .is_some())
        {
            return;
        }
        let request = std::sync::Arc::new(());
        self.state.update(cx, |state, _| {
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .source_request = Some(request.clone());
        });
        let owner = cx.entity().downgrade();
        cx.notify();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { sotf_audio_player::Album::from_audio_file(&path) })
                .await;
            let _ = owner.update(cx, |view, cx| {
                view.state.update(cx, |state, cx| {
                    let workspace = &mut state.app.plugin_state.plugin_ui_state.listening_workspace;
                    if !workspace
                        .source_request
                        .as_ref()
                        .is_some_and(|current| std::sync::Arc::ptr_eq(current, &request))
                    {
                        return;
                    }
                    workspace.source_request = None;
                    if state.app.ui_state.current_screen != crate::app::Screen::ListeningTest
                        || state.app.plugin_state.listening_test_state.surface != surface
                        || (surface == EarTrainingSurface::EqBands
                            && state
                                .app
                                .plugin_state
                                .listening_test_state
                                .eq_session
                                .is_some())
                        || (surface == EarTrainingSurface::BlindComparison && {
                            let session =
                                state.app.plugin_state.listening_test_state.ab_test.view();
                            session.runtime_active || session.completed_trials > 0
                        })
                    {
                        cx.notify();
                        return;
                    }
                    let text = state.app.ui_state.translations.listening_test.eq.clone();
                    match result.and_then(|album| state.app.play_single_audio_file(album)) {
                        Ok(Some(source)) => {
                            if surface == EarTrainingSurface::BlindComparison {
                                // Even reloading the same path invalidates a pending
                                // measurement: its file contents may have changed.
                                state
                                    .app
                                    .plugin_state
                                    .plugin_ui_state
                                    .listening_workspace
                                    .preparation_request = None;
                                let listening = &mut state.app.plugin_state.listening_test_state;
                                let _ = listening.ab_test.clear_session();
                                listening.status.clear();
                            } else if let Some(path) = source.as_path() {
                                let listening = &mut state.app.plugin_state.listening_test_state;
                                let index = listening
                                    .eq_sources
                                    .iter()
                                    .position(|existing| existing == path)
                                    .unwrap_or_else(|| {
                                        listening.eq_sources.push(path.to_path_buf());
                                        listening.eq_sources.len() - 1
                                    });
                                listening.eq_source_index = index;
                                listening.eq_loop_range = None;
                                listening.eq_loop_enabled = false;
                                listening.status =
                                    text.source_position(index + 1, listening.eq_sources.len());
                            }
                            Self::play_track(state, source);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            let message = text.source_error(&error);
                            state.app.plugin_state.listening_test_state.status = message.clone();
                            state.app.ui_state.toast_message =
                                Some(crate::app::types::ToastMessage::error(message));
                        }
                    }
                    cx.notify();
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn add_current_eq_source(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let Some(path) = state.app.get_current_track_path() else {
                return;
            };
            let text = state.app.ui_state.translations.listening_test.eq.clone();
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .source_request = None;
            let listening = &mut state.app.plugin_state.listening_test_state;
            let index = listening
                .eq_sources
                .iter()
                .position(|existing| existing == &path)
                .unwrap_or_else(|| {
                    listening.eq_sources.push(path);
                    listening.eq_sources.len() - 1
                });
            listening.eq_source_index = index;
            listening.status = text.source_position(index + 1, listening.eq_sources.len());
        });
        cx.notify();
    }

    pub(super) fn navigate_eq_source(&mut self, direction: i32, cx: &mut Context<Self>) {
        let path = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            if listening.eq_sources.is_empty() {
                return;
            }
            let index = (listening.eq_source_index as i64 + i64::from(direction))
                .rem_euclid(listening.eq_sources.len() as i64) as usize;
            listening.eq_sources[index].clone()
        };
        self.load_eq_training_source(path, cx);
    }

    pub(super) fn set_eq_loop_boundary(&mut self, start: bool, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let eq_text = state.app.ui_state.translations.listening_test.eq.clone();
            let position = state.app.playback.position_secs.max(0.0);
            let listening = &mut state.app.plugin_state.listening_test_state;
            let (mut loop_start, mut loop_end) =
                listening.eq_loop_range.unwrap_or((0.0, position + 5.0));
            if start {
                loop_start = position.min(loop_end - 0.1);
            } else {
                loop_end = position.max(loop_start + 0.1);
            }
            listening.eq_loop_range = Some((loop_start, loop_end));
            listening.status = eq_text.clip_loop_range(loop_start, loop_end);
        });
        cx.notify();
    }

    pub(super) fn toggle_eq_loop(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let eq_text = state.app.ui_state.translations.listening_test.eq.clone();
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_loop_enabled =
                !listening.eq_loop_enabled && listening.eq_loop_range.is_some();
            listening.status = eq_text.clip_loop_status(listening.eq_loop_enabled).into();
        });
        cx.notify();
    }

    pub(super) fn ensure_eq_audition_plugin(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let existing = state
                .app
                .plugin_state
                .graph
                .plugins_linear()
                .and_then(|plugins| {
                    plugins
                        .iter()
                        .find(|node| node.plugin.plugin_type() == PluginType::ABCompare)
                        .map(|node| node.id)
                });
            if existing.is_some() {
                return;
            }
            state.app.add_plugin(&PluginType::ABCompare);
            let injected = state
                .app
                .plugin_state
                .graph
                .plugins_linear()
                .and_then(|plugins| {
                    plugins
                        .iter()
                        .find(|node| node.plugin.plugin_type() == PluginType::ABCompare)
                        .map(|node| node.id)
                });
            state
                .app
                .plugin_state
                .listening_test_state
                .eq_audition_node_id = injected;
        });
    }

    pub(super) fn select_eq_training_band(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.eq_practice_locked(cx) {
            return;
        }
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            let answer_count = listening
                .eq_session
                .as_ref()
                .and_then(|session| {
                    session.current_question.as_ref().map(|question| {
                        question
                            .answer_labels(session.config.exercise, &session.band_frequencies)
                            .len()
                    })
                })
                .unwrap_or(0);
            if index < answer_count
                && !listening
                    .eq_session
                    .as_ref()
                    .is_some_and(EqTrainingSession::current_is_answered)
            {
                listening.eq_selected_band = index;
            }
        });
        cx.notify();
    }

    pub(super) fn move_eq_training_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        if self.eq_practice_locked(cx) {
            return;
        }
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            let answer_count = listening
                .eq_session
                .as_ref()
                .and_then(|session| {
                    session.current_question.as_ref().map(|question| {
                        question
                            .answer_labels(session.config.exercise, &session.band_frequencies)
                            .len()
                    })
                })
                .unwrap_or(0);
            if answer_count == 0
                || listening
                    .eq_session
                    .as_ref()
                    .is_some_and(EqTrainingSession::current_is_answered)
            {
                return;
            }
            listening.eq_selected_band = (listening.eq_selected_band as i32 + direction)
                .rem_euclid(answer_count as i32) as usize;
        });
        cx.notify();
    }

    fn eq_practice_locked(&self, cx: &Context<Self>) -> bool {
        let plugins = &self.state.read(cx).app.plugin_state;
        plugins
            .plugin_ui_state
            .listening_workspace
            .practice
            .interaction_locked()
            || plugins
                .listening_test_state
                .eq_session
                .as_ref()
                .is_none_or(|session| session.current_question.is_none())
    }

    pub(super) fn activate_eq_training_path(&mut self, filtered: bool, cx: &mut Context<Self>) {
        if !self.eq_practice_locked(cx) {
            self.configure_eq_training_path(filtered, cx);
        }
    }

    fn configure_eq_training_path(&mut self, filtered: bool, cx: &mut Context<Self>) -> bool {
        let succeeded = self.state.update(cx, |state, _| {
            let eq_text = state.app.ui_state.translations.listening_test.eq.clone();
            let question = state
                .app
                .plugin_state
                .listening_test_state
                .eq_session
                .as_ref()
                .and_then(|session| session.current_question.clone());
            let result = question
                .ok_or_else(|| eq_text.configure_start.to_owned())
                .and_then(|question| {
                    let plugin_idx = state
                        .app
                        .plugin_state
                        .graph
                        .plugins_linear()
                        .and_then(|plugins| {
                            plugins
                                .iter()
                                .position(|node| node.plugin.plugin_type() == PluginType::ABCompare)
                        })
                        .ok_or_else(|| eq_text.comparison.add_ab_plugin.to_owned())?;
                    let path_a = serde_json::to_string(&PathConfig::None)
                        .map_err(|error| error.to_string())?;
                    let path_b = serde_json::to_string(&PathConfig::Plugin {
                        plugin_type: "eq".into(),
                        parameters: question.plugin_parameters(),
                    })
                    .map_err(|error| error.to_string())?;
                    state.app.set_plugin_param_string(plugin_idx, 9, path_a)?;
                    state.app.set_plugin_param_string(plugin_idx, 10, path_b)?;
                    state.app.set_plugin_param(plugin_idx, 0, 0.0);
                    state.app.set_plugin_param(plugin_idx, 1, 1.0);
                    state
                        .app
                        .set_plugin_param(plugin_idx, 2, if filtered { 1.0 } else { 0.0 });
                    state.app.set_plugin_param(plugin_idx, 4, 0.0);
                    state.app.set_plugin_param(plugin_idx, 7, 100.0);
                    state.app.set_plugin_param(plugin_idx, 8, 20.0);
                    Ok(())
                });
            let succeeded = result.is_ok();
            let listening = &mut state.app.plugin_state.listening_test_state;
            match result {
                Ok(()) => {
                    listening.eq_filtered = filtered;
                    listening.status = if filtered {
                        eq_text.comparison.filtered_active
                    } else {
                        eq_text.comparison.original_active
                    }
                    .into();
                }
                Err(error) => listening.status = error,
            }
            succeeded
        });
        cx.notify();
        succeeded
    }

    pub(super) fn submit_eq_training_answer(&mut self, cx: &mut Context<Self>) {
        if self.eq_practice_locked(cx) || !self.configure_eq_training_path(false, cx) {
            return;
        }
        self.state.update(cx, |state, _| {
            let eq_text = state.app.ui_state.translations.listening_test.eq.clone();
            let listening = &mut state.app.plugin_state.listening_test_state;
            let selected = listening.eq_selected_band;
            let status = match listening.eq_session.as_mut() {
                Some(session) => match session.submit_answer(selected) {
                    Ok(result) if result.correct => format!("{}.", eq_text.correct),
                    Ok(result) => format!(
                        "{}: {}.",
                        eq_text.learning.answer,
                        format_frequency(result.question.center_frequency_hz)
                    ),
                    Err(error) => error.to_string(),
                },
                None => eq_text.configure_start.into(),
            };
            listening.status = status;
        });
        cx.notify();
    }

    pub(super) fn advance_eq_training_question(&mut self, cx: &mut Context<Self>) {
        if self.eq_practice_locked(cx) {
            return;
        }
        let (advanced, progress_to_save) = self.state.update(cx, |state, _| {
            let eq_text = state.app.ui_state.translations.listening_test.eq.clone();
            let next_trial = state.app.ui_state.translations.listening_test.eq.next;
            let configure_start = state
                .app
                .ui_state
                .translations
                .listening_test
                .eq
                .configure_start;
            let listening = &mut state.app.plugin_state.listening_test_state;
            let mut completed_session = None;
            let status = match listening.eq_session.as_mut() {
                Some(session) => match session.advance() {
                    Ok(Some(_)) => {
                        listening.eq_selected_band = 0;
                        listening.eq_filtered = false;
                        next_trial.into()
                    }
                    Ok(None) => {
                        completed_session = Some(session.clone());
                        eq_text.completion_status(
                            session.correct_count(),
                            session.trials.len(),
                            session.accuracy() * 100.0,
                        )
                    }
                    Err(error) => error.to_string(),
                },
                None => configure_start.into(),
            };
            let has_question = listening
                .eq_session
                .as_ref()
                .is_some_and(|session| session.current_question.is_some());
            listening.status = status;
            if let Some(session) = completed_session {
                listening
                    .eq_progress
                    .record(&session, listening.eq_active_course);
                if listening.eq_adaptive {
                    let exercise = listening.eq_config.exercise;
                    listening.eq_config = listening.eq_progress.adaptive_config();
                    listening.eq_config.exercise = exercise;
                }
                (has_question, Some(listening.eq_progress.clone()))
            } else {
                (has_question, None)
            }
        });
        if let (Some(path), Some(progress)) = (
            sotf_audio_player::config::get_ear_training_progress_path(),
            progress_to_save,
        ) && let Err(error) = progress.save_atomic(&path)
        {
            log::warn!("Failed to save ear-training progress: {error}");
        }
        if advanced {
            self.activate_eq_training_path(false, cx);
        }
        cx.notify();
    }

    pub(super) fn toggle_eq_practice_pause(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let practice = &mut state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .practice;
            if practice.confirm_end {
                return;
            }
            let playing = state.app.playback.is_playing;
            let result = if practice.paused && practice.resume_playback {
                state.player.resume()
            } else if !practice.paused && playing {
                state.player.pause()
            } else {
                Ok(())
            };
            match result {
                Ok(()) => {
                    let resumed = practice.paused && (playing || practice.resume_playback);
                    if !practice.paused {
                        practice.resume_playback = playing;
                    }
                    practice.paused = !practice.paused;
                    state.app.playback.is_playing = resumed;
                    if resumed && !playing {
                        state.app.record_playback_resumed();
                    } else if playing && !resumed {
                        state.app.record_playback_paused();
                    }
                }
                Err(error) => {
                    state.app.plugin_state.listening_test_state.status = error.to_string()
                }
            }
            cx.notify();
        });
    }

    pub(super) fn confirm_eq_practice_end(
        &mut self,
        confirm: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .practice
                .confirm_end = confirm;
        });
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn end_eq_practice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .practice
            .confirm_end
        {
            return;
        }
        // Restore the original audition path before discarding its question.
        if !self.configure_eq_training_path(false, cx) {
            return;
        }
        let progress = self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            let session = listening.eq_session.as_mut()?;
            if !session.finish_early() {
                return None;
            }
            let keep = !session.trials.is_empty();
            if keep {
                listening
                    .eq_progress
                    .record(session, listening.eq_active_course);
            }
            listening.status.clear();
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .practice = Default::default();
            keep.then(|| listening.eq_progress.clone())
        });
        if let (Some(path), Some(progress)) = (
            sotf_audio_player::config::get_ear_training_progress_path(),
            progress,
        ) && let Err(error) = progress.save_atomic(&path)
        {
            self.state.update(cx, |state, _| {
                state.app.plugin_state.listening_test_state.status = error.to_string();
            });
        }
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn is_eq_training_active(&self, cx: &Context<Self>) -> bool {
        self.is_listening_test_active(cx)
            && self
                .state
                .read(cx)
                .app
                .plugin_state
                .listening_test_state
                .surface
                == EarTrainingSurface::EqBands
    }

    pub(crate) fn ear_training_show_eq_bands(
        &mut self,
        _: &EarTrainingShowEqBands,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.set_ear_training_surface(EarTrainingSurface::EqBands, cx);
        }
    }

    pub(crate) fn ear_training_show_blind_comparison(
        &mut self,
        _: &EarTrainingShowBlindComparison,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.set_ear_training_surface(EarTrainingSurface::BlindComparison, cx);
        }
    }

    pub(crate) fn ear_training_start(
        &mut self,
        _: &EarTrainingStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.start_eq_training_session(cx);
        }
    }

    pub(crate) fn ear_training_play_original(
        &mut self,
        _: &EarTrainingPlayOriginal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.activate_eq_training_path(false, cx);
        }
    }

    pub(crate) fn ear_training_play_filtered(
        &mut self,
        _: &EarTrainingPlayFiltered,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.activate_eq_training_path(true, cx);
        }
    }

    pub(crate) fn ear_training_select_previous_band(
        &mut self,
        _: &EarTrainingSelectPreviousBand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.move_eq_training_selection(-1, cx);
        }
    }

    pub(crate) fn ear_training_select_next_band(
        &mut self,
        _: &EarTrainingSelectNextBand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.move_eq_training_selection(1, cx);
        }
    }

    pub(crate) fn ear_training_submit(
        &mut self,
        _: &EarTrainingSubmit,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.submit_eq_training_answer(cx);
        }
    }

    pub(crate) fn ear_training_next_question(
        &mut self,
        _: &EarTrainingNextQuestion,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_eq_training_active(cx) {
            self.advance_eq_training_question(cx);
        }
    }
}
