use super::*;

impl PlayerView {
    pub(super) fn listening_disclosure_shortcut(
        &self,
        action: &dyn Action,
        cx: &Context<Self>,
    ) -> String {
        let state = self.state.read(cx);
        crate::app::keybindings::get_documented_keybindings_for_screen_with_overrides(
            crate::app::Screen::ListeningTest,
            state.app.ui_state.keymap_preset,
            &state.app.settings.keybindings.overrides,
        )
        .into_iter()
        .find(|binding| binding.action_name == Some(action.name()))
        .map_or_else(String::new, |binding| binding.key)
    }

    pub(crate) fn listening_toggle_metadata(
        &mut self,
        _: &crate::app::actions::ListeningToggleMetadata,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_listening_test_active(cx) {
            return;
        }
        self.state.update(cx, |state, _| {
            let plugins = &mut state.app.plugin_state;
            let workspace = &mut plugins.plugin_ui_state.listening_workspace;
            if !workspace.interaction_locked()
                && plugins
                    .listening_test_state
                    .ab_test
                    .session()
                    .is_some_and(|session| session.pending_mode().is_some())
            {
                workspace.metadata_open = !workspace.metadata_open;
            }
        });
        cx.notify();
    }

    pub(crate) fn listening_toggle_trial_details(
        &mut self,
        _: &crate::app::actions::ListeningToggleTrialDetails,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_listening_test_active(cx) {
            return;
        }
        self.state.update(cx, |state, _| {
            let workspace = &mut state.app.plugin_state.plugin_ui_state.listening_workspace;
            if workspace.results_open && !workspace.confirm_end {
                workspace.trials_open = !workspace.trials_open;
            }
        });
        cx.notify();
    }

    pub(super) fn comparison_setup_locked(&self, cx: &Context<Self>) -> bool {
        let state = self.state.read(cx);
        let view = state.app.plugin_state.listening_test_state.ab_test.view();
        view.runtime_active || view.completed_trials > 0
    }

    pub(super) fn render_comparison_plan(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let labels = state.app.ui_state.translations.listening_test.workspace();
        let planned = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .planned_trials;
        let source = state
            .app
            .get_current_track_path()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| {
                state
                    .app
                    .ui_state
                    .translations
                    .listening_test
                    .eq
                    .no_track
                    .into()
            });
        let entity = self.state.clone();
        let d = Ds::from_cx(cx);
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(d.gap)
            .child(Text::label(labels.planned_trials))
            .child(dev_track!(
                NumberInput::new("comparison-planned-trials")
                    .value(planned as f64)
                    .range(1.0, 200.0)
                    .step(1.0)
                    .decimals(0)
                    .size(NumberInputSize::Sm)
                    .width(110.0)
                    .aria_label(labels.planned_trials)
                    .on_change(move |value, _, cx| {
                        if value.is_finite() {
                            entity.update(cx, |state, cx| {
                                state
                                    .app
                                    .plugin_state
                                    .plugin_ui_state
                                    .listening_workspace
                                    .planned_trials = value.round().clamp(1.0, 200.0) as usize;
                                cx.notify();
                            });
                        }
                    }),
                "listening.planned-trials"
            ))
            .child(Text::caption(source))
    }

    pub(super) fn render_comparison_workspace(
        &self,
        phase: ListeningWorkspacePhase,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = state.app.ui_state.translations.listening_test.clone();
        let labels = text.workspace();
        let listening = &state.app.plugin_state.listening_test_state;
        let session = listening.ab_test.session();
        let pending = session.and_then(|session| session.pending_mode());
        let completed = session.map_or(0, |session| session.trials.len());
        let mode = session
            .and_then(|session| session.trials.last())
            .map_or(listening.trial_mode, |trial| trial.mode);
        let planned = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .planned_trials;
        let source = session
            .and_then(|session| session.setup.media.media_path.as_deref())
            .and_then(|path| std::path::Path::new(path).file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let score = session.map(|session| session.abx_score());
        let status = listening.status.clone();
        let paused = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .paused;
        let d = Ds::from_cx(cx);
        let results = phase == ListeningWorkspacePhase::Results;
        let mut body = div()
            .id("comparison-workspace")
            .size_full()
            .min_w_0()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap)
                    .child(Text::caption(format!("1 · {}", labels.setup)).color(theme.text_muted))
                    .child(
                        Text::label(format!("2 · {}", labels.listen)).color(if results {
                            theme.text_muted
                        } else {
                            theme.accent
                        }),
                    )
                    .child(
                        Text::label(format!("3 · {}", labels.results)).color(if results {
                            theme.accent
                        } else {
                            theme.text_muted
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(d.gap)
                    .child(Text::section_header(if results {
                        labels.results
                    } else {
                        labels.listen
                    }))
                    .child(Text::caption(format!(
                        "{} / {} {}",
                        completed + usize::from(pending.is_some()),
                        planned,
                        text.trial.trials
                    ))),
            )
            .child(div().min_w_0().text_size(d.text_sm).child(source));
        if state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .confirm_end
        {
            let [title, explanation, keep, finish] = text.end_confirmation();
            return body
                .child(Text::section_header(title))
                .child(Text::body(explanation))
                .child(Text::caption(format!("{}: {completed}", text.trial.trials)))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(dev_track!(
                            Button::new("comparison-keep-listening", keep)
                                .variant(ButtonVariant::Primary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, window, cx| {
                                    view.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .plugin_state
                                            .plugin_ui_state
                                            .listening_workspace
                                            .confirm_end = false;
                                    });
                                    view.focus_handle.focus(window, cx);
                                    cx.notify();
                                })),
                            "listening.keep-listening"
                        ))
                        .child(dev_track!(
                            Button::new("comparison-confirm-end", finish)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, window, cx| {
                                    view.end_comparison_workspace(cx);
                                    view.focus_handle.focus(window, cx);
                                })),
                            "listening.confirm-end"
                        )),
                )
                .child(Text::caption(status).color(theme.text_secondary))
                .into_any_element();
        }

        if results {
            if completed < planned {
                body = body.child(Text::caption(text.session_ended_early()));
            }
            if completed == 0 {
                body = body.child(dev_track!(
                    div().child(Text::body(text.no_answers_submitted())),
                    "listening.empty-results"
                ));
            }
            if let Some((correct, total)) = score.filter(|(_, total)| *total > 0) {
                body = body.child(Text::section_header(format!("ABX · {correct} / {total}")));
                body = body.child(Text::caption(text.result_explanation(false)));
            }
            // Trial records reveal identities only in this explicit results surface.
            if let Some(session) = session.filter(|session| !session.trials.is_empty()) {
                let (preferred_a, preferred_b) = session.preference_counts();
                if preferred_a + preferred_b > 0 {
                    body = body.child(dev_track!(
                        div()
                            .child(Text::section_header(format!(
                                "A: {preferred_a} · B: {preferred_b}"
                            )))
                            .child(Text::caption(text.result_explanation(true))),
                        "listening.preference-totals"
                    ));
                }
                body = body.child(Text::caption(format!(
                    "A: {} ↔ B: {}",
                    session.setup.path_a.label, session.setup.path_b.label
                )));
                let mut trials = div().flex().flex_col().gap(d.gap);
                for trial in &session.trials {
                    use sotf_audio_player::controllers::ab_test_session::{
                        PathSelection, TrialResult,
                    };
                    let answer = match trial.answer {
                        TrialAnswer::A => "A",
                        TrialAnswer::B => "B",
                        TrialAnswer::First => "1",
                        TrialAnswer::Second => "2",
                    };
                    let result = match &trial.result {
                        TrialResult::Correct => format!("✓ · X = {answer}"),
                        TrialResult::Incorrect => format!(
                            "× · X = {}",
                            if trial.answer == TrialAnswer::A {
                                "B"
                            } else {
                                "A"
                            }
                        ),
                        TrialResult::Preference(path) => match path {
                            PathSelection::A => session.setup.path_a.label.clone(),
                            PathSelection::B => session.setup.path_b.label.clone(),
                        },
                    };
                    trials = trials.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(d.gap)
                            .child(Text::label(format!("{}", trial.index + 1)))
                            .child(Text::body(format!("{answer} · {result}")))
                            .when_some(trial.confidence, |row, confidence| {
                                row.child(Text::caption(format!(
                                    "{}: {confidence}%",
                                    text.trial.confidence
                                )))
                            })
                            .when_some(trial.notes.clone(), |row, notes| {
                                row.child(Text::caption(notes))
                            }),
                    );
                }
                let title = text.disclosures()[1];
                let expanded = state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .listening_workspace
                    .trials_open;
                let owner = cx.entity().downgrade();
                body = body.child(dev_track!(
                    Accordion::new()
                        .aria_label(title)
                        .bordered(false)
                        .expanded(if expanded {
                            vec!["listening-trials".into()]
                        } else {
                            vec![]
                        })
                        .item(
                            AccordionItem::new("listening-trials", title)
                                .trailing(self.listening_disclosure_shortcut(
                                    &crate::app::actions::ListeningToggleTrialDetails,
                                    cx
                                ))
                                .content(dev_track!(trials, "listening.trial-records"))
                        )
                        .on_change(move |_, expanded, _, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                view.state.update(cx, |state, _| {
                                    state
                                        .app
                                        .plugin_state
                                        .plugin_ui_state
                                        .listening_workspace
                                        .trials_open = expanded;
                                });
                                cx.notify();
                            });
                        }),
                    "listening.trials"
                ));
            }
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap)
                    .child(self.listening_test_file_button(
                        "comparison-save-results",
                        text.setup.save_session,
                        false,
                        completed == 0,
                        cx,
                    ))
                    .child(
                        Button::new("comparison-new", labels.new_comparison)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Primary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.reset_comparison_workspace(cx)),
                            ),
                    ),
            );
        } else {
            body = body.child(Text::caption(labels.locked).color(theme.text_secondary));
            if let Some(mode) = pending {
                if !paused {
                    body = body
                        .child(self.render_listening_cues(mode, cx))
                        .child(Text::caption(labels.answers_hidden))
                        .child(self.render_listening_answers(mode, cx))
                        .child(self.render_listening_trial_metadata(cx));
                }
                body = body.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(
                            Button::new("comparison-pause", text.pause_resume(paused))
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(
                                    cx.listener(|view, _, _, cx| view.toggle_comparison_pause(cx)),
                                ),
                        )
                        .child(dev_track!(
                            Button::new("comparison-end", text.end_session())
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, window, cx| {
                                    view.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .plugin_state
                                            .plugin_ui_state
                                            .listening_workspace
                                            .confirm_end = true;
                                    });
                                    view.focus_handle.focus(window, cx);
                                    cx.notify();
                                }),),
                            "listening.end"
                        )),
                );
            } else {
                body = body.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .when(completed < planned, |row| {
                            row.child(self.listening_trial_button(
                                "comparison-next",
                                labels.next_trial,
                                mode,
                                cx,
                            ))
                        })
                        .child(dev_track!(
                            Button::new("comparison-results", labels.view_results)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Primary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.state.update(cx, |state, cx| {
                                        match state.app.plugin_state.leave_ab_test_runtime() {
                                            Ok(()) => {
                                                state
                                                    .app
                                                    .plugin_state
                                                    .plugin_ui_state
                                                    .listening_workspace
                                                    .results_open = true
                                            }
                                            Err(error) => {
                                                state.app.plugin_state.listening_test_state.status =
                                                    error.to_string()
                                            }
                                        }
                                        cx.notify();
                                    });
                                })),
                            "listening.view-results"
                        )),
                );
            }
        }
        dev_track!(
            body.child(Text::caption(status).color(theme.text_secondary)),
            "listening.workspace.content"
        )
        .into_any_element()
    }

    fn reset_comparison_workspace(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let plugins = &mut state.app.plugin_state;
            let result = plugins
                .leave_ab_test_runtime()
                .and_then(|()| plugins.listening_test_state.ab_test.clear_session());
            match result {
                Ok(()) => {
                    let planned_trials = plugins.plugin_ui_state.listening_workspace.planned_trials;
                    plugins.plugin_ui_state.listening_workspace =
                        crate::app::state::plugin::ListeningWorkspaceState {
                            planned_trials,
                            ..Default::default()
                        };
                    plugins.listening_test_state.status.clear();
                }
                Err(error) => plugins.listening_test_state.status = error.to_string(),
            }
            cx.notify();
        });
    }

    fn toggle_comparison_pause(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let paused = state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .paused;
            let result = if paused {
                state.player.resume()
            } else {
                state.player.pause()
            };
            match result {
                Ok(()) => {
                    state
                        .app
                        .plugin_state
                        .plugin_ui_state
                        .listening_workspace
                        .paused = !paused;
                    state.app.playback.is_playing = paused;
                    if paused {
                        state.app.record_playback_resumed();
                    } else {
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

    fn end_comparison_workspace(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let plugins = &mut state.app.plugin_state;
            if !plugins.plugin_ui_state.listening_workspace.confirm_end {
                return;
            }
            match plugins.leave_ab_test_runtime() {
                Ok(()) => {
                    if let Some(session) = plugins.listening_test_state.ab_test.session_mut() {
                        session.cancel_pending_trial();
                    }
                    let workspace = &mut plugins.plugin_ui_state.listening_workspace;
                    workspace.results_open = true;
                    workspace.confirm_end = false;
                    workspace.selected_answer = None;
                    workspace.paused = false;
                    plugins.listening_test_state.status.clear();
                }
                Err(error) => plugins.listening_test_state.status = error.to_string(),
            }
            cx.notify();
        });
    }

    pub(super) fn submit_comparison_answer(&mut self, mode: TrialMode, cx: &mut Context<Self>) {
        let workspace = &self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace;
        if !workspace.can_submit(mode) {
            return;
        }
        if let Some(answer) = workspace.selected_answer {
            self.commit_listening_answer(answer, cx);
        }
    }
}
