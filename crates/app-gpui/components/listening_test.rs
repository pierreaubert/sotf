//! Embedded, reproducible chain-level A/B and ABX listening tests.

use crate::app::actions::{
    EarTrainingNextQuestion, EarTrainingPlayFiltered, EarTrainingPlayOriginal,
    EarTrainingSelectNextBand, EarTrainingSelectPreviousBand, EarTrainingShowBlindComparison,
    EarTrainingShowEqBands, EarTrainingStart, EarTrainingSubmit, ListeningCapturePathA,
    ListeningCapturePathB, ListeningCommitAnswer1, ListeningCommitAnswer2, ListeningPlayCue1,
    ListeningPlayCue2, ListeningPlayCue3, ListeningPrepare, ListeningStartAbx,
    ListeningStartBlindAb,
};
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::state::plugin::{ABPathTarget, EarTrainingSurface};
use crate::components::design::Ds;
use crate::components::graphs::response_graphs::{
    ChartConfig, Series, channel_color, render_line_chart,
};
use crate::components::plugins::editing::PluginEditingManager;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Input, InputSize, NumberInput, NumberInputSize, Text,
    TextWeight,
};
use sotf_audio::plugins::PluginType;
use sotf_audio_player::controllers::ab_compare_path::{
    GraphEdgeConfig, GraphNodeConfig, PathConfig, PluginInRack, allowed_plugin_types,
    path_config_from_plugin_graph, simplify_linear_path_config,
};
use sotf_audio_player::controllers::ab_test_execution::{
    AbTestSessionPreparationRequest, load_ab_test_session, prepare_ab_test_session,
    save_ab_test_session, verify_media_segment,
};
use sotf_audio_player::controllers::ab_test_session::{
    AbTestError, LevelMatchMetric, TrialAnswer, TrialCue, TrialMode,
};
use sotf_audio_player::{EarTrainingCourse, EqChangeMode, EqTrainingExercise, EqTrainingSession};

#[derive(Clone, Copy)]
enum ListeningPathTarget {
    A,
    B,
}

#[derive(Clone, Copy)]
#[repr(usize)]
enum EqConfigField {
    Bands,
    Gain,
    Q,
    Trials,
}

impl PlayerView {
    pub(crate) fn render_listening_test_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, eq_text, surface) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.listening_test.eq.clone(),
                state.app.plugin_state.listening_test_state.surface,
            )
        };

        let body = match surface {
            EarTrainingSurface::EqBands => self.render_eq_training_workbench(cx),
            EarTrainingSurface::Courses => self.render_eq_courses(cx),
            EarTrainingSurface::Progress => self.render_eq_progress(cx),
            EarTrainingSurface::BlindComparison => self.render_blind_comparison_screen(cx),
        };

        div()
            .id("ear-training-screen")
            .size_full()
            .bg(theme.background)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .p(d.pad_y)
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(
                        Button::new("ear-training-eq-mode", eq_text.mode_eq)
                            .size(ButtonSize::Sm)
                            .variant(if surface == EarTrainingSurface::EqBands {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.set_ear_training_surface(EarTrainingSurface::EqBands, cx);
                            })),
                    )
                    .child(
                        Button::new("ear-training-courses", eq_text.courses)
                            .size(ButtonSize::Sm)
                            .variant(if surface == EarTrainingSurface::Courses {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.set_ear_training_surface(EarTrainingSurface::Courses, cx)
                            })),
                    )
                    .child(
                        Button::new("ear-training-progress", eq_text.progress)
                            .size(ButtonSize::Sm)
                            .variant(if surface == EarTrainingSurface::Progress {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.set_ear_training_surface(EarTrainingSurface::Progress, cx)
                            })),
                    )
                    .child(
                        Button::new("ear-training-blind-mode", eq_text.mode_blind)
                            .size(ButtonSize::Sm)
                            .variant(if surface == EarTrainingSurface::BlindComparison {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.set_ear_training_surface(
                                    EarTrainingSurface::BlindComparison,
                                    cx,
                                );
                            })),
                    )
                    .child(Text::caption(eq_text.suite_subtitle).color(theme.text_secondary)),
            )
            .child(div().flex_1().min_h_0().child(body))
    }

    fn render_eq_courses(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let eq_text = &state.app.ui_state.translations.listening_test.eq;
        let progress = &state.app.plugin_state.listening_test_state.eq_progress;
        let mut courses = div().flex().flex_wrap().gap(d.section);
        for course in EarTrainingCourse::ALL {
            let config = course.config();
            let completed = progress
                .sessions
                .iter()
                .filter(|session| session.course == Some(course))
                .count();
            courses = courses.child(
                div()
                    .min_w(rems(16.))
                    .flex_1()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(Text::section_header(course.label()))
                    .child(Text::caption(format!(
                        "{} bands · {:+.0} dB · {} trials",
                        config.band_count, config.gain_db, config.trial_count
                    )))
                    .child(Text::caption(format!("{completed} sessions completed")))
                    .child(
                        Button::new(("ear-course-start", course as usize), eq_text.start_course)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Primary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(move |view, _, _, cx| view.start_eq_course(course, cx)),
                            ),
                    ),
            );
        }
        div()
            .id("eq-training-courses-screen")
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(Text::section_header(eq_text.guided_courses))
            .child(Text::caption(eq_text.guided_courses_subtitle))
            .child(courses)
            .into_any_element()
    }

    fn render_eq_progress(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let eq_text = &state.app.ui_state.translations.listening_test.eq;
        let progress = &state.app.plugin_state.listening_test_state.eq_progress;
        let recent = progress.sessions.iter().rev().take(8).fold(
            div().flex().flex_col().gap(d.grid),
            |list, session| {
                list.child(
                    div()
                        .flex()
                        .justify_between()
                        .child(Text::body(session.exercise.label()))
                        .child(Text::caption(format!(
                            "{}/{} · {:.0}%",
                            session.correct,
                            session.attempts,
                            session.accuracy * 100.0
                        ))),
                )
            },
        );
        div()
            .id("eq-training-progress-screen")
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(Text::section_header(eq_text.training_progress))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.section)
                    .child(
                        div()
                            .min_w(rems(10.))
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .child(Text::caption(format!(
                                "Sessions  {}",
                                progress.sessions.len()
                            ))),
                    )
                    .child(
                        div()
                            .min_w(rems(10.))
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .child(Text::caption(format!(
                                "Accuracy  {:.0}%",
                                progress.accuracy() * 100.0
                            ))),
                    )
                    .child(
                        div()
                            .min_w(rems(10.))
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .child(Text::caption(format!("70% streak  {}", progress.streak()))),
                    ),
            )
            .child(
                div()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(Text::section_header(eq_text.coach_recommendation))
                    .child(Text::body(progress.recommendation())),
            )
            .child(
                div()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(Text::section_header(eq_text.recent_sessions))
                    .child(recent),
            )
            .into_any_element()
    }

    fn render_blind_comparison_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_listening_path_canvas(ListeningPathTarget::A, cx);
        self.ensure_listening_path_canvas(ListeningPathTarget::B, cx);

        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.listening_test.clone();
        let listening = state.app.plugin_state.listening_test_state.clone();
        let has_session = listening.session.is_some();
        let trial_ready = listening
            .session
            .as_ref()
            .is_some_and(|session| session.setup.level_match.within_tolerance());
        let paths_ready = listening.path_a.is_some() && listening.path_b.is_some();
        let pending_mode = listening
            .session
            .as_ref()
            .and_then(|session| session.pending_mode());
        let trial_count = listening
            .session
            .as_ref()
            .map_or(0, |session| session.trials.len());
        let score = listening
            .session
            .as_ref()
            .map(|session| session.abx_score());
        let level_text = translations.setup.level.clone();
        let level_match = listening.level_match_config;
        let prepared_evidence = listening.session.as_ref().map(|session| {
            let measurement = &session.setup.level_match;
            let confidence = if measurement.within_tolerance() {
                level_text.within_tolerance
            } else {
                level_text.outside_tolerance
            };
            let confidence_color = if measurement.within_tolerance() {
                theme.success
            } else {
                theme.warning
            };
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .p(d.pad_y)
                .rounded(d.r_sm)
                .bg(theme.background_secondary)
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(d.text_xs)
                        .text_color(theme.text_secondary)
                        .child(format!(
                            "{}: {} · {:.2}–{:.2} {}",
                            level_text.media_segment,
                            session
                                .setup
                                .media
                                .media_path
                                .as_deref()
                                .unwrap_or(&session.setup.media.media_id),
                            session.setup.media.start_ms as f64 / 1_000.0,
                            (session.setup.media.start_ms + session.setup.media.duration_ms) as f64
                                / 1_000.0,
                            level_text.seconds_unit,
                        )),
                )
                .child(
                    Text::caption(format!(
                        "{}: {}",
                        level_text.media_identity, session.setup.media.media_id
                    ))
                    .color(theme.text_muted),
                )
                .child(
                    Text::caption(format!(
                        "{} · {}: {:+.2} dB · {}: {:.3} dB / {:.3} dB · {}: ±{:.2} dB",
                        level_text.metric_label(measurement.metric),
                        level_text.correction,
                        measurement.correction_b_db,
                        level_text.residual,
                        measurement.residual_error_db(),
                        measurement.tolerance_db,
                        level_text.max_correction,
                        measurement.max_correction_db,
                    ))
                    .color(theme.text_secondary),
                )
                .child(Text::label(confidence).color(confidence_color))
                .child(
                    Button::new("load-listening-media", level_text.load_saved_media)
                        .size(ButtonSize::Xs)
                        .variant(ButtonVariant::Secondary)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(|view, _, _, cx| {
                            view.load_listening_session_media(cx);
                        })),
                )
                .into_any_element()
        });
        div()
            .id("listening-test-screen")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .child(
                                Text::new(translations.setup.title)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::caption(translations.setup.subtitle)
                                    .color(theme.text_secondary),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(d.gap)
                            .child(self.listening_test_file_button(
                                "listening-load-session",
                                translations.setup.load_session,
                                true,
                                cx,
                            ))
                            .when(has_session, |row| {
                                row.child(self.listening_test_file_button(
                                    "listening-save-session",
                                    translations.setup.save_session,
                                    false,
                                    cx,
                                ))
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(d.section)
                    .child(self.render_listening_path_card(
                        ListeningPathTarget::A,
                        &listening.path_a_label,
                        listening.path_a.as_ref(),
                        cx,
                    ))
                    .child(self.render_listening_path_card(
                        ListeningPathTarget::B,
                        &listening.path_b_label,
                        listening.path_b.as_ref(),
                        cx,
                    )),
            )
            .child(
                div()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(
                                        Text::new(translations.setup.level_title)
                                            .weight(TextWeight::Semibold),
                                    )
                                    .child(Text::caption(translations.setup.level_description)),
                            )
                            .when(paths_ready, |row| {
                                row.child(
                                    Button::new(
                                        "prepare-listening-session",
                                        translations.setup.measure_prepare,
                                    )
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Primary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, _, cx| {
                                            view.prepare_current_listening_session(cx);
                                        }),
                                    ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .child(Text::caption(level_text.target_metric))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.grid)
                                    .child(self.listening_metric_button(
                                        "listening-metric-momentary",
                                        level_text.momentary_lufs,
                                        LevelMatchMetric::MomentaryLufs,
                                        level_match.metric,
                                        cx,
                                    ))
                                    .child(self.listening_metric_button(
                                        "listening-metric-short-term",
                                        level_text.short_term_lufs,
                                        LevelMatchMetric::ShortTermLufs,
                                        level_match.metric,
                                        cx,
                                    ))
                                    .child(self.listening_metric_button(
                                        "listening-metric-rms",
                                        level_text.rms_dbfs,
                                        LevelMatchMetric::Rms,
                                        level_match.metric,
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .gap(d.gap)
                            .child({
                                let state_entity = self.state.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(Text::caption(level_text.segment_start))
                                    .child(
                                        NumberInput::new("listening-segment-start")
                                            .value(listening.segment_start_ms as f64 / 1_000.0)
                                            .range(0.0, 86_400.0)
                                            .step(0.1)
                                            .decimals(2)
                                            .unit(level_text.seconds_unit)
                                            .aria_label(level_text.segment_start)
                                            .size(NumberInputSize::Sm)
                                            .width(140.0)
                                            .on_change(move |value, _window, cx| {
                                                state_entity.update(cx, |state, _| {
                                                    let listening = &mut state
                                                        .app
                                                        .plugin_state
                                                        .listening_test_state;
                                                    listening.segment_start_ms =
                                                        (value.max(0.0) * 1_000.0).round() as u64;
                                                    listening.session = None;
                                                });
                                            }),
                                    )
                            })
                            .child(
                                Button::new(
                                    "listening-use-current-position",
                                    level_text.use_current_position,
                                )
                                .size(ButtonSize::Xs)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.use_current_listening_position(cx);
                                })),
                            )
                            .child({
                                let state_entity = self.state.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(Text::caption(level_text.window))
                                    .child(
                                        NumberInput::new("listening-window")
                                            .value(level_match.window_ms as f64 / 1_000.0)
                                            .range(
                                                level_match.metric.minimum_window_ms() as f64
                                                    / 1_000.0,
                                                60.0,
                                            )
                                            .step(0.1)
                                            .decimals(2)
                                            .unit(level_text.seconds_unit)
                                            .aria_label(level_text.window)
                                            .size(NumberInputSize::Sm)
                                            .width(140.0)
                                            .on_change(move |value, _window, cx| {
                                                state_entity.update(cx, |state, _| {
                                                    let listening = &mut state
                                                        .app
                                                        .plugin_state
                                                        .listening_test_state;
                                                    listening.level_match_config.window_ms =
                                                        (value.max(0.001) * 1_000.0).round() as u64;
                                                    listening.session = None;
                                                });
                                            }),
                                    )
                            })
                            .child({
                                let state_entity = self.state.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(Text::caption(level_text.tolerance))
                                    .child(
                                        NumberInput::new("listening-tolerance")
                                            .value(level_match.tolerance_db)
                                            .range(0.0, 3.0)
                                            .step(0.01)
                                            .decimals(2)
                                            .unit("dB")
                                            .aria_label(level_text.tolerance)
                                            .size(NumberInputSize::Sm)
                                            .width(140.0)
                                            .on_change(move |value, _window, cx| {
                                                state_entity.update(cx, |state, _| {
                                                    let listening = &mut state
                                                        .app
                                                        .plugin_state
                                                        .listening_test_state;
                                                    listening.level_match_config.tolerance_db =
                                                        value.max(0.0);
                                                    listening.session = None;
                                                });
                                            }),
                                    )
                            })
                            .child({
                                let state_entity = self.state.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(Text::caption(level_text.max_correction))
                                    .child(
                                        NumberInput::new("listening-max-correction")
                                            .value(level_match.max_correction_db)
                                            .range(0.0, 24.0)
                                            .step(0.5)
                                            .decimals(1)
                                            .unit("dB")
                                            .aria_label(level_text.max_correction)
                                            .size(NumberInputSize::Sm)
                                            .width(140.0)
                                            .on_change(move |value, _window, cx| {
                                                state_entity.update(cx, |state, _| {
                                                    let listening = &mut state
                                                        .app
                                                        .plugin_state
                                                        .listening_test_state;
                                                    listening
                                                        .level_match_config
                                                        .max_correction_db = value.max(0.0);
                                                    listening.session = None;
                                                });
                                            }),
                                    )
                            }),
                    )
                    .when_some(prepared_evidence, |panel, evidence| panel.child(evidence)),
            )
            .child(
                div()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(Text::new(translations.trial.title).weight(TextWeight::Semibold))
                            .child(Text::caption(match score {
                                Some((correct, total)) if total > 0 => {
                                    format!(
                                        "{trial_count} {} · ABX {correct}/{total}",
                                        translations.trial.trials
                                    )
                                }
                                _ => {
                                    format!("{trial_count} {}", translations.trial.trials)
                                }
                            })),
                    )
                    .when(trial_ready && pending_mode.is_none(), |panel| {
                        panel.child(
                            div()
                                .flex()
                                .gap(d.gap)
                                .child(self.listening_trial_button(
                                    "start-blind-ab",
                                    translations.trial.start_blind_ab,
                                    TrialMode::BlindAb,
                                    cx,
                                ))
                                .child(self.listening_trial_button(
                                    "start-abx",
                                    translations.trial.start_abx,
                                    TrialMode::Abx,
                                    cx,
                                )),
                        )
                    })
                    .when_some(pending_mode, |panel, mode| {
                        panel
                            .child(self.render_listening_cues(mode, cx))
                            .child(self.render_listening_trial_metadata(cx))
                            .child(self.render_listening_answers(mode, cx))
                    })
                    .when(!has_session, |panel| {
                        panel.child(Text::caption(translations.trial.no_session))
                    })
                    .when(has_session && !trial_ready, |panel| {
                        panel
                            .child(Text::caption(level_text.outside_tolerance).color(theme.warning))
                    }),
            )
            .child(
                div()
                    .p(d.pad_y)
                    .rounded(d.r_sm)
                    .bg(theme.background_secondary)
                    .child(
                        Text::caption(if listening.status.is_empty() {
                            translations.status.select_paths.to_owned()
                        } else {
                            listening.status
                        })
                        .color(theme.text_secondary),
                    ),
            )
            .into_any_element()
    }

    fn render_eq_training_workbench(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let eq_text = state.app.ui_state.translations.listening_test.eq.clone();
        let listening = state.app.plugin_state.listening_test_state.clone();
        let current_track = state
            .app
            .get_current_track_path()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| eq_text.no_track.into());
        let session = listening.eq_session.as_ref();
        let answered = session.is_some_and(EqTrainingSession::current_is_answered);
        let complete = session.is_some_and(EqTrainingSession::is_complete);
        let has_question = session.is_some_and(|session| session.current_question.is_some());
        let trial_number = session
            .and_then(|session| session.current_question.as_ref())
            .map_or_else(
                || session.map_or(0, |session| session.trials.len()),
                |question| question.number + 1,
            );
        let accuracy = session.map_or(0.0, EqTrainingSession::accuracy) * 100.0;
        let answer_labels = session
            .and_then(|session| {
                session.current_question.as_ref().map(|question| {
                    question.answer_labels(session.config.exercise, &session.band_frequencies)
                })
            })
            .unwrap_or_default();
        let selected_band = listening
            .eq_selected_band
            .min(answer_labels.len().saturating_sub(1));

        let curve_points = session
            .and_then(|session| session.current_question.as_ref())
            .filter(|_| answered)
            .map(|question| question.preview_curve(160))
            .unwrap_or_else(|| vec![(20.0, 0.0), (20_000.0, 0.0)]);
        let (x_values, y_values): (Vec<_>, Vec<_>) = curve_points.into_iter().unzip();
        let chart = render_line_chart(
            vec![Series::new(
                "Training EQ",
                channel_color(&theme, 0),
                x_values,
                y_values,
            )],
            ChartConfig {
                title: None,
                x_label: Some("Frequency (Hz)".into()),
                y_label: Some("Gain (dB)".into()),
                x_range: (20.0, 20_000.0),
                y_range: (-16.0, 16.0),
                x_scale: gpui_px::ScaleType::Log,
                width: 760.0,
                height: 220.0,
            },
            &theme,
            None,
        );

        let mut answers = div().flex().flex_wrap().gap(d.gap);
        for (index, answer_label) in answer_labels.iter().enumerate() {
            let is_selected = index == selected_band;
            let is_answer = answered
                && session
                    .and_then(|session| session.current_question.as_ref())
                    .is_some_and(|question| {
                        question.correct_answer(listening.eq_config.exercise) == index
                    });
            let label = if is_answer {
                format!("{answer_label} ✓")
            } else if answered && is_selected {
                format!("{answer_label} •")
            } else {
                answer_label.clone()
            };
            answers = answers.child(
                Button::new(("eq-training-band", index), label)
                    .size(ButtonSize::Sm)
                    .variant(if is_selected || is_answer {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(move |view, _, _, cx| {
                        view.select_eq_training_band(index, cx);
                    })),
            );
        }

        let feedback = session
            .and_then(|session| session.trials.last())
            .filter(|_| answered)
            .map(|result| {
                if result.correct {
                    format!(
                        "{} — {} at {:+.0} dB, Q {:.1}",
                        eq_text.correct,
                        format_frequency(result.question.center_frequency_hz),
                        result.question.signed_gain_db(),
                        result.question.q
                    )
                } else {
                    format!(
                        "{}: {} at {:+.0} dB, Q {:.1}",
                        eq_text.answer,
                        format_frequency(result.question.center_frequency_hz),
                        result.question.signed_gain_db(),
                        result.question.q
                    )
                }
            });

        div()
            .id("eq-training-workbench")
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .child(Text::section_header(eq_text.title))
                            .child(Text::caption(eq_text.subtitle)),
                    )
                    .child(Text::caption(format!(
                        "Trial {trial_number}/{} · Accuracy {accuracy:.0}%",
                        listening.eq_config.trial_count
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.section)
                    .child(
                        div()
                            .flex_1()
                            .min_w(rems(18.0))
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(Text::section_header(eq_text.session_setup))
                            .child(self.render_eq_source_row(eq_text.source, &current_track, cx))
                            .child(
                                Button::new(
                                    "eq-training-exercise",
                                    format!("Exercise: {}", listening.eq_config.exercise.label()),
                                )
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(
                                    cx.listener(|view, _, _, cx| {
                                        view.cycle_eq_training_exercise(cx)
                                    }),
                                ),
                            )
                            .child(
                                Button::new(
                                    "eq-training-adaptive",
                                    if listening.eq_adaptive {
                                        "Adaptive difficulty: on"
                                    } else {
                                        "Adaptive difficulty: off"
                                    },
                                )
                                .size(ButtonSize::Sm)
                                .variant(if listening.eq_adaptive {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.toggle_eq_training_adaptive(cx)
                                })),
                            )
                            .child(self.render_eq_config_row(
                                eq_text.bands,
                                listening.eq_config.band_count.to_string(),
                                EqConfigField::Bands,
                                cx,
                            ))
                            .child(self.render_eq_config_row(
                                eq_text.gain,
                                format!("±{:.0} dB", listening.eq_config.gain_db),
                                EqConfigField::Gain,
                                cx,
                            ))
                            .child(self.render_eq_config_row(
                                "Q",
                                format!("{:.1}", listening.eq_config.q),
                                EqConfigField::Q,
                                cx,
                            ))
                            .child(self.render_eq_config_row(
                                eq_text.trials,
                                listening.eq_config.trial_count.to_string(),
                                EqConfigField::Trials,
                                cx,
                            ))
                            .child(
                                Button::new(
                                    "eq-training-change-mode",
                                    format!(
                                        "{}: {}",
                                        eq_text.change,
                                        eq_change_mode_symbol(listening.eq_config.change_mode)
                                    ),
                                )
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.cycle_eq_training_change_mode(cx);
                                })),
                            )
                            .child(
                                Button::new(
                                    "eq-training-start",
                                    if session.is_some() {
                                        eq_text.restart
                                    } else {
                                        eq_text.start
                                    },
                                )
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Primary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.start_eq_training_session(cx);
                                })),
                            )
                            .child(Text::caption(eq_text.audition_path_hint)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(rems(30.0))
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(Text::section_header(if complete {
                                eq_text.complete
                            } else if session.is_some() {
                                eq_text.question
                            } else {
                                eq_text.start_prompt
                            }))
                            .child(answers)
                            .child(div().w_full().min_h(rems(14.0)).child(chart))
                            .when_some(feedback, |panel, feedback| {
                                panel.child(
                                    Text::body(feedback).color(
                                        if session
                                            .and_then(|session| session.trials.last())
                                            .is_some_and(|result| result.correct)
                                        {
                                            theme.success
                                        } else {
                                            theme.warning
                                        },
                                    ),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.gap)
                                    .child(
                                        Button::new(
                                            "eq-training-original",
                                            format!("1  {}", eq_text.original),
                                        )
                                        .size(ButtonSize::Sm)
                                        .variant(if !listening.eq_filtered {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
                                            cx.listener(|view, _, _, cx| {
                                                view.activate_eq_training_path(false, cx);
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(
                                            "eq-training-filtered",
                                            format!("2  {}", eq_text.filtered),
                                        )
                                        .size(ButtonSize::Sm)
                                        .variant(if listening.eq_filtered {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
                                            cx.listener(|view, _, _, cx| {
                                                view.activate_eq_training_path(true, cx);
                                            }),
                                        ),
                                    )
                                    .when(has_question && !answered, |row| {
                                        row.child(
                                            Button::new(
                                                "eq-training-submit",
                                                format!("Enter  {}", eq_text.submit),
                                            )
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Primary)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.submit_eq_training_answer(cx);
                                            })),
                                        )
                                    })
                                    .when(answered, |row| {
                                        row.child(
                                            Button::new(
                                                "eq-training-next",
                                                format!("N  {}", eq_text.next),
                                            )
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Primary)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.advance_eq_training_question(cx);
                                            })),
                                        )
                                    }),
                            )
                            .child(Text::caption(eq_text.shortcuts)),
                    ),
            )
            .child(
                div()
                    .p(d.pad_y)
                    .rounded(d.r_sm)
                    .bg(theme.background_secondary)
                    .child(Text::caption(if listening.status.is_empty() {
                        eq_text.configure_start
                    } else {
                        &listening.status
                    })),
            )
            .into_any_element()
    }

    fn render_eq_source_row(
        &self,
        source_label: &'static str,
        current_track: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let eq_text = &state.app.ui_state.translations.listening_test.eq;
        div()
            .flex()
            .flex_col()
            .gap(d.grid)
            .child(Text::label(source_label))
            .child(Text::caption(current_track.to_owned()))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.grid)
                    .child(
                        Button::new("eq-source-add", eq_text.add_current)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.add_current_eq_source(cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-source-prev", eq_text.previous)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.navigate_eq_source(-1, cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-source-next", eq_text.next)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.navigate_eq_source(1, cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-loop-start", eq_text.set_loop_start)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.set_eq_loop_boundary(true, cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-loop-end", eq_text.set_loop_end)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.set_eq_loop_boundary(false, cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-loop-toggle", eq_text.toggle_loop)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| view.toggle_eq_loop(cx))),
                    ),
            )
    }

    fn render_eq_config_row(
        &self,
        label: &'static str,
        value: impl Into<SharedString>,
        field: EqConfigField,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(d.gap)
            .child(Text::label(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(
                        Button::new(("eq-config-minus", field as usize), "−")
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(move |view, _, _, cx| {
                                view.adjust_eq_training_config(field, -1, cx);
                            })),
                    )
                    .child(Text::new(value).color(theme.text_primary))
                    .child(
                        Button::new(("eq-config-plus", field as usize), "+")
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(move |view, _, _, cx| {
                                view.adjust_eq_training_config(field, 1, cx);
                            })),
                    ),
            )
    }

    fn ensure_listening_path_canvas(&self, target: ListeningPathTarget, cx: &mut Context<Self>) {
        let (config, has_canvas) = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            match target {
                ListeningPathTarget::A => {
                    (listening.path_a.clone(), listening.path_a_canvas.is_some())
                }
                ListeningPathTarget::B => {
                    (listening.path_b.clone(), listening.path_b_canvas.is_some())
                }
            }
        };
        if has_canvas {
            return;
        }
        let Some(PathConfig::Graph { nodes, edges }) = config else {
            return;
        };

        let workflow_graph = build_listening_workflow_graph(&nodes, &edges);
        let canvas = cx.new(|cx| WorkflowCanvas::with_graph(workflow_graph, cx));
        let state_for_change = self.state.clone();
        let canvas_for_change = canvas.clone();
        let state_for_edit = self.state.clone();
        let canvas_for_edit = canvas.clone();
        canvas.update(cx, |canvas, _| {
            canvas.set_menu_items(Vec::new());
            canvas.set_on_graph_change(move |cx| {
                let canvas = canvas_for_change.clone();
                let state = state_for_change.clone();
                cx.defer(move |cx| {
                    let workflow = canvas.read(cx).graph().clone();
                    state.update(cx, |state, _| {
                        sync_listening_path_from_workflow(state, target, &workflow);
                    });
                });
            });
            canvas.set_on_node_double_click(move |node_id, _window, cx| {
                let canvas = canvas_for_edit.clone();
                let state = state_for_edit.clone();
                cx.defer(move |cx| {
                    let path_node_id = canvas
                        .read(cx)
                        .graph()
                        .nodes
                        .get(&node_id)
                        .and_then(|node| node.user_data.get("path_node_id"))
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned);
                    state.update(cx, |state, _| {
                        let listening = &mut state.app.plugin_state.listening_test_state;
                        listening.editing_path_target = Some(target.into());
                        listening.editing_path_parameters = path_node_id
                            .as_deref()
                            .and_then(|id| listening_graph_node(listening, target, id))
                            .and_then(|node| serde_json::to_string_pretty(&node.parameters).ok())
                            .unwrap_or_default();
                        listening.editing_path_node_id = path_node_id;
                    });
                });
            });
        });
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            match target {
                ListeningPathTarget::A => listening.path_a_canvas = Some(canvas),
                ListeningPathTarget::B => listening.path_b_canvas = Some(canvas),
            }
        });
    }

    fn render_listening_path_card(
        &self,
        target: ListeningPathTarget,
        label: &str,
        config: Option<&PathConfig>,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.listening_test.clone();
        let suffix = match target {
            ListeningPathTarget::A => "a",
            ListeningPathTarget::B => "b",
        };
        let (canvas, add_menu_open, editing_node_id, editing_parameters) = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            (
                match target {
                    ListeningPathTarget::A => listening.path_a_canvas.clone(),
                    ListeningPathTarget::B => listening.path_b_canvas.clone(),
                },
                listening.graph_add_menu_target == Some(target.into()),
                if listening.editing_path_target == Some(target.into()) {
                    listening.editing_path_node_id.clone()
                } else {
                    None
                },
                listening.editing_path_parameters.clone(),
            )
        };
        let linear_plugins: Vec<PluginInRack> = match config {
            Some(PathConfig::Plugin {
                plugin_type,
                parameters,
            }) => vec![PluginInRack {
                plugin_type: plugin_type.clone(),
                parameters: parameters.clone(),
            }],
            Some(PathConfig::Rack { plugins }) => plugins.clone(),
            _ => Vec::new(),
        };
        div()
            .flex_1()
            .p(d.pad_x)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Text::new(label.to_owned()).weight(TextWeight::Semibold))
                    .child(Text::caption(path_summary(config, &translations))),
            )
            .child(
                div()
                    .flex()
                    .gap(d.gap)
                    .child({
                        let button = Button::new(
                            SharedString::from(format!("capture-listening-{suffix}")),
                            translations.setup.use_current,
                        )
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(
                            move |view, _, _, cx| {
                                view.capture_listening_path(target, cx);
                            },
                        ));
                        #[cfg(feature = "dev-api")]
                        let button = button.dev_track(format!("listening.capture.{suffix}"));
                        button
                    })
                    .child({
                        let button = Button::new(
                            SharedString::from(format!("load-listening-{suffix}")),
                            translations.setup.load_path,
                        )
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(
                            move |view, _, _, cx| {
                                view.load_listening_path(target, cx);
                            },
                        ));
                        #[cfg(feature = "dev-api")]
                        let button = button.dev_track(format!("listening.load-path.{suffix}"));
                        button
                    }),
            )
            .when(!matches!(config, Some(PathConfig::Graph { .. })), |card| {
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(Text::caption(translations.setup.simple_rack))
                        .child(
                            div()
                                .flex()
                                .gap(d.grid)
                                .child(
                                    Button::new(
                                        SharedString::from(format!("listening-add-rack-{suffix}")),
                                        translations.setup.add_processor,
                                    )
                                    .size(ButtonSize::Xs)
                                    .variant(if add_menu_open {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    })
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(move |view, _, _, cx| {
                                            view.toggle_listening_graph_add_menu(target, cx);
                                        }),
                                    ),
                                )
                                .child(
                                    Button::new(
                                        SharedString::from(format!("route-listening-{suffix}")),
                                        translations.setup.edit_graph,
                                    )
                                    .size(ButtonSize::Xs)
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(move |view, _, _, cx| {
                                            view.convert_listening_path_to_graph(target, cx);
                                        }),
                                    ),
                                ),
                        ),
                )
                .when(add_menu_open, |card| {
                    card.child(self.render_listening_graph_add_menu(target, cx))
                })
                .children(linear_plugins.iter().enumerate().map(|(index, plugin)| {
                    self.render_listening_rack_row(target, index, linear_plugins.len(), plugin, cx)
                }))
            })
            .when(matches!(config, Some(PathConfig::Graph { .. })), |card| {
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(Text::caption(translations.setup.graph_hint))
                        .child(
                            Button::new(
                                SharedString::from(format!("listening-add-graph-{suffix}")),
                                translations.setup.add_processor,
                            )
                            .size(ButtonSize::Xs)
                            .variant(if add_menu_open {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(
                                move |view, _, _, cx| {
                                    view.toggle_listening_graph_add_menu(target, cx);
                                },
                            )),
                        ),
                )
                .when(add_menu_open, |card| {
                    card.child(self.render_listening_graph_add_menu(target, cx))
                })
                .when_some(canvas, |card, canvas| {
                    card.child(
                        div()
                            .id(SharedString::from(format!("listening-canvas-{suffix}")))
                            .h(rems(20.0))
                            .min_h(rems(14.0))
                            .overflow_hidden()
                            .rounded(d.r_sm)
                            .border_1()
                            .border_color(theme.border)
                            .child(canvas),
                    )
                })
                .when_some(editing_node_id, |card, node_id| {
                    let state_for_params = self.state.clone();
                    let state_for_close = self.state.clone();
                    card.child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_sm)
                            .bg(theme.background_secondary)
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(Text::caption(format!("{node_id} · JSON")))
                                    .child(
                                        Button::new(
                                            SharedString::from(format!(
                                                "listening-close-node-{suffix}"
                                            )),
                                            translations.setup.done,
                                        )
                                        .size(ButtonSize::Xs)
                                        .variant(ButtonVariant::Secondary)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
                                            move |_, _, cx| {
                                                state_for_close.update(cx, |state, _| {
                                                    let listening = &mut state
                                                        .app
                                                        .plugin_state
                                                        .listening_test_state;
                                                    listening.editing_path_target = None;
                                                    listening.editing_path_node_id = None;
                                                    listening.editing_path_parameters.clear();
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                Input::new(SharedString::from(format!(
                                    "listening-node-params-{suffix}"
                                )))
                                .value(editing_parameters)
                                .placeholder(r#"{"parameter": "value"}"#)
                                .size(InputSize::Sm)
                                .on_text_change(
                                    move |value, _window, cx| {
                                        state_for_params.update(cx, |state, _| {
                                            update_listening_node_parameters(
                                                state, target, &node_id, value,
                                            );
                                        });
                                    },
                                ),
                            ),
                    )
                })
            })
    }

    fn render_listening_cues(&self, mode: TrialMode, cx: &mut Context<Self>) -> Div {
        let d = Ds::from_cx(cx);
        let translations = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .listening_test
            .trial
            .clone();
        let mut row = div().flex().flex_wrap().gap(d.gap);
        let cues: &[(TrialCue, &str)] = match mode {
            TrialMode::BlindAb => &[
                (TrialCue::First, translations.play_first),
                (TrialCue::Second, translations.play_second),
            ],
            TrialMode::Abx => &[
                (TrialCue::ReferenceA, translations.reference_a),
                (TrialCue::ReferenceB, translations.reference_b),
                (TrialCue::Unknown, translations.unknown_x),
            ],
        };
        for &(cue, label) in cues {
            let theme = self.state.read(cx).app.ui_state.theme.clone();
            row = row.child(
                Button::new(SharedString::from(format!("listening-cue-{label}")), label)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Primary)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(move |view, _, _, cx| {
                        view.activate_listening_cue(cue, cx);
                    })),
            );
        }
        row
    }

    fn render_listening_graph_add_menu(
        &self,
        target: ListeningPathTarget,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let mut menu = div()
            .flex()
            .flex_wrap()
            .gap(d.grid)
            .p(d.pad_y)
            .rounded(d.r_sm)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background);
        for (plugin_type, label) in allowed_plugin_types() {
            let plugin_type = plugin_type.to_owned();
            menu = menu.child(
                Button::new(
                    SharedString::from(format!("listening-add-{plugin_type}")),
                    label,
                )
                .size(ButtonSize::Xs)
                .variant(ButtonVariant::Secondary)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                    view.add_listening_graph_node(target, &plugin_type, cx);
                })),
            );
        }
        menu
    }

    fn render_listening_rack_row(
        &self,
        target: ListeningPathTarget,
        index: usize,
        count: usize,
        plugin: &PluginInRack,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.listening_test.setup.clone();
        let suffix = match target {
            ListeningPathTarget::A => "a",
            ListeningPathTarget::B => "b",
        };
        let edit_id = format!("rack:{index}");
        let (is_editing, edit_value) = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            (
                listening.editing_path_target == Some(target.into())
                    && listening.editing_path_node_id.as_deref() == Some(edit_id.as_str()),
                listening.editing_path_parameters.clone(),
            )
        };
        let state_for_begin_edit = self.state.clone();
        let state_for_edit = self.state.clone();
        let parameters = plugin.parameters.clone();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .p(d.pad_y)
            .rounded(d.r_sm)
            .bg(theme.background_secondary)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(Text::caption(format!("{}.", index + 1)))
                    .child(Text::new(plugin.plugin_type.clone())),
            )
            .child(
                div()
                    .flex()
                    .gap(d.grid)
                    .child(
                        Button::new(
                            SharedString::from(format!("listening-{suffix}-edit-{index}")),
                            if is_editing {
                                translations.editing
                            } else {
                                translations.edit_json
                            },
                        )
                        .size(ButtonSize::Xs)
                        .variant(if is_editing {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .theme(theme.to_button_theme())
                        .on_click_event(move |_, _, cx| {
                            let edit_id = edit_id.clone();
                            let parameters = parameters.clone();
                            state_for_begin_edit.update(cx, |state, _| {
                                let listening = &mut state.app.plugin_state.listening_test_state;
                                listening.editing_path_target = Some(target.into());
                                listening.editing_path_node_id = Some(edit_id);
                                listening.editing_path_parameters =
                                    serde_json::to_string_pretty(&parameters)
                                        .unwrap_or_else(|_| "{}".into());
                            });
                        }),
                    )
                    .when(index > 0, |buttons| {
                        buttons.child(
                            Button::new(
                                SharedString::from(format!("listening-{suffix}-up-{index}")),
                                "↑",
                            )
                            .size(ButtonSize::Xs)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(
                                move |view, _, _, cx| {
                                    view.move_listening_rack_plugin(target, index, index - 1, cx);
                                },
                            )),
                        )
                    })
                    .when(index + 1 < count, |buttons| {
                        buttons.child(
                            Button::new(
                                SharedString::from(format!("listening-{suffix}-down-{index}")),
                                "↓",
                            )
                            .size(ButtonSize::Xs)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(
                                move |view, _, _, cx| {
                                    view.move_listening_rack_plugin(target, index, index + 1, cx);
                                },
                            )),
                        )
                    })
                    .child(
                        Button::new(
                            SharedString::from(format!("listening-{suffix}-remove-{index}")),
                            translations.remove,
                        )
                        .size(ButtonSize::Xs)
                        .variant(ButtonVariant::Destructive)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(
                            move |view, _, _, cx| {
                                view.remove_listening_rack_plugin(target, index, cx);
                            },
                        )),
                    ),
            )
            .when(is_editing, |row| {
                row.child(
                    div().w_full().child(
                        Input::new(SharedString::from(format!(
                            "listening-{suffix}-params-{index}"
                        )))
                        .value(edit_value)
                        .placeholder(r#"{"parameter": "value"}"#)
                        .size(InputSize::Sm)
                        .on_text_change(move |value, _window, cx| {
                            state_for_edit.update(cx, |state, _| {
                                update_listening_rack_parameters(state, target, index, value);
                            });
                        }),
                    ),
                )
            })
    }

    fn toggle_listening_graph_add_menu(
        &mut self,
        target: ListeningPathTarget,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            let target = target.into();
            listening.graph_add_menu_target =
                (listening.graph_add_menu_target != Some(target)).then_some(target);
        });
        cx.notify();
    }

    fn convert_listening_path_to_graph(
        &mut self,
        target: ListeningPathTarget,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let localized = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .clone();
            let listening = &mut state.app.plugin_state.listening_test_state;
            let Some(config) = listening_path_config_mut(listening, target) else {
                listening.status = localized.select_path.into();
                return;
            };
            let plugins: Vec<(String, serde_json::Value)> = match config {
                PathConfig::None => Vec::new(),
                PathConfig::Plugin {
                    plugin_type,
                    parameters,
                } => vec![(plugin_type.clone(), parameters.clone())],
                PathConfig::Rack { plugins } => plugins
                    .iter()
                    .map(|plugin| (plugin.plugin_type.clone(), plugin.parameters.clone()))
                    .collect(),
                PathConfig::Graph { .. } => return,
            };
            let nodes: Vec<_> = plugins
                .into_iter()
                .enumerate()
                .map(|(index, (plugin_type, parameters))| GraphNodeConfig {
                    id: format!("processor_{}", index + 1),
                    plugin_type,
                    parameters,
                })
                .collect();
            let edges = nodes
                .windows(2)
                .map(|pair| GraphEdgeConfig {
                    from: pair[0].id.clone(),
                    to: pair[1].id.clone(),
                    channel_map: None,
                    destination_offset: 0,
                })
                .collect();
            *config = PathConfig::Graph { nodes, edges };
            clear_listening_canvas(listening, target);
            listening.session = None;
            listening.status = localized.graph_converted.into();
        });
        cx.notify();
    }

    fn add_listening_graph_node(
        &mut self,
        target: ListeningPathTarget,
        plugin_type: &str,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let localized = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .clone();
            let listening = &mut state.app.plugin_state.listening_test_state;
            let Some(config) = listening_path_config_mut(listening, target) else {
                listening.status = localized.select_path.into();
                return;
            };
            match config {
                PathConfig::None => {
                    *config = PathConfig::Rack {
                        plugins: vec![PluginInRack {
                            plugin_type: plugin_type.to_owned(),
                            parameters: serde_json::json!({}),
                        }],
                    };
                }
                PathConfig::Plugin {
                    plugin_type: existing_type,
                    parameters,
                } => {
                    *config = PathConfig::Rack {
                        plugins: vec![
                            PluginInRack {
                                plugin_type: existing_type.clone(),
                                parameters: parameters.clone(),
                            },
                            PluginInRack {
                                plugin_type: plugin_type.to_owned(),
                                parameters: serde_json::json!({}),
                            },
                        ],
                    };
                }
                PathConfig::Rack { plugins } => plugins.push(PluginInRack {
                    plugin_type: plugin_type.to_owned(),
                    parameters: serde_json::json!({}),
                }),
                PathConfig::Graph { nodes, .. } => {
                    let base = plugin_type.replace(['/', ':', ' '], "_");
                    let mut suffix = nodes.len() + 1;
                    let mut id = format!("{base}_{suffix}");
                    while nodes.iter().any(|node| node.id == id) {
                        suffix += 1;
                        id = format!("{base}_{suffix}");
                    }
                    nodes.push(GraphNodeConfig {
                        id,
                        plugin_type: plugin_type.to_owned(),
                        parameters: serde_json::json!({}),
                    });
                    clear_listening_canvas(listening, target);
                }
            }
            listening.graph_add_menu_target = None;
            listening.session = None;
            listening.status = localized.processor_added.into();
        });
        cx.notify();
    }

    fn remove_listening_rack_plugin(
        &mut self,
        target: ListeningPathTarget,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let processor_removed = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .processor_removed;
            let listening = &mut state.app.plugin_state.listening_test_state;
            let Some(config) = listening_path_config_mut(listening, target) else {
                return;
            };
            match config {
                PathConfig::Plugin { .. } if index == 0 => *config = PathConfig::None,
                PathConfig::Rack { plugins } if index < plugins.len() => {
                    plugins.remove(index);
                    if plugins.is_empty() {
                        *config = PathConfig::None;
                    }
                }
                _ => return,
            }
            listening.session = None;
            listening.status = processor_removed.into();
        });
        cx.notify();
    }

    fn move_listening_rack_plugin(
        &mut self,
        target: ListeningPathTarget,
        from: usize,
        to: usize,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _| {
            let rack_reordered = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .rack_reordered;
            let listening = &mut state.app.plugin_state.listening_test_state;
            let Some(PathConfig::Rack { plugins }) = listening_path_config_mut(listening, target)
            else {
                return;
            };
            if from >= plugins.len() || to >= plugins.len() || from == to {
                return;
            }
            let plugin = plugins.remove(from);
            plugins.insert(to, plugin);
            listening.session = None;
            listening.status = rack_reordered.into();
        });
        cx.notify();
    }

    fn render_listening_answers(&self, mode: TrialMode, cx: &mut Context<Self>) -> Div {
        let d = Ds::from_cx(cx);
        let translations = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .listening_test
            .trial
            .clone();
        let answers: &[(TrialAnswer, &str)] = match mode {
            TrialMode::BlindAb => &[
                (TrialAnswer::First, translations.prefer_first),
                (TrialAnswer::Second, translations.prefer_second),
            ],
            TrialMode::Abx => &[
                (TrialAnswer::A, translations.x_is_a),
                (TrialAnswer::B, translations.x_is_b),
            ],
        };
        let mut row = div().flex().flex_wrap().gap(d.gap);
        for &(answer, label) in answers {
            let theme = self.state.read(cx).app.ui_state.theme.clone();
            row = row.child(
                Button::new(
                    SharedString::from(format!("listening-answer-{label}")),
                    label,
                )
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Secondary)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                    view.commit_listening_answer(answer, cx);
                })),
            );
        }
        row
    }

    fn render_listening_trial_metadata(&self, cx: &mut Context<Self>) -> Div {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let translations = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .listening_test
            .trial
            .clone();
        let listening = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .clone();
        let state_for_confidence = self.state.clone();
        let state_for_notes = self.state.clone();
        div()
            .flex()
            .items_center()
            .gap(d.gap)
            .child(
                div().w(rems(9.0)).child(
                    NumberInput::new("listening-confidence")
                        .value(f64::from(listening.confidence))
                        .range(0.0, 100.0)
                        .step(5.0)
                        .decimals(0)
                        .unit(translations.confidence)
                        .size(NumberInputSize::Sm)
                        .on_change(move |value, _window, cx| {
                            state_for_confidence.update(cx, |state, _| {
                                state.app.plugin_state.listening_test_state.confidence =
                                    value.clamp(0.0, 100.0) as u8;
                            });
                        }),
                ),
            )
            .child(
                div().flex_1().child(
                    Input::new("listening-notes")
                        .value(listening.notes)
                        .placeholder(translations.notes_placeholder)
                        .size(InputSize::Sm)
                        .on_text_change(move |value, _window, cx| {
                            state_for_notes.update(cx, |state, _| {
                                state.app.plugin_state.listening_test_state.notes = value;
                            });
                        }),
                ),
            )
            .text_color(theme.text_primary)
    }

    fn listening_trial_button(
        &self,
        id: &'static str,
        label: &'static str,
        mode: TrialMode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let button = Button::new(id, label)
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Primary)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |view, _, _, cx| {
                view.start_listening_trial(mode, cx);
            }));
        #[cfg(feature = "dev-api")]
        let button = button.dev_track(format!("listening.trial.{id}"));
        button
    }

    fn listening_metric_button(
        &self,
        id: &'static str,
        label: &'static str,
        metric: LevelMatchMetric,
        selected: LevelMatchMetric,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        Button::new(id, label)
            .size(ButtonSize::Xs)
            .variant(if metric == selected {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            })
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |view, _, _, cx| {
                view.state.update(cx, |state, _| {
                    let listening = &mut state.app.plugin_state.listening_test_state;
                    listening.level_match_config.metric = metric;
                    listening.level_match_config.window_ms = listening
                        .level_match_config
                        .window_ms
                        .max(metric.minimum_window_ms().max(1));
                    listening.session = None;
                });
                cx.notify();
            }))
    }

    fn use_current_listening_position(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let start_ms = (state.app.playback.position_secs.max(0.0) * 1_000.0).round() as u64;
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.segment_start_ms = start_ms;
            listening.session = None;
        });
        cx.notify();
    }

    fn load_listening_session_media(&mut self, cx: &mut Context<Self>) {
        let request = {
            let state = self.state.read(cx);
            state
                .app
                .plugin_state
                .listening_test_state
                .session
                .as_ref()
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

    fn listening_test_file_button(
        &self,
        id: &'static str,
        label: &'static str,
        load: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let button = Button::new(id, label)
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |view, _, _, cx| {
                view.pick_listening_session_file(load, cx);
            }));
        #[cfg(feature = "dev-api")]
        let button = button.dev_track(format!("listening.session.{id}"));
        button
    }

    fn capture_listening_path(&mut self, target: ListeningPathTarget, cx: &mut Context<Self>) {
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
                    listening.session = None;
                    listening.status = captured.into();
                }
                Err(error) => listening.status = error,
            }
        });
        cx.notify();
    }

    fn start_listening_trial(&mut self, mode: TrialMode, cx: &mut Context<Self>) {
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
            let listening = &mut state.app.plugin_state.listening_test_state;
            let Some(session) = listening.session.as_mut() else {
                listening.status = load_or_prepare.into();
                return;
            };
            match session.start_trial(mode) {
                Ok(index) => listening.status = format!("{trial_started} · #{}", index + 1),
                Err(error) => listening.status = error.to_string(),
            }
        });
        cx.notify();
    }

    fn prepare_current_listening_session(&mut self, cx: &mut Context<Self>) {
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
        self.state.update(cx, |state, _| {
            state.app.plugin_state.listening_test_state.status = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .measuring
                .into();
        });

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
                        listening.session = Some(session);
                        listening.status = format!("{prepared_label}: {correction:+.2} dB.");
                    }
                    Err(error) => listening.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn activate_listening_cue(&mut self, cue: TrialCue, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let localized = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .clone();
            let runtime = state
                .app
                .plugin_state
                .listening_test_state
                .session
                .as_ref()
                .ok_or_else(|| localized.no_session.to_owned())
                .and_then(|session| {
                    session
                        .runtime_config_for_pending_cue(cue)
                        .map_err(|error| error.to_string())
                });
            let result = runtime.and_then(|runtime| {
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
                    .ok_or_else(|| localized.add_ab_plugin.to_owned())?;
                let path_a = serde_json::to_string(&runtime.path_a).map_err(|e| e.to_string())?;
                let path_b = serde_json::to_string(&runtime.path_b).map_err(|e| e.to_string())?;
                state.app.set_plugin_param_string(plugin_idx, 9, path_a)?;
                state.app.set_plugin_param_string(plugin_idx, 10, path_b)?;
                state
                    .app
                    .set_plugin_param(plugin_idx, 0, f64::from(runtime.mix));
                state.app.set_plugin_param(plugin_idx, 1, 1.0);
                state
                    .app
                    .set_plugin_param(plugin_idx, 2, f64::from(runtime.selected_path));
                state.app.set_plugin_param(plugin_idx, 4, 0.0);
                state
                    .app
                    .set_plugin_param(plugin_idx, 6, f64::from(runtime.max_auto_gain_db));
                state.app.set_plugin_param(plugin_idx, 7, 0.0);
                state
                    .app
                    .set_plugin_param(plugin_idx, 8, f64::from(runtime.mix_transition_ms));
                Ok(())
            });
            state.app.plugin_state.listening_test_state.status = match result {
                Ok(()) => localized.cue_active.into(),
                Err(error) => error,
            };
        });
        cx.notify();
    }

    fn commit_listening_answer(&mut self, answer: TrialAnswer, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let answer_committed = state
                .app
                .ui_state
                .translations
                .listening_test
                .status
                .answer_committed;
            let listening = &mut state.app.plugin_state.listening_test_state;
            let Some(session) = listening.session.as_mut() else {
                return;
            };
            match session.commit_trial(
                answer,
                Some(listening.confidence),
                Some(listening.notes.clone()),
            ) {
                Ok(_) => {
                    listening.notes.clear();
                    listening.status = answer_committed.into();
                }
                Err(error) => listening.status = error.to_string(),
            }
        });
        cx.notify();
    }

    fn load_listening_path(&mut self, target: ListeningPathTarget, cx: &mut Context<Self>) {
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
                        listening.session = None;
                        listening.status = path_loaded.into();
                    }
                    Err(error) => listening.status = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn pick_listening_session_file(&mut self, load: bool, cx: &mut Context<Self>) {
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
        let session = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .session
            .clone();
        cx.spawn(async move |_, cx| {
            let file = if load {
                rfd::AsyncFileDialog::new()
                    .add_filter(session_filter, &["json"])
                    .pick_file()
                    .await
            } else {
                rfd::AsyncFileDialog::new()
                    .set_file_name("sotf-listening-session.json")
                    .save_file()
                    .await
            };
            let Some(file) = file else { return };
            let result = if load {
                load_ab_test_session(file.path()).map(Some)
            } else if let Some(session) = session.as_ref() {
                save_ab_test_session(session, file.path()).map(|_| None)
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
                        listening.path_a = Some(session.setup.path_a.config.clone());
                        listening.path_b = Some(session.setup.path_b.config.clone());
                        listening.path_a_label = session.setup.path_a.label.clone();
                        listening.path_b_label = session.setup.path_b.label.clone();
                        listening.path_a_canvas = None;
                        listening.path_b_canvas = None;
                        listening.level_match_config = session.setup.level_match.config();
                        listening.segment_start_ms = session.setup.media.start_ms;
                        listening.session = Some(session);
                        listening.status = localized.session_loaded.into();
                    }
                    Ok(None) => listening.status = localized.session_saved.into(),
                    Err(error) => listening.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn set_ear_training_surface(&mut self, surface: EarTrainingSurface, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.app.plugin_state.listening_test_state.surface = surface;
        });
        cx.notify();
    }

    fn adjust_eq_training_config(
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

    fn cycle_eq_training_change_mode(&mut self, cx: &mut Context<Self>) {
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

    fn start_eq_training_session(&mut self, cx: &mut Context<Self>) {
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

    fn start_eq_course(&mut self, course: EarTrainingCourse, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_config = course.config();
            listening.eq_active_course = Some(course);
            listening.surface = EarTrainingSurface::EqBands;
        });
        self.start_eq_training_session(cx);
    }

    fn cycle_eq_training_exercise(&mut self, cx: &mut Context<Self>) {
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

    fn toggle_eq_training_adaptive(&mut self, cx: &mut Context<Self>) {
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

    fn add_current_eq_source(&mut self, cx: &mut Context<Self>) {
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

    fn navigate_eq_source(&mut self, direction: i32, cx: &mut Context<Self>) {
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

    fn set_eq_loop_boundary(&mut self, start: bool, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
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
            listening.status = format!("Loop {loop_start:.1}–{loop_end:.1} s");
        });
        cx.notify();
    }

    fn toggle_eq_loop(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            let listening = &mut state.app.plugin_state.listening_test_state;
            listening.eq_loop_enabled =
                !listening.eq_loop_enabled && listening.eq_loop_range.is_some();
            listening.status = if listening.eq_loop_enabled {
                "Clip loop enabled".into()
            } else {
                "Clip loop disabled".into()
            };
        });
        cx.notify();
    }

    fn ensure_eq_audition_plugin(&mut self, cx: &mut Context<Self>) {
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

    fn select_eq_training_band(&mut self, index: usize, cx: &mut Context<Self>) {
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

    fn move_eq_training_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
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

    fn activate_eq_training_path(&mut self, filtered: bool, cx: &mut Context<Self>) {
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

    fn submit_eq_training_answer(&mut self, cx: &mut Context<Self>) {
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
                        eq_text.answer,
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

    fn advance_eq_training_question(&mut self, cx: &mut Context<Self>) {
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

    fn is_eq_training_active(&self, cx: &Context<Self>) -> bool {
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

    fn is_listening_test_active(&self, cx: &Context<Self>) -> bool {
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.start_listening_trial(TrialMode::BlindAb, cx);
        }
    }

    pub(crate) fn listening_start_abx(
        &mut self,
        _: &ListeningStartAbx,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_listening_test_active(cx) {
            self.start_listening_trial(TrialMode::Abx, cx);
        }
    }

    fn play_listening_cue_position(&mut self, position: usize, cx: &mut Context<Self>) {
        if !self.is_listening_test_active(cx) {
            return;
        }
        let mode = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .session
            .as_ref()
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

    fn commit_listening_answer_position(&mut self, position: usize, cx: &mut Context<Self>) {
        if !self.is_listening_test_active(cx) {
            return;
        }
        let mode = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .session
            .as_ref()
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

fn format_frequency(frequency_hz: f64) -> String {
    if frequency_hz >= 1_000.0 {
        let khz = frequency_hz / 1_000.0;
        if (khz - khz.round()).abs() < 0.05 {
            format!("{khz:.0}k")
        } else {
            format!("{khz:.1}k")
        }
    } else {
        format!("{frequency_hz:.0}")
    }
}

fn eq_change_mode_symbol(mode: EqChangeMode) -> &'static str {
    match mode {
        EqChangeMode::Boost => "+",
        EqChangeMode::Cut => "−",
        EqChangeMode::Mixed => "±",
    }
}

fn listening_cue_for_position(mode: Option<TrialMode>, position: usize) -> Option<TrialCue> {
    match (mode, position) {
        (Some(TrialMode::BlindAb), 0) => Some(TrialCue::First),
        (Some(TrialMode::BlindAb), 1) => Some(TrialCue::Second),
        (Some(TrialMode::Abx), 0) => Some(TrialCue::ReferenceA),
        (Some(TrialMode::Abx), 1) => Some(TrialCue::ReferenceB),
        (Some(TrialMode::Abx), 2) => Some(TrialCue::Unknown),
        _ => None,
    }
}

fn listening_answer_for_position(mode: Option<TrialMode>, position: usize) -> Option<TrialAnswer> {
    match (mode, position) {
        (Some(TrialMode::BlindAb), 0) => Some(TrialAnswer::First),
        (Some(TrialMode::BlindAb), 1) => Some(TrialAnswer::Second),
        (Some(TrialMode::Abx), 0) => Some(TrialAnswer::A),
        (Some(TrialMode::Abx), 1) => Some(TrialAnswer::B),
        _ => None,
    }
}

fn path_summary(
    config: Option<&PathConfig>,
    translations: &crate::i18n::ListeningTestTranslations,
) -> String {
    match config {
        None => translations.status.not_selected.into(),
        Some(PathConfig::None) => translations.status.pass_through.into(),
        Some(PathConfig::Plugin { .. }) => format!("1 {}", translations.setup.plugin),
        Some(PathConfig::Rack { plugins }) => {
            format!(
                "{} · {} {}",
                translations.status.linear_rack,
                plugins.len(),
                translations.setup.plugins
            )
        }
        Some(PathConfig::Graph { nodes, edges }) => {
            format!(
                "{} · {} {} · {} {}",
                translations.status.routed_graph,
                nodes.len(),
                translations.setup.nodes,
                edges.len(),
                translations.setup.routes
            )
        }
    }
}

impl From<ListeningPathTarget> for ABPathTarget {
    fn from(value: ListeningPathTarget) -> Self {
        match value {
            ListeningPathTarget::A => Self::A,
            ListeningPathTarget::B => Self::B,
        }
    }
}

fn listening_path_config_mut(
    listening: &mut crate::app::state::plugin::ListeningTestState,
    target: ListeningPathTarget,
) -> Option<&mut PathConfig> {
    match target {
        ListeningPathTarget::A => listening.path_a.as_mut(),
        ListeningPathTarget::B => listening.path_b.as_mut(),
    }
}

fn listening_graph_node<'a>(
    listening: &'a crate::app::state::plugin::ListeningTestState,
    target: ListeningPathTarget,
    node_id: &str,
) -> Option<&'a GraphNodeConfig> {
    let config = match target {
        ListeningPathTarget::A => listening.path_a.as_ref(),
        ListeningPathTarget::B => listening.path_b.as_ref(),
    };
    let Some(PathConfig::Graph { nodes, .. }) = config else {
        return None;
    };
    nodes.iter().find(|node| node.id == node_id)
}

fn clear_listening_canvas(
    listening: &mut crate::app::state::plugin::ListeningTestState,
    target: ListeningPathTarget,
) {
    match target {
        ListeningPathTarget::A => listening.path_a_canvas = None,
        ListeningPathTarget::B => listening.path_b_canvas = None,
    }
}

fn build_listening_workflow_graph(
    nodes: &[GraphNodeConfig],
    edges: &[GraphEdgeConfig],
) -> WorkflowGraph {
    use std::collections::HashMap;

    let mut input_counts: HashMap<&str, usize> =
        nodes.iter().map(|node| (node.id.as_str(), 2)).collect();
    let mut output_counts = input_counts.clone();
    for edge in edges {
        let source_count = edge
            .channel_map
            .as_ref()
            .and_then(|channels| channels.iter().max().copied())
            .map_or(2, |channel| channel + 1);
        let routed_count = edge.channel_map.as_ref().map_or(2, Vec::len);
        output_counts
            .entry(edge.from.as_str())
            .and_modify(|count| *count = (*count).max(source_count));
        input_counts.entry(edge.to.as_str()).and_modify(|count| {
            *count = (*count).max(edge.destination_offset + routed_count);
        });
    }

    let mut workflow = WorkflowGraph::new();
    let mut workflow_ids = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let input_count = input_counts.get(node.id.as_str()).copied().unwrap_or(2);
        let output_count = output_counts.get(node.id.as_str()).copied().unwrap_or(2);
        let column = index % 3;
        let row = index / 3;
        let workflow_node = WorkflowNodeData::new(
            format!("{} · {}", node.id, node.plugin_type),
            Position::new(60.0 + column as f32 * 230.0, 60.0 + row as f32 * 150.0),
        )
        .with_ports(input_count, output_count)
        .with_max_ports(Some(32), Some(32))
        .with_size(200.0, 90.0 + input_count.max(output_count) as f32 * 8.0)
        .with_user_data(serde_json::json!({
            "path_node_id": node.id,
            "plugin_type": node.plugin_type,
        }));
        let workflow_id = workflow_node.id;
        workflow.add_node(workflow_node);
        workflow_ids.insert(node.id.as_str(), workflow_id);
    }

    for edge in edges {
        let (Some(&from), Some(&to)) = (
            workflow_ids.get(edge.from.as_str()),
            workflow_ids.get(edge.to.as_str()),
        ) else {
            continue;
        };
        let source_channels: Vec<usize> =
            edge.channel_map.clone().unwrap_or_else(|| (0..2).collect());
        for (index, source_channel) in source_channels.into_iter().enumerate() {
            let _ =
                workflow.add_connection(from, source_channel, to, edge.destination_offset + index);
        }
    }
    workflow
}

fn sync_listening_path_from_workflow(
    state: &mut crate::app::AppState,
    target: ListeningPathTarget,
    workflow: &WorkflowGraph,
) {
    use std::collections::{HashMap, HashSet};

    let graph_updated = state
        .app
        .ui_state
        .translations
        .listening_test
        .status
        .graph_updated;
    let listening = &mut state.app.plugin_state.listening_test_state;
    let mut workflow_to_path = HashMap::new();
    for (&workflow_id, node) in &workflow.nodes {
        if let Some(path_node_id) = node
            .user_data
            .get("path_node_id")
            .and_then(|value| value.as_str())
        {
            workflow_to_path.insert(workflow_id, path_node_id.to_owned());
        }
    }
    let surviving: HashSet<&str> = workflow_to_path.values().map(String::as_str).collect();
    let Some(PathConfig::Graph { nodes, edges }) = listening_path_config_mut(listening, target)
    else {
        return;
    };
    nodes.retain(|node| surviving.contains(node.id.as_str()));
    edges.clear();
    for connection in &workflow.connections {
        let (Some(from), Some(to)) = (
            workflow_to_path.get(&connection.from_node),
            workflow_to_path.get(&connection.to_node),
        ) else {
            continue;
        };
        edges.push(GraphEdgeConfig {
            from: from.clone(),
            to: to.clone(),
            channel_map: Some(vec![connection.from_port]),
            destination_offset: connection.to_port,
        });
    }
    listening.session = None;
    listening.status = graph_updated.into();
}

fn update_listening_node_parameters(
    state: &mut crate::app::AppState,
    target: ListeningPathTarget,
    node_id: &str,
    value: String,
) {
    let localized = state
        .app
        .ui_state
        .translations
        .listening_test
        .status
        .clone();
    let listening = &mut state.app.plugin_state.listening_test_state;
    listening.editing_path_parameters = value.clone();
    let parameters = match serde_json::from_str::<serde_json::Value>(&value) {
        Ok(parameters) if parameters.is_object() => parameters,
        Ok(_) => {
            listening.status = localized.params_object.into();
            return;
        }
        Err(error) => {
            listening.status = format!("{}: {error}", localized.params_invalid);
            return;
        }
    };
    let Some(PathConfig::Graph { nodes, .. }) = listening_path_config_mut(listening, target) else {
        return;
    };
    let Some(node) = nodes.iter_mut().find(|node| node.id == node_id) else {
        return;
    };
    node.parameters = parameters;
    listening.session = None;
    listening.status = localized.params_updated.into();
}

fn update_listening_rack_parameters(
    state: &mut crate::app::AppState,
    target: ListeningPathTarget,
    index: usize,
    value: String,
) {
    let localized = state
        .app
        .ui_state
        .translations
        .listening_test
        .status
        .clone();
    let listening = &mut state.app.plugin_state.listening_test_state;
    listening.editing_path_parameters = value.clone();
    let parameters = match serde_json::from_str::<serde_json::Value>(&value) {
        Ok(parameters) if parameters.is_object() => parameters,
        Ok(_) => {
            listening.status = localized.params_object.into();
            return;
        }
        Err(error) => {
            listening.status = format!("{}: {error}", localized.params_invalid);
            return;
        }
    };
    let Some(config) = listening_path_config_mut(listening, target) else {
        return;
    };
    match config {
        PathConfig::Plugin {
            parameters: current,
            ..
        } if index == 0 => *current = parameters,
        PathConfig::Rack { plugins } => {
            let Some(plugin) = plugins.get_mut(index) else {
                return;
            };
            plugin.parameters = parameters;
        }
        _ => return,
    }
    listening.session = None;
    listening.status = localized.rack_updated.into();
}
