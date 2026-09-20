use super::*;

pub(super) struct PreparedMeasurementSnapshot {
    pub media_path: String,
    pub media_id: String,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub metric: LevelMatchMetric,
    pub correction_b_db: f64,
    pub residual_error_db: f64,
    pub tolerance_db: f64,
    pub max_correction_db: f64,
    pub within_tolerance: bool,
}

pub(super) struct BlindComparisonSnapshot {
    pub theme: crate::theme::Theme,
    pub translations: crate::app::i18n::ListeningTestTranslations,
    pub has_session: bool,
    pub trial_ready: bool,
    pub paths_ready: bool,
    pub pending_mode: Option<TrialMode>,
    pub trial_count: usize,
    pub score: Option<(usize, usize)>,
    pub segment_start_ms: u64,
    pub level_match: sotf_audio_player::controllers::ab_test_session::LevelMatchConfig,
    pub status: String,
    pub prepared_measurement: Option<PreparedMeasurementSnapshot>,
}

impl BlindComparisonSnapshot {
    pub fn capture(state: &crate::app::AppState) -> Self {
        let listening = &state.app.plugin_state.listening_test_state;
        let session = listening.ab_test.session();
        let prepared_measurement = session.map(|session| {
            let measurement = &session.setup.level_match;
            PreparedMeasurementSnapshot {
                media_path: session
                    .setup
                    .media
                    .media_path
                    .clone()
                    .unwrap_or_else(|| session.setup.media.media_id.clone()),
                media_id: session.setup.media.media_id.clone(),
                start_ms: session.setup.media.start_ms,
                duration_ms: session.setup.media.duration_ms,
                metric: measurement.metric,
                correction_b_db: measurement.correction_b_db,
                residual_error_db: measurement.residual_error_db(),
                tolerance_db: measurement.tolerance_db,
                max_correction_db: measurement.max_correction_db,
                within_tolerance: measurement.within_tolerance(),
            }
        });

        Self {
            theme: state.app.ui_state.theme.clone(),
            translations: state.app.ui_state.translations.listening_test.clone(),
            has_session: session.is_some(),
            trial_ready: session
                .is_some_and(|session| session.setup.level_match.within_tolerance()),
            paths_ready: listening.path_a.is_some() && listening.path_b.is_some(),
            pending_mode: session.and_then(|session| session.pending_mode()),
            trial_count: session.map_or(0, |session| session.trials.len()),
            score: session.map(|session| session.abx_score()),
            segment_start_ms: listening.segment_start_ms,
            level_match: listening.level_match_config,
            status: listening.status.clone(),
            prepared_measurement,
        }
    }
}

impl PlayerView {
    pub(super) fn use_current_listening_position(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let start_ms = (state.app.playback.position_secs.max(0.0) * 1_000.0).round() as u64;
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.segment_start_ms = start_ms;
            let _ = listening.ab_test.clear_session();
        });
        cx.notify();
    }

    pub(super) fn load_listening_session_media(&mut self, cx: &mut Context<Self>) {
        let request = {
            let state = self.state.read(cx);
            state
                .app
                .plugin_state
                .listening_test_state
                .ab_test
                .session()
                .map(|session| {
                    (
                        session.setup.media.clone(),
                        session.setup.media.start_ms as f64 / 1_000.0,
                        session.setup.channels,
                        session.setup.sample_rate,
                    )
                })
        };
        let Some((media, position, channels, sample_rate)) = request else {
            return;
        };
        let weak_state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { verify_media_segment(&media) })
                .await;
            let Some(entity) = weak_state.upgrade() else {
                return;
            };
            entity.update(&mut cx.clone(), |state, cx| {
                let text = state
                    .app
                    .ui_state
                    .translations
                    .listening_test
                    .setup
                    .level
                    .clone();
                match result {
                    Ok(path) => {
                        if Self::play_listening_source_at(
                            state,
                            sotf_audio::decoder::AudioSource::from(path),
                            position,
                            channels,
                            sample_rate,
                        ) {
                            state.app.plugin_state.listening_test_state.status =
                                text.media_loaded.into();
                        }
                    }
                    Err(AbTestError::MediaIdentityMismatch) => {
                        state.app.plugin_state.listening_test_state.status =
                            text.media_identity_mismatch.into();
                    }
                    Err(error) => {
                        state.app.plugin_state.listening_test_state.status =
                            format!("{}: {error}", text.media_unavailable);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn listening_test_file_button(
        &self,
        id: &'static str,
        label: &'static str,
        load: bool,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let button = Button::new(id, label)
            .disabled(disabled)
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |view, _, _, cx| {
                view.pick_listening_session_file(load, cx);
            }));
        #[cfg(feature = "dev-api")]
        let button = button.dev_track_with_state(
            format!("listening.session.{id}"),
            crate::app::dev_api::DevElementState::default().enabled(!disabled),
        );
        button
    }

    pub(super) fn capture_listening_path(
        &mut self,
        target: ListeningPathTarget,
        cx: &mut Context<Self>,
    ) {
        if self.comparison_setup_locked(cx) {
            return;
        }
        self.state.update(cx, |state, _| {
            let sample_rate = f64::from(state.app.audio_device_state.hal_config.sample_rate);
            let result = path_config_from_plugin_graph(&state.app.plugin_state.graph, sample_rate);
            let captured = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .captured;
            let current_chain = state
                .app
                .ui_state
                .translations
                .listening_test
                .setup
                .current_chain;
            let listening = &mut state.app.plugin_state.listening_test_state;
            match result {
                Ok(config) => {
                    match target {
                        ListeningPathTarget::A => {
                            listening.path_a = Some(config);
                            listening.path_a_label = format!("{current_chain} A");
                        }
                        ListeningPathTarget::B => {
                            listening.path_b = Some(config);
                            listening.path_b_label = format!("{current_chain} B");
                        }
                    }
                    clear_listening_canvas(listening, target);
                    let _ = listening.ab_test.clear_session();
                    listening.status = captured.into();
                }
                Err(error) => listening.status = error,
            }
        });
        cx.notify();
    }

    pub(super) fn start_listening_trial(
        &mut self,
        mode: TrialMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.state.read(cx);
        let workspace = &state.app.plugin_state.plugin_ui_state.listening_workspace;
        if workspace.results_open
            || state
                .app
                .plugin_state
                .listening_test_state
                .ab_test
                .view()
                .completed_trials
                >= workspace.planned_trials
        {
            return;
        }
        self.state.update(cx, |state, _| {
            let load_or_prepare = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .load_or_prepare;
            let trial_started = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .trial_started;
            let result = state.app.plugin_state.start_ab_test_trial(mode);
            if result.is_ok() {
                let workspace = &mut state.app.plugin_state.plugin_ui_state.listening_workspace;
                workspace.results_open = false;
                workspace.selected_answer = None;
                workspace.auditioned = false;
                workspace.paused = false;
                state.app.plugin_state.listening_test_state.trial_mode = mode;
            }
            let listening = &mut state.app.plugin_state.listening_test_state;
            match result {
                Ok(index) => listening.status = format!("{trial_started} · #{}", index + 1),
                Err(AbTestError::InvalidSetup) => listening.status = load_or_prepare.into(),
                Err(error) => listening.status = error.to_string(),
            }
        });
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn cancel_listening_preparation(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .cancel_preparation();
            state.app.plugin_state.listening_test_state.status = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .load_or_prepare
                .into();
            cx.notify();
        });
        cx.notify();
    }

    pub(super) fn prepare_current_listening_session(&mut self, cx: &mut Context<Self>) {
        if self.comparison_setup_locked(cx) {
            return;
        }
        let request_data = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            match (
                listening.path_a.clone(),
                listening.path_b.clone(),
                state.app.get_current_track_path(),
            ) {
                (Some(path_a), Some(path_b), Some(media_path)) => Some((
                    path_a,
                    path_b,
                    listening.path_a_label.clone(),
                    listening.path_b_label.clone(),
                    media_path,
                    listening.segment_start_ms,
                    listening.level_match_config,
                )),
                _ => None,
            }
        };
        let Some((path_a, path_b, label_a, label_b, media_path, start_ms, level_match)) =
            request_data
        else {
            self.state.update(cx, |state, _| {
                state.app.plugin_state.listening_test_state.status = state
                    .app
                    .ui_state
                    .translations
                    .listening_test
                    .status
                    .select_paths_and_track
                    .into();
            });
            cx.notify();
            return;
        };
        let expected_inputs = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .preparation_snapshot(Some(media_path.clone()));
        let Some(expected_inputs) = expected_inputs else {
            return;
        };
        let request = self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            let _ = listening.ab_test.clear_session();
            listening.status = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .measuring
                .into();
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .begin_preparation()
        });
        cx.notify();
        let weak_state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let session_id = format!("sotf-listening-{timestamp}");
            let assignment_seed = timestamp as u64;
            let result = cx
                .background_executor()
                .spawn(async move {
                    prepare_ab_test_session(AbTestSessionPreparationRequest {
                        session_id: &session_id,
                        assignment_seed,
                        path_a_label: &label_a,
                        path_b_label: &label_b,
                        path_a: &path_a,
                        path_b: &path_b,
                        media_path: &media_path,
                        start_ms,
                        level_match,
                        block_frames: 1_024,
                        switch_transition_ms: 20.0,
                        participant_id: None,
                        app_version: env!("CARGO_PKG_VERSION"),
                    })
                })
                .await;
            let Some(entity) = weak_state.upgrade() else {
                return;
            };
            entity.update(&mut cx.clone(), |state, cx| {
                if !state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .listening_workspace
                    .finish_preparation(&request)
                {
                    return;
                }
                let listening = &state.app.plugin_state.listening_test_state;
                let view = listening.ab_test.view();
                let unlocked = !view.runtime_active && view.completed_trials == 0;
                let visible = state.app.ui_state.current_screen
                    == crate::app::Screen::ListeningTest
                    && listening.surface == EarTrainingSurface::BlindComparison;
                let current_inputs =
                    listening.preparation_snapshot(state.app.get_current_track_path());
                if !visible || !unlocked || current_inputs.as_ref() != Some(&expected_inputs) {
                    if visible && unlocked {
                        state.app.plugin_state.listening_test_state.status = state
                            .app
                            .ui_state
                            .translations
                            .listening_test
                            .status
                            .load_or_prepare
                            .into();
                    }
                    cx.notify();
                    return;
                }

                let prepared_label = state
                    .app
                    .ui_state
                    .translations
                    .listening_test
                    .status
                    .prepared;
                let listening = &mut state.app.plugin_state.listening_test_state;
                match result {
                    Ok((session, preparation)) => {
                        let correction = preparation.measurement.correction_b_db;
                        match listening.ab_test.replace_session(session) {
                            Ok(()) => {
                                listening.status =
                                    format!("{prepared_label}: {correction:+.2} dB.");
                            }
                            Err(error) => listening.status = error.to_string(),
                        }
                    }
                    Err(error) => listening.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn activate_listening_cue(&mut self, cue: TrialCue, cx: &mut Context<Self>) {
        if self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .interaction_locked()
        {
            return;
        }
        self.state.update(cx, |state, _| {
            let localized = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .clone();
            let result = state
                .app
                .plugin_state
                .activate_ab_test_cue(cue)
                .map_err(|error| error.to_string());
            if result.is_ok() {
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .listening_workspace
                    .auditioned = true;
            }
            state.app.plugin_state.listening_test_state.status = match result {
                Ok(()) => localized.cue_active.into(),
                Err(error) => error,
            };
        });
        cx.notify();
    }

    pub(super) fn commit_listening_answer(&mut self, answer: TrialAnswer, cx: &mut Context<Self>) {
        let workspace = &self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace;
        if !workspace.auditioned || workspace.interaction_locked() {
            return;
        }
        self.state.update(cx, |state, _| {
            let answer_committed = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .answer_committed;
            let (confidence, notes) = {
                let listening = &state.app.plugin_state.listening_test_state;
                (listening.confidence, listening.notes.clone())
            };
            let result =
                state
                    .app
                    .plugin_state
                    .commit_ab_test_answer(answer, confidence, Some(notes));
            match result {
                Ok(_) => {
                    state
                        .app
                        .plugin_state
                        .plugin_ui_state
                        .listening_workspace
                        .selected_answer = None;
                    let completed_trials = state
                        .app
                        .plugin_state
                        .listening_test_state
                        .ab_test
                        .view()
                        .completed_trials;
                    {
                        let listening = &mut state.app.plugin_state.listening_test_state;
                        listening.notes.clear();
                        listening.confidence = None;
                        listening.status = answer_committed.into();
                    }
                    let tutorial = &mut state.app.tutorial;
                    if completed_trials >= tutorial.listening_break_interval
                        && completed_trials % tutorial.listening_break_interval == 0
                        && completed_trials != tutorial.listening_break_dismissed_at
                    {
                        tutorial.listening_break_prompt_open = true;
                    }
                }
                Err(error) => {
                    state.app.plugin_state.listening_test_state.status = error.to_string();
                }
            }
        });
        cx.notify();
    }

    pub(super) fn load_listening_path(
        &mut self,
        target: ListeningPathTarget,
        cx: &mut Context<Self>,
    ) {
        let weak_state = self.state.downgrade();
        let path_filter = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .listening_test
            .setup
            .path_json_filter;
        cx.spawn(async move |_, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter(path_filter, &["json"])
                .pick_file()
                .await;
            let Some(file) = file else { return };
            let path = file.path().to_path_buf();
            let result = std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<PathConfig>(&json).map_err(|error| error.to_string())
                });
            let Some(entity) = weak_state.upgrade() else {
                return;
            };
            entity.update(&mut cx.clone(), |state, cx| {
                let path_loaded = state
                    .app
                    .ui_state
                    .translations
                    .listening_test
                    .status
                    .path_loaded;
                let listening = &mut state.app.plugin_state.listening_test_state;
                match result {
                    Ok(config) => {
                        let config = simplify_linear_path_config(config);
                        let label = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(path_filter)
                            .to_owned();
                        match target {
                            ListeningPathTarget::A => {
                                listening.path_a = Some(config);
                                listening.path_a_label = label;
                            }
                            ListeningPathTarget::B => {
                                listening.path_b = Some(config);
                                listening.path_b_label = label;
                            }
                        }
                        clear_listening_canvas(listening, target);
                        let _ = listening.ab_test.clear_session();
                        listening.status = path_loaded.into();
                    }
                    Err(error) => listening.status = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn pick_listening_session_file(&mut self, load: bool, cx: &mut Context<Self>) {
        let weak_state = self.state.downgrade();
        let session_filter = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .listening_test
            .setup
            .session_filter;
        let mut session = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .ab_test
            .session()
            .cloned();
        if let Some(session) = session.as_mut() {
            session.planned_trials = u32::try_from(
                self.state
                    .read(cx)
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .listening_workspace
                    .planned_trials,
            )
            .ok();
        }
        #[cfg(feature = "dev-api")]
        let qa_path = std::env::var_os("SOTF_QA_DIR")
            .map(std::path::PathBuf::from)
            .map(|directory| directory.join("listening-session.json"));
        #[cfg(not(feature = "dev-api"))]
        let qa_path: Option<std::path::PathBuf> = None;
        cx.spawn(async move |_, cx| {
            let path = if let Some(path) = qa_path {
                Some(path)
            } else if load {
                rfd::AsyncFileDialog::new()
                    .add_filter(session_filter, &["json"])
                    .pick_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            } else {
                rfd::AsyncFileDialog::new()
                    .set_file_name("sotf-listening-session.json")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            };
            let Some(path) = path else { return };
            let result = if load {
                load_ab_test_session(&path).map(Some)
            } else if let Some(session) = session.as_ref() {
                save_ab_test_session(session, &path).map(|_| None)
            } else {
                return;
            };
            let Some(entity) = weak_state.upgrade() else {
                return;
            };
            entity.update(&mut cx.clone(), |state, cx| {
                let localized = state
                    .app
                    .ui_state
                    .translations
                    .listening_test
                    .status
                    .clone();
                let listening = &mut state.app.plugin_state.listening_test_state;
                match result {
                    Ok(Some(session)) => {
                        let setup = session.setup.clone();
                        let planned_trials = session.planned_trials.map(|count| count as usize);
                        match listening.ab_test.replace_session(session) {
                            Ok(()) => {
                                // A running comparison can reject replacement. Publish
                                // its setup only after the controller accepts the session.
                                listening.path_a = Some(setup.path_a.config);
                                listening.path_b = Some(setup.path_b.config);
                                listening.path_a_label = setup.path_a.label;
                                listening.path_b_label = setup.path_b.label;
                                listening.path_a_canvas = None;
                                listening.path_b_canvas = None;
                                listening.level_match_config = setup.level_match.config();
                                listening.segment_start_ms = setup.media.start_ms;
                                listening.status = localized.session_loaded.into();
                                state.app.plugin_state.plugin_ui_state.listening_workspace =
                                    crate::app::state::plugin::ListeningWorkspaceState::default();
                                if let Some(planned_trials) = planned_trials {
                                    state
                                        .app
                                        .plugin_state
                                        .plugin_ui_state
                                        .listening_workspace
                                        .planned_trials = planned_trials;
                                }
                            }
                            Err(error) => listening.status = error.to_string(),
                        }
                    }
                    Ok(None) => listening.status = localized.session_saved.into(),
                    Err(error) => listening.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn is_listening_test_active(&self, cx: &Context<Self>) -> bool {
        self.state.read(cx).app.ui_state.current_screen == crate::app::Screen::ListeningTest
    }

    pub(crate) fn listening_capture_path_a(
        &mut self,
        _: &ListeningCapturePathA,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.capture_listening_path(ListeningPathTarget::A, cx);
        }
    }

    pub(crate) fn listening_capture_path_b(
        &mut self,
        _: &ListeningCapturePathB,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.capture_listening_path(ListeningPathTarget::B, cx);
        }
    }

    pub(crate) fn listening_prepare(
        &mut self,
        _: &ListeningPrepare,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.prepare_current_listening_session(cx);
        }
    }

    pub(crate) fn listening_start_blind_ab(
        &mut self,
        _: &ListeningStartBlindAb,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.start_listening_trial(TrialMode::BlindAb, window, cx);
        }
    }

    pub(crate) fn listening_start_abx(
        &mut self,
        _: &ListeningStartAbx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.start_listening_trial(TrialMode::Abx, window, cx);
        }
    }

    pub(super) fn play_listening_cue_position(&mut self, position: usize, cx: &mut Context<Self>) {
        if !self.is_listening_test_active(cx) {
            return;
        }
        let mode = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .ab_test
            .session()
            .and_then(|session| session.pending_mode());
        if let Some(cue) = listening_cue_for_position(mode, position) {
            self.activate_listening_cue(cue, cx);
        }
    }

    pub(crate) fn listening_play_cue_1(
        &mut self,
        _: &ListeningPlayCue1,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.play_listening_cue_position(0, cx);
    }

    pub(crate) fn listening_play_cue_2(
        &mut self,
        _: &ListeningPlayCue2,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.play_listening_cue_position(1, cx);
    }

    pub(crate) fn listening_play_cue_3(
        &mut self,
        _: &ListeningPlayCue3,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.play_listening_cue_position(2, cx);
    }

    pub(super) fn commit_listening_answer_position(
        &mut self,
        position: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.is_listening_test_active(cx) {
            return;
        }
        let mode = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .ab_test
            .session()
            .and_then(|session| session.pending_mode());
        if let Some(answer) = listening_answer_for_position(mode, position) {
            self.commit_listening_answer(answer, cx);
        }
    }

    pub(crate) fn listening_commit_answer_1(
        &mut self,
        _: &ListeningCommitAnswer1,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_listening_answer_position(0, cx);
    }

    pub(crate) fn listening_commit_answer_2(
        &mut self,
        _: &ListeningCommitAnswer2,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_listening_answer_position(1, cx);
    }
}
