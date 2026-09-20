//! Embedded, reproducible chain-level A/B and ABX listening tests.

use crate::app::App;
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

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}
use crate::app::state::plugin::{ABPathTarget, EarTrainingSurface, ListeningWorkspacePhase};
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
    Accordion, AccordionItem, Button, ButtonSize, ButtonVariant, Input, InputSize, NumberInput,
    NumberInputSize, Text, TextWeight, Toggle, ToggleSize,
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
use sotf_plugins::param_specs::ParamType;

mod blind_controller;
mod eq_training_controller;
mod workspace;

use blind_controller::{BlindComparisonSnapshot, PreparedMeasurementSnapshot};

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
    MinFrequency,
    MaxFrequency,
}

impl PlayerView {
    pub(crate) fn render_listening_test_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, translations, surface, show_listening_guide, show_break_prompt) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.clone(),
                state.app.plugin_state.listening_test_state.surface,
                state.app.tutorial.listening_guide_open,
                state.app.plugin_state.listening_test_state.surface
                    == EarTrainingSurface::BlindComparison
                    && state.app.tutorial.listening_break_prompt_open,
            )
        };
        let listening_text = &translations.listening_test;
        let eq_text = &listening_text.eq;
        let is_learning = surface != EarTrainingSurface::BlindComparison;
        let (title, subtitle) = if is_learning {
            (translations.screen_listening_test, eq_text.suite_subtitle)
        } else {
            (eq_text.mode_blind, eq_text.suite_subtitle)
        };

        let body = match surface {
            EarTrainingSurface::EqBands => self.render_eq_training_workbench(cx),
            EarTrainingSurface::Courses => self.render_eq_courses(cx),
            EarTrainingSurface::Progress => self.render_eq_progress(cx),
            EarTrainingSurface::BlindComparison => self.render_blind_comparison_screen(cx),
        };
        let measured_state = self.state.downgrade();

        div()
            .id("ear-training-screen")
            .relative()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(theme.background)
            .child(
                canvas(
                    move |bounds, _window, cx| {
                        let width = f32::from(bounds.size.width);
                        let Some(state) = measured_state.upgrade() else {
                            return;
                        };
                        if !width.is_finite()
                            || width <= 0.0
                            || state
                                .read(cx)
                                .app
                                .plugin_state
                                .plugin_ui_state
                                .listening_width
                                .is_some_and(|old| (old - width).abs() < 0.5)
                        {
                            return;
                        }
                        cx.defer(move |cx| {
                            state.update(cx, |state, cx| {
                                state.app.plugin_state.plugin_ui_state.listening_width =
                                    Some(width);
                                cx.notify();
                            })
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .flex()
            .flex_col()
            .child(dev_track!(
                div()
                    .id("ear-training-header")
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(show_listening_guide, |header| header.flex_1())
                    .p(d.pad_y)
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(d.gap)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(Text::section_header(title))
                                    .child(Text::caption(subtitle).color(theme.text_secondary)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.grid)
                                    .child(
                                        Button::new(
                                            "ear-training-learning-app",
                                            translations.screen_listening_test,
                                        )
                                        .size(ButtonSize::Sm)
                                        .variant(if is_learning {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
                                            cx.listener(|view, _, _, cx| {
                                                view.set_ear_training_surface(
                                                    EarTrainingSurface::EqBands,
                                                    cx,
                                                );
                                            }),
                                        ),
                                    )
                                    .child(dev_track!(
                                        Button::new(
                                            "listening-guide-reopen",
                                            listening_text.how_to_listen_reopen(),
                                        )
                                        .size(ButtonSize::Sm)
                                        .variant(ButtonVariant::Secondary)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
                                            cx.listener(|view, _, _, cx| {
                                                view.state.update(cx, |state, _| {
                                                    state.app.tutorial.listening_guide_open = true;
                                                });
                                                cx.notify();
                                            })
                                        ),
                                        "listening.guide.reopen"
                                    ))
                                    .child(dev_track!(
                                        Button::new("ear-training-ab-app", eq_text.mode_blind)
                                            .size(ButtonSize::Sm)
                                            .variant(if is_learning {
                                                ButtonVariant::Secondary
                                            } else {
                                                ButtonVariant::Primary
                                            })
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.set_ear_training_surface(
                                                    EarTrainingSurface::BlindComparison,
                                                    cx,
                                                );
                                            })),
                                        "listening.tab-comparison"
                                    )),
                            )
                    )
                    .when(show_listening_guide, |header| {
                        header.child(self.render_listening_guide(cx))
                    })
                    .when(show_break_prompt, |header| {
                        header.child(self.render_listening_break_prompt(cx))
                    })
                    .when(is_learning, |header| {
                        header.child(
                            div()
                                .id("ear-training-learning-navigation")
                                .flex()
                                .flex_wrap()
                                .gap(d.grid)
                                .child(dev_track!(
                                    Button::new("ear-training-eq-mode", eq_text.title)
                                        .size(ButtonSize::Sm)
                                        .variant(if surface == EarTrainingSurface::EqBands {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                            view.set_ear_training_surface(
                                                EarTrainingSurface::EqBands,
                                                cx,
                                            );
                                        })),
                                    "listening.eq.nav.practice"
                                ))
                                .child(dev_track!(
                                    Button::new("ear-training-courses", eq_text.learning.courses)
                                        .size(ButtonSize::Sm)
                                        .variant(if surface == EarTrainingSurface::Courses {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                            view.set_ear_training_surface(
                                                EarTrainingSurface::Courses,
                                                cx,
                                            )
                                        })),
                                    "listening.eq.nav.courses"
                                ))
                                .child(dev_track!(
                                    Button::new("ear-training-progress", eq_text.learning.progress)
                                        .size(ButtonSize::Sm)
                                        .variant(if surface == EarTrainingSurface::Progress {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        })
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                            view.set_ear_training_surface(
                                                EarTrainingSurface::Progress,
                                                cx,
                                            )
                                        })),
                                    "listening.eq.nav.progress"
                                )),
                        )
                    }),
                "listening.header"
            ))
            .when(!show_listening_guide, |screen| {
                screen.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .overflow_hidden()
                        .child(body),
                )
            })
    }

    fn render_listening_guide(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, text) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.listening_test.clone(),
            )
        };
        let items = text.how_to_listen_items().into_iter().enumerate().fold(
            div()
                .id("listening-guide-items")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(d.grid),
            |list, (index, item)| list.child(Text::caption(format!("{}. {item}", index + 1))),
        );

        dev_track!(
            div()
                .id("listening-guide")
                .w_full()
                .flex_1()
                .min_h_0()
                .p(d.card)
                .rounded(d.r_md)
                .border_1()
                .border_color(theme.border)
                .bg(theme.background_secondary)
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(Text::section_header(text.how_to_listen_title()))
                .child(items)
                .child(dev_track!(
                    Button::new(
                        "listening-guide-acknowledge",
                        text.how_to_listen_acknowledge(),
                    )
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Primary)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _, _, cx| {
                        let progress = view.state.update(cx, |state, _| {
                            state
                                .app
                                .plugin_state
                                .listening_test_state
                                .eq_progress
                                .mark_how_to_listen_completed();
                            state.app.tutorial.listening_guide_open = false;
                            state
                                .app
                                .plugin_state
                                .listening_test_state
                                .eq_progress
                                .clone()
                        });
                        if let Some(path) =
                            sotf_audio_player::config::get_ear_training_progress_path()
                            && let Err(error) = progress.save_atomic(&path)
                        {
                            log::warn!("Failed to save Listening Lab guide completion: {error}");
                        }
                        cx.notify();
                    })),
                    "listening.guide.acknowledge"
                )),
            "listening.guide"
        )
        .into_any_element()
    }

    fn render_listening_break_prompt(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, text, interval, completed_trials) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.listening_test.clone(),
                state.app.tutorial.listening_break_interval,
                state
                    .app
                    .plugin_state
                    .listening_test_state
                    .ab_test
                    .view()
                    .completed_trials,
            )
        };
        let state_for_interval = self.state.clone();
        let state_for_dismiss = self.state.clone();

        div()
            .id("listening-fatigue-prompt")
            .w_full()
            .p(d.card)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.warning)
            .bg(theme.background_secondary)
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(Text::section_header(text.how_to_listen_reopen()).color(theme.warning))
            .child(Text::body(text.fatigue_prompt(completed_trials)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(d.gap)
                    .child(Text::caption(text.fatigue_interval()))
                    .child(
                        NumberInput::new("listening-break-interval")
                            .value(interval as f64)
                            .range(1.0, 100.0)
                            .step(1.0)
                            .decimals(0)
                            .aria_label(text.fatigue_interval())
                            .size(NumberInputSize::Sm)
                            .width(110.0)
                            .on_change(move |value, _window, cx| {
                                state_for_interval.update(cx, |state, _| {
                                    state.app.tutorial.listening_break_interval =
                                        value.round().clamp(1.0, 100.0) as usize;
                                });
                            }),
                    )
                    .child(dev_track!(
                        Button::new("listening-fatigue-continue", text.fatigue_continue())
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(move |_, _, _window, cx| {
                                state_for_dismiss.update(cx, |state, _| {
                                    state.app.tutorial.listening_break_dismissed_at =
                                        completed_trials;
                                    state.app.tutorial.listening_break_prompt_open = false;
                                });
                                cx.notify();
                            })),
                        "listening.break.continue"
                    )),
            )
            .into_any_element()
    }

    fn render_eq_courses(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let eq_text = &state.app.ui_state.translations.listening_test.eq;
        let progress = &state.app.plugin_state.listening_test_state.eq_progress;
        let course_text =
            crate::app::i18n::EarTrainingCourseTranslations::new(state.app.ui_state.language);
        let mut courses = div().flex().flex_col().gap(d.gap);
        for course in EarTrainingCourse::ALL {
            let config = course.config();
            let completed = progress
                .sessions
                .iter()
                .filter(|session| session.course == Some(course) && !session.ended_early)
                .count();
            courses = courses.child(
                div()
                    .min_w_0()
                    .w_full()
                    .flex_1()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(Text::section_header(course_text.title(course)))
                    .child(Text::caption(format!(
                        "{} {} · {} {:.0} dB · {} {}",
                        config.band_count,
                        course_text.bands,
                        crate::app::i18n::EarTrainingCourseTranslations::change(
                            state.app.ui_state.language,
                            config.change_mode
                        ),
                        config.gain_db,
                        config.trial_count,
                        course_text.trials
                    )))
                    .child(Text::caption(format!(
                        "{completed} {}",
                        course_text.completed
                    )))
                    .child(dev_track!(
                        Button::new(
                            ("ear-course-start", course as usize),
                            eq_text.learning.start_course,
                        )
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Primary)
                        .theme(theme.to_button_theme())
                        .on_click_event(
                            cx.listener(move |view, _, _, cx| view.start_eq_course(course, cx)),
                        ),
                        format!("listening.eq.course.{}", course as usize)
                    )),
            );
        }
        div()
            .id("eq-training-courses-screen")
            .track_scroll(&self.scroll.listening_courses)
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(Text::section_header(eq_text.learning.guided_courses))
            .child(Text::caption(eq_text.learning.guided_courses_subtitle))
            .child(courses)
            .into_any_element()
    }

    fn render_eq_progress(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let eq_text = &state.app.ui_state.translations.listening_test.eq;
        let progress = &state.app.plugin_state.listening_test_state.eq_progress;
        let language = state.app.ui_state.language;
        let history_text = crate::app::i18n::EarTrainingHistoryTranslations::new(language);
        let course_text = crate::app::i18n::EarTrainingCourseTranslations::new(language);
        let recent = progress.sessions.iter().enumerate().rev().fold(
            div().flex().flex_col().gap(d.grid),
            |list, (index, session)| {
                let exercise = history_text.exercise(session.exercise);
                let title = session.course.map_or_else(
                    || exercise.to_owned(),
                    |course| format!("{} · {exercise}", course_text.title(course)),
                );
                let details = session
                    .config
                    .as_ref()
                    .map(|config| {
                        let gain = if config.exercise == EqTrainingExercise::GainIdentification {
                            sotf_audio_player::ear_training::GAIN_CHOICES_DB
                                .iter()
                                .map(|gain| format!("{gain:.0}"))
                                .collect::<Vec<_>>()
                                .join("/")
                        } else {
                            format!("{:.1}", config.gain_db)
                        };
                        format!(
                            "{} {} · {} {} dB · {:.0}–{:.0} Hz · Q {:.1}",
                            config.band_count,
                            course_text.bands,
                            crate::app::i18n::EarTrainingCourseTranslations::change(
                                language,
                                config.change_mode
                            ),
                            gain,
                            config.min_frequency_hz,
                            config.max_frequency_hz,
                            config.q
                        )
                    })
                    .unwrap_or_else(|| history_text.unavailable.to_owned());
                list.child(dev_track!(
                    div()
                        .id(("listening-history-session", index))
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(d.gap)
                        .child(Text::body(title))
                        .when(session.ended_early, |row| {
                            row.child(Text::caption(format!(
                                "{} ({}/{})",
                                state
                                    .app
                                    .ui_state
                                    .translations
                                    .listening_test
                                    .practice_lifecycle()[3],
                                session.attempts,
                                session.planned_trials.unwrap_or(session.attempts)
                            )))
                        })
                        .child(Text::caption(format!(
                            "{}/{} · {:.0}%",
                            session.correct,
                            session.attempts,
                            session.accuracy * 100.0
                        )))
                        .child(Text::caption(details)),
                    format!("listening.history.session.{index}")
                ))
            },
        );
        dev_track!(
            div()
                .id("eq-training-progress-screen")
                .track_scroll(&self.scroll.listening_history)
                .size_full()
                .overflow_y_scroll()
                .p(d.card)
                .flex()
                .flex_col()
                .gap(d.section)
                .child(Text::section_header(eq_text.learning.training_progress))
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
                                    "{}  {}",
                                    history_text.sessions,
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
                                    "{}  {:.0}%",
                                    history_text.accuracy,
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
                                .child(Text::caption(format!(
                                    "{}  {}",
                                    history_text.streak,
                                    progress.streak()
                                ))),
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
                        .child(Text::section_header(eq_text.learning.coach_recommendation))
                        .child(Text::body(
                            crate::app::i18n::EarTrainingHistoryTranslations::recommendation(
                                language,
                                progress.recommendation_details()
                            )
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
                        .child(Text::section_header(history_text.heading))
                        .child(Text::caption(history_text.compare))
                        .when(progress.sessions.is_empty(), |list| {
                            list.child(Text::body(history_text.empty))
                        })
                        .child(recent),
                ),
            "listening.history.content"
        )
        .into_any_element()
    }

    fn render_blind_comparison_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let phase = {
            let state = self.state.read(cx);
            let view = state.app.plugin_state.listening_test_state.ab_test.view();
            state
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .phase(
                    state
                        .app
                        .plugin_state
                        .listening_test_state
                        .ab_test
                        .session()
                        .and_then(|session| session.pending_mode()),
                    view.completed_trials,
                )
        };
        if phase != ListeningWorkspacePhase::Setup {
            return self.render_comparison_workspace(phase, cx);
        }
        self.ensure_listening_path_canvas(ListeningPathTarget::A, cx);
        self.ensure_listening_path_canvas(ListeningPathTarget::B, cx);

        let d = Ds::from_cx(cx);
        let preparing = self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .preparation_request
            .is_some();
        let BlindComparisonSnapshot {
            theme,
            translations,
            has_session,
            trial_ready,
            paths_ready,
            pending_mode,
            trial_count,
            score,
            segment_start_ms,
            level_match,
            status,
            prepared_measurement,
        } = BlindComparisonSnapshot::capture(self.state.read(cx));
        let status = crate::app::i18n::RuntimeMessageTranslations::for_language(
            self.state.read(cx).app.ui_state.language,
        )
        .translate(&status)
        .into_owned();
        let level_text = translations.setup.level.clone();
        let prepared_evidence = prepared_measurement.map(
            |PreparedMeasurementSnapshot {
                 media_path,
                 media_id,
                 start_ms,
                 duration_ms,
                 metric,
                 correction_b_db,
                 residual_error_db,
                 tolerance_db,
                 max_correction_db,
                 within_tolerance,
             }| {
                let confidence = if within_tolerance {
                    level_text.within_tolerance
                } else {
                    level_text.outside_tolerance
                };
                let confidence_color = if within_tolerance {
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
                                media_path,
                                start_ms as f64 / 1_000.0,
                                (start_ms + duration_ms) as f64 / 1_000.0,
                                level_text.seconds_unit,
                            )),
                    )
                    .child(
                        Text::caption(format!("{}: {}", level_text.media_identity, media_id))
                            .color(theme.text_muted),
                    )
                    .child(
                        Text::caption(format!(
                            "{} · {}: {:+.2} dB · {}: {:.3} dB / {:.3} dB · {}: ±{:.2} dB",
                            level_text.metric_label(metric),
                            level_text.correction,
                            correction_b_db,
                            level_text.residual,
                            residual_error_db,
                            tolerance_db,
                            level_text.max_correction,
                            max_correction_db,
                        ))
                        .color(theme.text_secondary),
                    )
                    .child(Text::label(confidence).color(confidence_color))
                    .child(dev_track!(
                        Button::new("load-listening-media", level_text.load_saved_media)
                            .size(ButtonSize::Xs)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.load_listening_session_media(cx);
                            })),
                        "listening.media.load"
                    ))
                    .into_any_element()
            },
        );
        let setup_step =
            |id: &'static str, number: usize, label: &'static str, complete: bool, active: bool| {
                div()
                    .id(id)
                    .flex_1()
                    .min_w(rems(12.0))
                    .p(d.pad_y)
                    .rounded(d.r_sm)
                    .border_1()
                    .border_color(if active { theme.accent } else { theme.border })
                    .bg(if active {
                        theme.accent_muted
                    } else {
                        theme.background_secondary
                    })
                    .child(
                        Text::label(if complete {
                            format!("✓ {label}")
                        } else {
                            format!("{number}. {label}")
                        })
                        .color(if active {
                            theme.text_primary
                        } else if complete {
                            theme.success
                        } else {
                            theme.text_secondary
                        }),
                    )
            };
        div()
            .id("listening-test-screen")
            .track_scroll(&self.scroll.listening_comparison)
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
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(d.gap)
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
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
                            .flex_wrap()
                            .gap(d.gap)
                            .child(self.listening_test_file_button(
                                "listening-load-session",
                                translations.setup.load_session,
                                true,
                                false,
                                cx,
                            ))
                            .when(has_session, |row| {
                                row.child(self.listening_test_file_button(
                                    "listening-save-session",
                                    translations.setup.save_session,
                                    false,
                                    false,
                                    cx,
                                ))
                            }),
                    ),
            )
            .child(
                div()
                    .id("listening-setup-steps")
                    .flex()
                    .flex_wrap()
                    .gap(d.grid)
                    .child(setup_step(
                        "listening-step-paths",
                        1,
                        translations.status.select_paths,
                        paths_ready,
                        !paths_ready,
                    ))
                    .child(setup_step(
                        "listening-step-level-match",
                        2,
                        translations.setup.level_title,
                        trial_ready,
                        paths_ready && !trial_ready,
                    ))
                    .child(setup_step(
                        "listening-step-trials",
                        3,
                        translations.trial.title,
                        trial_count > 0,
                        trial_ready,
                    )),
            )
            .child(self.render_comparison_plan(cx))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.section)
                    .child(self.render_listening_path_card(ListeningPathTarget::A, cx))
                    .child(self.render_listening_path_card(ListeningPathTarget::B, cx)),
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
                            .flex_wrap()
                            .gap(d.gap)
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .child(
                                        Text::new(translations.setup.level_title)
                                            .weight(TextWeight::Semibold),
                                    )
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_secondary)
                                            .child(translations.setup.level_description),
                                    ),
                            )
                            .when(paths_ready, |row| {
                                row.child(dev_track!(
                                    Button::new(
                                        "prepare-listening-session",
                                        if preparing {
                                            translations.preparing_label()
                                        } else {
                                            translations.setup.measure_prepare
                                        },
                                    )
                                    .size(ButtonSize::Sm)
                                    .disabled(preparing)
                                    .variant(ButtonVariant::Primary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, _, cx| {
                                            view.prepare_current_listening_session(cx);
                                        }),
                                    ),
                                    "listening.session.prepare"
                                ))
                            })
                            .when(preparing, |row| {
                                row.child(dev_track!(
                                    Button::new(
                                        "cancel-listening-preparation",
                                        translations.setup.cancel
                                    )
                                    .size(ButtonSize::Sm)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, _, cx| {
                                            view.cancel_listening_preparation(cx);
                                        })
                                    ),
                                    "listening.session.cancel_preparation"
                                ))
                            }),
                    )
                    .child({
                        let state = self.state.read(cx);
                        let source = state
                            .app
                            .get_current_track_path()
                            .and_then(|path| {
                                path.file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                            })
                            .unwrap_or_else(|| translations.eq.no_track.into());
                        let loading = state
                            .app
                            .plugin_state
                            .plugin_ui_state
                            .listening_workspace
                            .source_request
                            .is_some();
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(d.gap)
                            .child(Text::caption(source))
                            .when(!cfg!(any(target_os = "ios", target_os = "tvos")), |row| {
                                row.child(dev_track!(
                                    Button::new(
                                        "comparison-source-browse",
                                        translations.eq.choose_source()
                                    )
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Secondary)
                                    .disabled(loading || self.comparison_setup_locked(cx))
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, _, cx| {
                                            view.browse_listening_source(
                                                EarTrainingSurface::BlindComparison,
                                                cx,
                                            );
                                        })
                                    ),
                                    "listening.comparison.source.browse"
                                ))
                            })
                    })
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
                                            .value(segment_start_ms as f64 / 1_000.0)
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
                                                    let _ = listening.ab_test.clear_session();
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
                                                    let _ = listening.ab_test.clear_session();
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
                                                    let _ = listening.ab_test.clear_session();
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
                                                    let _ = listening.ab_test.clear_session();
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
                        Text::caption(if status.is_empty() {
                            translations.status.select_paths.to_owned()
                        } else {
                            status
                        })
                        .color(theme.text_secondary),
                    )
                    .map(|el| dev_track!(el, "listening.comparison.status")),
            )
            .map(|el| dev_track!(el, "listening.comparison.content"))
            .into_any_element()
    }

    fn render_eq_training_workbench(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let ui_state = &self.state.read(cx).app.ui_state;
        let scale = crate::ui::compute_combined_scale(
            ui_state.window_width,
            ui_state.window_height,
            ui_state.font_scale,
            ui_state.min_font_size_px,
            ui_state.max_font_size_px,
        );
        let workspace_width = self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_width
            .unwrap_or(480.0);
        let chart_width = (workspace_width - 2.0 * (d.card.0 + d.pad_x.0) * 16.0 * scale).max(1.0);
        let (
            theme,
            eq_text,
            current_track,
            answered,
            complete,
            has_question,
            trial_number,
            accuracy,
            answer_labels,
            selected_band,
            curve_points,
            feedback_result,
            has_session,
            eq_config,
            eq_adaptive,
            eq_filtered,
            status,
        ) = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            let session = listening.eq_session.as_ref();
            let answered = session.is_some_and(EqTrainingSession::current_is_answered);
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
            let correct_answer = session.and_then(|session| {
                session
                    .current_question
                    .as_ref()
                    .map(|question| question.correct_answer(listening.eq_config.exercise))
            });
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.listening_test.eq.clone(),
                state.app.get_current_track_path().and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                }),
                answered,
                session.is_some_and(EqTrainingSession::is_finished),
                session.is_some_and(|session| session.current_question.is_some()),
                session
                    .and_then(|session| session.current_question.as_ref())
                    .map_or_else(
                        || session.map_or(0, |session| session.trials.len()),
                        |question| question.number + 1,
                    ),
                session.map_or(0.0, EqTrainingSession::accuracy) * 100.0,
                answer_labels
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| (label, answered && correct_answer == Some(index)))
                    .collect::<Vec<_>>(),
                selected_band,
                session
                    .and_then(|session| session.current_question.as_ref())
                    .filter(|_| answered)
                    .map(|question| question.preview_curve(160))
                    .unwrap_or_else(|| vec![(20.0, 0.0), (20_000.0, 0.0)]),
                session
                    .and_then(|session| session.trials.last())
                    .filter(|_| answered)
                    .map(|result| {
                        (
                            result.correct,
                            result.question.center_frequency_hz,
                            result.question.signed_gain_db(),
                            result.question.q,
                        )
                    }),
                session.is_some(),
                listening.eq_config.clone(),
                listening.eq_adaptive,
                listening.eq_filtered,
                listening.status.clone(),
            )
        };
        let current_track = current_track.unwrap_or_else(|| eq_text.no_track.into());
        let (x_values, y_values): (Vec<_>, Vec<_>) = curve_points.into_iter().unzip();
        let chart = render_line_chart(
            vec![Series::new(
                eq_text.chart_series(),
                channel_color(&theme, 0),
                x_values,
                y_values,
            )],
            ChartConfig {
                title: None,
                x_label: Some(eq_text.frequency_axis().into()),
                y_label: Some(eq_text.gain_axis().into()),
                x_range: (20.0, 20_000.0),
                y_range: (-16.0, 16.0),
                x_scale: gpui_px::ScaleType::Log,
                width: chart_width,
                height: 220.0,
            },
            &theme,
            None,
        );

        let practice = &self
            .state
            .read(cx)
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .practice;
        let paused = practice.paused;
        let confirming = practice.confirm_end;
        let locked = practice.interaction_locked();
        let lifecycle = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .listening_test
            .practice_lifecycle();
        let early = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .eq_session
            .as_ref()
            .is_some_and(|session| session.ended_early);
        let submitted = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .eq_session
            .as_ref()
            .map_or(0, |session| session.trials.len());
        let planned = self
            .state
            .read(cx)
            .app
            .plugin_state
            .listening_test_state
            .eq_session
            .as_ref()
            .map_or(0, |session| session.config.trial_count);
        let mut answers = div().flex().flex_wrap().gap(d.gap);
        for (index, (answer_label, is_answer)) in answer_labels.iter().enumerate() {
            let is_selected = index == selected_band;
            let label = if *is_answer {
                format!("{answer_label} ✓")
            } else if answered && is_selected {
                format!("{answer_label} •")
            } else {
                answer_label.clone()
            };
            let button = Button::new(("eq-training-band", index), label)
                .disabled(locked || answered)
                .size(ButtonSize::Sm)
                .variant(if is_selected || *is_answer {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                    view.select_eq_training_band(index, cx);
                }));
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(format!("listening.eq.answer.{index}"));
            answers = answers.child(button);
        }

        let feedback = feedback_result.map(|(correct, center_frequency_hz, signed_gain_db, q)| {
            if correct {
                format!(
                    "{} — {} at {:+.0} dB, Q {:.1}",
                    eq_text.correct,
                    format_frequency(center_frequency_hz),
                    signed_gain_db,
                    q
                )
            } else {
                format!(
                    "{}: {} at {:+.0} dB, Q {:.1}",
                    eq_text.learning.answer,
                    format_frequency(center_frequency_hz),
                    signed_gain_db,
                    q
                )
            }
        });

        div()
            .id("eq-training-workbench")
            .track_scroll(&self.scroll.listening_practice)
            .size_full()
            .min_h_0()
            .min_w_0()
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
                            .min_w_0()
                            .flex_1()
                            .child(Text::section_header(eq_text.title))
                            .child(Text::caption(eq_text.subtitle)),
                    )
                    .child(Text::caption(eq_text.trial_progress(
                        trial_number,
                        eq_config.trial_count,
                        accuracy,
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.section)
                    .when(!has_session, |row| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .w_full()
                                .p(d.pad_x)
                                .rounded(d.r_md)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.surface)
                                .flex()
                                .flex_col()
                                .gap(d.gap)
                                .max_w(rems(64.0))
                                .items_start()
                                .child(Text::section_header(eq_text.session_setup))
                                .child(self.render_eq_source_row(
                                    eq_text.source,
                                    &current_track,
                                    cx,
                                ))
                                .child(
                                    Button::new(
                                        "eq-training-exercise",
                                        eq_text.exercise_display(eq_config.exercise),
                                    )
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(cx.listener(|view, _, _, cx| {
                                        view.cycle_eq_training_exercise(cx)
                                    }))
                                    .map(|button| dev_track!(button, "listening.eq.exercise")),
                                )
                                .child(
                                    Button::new(
                                        "eq-training-adaptive",
                                        eq_text.adaptive_status(eq_adaptive),
                                    )
                                    .size(ButtonSize::Sm)
                                    .variant(if eq_adaptive {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    })
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, _, cx| {
                                            view.toggle_eq_training_adaptive(cx)
                                        }),
                                    ),
                                )
                                .child(self.render_eq_config_row(
                                    eq_text.bands,
                                    EqConfigField::Bands,
                                    cx,
                                ))
                                .child(
                                    if eq_config.exercise == EqTrainingExercise::GainIdentification
                                    {
                                        let choices =
                                            sotf_audio_player::ear_training::GAIN_CHOICES_DB
                                                .iter()
                                                .map(|gain| format!("{gain} dB"))
                                                .collect::<Vec<_>>()
                                                .join(", ");
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(d.gap)
                                            .child(Text::label(eq_text.gain_choices()))
                                            .child(dev_track!(
                                                Text::body(choices),
                                                "listening.eq.gain-choices"
                                            ))
                                            .into_any_element()
                                    } else {
                                        self.render_eq_config_row(
                                            eq_text.gain,
                                            EqConfigField::Gain,
                                            cx,
                                        )
                                        .into_any_element()
                                    },
                                )
                                .child(self.render_eq_frequency_details(cx))
                                .child(self.render_eq_config_row(
                                    eq_text.trials,
                                    EqConfigField::Trials,
                                    cx,
                                ))
                                .child(
                                    Button::new(
                                        "eq-training-change-mode",
                                        format!(
                                            "{}: {}",
                                            eq_text.change,
                                            eq_change_mode_symbol(eq_config.change_mode)
                                        ),
                                    )
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, _, cx| {
                                            view.cycle_eq_training_change_mode(cx);
                                        }),
                                    ),
                                )
                                .child(dev_track!(
                                    Button::new(
                                        "eq-training-start",
                                        if has_session {
                                            eq_text.restart
                                        } else {
                                            eq_text.start
                                        },
                                    )
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Primary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(|view, _, window, cx| {
                                            view.start_eq_training_session(cx);
                                            view.focus_handle.focus(window, cx);
                                        })
                                    ),
                                    "listening.eq.start"
                                ))
                                .child(Text::caption(eq_text.learning.audition_path_hint)),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .w_full()
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(Text::section_header(if confirming {
                                lifecycle[0]
                            } else if early {
                                lifecycle[3]
                            } else if complete {
                                eq_text.complete
                            } else if has_session {
                                eq_text.question
                            } else {
                                eq_text.start_prompt
                            }))
                            .when(has_question && !confirming && !complete, |panel| {
                                panel.child(Text::caption(eq_text.trial_progress(
                                    trial_number,
                                    planned,
                                    accuracy,
                                )))
                            })
                            .when(complete, |panel| {
                                panel.child(Text::body(format!(
                                    "{submitted}/{planned} · {accuracy:.0}%"
                                )))
                            })
                            .when(complete, |panel| {
                                panel.child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap(d.gap)
                                        .child(dev_track!(
                                            Button::new("eq-practice-retry", eq_text.restart)
                                                .size(ButtonSize::Sm)
                                                .variant(ButtonVariant::Primary)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(
                                                    |view, _, window, cx| {
                                                        view.start_eq_training_session(cx);
                                                        view.focus_handle.focus(window, cx);
                                                    }
                                                )),
                                            "listening.eq.retry"
                                        ))
                                        .child(dev_track!(
                                            Button::new("eq-practice-new", lifecycle[6])
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(
                                                    |view, _, window, cx| {
                                                        view.state.update(cx, |state, _| {
                                                            state
                                                                .app
                                                                .plugin_state
                                                                .listening_test_state
                                                                .eq_session = None;
                                                        });
                                                        view.focus_handle.focus(window, cx);
                                                        cx.notify();
                                                    }
                                                )),
                                            "listening.eq.new-practice"
                                        )),
                                )
                            })
                            .when(paused, |panel| panel.child(Text::body(lifecycle[5])))
                            .when(has_question && !confirming, |panel| {
                                let text =
                                    &self.state.read(cx).app.ui_state.translations.listening_test;
                                panel.child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap(d.gap)
                                        .child(dev_track!(
                                            Button::new(
                                                "eq-practice-pause",
                                                text.pause_resume(paused)
                                            )
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, window, cx| {
                                                view.toggle_eq_practice_pause(cx);
                                                view.focus_handle.focus(window, cx);
                                            })),
                                            "listening.eq.pause"
                                        ))
                                        .child(dev_track!(
                                            Button::new("eq-practice-end", lifecycle[2])
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(
                                                    |view, _, window, cx| {
                                                        view.confirm_eq_practice_end(
                                                            true, window, cx,
                                                        );
                                                    }
                                                )),
                                            "listening.eq.end"
                                        )),
                                )
                            })
                            .when(confirming, |panel| {
                                panel.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(d.gap)
                                        .child(Text::body(format!("{}: {submitted}", lifecycle[4])))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_wrap()
                                                .gap(d.gap)
                                                .child(dev_track!(
                                                    Button::new("eq-practice-keep", lifecycle[1])
                                                        .size(ButtonSize::Sm)
                                                        .theme(theme.to_button_theme())
                                                        .on_click_event(cx.listener(
                                                            |view, _, window, cx| {
                                                                view.confirm_eq_practice_end(
                                                                    false, window, cx,
                                                                );
                                                            }
                                                        )),
                                                    "listening.eq.keep-practicing"
                                                ))
                                                .child(dev_track!(
                                                    Button::new(
                                                        "eq-practice-confirm-end",
                                                        lifecycle[2]
                                                    )
                                                    .size(ButtonSize::Sm)
                                                    .variant(ButtonVariant::Primary)
                                                    .theme(theme.to_button_theme())
                                                    .on_click_event(cx.listener(
                                                        |view, _, window, cx| {
                                                            view.end_eq_practice(window, cx);
                                                        }
                                                    )),
                                                    "listening.eq.confirm-end"
                                                )),
                                        ),
                                )
                            })
                            .when(has_question && !complete && !locked, |panel| {
                                panel.child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap(d.gap)
                                        .child(dev_track!(
                                            Button::new(
                                                "eq-training-original",
                                                format!("1  {}", eq_text.original),
                                            )
                                            .disabled(locked)
                                            .size(ButtonSize::Sm)
                                            .variant(if !eq_filtered {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.activate_eq_training_path(false, cx);
                                            }),),
                                            "listening.eq.original"
                                        ))
                                        .child(dev_track!(
                                            Button::new(
                                                "eq-training-filtered",
                                                format!("2  {}", eq_text.filtered),
                                            )
                                            .disabled(locked)
                                            .size(ButtonSize::Sm)
                                            .variant(if eq_filtered {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.activate_eq_training_path(true, cx);
                                            }),),
                                            "listening.eq.filtered"
                                        )),
                                )
                            })
                            .when(has_question && !locked, |panel| panel.child(answers))
                            .when(answered && !locked, |panel| {
                                panel.child(div().w_full().min_w_0().child(chart))
                            })
                            .when_some(feedback.filter(|_| !locked), |panel, feedback| {
                                panel.child(Text::body(feedback).color(
                                    if feedback_result.is_some_and(|result| result.0) {
                                        theme.success
                                    } else {
                                        theme.warning
                                    },
                                ))
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.gap)
                                    .when(has_question && !answered && !locked, |row| {
                                        row.child(dev_track!(
                                            Button::new(
                                                "eq-training-submit",
                                                format!("Enter  {}", eq_text.submit),
                                            )
                                            .disabled(locked)
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Primary)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.submit_eq_training_answer(cx);
                                            })),
                                            "listening.eq.submit"
                                        ))
                                    })
                                    .when(answered && !locked, |row| {
                                        row.child(dev_track!(
                                            Button::new(
                                                "eq-training-next",
                                                format!("N  {}", eq_text.next),
                                            )
                                            .disabled(locked)
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Primary)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.advance_eq_training_question(cx);
                                            })),
                                            "listening.eq.next"
                                        ))
                                    }),
                            )
                            .when(has_question && !locked, |panel| {
                                panel.child(Text::caption(eq_text.shortcuts))
                            }),
                    ),
            )
            .child(
                div()
                    .p(d.pad_y)
                    .rounded(d.r_sm)
                    .bg(theme.background_secondary)
                    .child(Text::caption(if status.is_empty() && complete {
                        if submitted > 0 {
                            lifecycle[4]
                        } else {
                            lifecycle[3]
                        }
                    } else if status.is_empty() {
                        eq_text.configure_start
                    } else {
                        &status
                    })),
            )
            .map(|body| dev_track!(body, "listening.eq.content"))
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
        let source_loading = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .source_request
            .is_some();
        div()
            .flex()
            .flex_col()
            .gap(d.grid)
            .child(Text::label(source_label))
            .child(Text::caption(current_track.to_owned()))
            .when(!cfg!(any(target_os = "ios", target_os = "tvos")), |row| {
                row.child(dev_track!(
                    Button::new("eq-source-browse", eq_text.choose_source())
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .disabled(source_loading)
                        .theme(theme.to_button_theme())
                        .on_click_event(
                            cx.listener(|view, _, _, cx| view.browse_eq_training_source(cx))
                        ),
                    "listening.eq.source.browse"
                ))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.grid)
                    .child(dev_track!(
                        Button::new("eq-source-add", eq_text.learning.add_current)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.add_current_eq_source(cx)),
                            ),
                        "listening.eq.source.add"
                    ))
                    .child(dev_track!(
                        Button::new("eq-source-prev", eq_text.learning.previous)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.navigate_eq_source(-1, cx)),
                            ),
                        "listening.eq.source.previous"
                    ))
                    .child(dev_track!(
                        Button::new("eq-source-next", eq_text.next)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.navigate_eq_source(1, cx)),
                            ),
                        "listening.eq.source.next"
                    ))
                    .child(
                        Button::new("eq-loop-start", eq_text.learning.set_loop_start)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.set_eq_loop_boundary(true, cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-loop-end", eq_text.learning.set_loop_end)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(
                                cx.listener(|view, _, _, cx| view.set_eq_loop_boundary(false, cx)),
                            ),
                    )
                    .child(
                        Button::new("eq-loop-toggle", eq_text.learning.toggle_loop)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| view.toggle_eq_loop(cx))),
                    ),
            )
    }

    fn render_eq_frequency_details(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let text = state.app.ui_state.translations.listening_test.eq.clone();
        let expanded = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .practice
            .frequency_details_open;
        let owner = cx.entity().downgrade();
        let d = Ds::from_cx(cx);
        dev_track!(
            Accordion::new()
                .aria_label(text.frequency_details())
                .bordered(false)
                .expanded(if expanded {
                    vec!["practice-frequency".into()]
                } else {
                    vec![]
                })
                .item(
                    AccordionItem::new("practice-frequency", text.frequency_details()).content(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(self.render_eq_config_row(
                                text.minimum_frequency(),
                                EqConfigField::MinFrequency,
                                cx
                            ))
                            .child(self.render_eq_config_row(
                                text.maximum_frequency(),
                                EqConfigField::MaxFrequency,
                                cx
                            ))
                            .child(self.render_eq_config_row("Q", EqConfigField::Q, cx))
                    )
                )
                .on_change(move |_, expanded, _, cx| {
                    let _ = owner.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state
                                .app
                                .plugin_state
                                .plugin_ui_state
                                .listening_workspace
                                .practice
                                .frequency_details_open = expanded;
                        });
                        cx.notify();
                    });
                }),
            "listening.eq.frequency-details"
        )
    }

    fn render_eq_config_row(
        &self,
        label: &'static str,
        field: EqConfigField,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let config = &state.app.plugin_state.listening_test_state.eq_config;
        let (value, min, max, step, decimals, unit, _key) = match field {
            EqConfigField::Bands => (config.band_count as f64, 2.0, 25.0, 1.0, 0, "", "bands"),
            EqConfigField::Gain => (config.gain_db, 1.0, 15.0, 0.5, 1, "dB", "gain"),
            EqConfigField::Q => (config.q, 0.2, 10.0, 0.1, 1, "", "q"),
            EqConfigField::Trials => (config.trial_count as f64, 1.0, 500.0, 1.0, 0, "", "trials"),
            EqConfigField::MinFrequency => (
                config.min_frequency_hz,
                20.0,
                config.max_frequency_hz - 0.1,
                0.1,
                1,
                "Hz",
                "min-frequency",
            ),
            EqConfigField::MaxFrequency => (
                config.max_frequency_hz,
                config.min_frequency_hz + 0.1,
                20000.0,
                0.1,
                1,
                "Hz",
                "max-frequency",
            ),
        };
        let owner = cx.entity().downgrade();
        let input = NumberInput::new(("eq-config-value", field as usize))
            .value(value)
            .scroll_requires_alt(true)
            .range(min, max)
            .step(step)
            .decimals(decimals)
            .unit(unit)
            .size(NumberInputSize::Sm)
            .width(150.0)
            .aria_label(label)
            .on_change(move |value, _, cx| {
                let _ = owner.update(cx, |view, cx| view.set_eq_training_config(field, value, cx));
            });
        div()
            .w_full()
            .max_w(rems(32.0))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(d.gap)
            .child(Text::label(label))
            .child(dev_track!(input, format!("listening.eq.config.{_key}")))
    }

    fn ensure_listening_path_canvas(&self, target: ListeningPathTarget, cx: &mut Context<Self>) {
        let config = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            let (config, has_canvas) = match target {
                ListeningPathTarget::A => {
                    (listening.path_a.as_ref(), listening.path_a_canvas.is_some())
                }
                ListeningPathTarget::B => {
                    (listening.path_b.as_ref(), listening.path_b_canvas.is_some())
                }
            };
            if has_canvas || !matches!(config, Some(PathConfig::Graph { .. })) {
                return;
            }
            config.cloned()
        };
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
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let suffix = match target {
            ListeningPathTarget::A => "a",
            ListeningPathTarget::B => "b",
        };
        let (
            theme,
            translations,
            label,
            summary,
            is_graph,
            linear_plugins,
            canvas,
            add_menu_open,
            editing_node_id,
            editing_node_plugin_type,
            editing_parameters,
        ) = {
            let state = self.state.read(cx);
            let listening = &state.app.plugin_state.listening_test_state;
            let (label, config, canvas) = match target {
                ListeningPathTarget::A => (
                    &listening.path_a_label,
                    listening.path_a.as_ref(),
                    listening.path_a_canvas.clone(),
                ),
                ListeningPathTarget::B => (
                    &listening.path_b_label,
                    listening.path_b.as_ref(),
                    listening.path_b_canvas.clone(),
                ),
            };
            let editing_node_id = if listening.editing_path_target == Some(target.into()) {
                listening.editing_path_node_id.clone()
            } else {
                None
            };
            let editing_node_plugin_type = match (config, editing_node_id.as_ref()) {
                (Some(PathConfig::Graph { nodes, .. }), Some(node_id)) => nodes
                    .iter()
                    .find(|node| node.id == *node_id)
                    .map(|node| node.plugin_type.clone()),
                _ => None,
            };
            let linear_plugins = match config {
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
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.listening_test.clone(),
                label.to_owned(),
                path_summary(config, &state.app.ui_state.translations.listening_test),
                matches!(config, Some(PathConfig::Graph { .. })),
                linear_plugins,
                canvas,
                listening.graph_add_menu_target == Some(target.into()),
                editing_node_id,
                editing_node_plugin_type,
                listening.editing_path_parameters.clone(),
            )
        };
        div()
            .flex_1()
            .min_w_0()
            .flex_basis(rems(20.0))
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
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .child(Text::new(label.to_owned()).weight(TextWeight::Semibold))
                    .child(Text::caption(summary)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
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
            .when(!is_graph, |card| {
                card.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .child(Text::caption(translations.setup.simple_rack))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
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
            .when(is_graph, |card| {
                card.child(
                    div()
                        .flex()
                        .flex_wrap()
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
                    let state_for_cancel = self.state.clone();
                    let node_id_for_close = node_id.clone();
                    let node_id_for_input = node_id.clone();
                    let node_plugin_type = editing_node_plugin_type.clone();
                    let node_editor_id = format!("{suffix}-graph-{node_id}");
                    let reset_label = translations.parameter_reset();
                    let use_expert_editor = node_plugin_type
                        .as_deref()
                        .and_then(sotf_plugins::param_specs::for_plugin_type)
                        .is_none_or(|specs| specs.is_empty());
                    let editor_label = translations.parameter_editor(use_expert_editor);
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
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .child(Text::caption(format!("{node_id} · {}", editor_label)))
                                    .child(
                                        Button::new(
                                            SharedString::from(format!(
                                                "listening-cancel-node-{suffix}"
                                            )),
                                            translations.setup.cancel,
                                        )
                                        .size(ButtonSize::Xs)
                                        .variant(ButtonVariant::Secondary)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(
                                            move |_, _, cx| {
                                                state_for_cancel.update(cx, |state, _| {
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
                                    )
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
                                        let draft = state
                                            .app
                                            .plugin_state
                                            .listening_test_state
                                            .editing_path_parameters
                                            .clone();
                                        let valid_draft = matches!(
                                            serde_json::from_str::<serde_json::Value>(&draft),
                                            Ok(parameters) if parameters.is_object()
                                        );
                                        if !valid_draft {
                                            let localized = state
                                                .app
                                                .ui_state
                                                .translations
                                                .listening_test
                                                .status
                                                .clone();
                                            state
                                                .app
                                                .plugin_state
                                                .listening_test_state
                                                .status = localized.params_invalid.into();
                                            return;
                                        }
                                        update_listening_node_parameters(
                                            state,
                                            target,
                                            &node_id_for_close,
                                            draft,
                                        );
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
                                div()
                                    .when_some(node_plugin_type.clone(), |editor, plugin_type| {
                                        editor.child(self.render_listening_parameter_editor(
                                            &node_editor_id,
                                            &plugin_type,
                                            &editing_parameters,
                                            reset_label,
                                            cx,
                                        ))
                                    })
                                    .when(
                                        node_plugin_type
                                            .as_deref()
                                            .and_then(sotf_plugins::param_specs::for_plugin_type)
                                            .is_none_or(|specs| specs.is_empty()),
                                        |editor| {
                                            editor.child(
                                                Input::new(SharedString::from(format!(
                                                    "listening-node-params-{suffix}"
                                                )))
                                                .value(editing_parameters)
                                                .placeholder(r#"{"parameter": "value"}"#)
                                                .size(InputSize::Sm)
                                                .on_text_change(move |value, _window, cx| {
                                                    state_for_params.update(cx, |state, _| {
                                                        stage_listening_node_parameters(
                                                            state,
                                                            target,
                                                            &node_id_for_input,
                                                            value.to_string(),
                                                        );
                                                    });
                                                }),
                                            )
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
            let button = Button::new(SharedString::from(format!("listening-cue-{label}")), label)
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Primary)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, _, cx| {
                    view.activate_listening_cue(cue, cx);
                }));
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(format!("listening.cue.{cue:?}"));
            row = row.child(button);
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

    fn render_listening_parameter_editor(
        &self,
        editor_id: &str,
        plugin_type: &str,
        edit_value: &str,
        reset_label: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let values = serde_json::from_str::<serde_json::Value>(edit_value)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        let Some(specs) = sotf_plugins::param_specs::for_plugin_type(plugin_type) else {
            return div();
        };

        let mut controls = div().flex().flex_wrap().gap(d.gap).w_full();
        for spec in specs {
            let value = values
                .get(spec.engine_key)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or_else(|| spec.default_f64());
            let key = spec.engine_key.to_string();
            let state_for_value = self.state.clone();
            let state_for_reset = self.state.clone();
            let mut control = div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .min_w(rems(10.0))
                .child(Text::label(spec.name));

            match spec.param_type {
                ParamType::Float { step, .. } => {
                    let mut input = NumberInput::new(SharedString::from(format!(
                        "listening-param-{editor_id}-{}",
                        spec.engine_key
                    )))
                    .value(value)
                    .range(spec.min_f64(), spec.max_f64())
                    .step(step)
                    .decimals(spec.precision())
                    .size(NumberInputSize::Xs)
                    .aria_label(format!("{} value", spec.name))
                    .on_change(move |value, _window, cx| {
                        state_for_value.update(cx, |state, _| {
                            set_listening_staged_parameter(
                                state,
                                &key,
                                serde_json::Value::from(value),
                            );
                        });
                    });
                    if !spec.unit.is_empty() {
                        input = input.unit(spec.unit);
                    }
                    control = control.child(input);
                }
                ParamType::Int { step, .. } => {
                    let mut input = NumberInput::new(SharedString::from(format!(
                        "listening-param-{editor_id}-{}",
                        spec.engine_key
                    )))
                    .value(value)
                    .range(spec.min_f64(), spec.max_f64())
                    .step(step as f64)
                    .decimals(0)
                    .size(NumberInputSize::Xs)
                    .aria_label(format!("{} value", spec.name))
                    .on_change(move |value, _window, cx| {
                        state_for_value.update(cx, |state, _| {
                            set_listening_staged_parameter(
                                state,
                                &key,
                                serde_json::Value::from(value.round() as i64),
                            );
                        });
                    });
                    if !spec.unit.is_empty() {
                        input = input.unit(spec.unit);
                    }
                    control = control.child(input);
                }
                ParamType::Bool { .. } => {
                    let checked = values
                        .get(spec.engine_key)
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or_else(|| spec.default_bool());
                    control = control.child(
                        Toggle::new(SharedString::from(format!(
                            "listening-param-{editor_id}-{}",
                            spec.engine_key
                        )))
                        .checked(checked)
                        .size(ToggleSize::Sm)
                        .on_change(move |checked, _window, cx| {
                            state_for_value.update(cx, |state, _| {
                                set_listening_staged_parameter(
                                    state,
                                    &key,
                                    serde_json::Value::from(checked),
                                );
                            });
                        }),
                    );
                }
                ParamType::Choice { .. } => {
                    let choice_label = spec.format_value(value);
                    control = control.child(
                        Button::new(
                            SharedString::from(format!(
                                "listening-param-{editor_id}-{}",
                                spec.engine_key
                            )),
                            choice_label,
                        )
                        .size(ButtonSize::Xs)
                        .variant(ButtonVariant::Secondary)
                        .theme(theme.to_button_theme())
                        .aria_label(format!("{}: select next option", spec.name))
                        .on_click_event(move |_, _, cx| {
                            state_for_value.update(cx, |state, _| {
                                let current = state
                                    .app
                                    .plugin_state
                                    .listening_test_state
                                    .editing_path_parameters
                                    .parse::<serde_json::Value>()
                                    .ok()
                                    .and_then(|value| {
                                        value.get(&key).and_then(serde_json::Value::as_f64)
                                    })
                                    .unwrap_or_else(|| spec.default_f64());
                                set_listening_staged_parameter(
                                    state,
                                    &key,
                                    serde_json::Value::from(spec.adjust_f64(current, 1.0) as i64),
                                );
                            });
                        }),
                    );
                }
                ParamType::FilePath => continue,
            }

            let key_for_reset = spec.engine_key.to_string();
            let default = match spec.param_type {
                ParamType::Bool { .. } => serde_json::Value::from(spec.default_bool()),
                ParamType::Choice { .. } | ParamType::Int { .. } => {
                    serde_json::Value::from(spec.default_f64() as i64)
                }
                ParamType::Float { .. } => serde_json::Value::from(spec.default_f64()),
                ParamType::FilePath => unreachable!(),
            };
            controls = controls.child(
                control.child(
                    Button::new(
                        SharedString::from(format!(
                            "listening-param-reset-{editor_id}-{}",
                            spec.engine_key
                        )),
                        reset_label,
                    )
                    .size(ButtonSize::Xs)
                    .variant(ButtonVariant::Ghost)
                    .theme(theme.to_button_theme())
                    .aria_label(format!("Reset {}", spec.name))
                    .on_click_event(move |_, _, cx| {
                        state_for_reset.update(cx, |state, _| {
                            set_listening_staged_parameter(state, &key_for_reset, default.clone());
                        });
                    }),
                ),
            );
        }
        controls
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
        let state_for_cancel = self.state.clone();
        let parameters = plugin.parameters.clone();
        let reset_label = state
            .app
            .ui_state
            .translations
            .listening_test
            .parameter_reset();
        let plugin_type = plugin.plugin_type.clone();
        let parameter_editor_id = format!("{suffix}-rack-{index}");
        let use_expert_editor = sotf_plugins::param_specs::for_plugin_type(&plugin_type)
            .is_none_or(|specs| specs.is_empty());
        let editor_label = state
            .app
            .ui_state
            .translations
            .listening_test
            .parameter_editor(use_expert_editor);
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
                                translations.done
                            } else {
                                editor_label
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
                                if is_editing {
                                    let draft = state
                                        .app
                                        .plugin_state
                                        .listening_test_state
                                        .editing_path_parameters
                                        .clone();
                                    let valid_draft = matches!(
                                        serde_json::from_str::<serde_json::Value>(&draft),
                                        Ok(parameters) if parameters.is_object()
                                    );
                                    if !valid_draft {
                                        let localized = state
                                            .app
                                            .ui_state
                                            .translations
                                            .listening_test
                                            .status
                                            .clone();
                                        state.app.plugin_state.listening_test_state.status =
                                            localized.params_invalid.into();
                                        return;
                                    }
                                    update_listening_rack_parameters(state, target, index, draft);
                                    let listening =
                                        &mut state.app.plugin_state.listening_test_state;
                                    listening.editing_path_target = None;
                                    listening.editing_path_node_id = None;
                                    listening.editing_path_parameters.clear();
                                    return;
                                }
                                let listening = &mut state.app.plugin_state.listening_test_state;
                                listening.editing_path_target = Some(target.into());
                                listening.editing_path_node_id = Some(edit_id);
                                listening.editing_path_parameters =
                                    serde_json::to_string_pretty(&parameters)
                                        .unwrap_or_else(|_| "{}".into());
                            });
                        }),
                    )
                    .when(is_editing, |buttons| {
                        buttons.child(
                            Button::new(
                                SharedString::from(format!("listening-{suffix}-cancel-{index}")),
                                translations.cancel,
                            )
                            .size(ButtonSize::Xs)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(move |_, _, cx| {
                                state_for_cancel.update(cx, |state, _| {
                                    let listening =
                                        &mut state.app.plugin_state.listening_test_state;
                                    listening.editing_path_target = None;
                                    listening.editing_path_node_id = None;
                                    listening.editing_path_parameters.clear();
                                });
                            }),
                        )
                    })
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
                let specs = sotf_plugins::param_specs::for_plugin_type(&plugin_type);
                row.child(
                    div()
                        .w_full()
                        .child(self.render_listening_parameter_editor(
                            &parameter_editor_id,
                            &plugin_type,
                            &edit_value,
                            reset_label,
                            cx,
                        ))
                        .when(specs.is_none_or(|specs| specs.is_empty()), |editor| {
                            editor.child(
                                Input::new(SharedString::from(format!(
                                    "listening-{suffix}-params-{index}"
                                )))
                                .value(edit_value)
                                .placeholder(r#"{"parameter": "value"}"#)
                                .size(InputSize::Sm)
                                .on_text_change(
                                    move |value, _window, cx| {
                                        state_for_edit.update(cx, |state, _| {
                                            stage_listening_rack_parameters(
                                                state,
                                                target,
                                                index,
                                                value.to_string(),
                                            );
                                        });
                                    },
                                ),
                            )
                        }),
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
            let _ = listening.ab_test.clear_session();
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
            let _ = listening.ab_test.clear_session();
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
            let _ = listening.ab_test.clear_session();
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
            let _ = listening.ab_test.clear_session();
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
            let selected = self
                .state
                .read(cx)
                .app
                .plugin_state
                .plugin_ui_state
                .listening_workspace
                .selected_answer
                == Some(answer);
            let button = Button::new(
                SharedString::from(format!("listening-answer-{label}")),
                label,
            )
            .size(ButtonSize::Sm)
            .variant(if selected {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            })
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |view, _, _, cx| {
                view.state.update(cx, |state, cx| {
                    state
                        .app
                        .plugin_state
                        .plugin_ui_state
                        .listening_workspace
                        .selected_answer = Some(answer);
                    cx.notify();
                });
            }));
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(format!("listening.answer.{answer:?}"));
            row = row.child(button);
        }
        let state = self.state.read(cx);
        let can_submit = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .can_submit(mode);
        let theme = state.app.ui_state.theme.clone();
        let submit = state
            .app
            .ui_state
            .translations
            .listening_test
            .workspace()
            .submit;
        row.child(
            Button::new("comparison-submit", submit)
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Primary)
                .theme(theme.to_button_theme())
                .disabled(!can_submit)
                .on_click_event(
                    cx.listener(move |view, _, _, cx| view.submit_comparison_answer(mode, cx)),
                ),
        )
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
        let state = self.state.read(cx);
        let listening = &state.app.plugin_state.listening_test_state;
        let confidence = listening.confidence;
        let notes = listening.notes.clone();
        let expanded = state
            .app
            .plugin_state
            .plugin_ui_state
            .listening_workspace
            .metadata_open;
        let title = state.app.ui_state.translations.listening_test.disclosures()[0];
        let owner = cx.entity().downgrade();
        let state_for_confidence = self.state.clone();
        let state_for_notes = self.state.clone();
        let content = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(d.gap)
            .child(
                div().w(rems(9.0)).child(
                    NumberInput::new("listening-confidence")
                        .label(translations.confidence)
                        .aria_label(translations.confidence)
                        .value(confidence.map_or(50.0, f64::from))
                        .range(0.0, 100.0)
                        .step(5.0)
                        .decimals(0)
                        .unit("%")
                        .size(NumberInputSize::Sm)
                        .on_change(move |value, _window, cx| {
                            state_for_confidence.update(cx, |state, _| {
                                state.app.plugin_state.listening_test_state.confidence =
                                    Some(value.clamp(0.0, 100.0) as u8);
                            });
                        }),
                ),
            )
            .child(
                div().flex_1().min_w(rems(12.0)).child(
                    Input::new("listening-notes")
                        .value(notes)
                        .aria_label(translations.notes_placeholder)
                        .placeholder(translations.notes_placeholder)
                        .size(InputSize::Sm)
                        .on_text_change(move |value, _window, cx| {
                            state_for_notes.update(cx, |state, _| {
                                state.app.plugin_state.listening_test_state.notes =
                                    value.to_string();
                            });
                        }),
                ),
            )
            .text_color(theme.text_primary);
        div().child(dev_track!(
            Accordion::new()
                .aria_label(title)
                .bordered(false)
                .expanded(if expanded {
                    vec!["listening-metadata".into()]
                } else {
                    vec![]
                })
                .item(
                    AccordionItem::new("listening-metadata", title)
                        .trailing(self.listening_disclosure_shortcut(
                            &crate::app::actions::ListeningToggleMetadata,
                            cx
                        ))
                        .content(dev_track!(content, "listening.metadata-fields"))
                )
                .on_change(move |_, expanded, _, cx| {
                    let _ = owner.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state
                                .app
                                .plugin_state
                                .plugin_ui_state
                                .listening_workspace
                                .metadata_open = expanded;
                        });
                        cx.notify();
                    });
                }),
            "listening.metadata"
        ))
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
            .on_click_event(cx.listener(move |view, _, window, cx| {
                view.start_listening_trial(mode, window, cx);
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
                    let _ = listening.ab_test.clear_session();
                });
                cx.notify();
            }))
    }
}

pub(crate) fn activate_listening_surface(app: &mut App, surface: EarTrainingSurface) {
    if app.plugin_state.listening_test_state.surface == EarTrainingSurface::BlindComparison
        && surface != EarTrainingSurface::BlindComparison
    {
        let _ = app.plugin_state.leave_ab_test_runtime();
    }
    app.plugin_state.listening_test_state.surface = surface;
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
    let _ = listening.ab_test.clear_session();
    listening.status = graph_updated.into();
}

fn stage_listening_node_parameters(
    state: &mut crate::app::AppState,
    _target: ListeningPathTarget,
    _node_id: &str,
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
    match serde_json::from_str::<serde_json::Value>(&value) {
        Ok(parameters) if parameters.is_object() => listening.status.clear(),
        Ok(_) => listening.status = localized.params_object.into(),
        Err(error) => listening.status = format!("{}: {error}", localized.params_invalid),
    }
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
    let _ = listening.ab_test.clear_session();
    listening.status = localized.params_updated.into();
}

fn stage_listening_rack_parameters(
    state: &mut crate::app::AppState,
    _target: ListeningPathTarget,
    _index: usize,
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
    match serde_json::from_str::<serde_json::Value>(&value) {
        Ok(parameters) if parameters.is_object() => listening.status.clear(),
        Ok(_) => listening.status = localized.params_object.into(),
        Err(error) => listening.status = format!("{}: {error}", localized.params_invalid),
    }
}

/// Update one value in the staged parameter object. The edited A/B path is
/// intentionally left untouched until the user presses Done, so Cancel keeps
/// the same transactional behavior for typed controls and Expert JSON.
fn set_listening_staged_parameter(
    state: &mut crate::app::AppState,
    key: &str,
    value: serde_json::Value,
) {
    let listening = &mut state.app.plugin_state.listening_test_state;
    let mut parameters =
        serde_json::from_str::<serde_json::Value>(&listening.editing_path_parameters)
            .ok()
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
    parameters[key] = value;
    listening.editing_path_parameters =
        serde_json::to_string_pretty(&parameters).unwrap_or_else(|_| "{}".to_string());
    listening.status.clear();
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
    let _ = listening.ab_test.clear_session();
    listening.status = localized.rack_updated.into();
}
