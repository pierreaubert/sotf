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

    pub(super) fn adjust_eq_training_config(
        &mut self,
        field: EqConfigField,
        direction: i32,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let settings_changed = state
                .app
                .ui_state
                .translations
                .listening_test
                .eq
                .configure_start;
            let listening = &mut state.app.plugin_state.listening_test_state;
            match field {
                EqConfigField::Bands => {
                    listening.eq_config.band_count =
                        (listening.eq_config.band_count as i32 + direction).clamp(2, 25) as usize;
                }
                EqConfigField::Gain => {
                    listening.eq_config.gain_db =
                        (listening.eq_config.gain_db + f64::from(direction)).clamp(1.0, 15.0);
                }
                EqConfigField::Q => {
                    listening.eq_config.q =
                        (listening.eq_config.q + f64::from(direction) * 0.1).clamp(0.2, 10.0);
                }
                EqConfigField::Trials => {
                    listening.eq_config.trial_count =
                        (listening.eq_config.trial_count as i32 + direction * 5).clamp(5, 100)
                            as usize;
                }
            }
            listening.eq_session = None;
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

    pub(super) fn add_current_eq_source(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let Some(path) = state.app.get_current_track_path() else {
                return;
            };
            let listening = &mut state.app.plugin_state.listening_test_state;
            if !listening.eq_sources.contains(&path) {
                listening.eq_sources.push(path);
                listening.eq_source_index = listening.eq_sources.len() - 1;
            }
            listening.status = format!("{} training sources", listening.eq_sources.len());
        });
        cx.notify();
    }

    pub(super) fn navigate_eq_source(&mut self, direction: i32, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            if listening.eq_sources.is_empty() {
                return;
            }
            listening.eq_source_index = (listening.eq_source_index as i32 + direction)
                .rem_euclid(listening.eq_sources.len() as i32)
                as usize;
            let path = listening.eq_sources[listening.eq_source_index].clone();
            listening.status = format!(
                "Source {}/{}",
                listening.eq_source_index + 1,
                listening.eq_sources.len()
            );
            Self::play_track(state, sotf_audio::decoder::AudioSource::File(path));
        });
        cx.notify();
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

    pub(super) fn activate_eq_training_path(&mut self, filtered: bool, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
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
                        .ok_or_else(|| eq_text.add_ab_plugin.to_owned())?;
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
                    state.app.set_plugin_param(plugin_idx, 7, 0.0);
                    state.app.set_plugin_param(plugin_idx, 8, 20.0);
                    Ok(())
                });
            let listening = &mut state.app.plugin_state.listening_test_state;
            match result {
                Ok(()) => {
                    listening.eq_filtered = filtered;
                    listening.status = if filtered {
                        eq_text.filtered_active
                    } else {
                        eq_text.original_active
                    }
                    .into();
                }
                Err(error) => listening.status = error,
            }
        });
        cx.notify();
    }

    pub(super) fn submit_eq_training_answer(&mut self, cx: &mut Context<Self>) {
        self.activate_eq_training_path(false, cx);
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
        let (advanced, progress_to_save) = self.state.update(cx, |state, _| {
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
                        format!(
                            "Session complete: {}/{} correct ({:.0}%).",
                            session.correct_count(),
                            session.trials.len(),
                            session.accuracy() * 100.0
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
