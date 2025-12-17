//! Spinorama EQ Screen
//!
//! Multi-step wizard for speaker EQ optimization using spinorama.org data:
//! 1. Select Speaker - Search and select speaker from spinorama.org API
//! 2. Configure - Optimization parameters and mode selection
//! 3. Optimize - Run optimization with progress display
//! 4. Review - View results, apply to playback, export

use crate::app::types::{PluginUpdateType, SpinoramaOptimizationMode, SpinoramaStep};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack,
    Input, InputSize, Progress, ProgressSize, StackAlign, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};
use sotf_audio_player::autoeq::speaker::{
    CallbackConfig, MeasurementInput, SpeakerOptimizationCallback, SpeakerOptimizationConfig,
    SpeakerOptimizationProgress,
};
use sotf_audio_player::autoeq::types::SpeakerConfigType;
use std::sync::Mutex;

// Global for sharing optimization result between threads
// Format: (success, result, error_message)
static SPINORAMA_RESULT: Mutex<
    Option<(
        bool,
        Option<crate::app::types::SpinoramaEqResult>,
        Option<String>,
    )>,
> = Mutex::new(None);

impl PlayerView {
    // ========================================================================
    // Spinorama EQ Wizard Screen
    // ========================================================================

    /// Clear the spinorama EQ from the playback chain
    pub fn clear_spinorama_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Find and remove EQ plugins
            let plugins = state.app.plugin_chain.plugins();
            let eq_indices: Vec<_> = plugins
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            // Remove in reverse order to maintain correct indices
            for idx in eq_indices.into_iter().rev() {
                state.app.plugin_chain.remove_plugin(idx);
            }

            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }

    /// Main Spinorama EQ screen entry point (wizard)
    pub(crate) fn render_spinorama_eq_screen(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Check if we need to auto-fetch speakers before reading state
        let needs_fetch = {
            let state = self.state.read(cx);
            state.app.spinorama_eq_state.needs_speaker_refresh()
                && !state.app.spinorama_eq_state.loading_speakers
        };

        if needs_fetch {
            // Set loading flag immediately to prevent duplicate fetches
            self.state.update(cx, |state, _| {
                state.app.spinorama_eq_state.loading_speakers = true;
            });
            // Schedule fetch
            cx.spawn(async move |view, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.fetch_spinorama_speakers(cx);
                });
            })
            .detach();
        }

        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.spinorama_eq_state.step;

        // Content for current step
        let content = match current_step {
            SpinoramaStep::SelectSpeaker => {
                self.render_spinorama_select_speaker(cx).into_any_element()
            }
            SpinoramaStep::Configure => self.render_spinorama_configure(cx).into_any_element(),
            SpinoramaStep::Optimize => self.render_spinorama_optimize(cx).into_any_element(),
            SpinoramaStep::Review => self.render_spinorama_review(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_spinorama_header(cx))
            .child(
                div()
                    .id("spinorama-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
    }

    /// Render the spinorama EQ screen header with step indicators
    fn render_spinorama_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.spinorama_eq_state.step;

        // Helper function to build step indicator
        let build_step_indicator =
            |step: SpinoramaStep, label: &'static str, number: u8, theme: &crate::theme::Theme| {
                let is_active = current_step == step;
                let is_past = current_step.index() > step.index();

                let (bg_color, text_color, border_color) = if is_active {
                    (theme.accent, theme.text_on_accent, theme.accent)
                } else if is_past {
                    (theme.success, theme.text_on_accent, theme.success)
                } else {
                    (theme.surface, theme.text_muted, theme.border)
                };

                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .rounded_full()
                            .bg(bg_color)
                            .border_2()
                            .border_color(border_color)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Text::new(number.to_string())
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Bold)
                                    .color(text_color),
                            ),
                    )
                    .child(
                        Text::new(label)
                            .size(TextSize::Sm)
                            .weight(if is_active {
                                TextWeight::Bold
                            } else {
                                TextWeight::Normal
                            })
                            .color(if is_active {
                                theme.text_primary
                            } else {
                                theme.text_muted
                            }),
                    )
            };

        // Build step connector
        let connector = |from: SpinoramaStep, theme: &crate::theme::Theme| {
            let is_completed = current_step.index() > from.index();
            div().w(px(32.0)).h(px(2.0)).bg(if is_completed {
                theme.success
            } else {
                theme.border
            })
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::Center)
                    .child(
                        Text::new("Spinorama EQ")
                            .size(TextSize::Xl)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().w(px(1.0)).h(px(24.0)).bg(theme.border))
                    .child(build_step_indicator(
                        SpinoramaStep::SelectSpeaker,
                        "Select",
                        1,
                        &theme,
                    ))
                    .child(connector(SpinoramaStep::SelectSpeaker, &theme))
                    .child(build_step_indicator(
                        SpinoramaStep::Configure,
                        "Configure",
                        2,
                        &theme,
                    ))
                    .child(connector(SpinoramaStep::Configure, &theme))
                    .child(build_step_indicator(
                        SpinoramaStep::Optimize,
                        "Optimize",
                        3,
                        &theme,
                    ))
                    .child(connector(SpinoramaStep::Optimize, &theme))
                    .child(build_step_indicator(
                        SpinoramaStep::Review,
                        "Review",
                        4,
                        &theme,
                    )),
            )
            .child(self.render_spinorama_nav_buttons(cx))
    }

    /// Render navigation buttons (Close/Back and Next/Finish)
    fn render_spinorama_nav_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_step = state.app.spinorama_eq_state.step;
        let can_go_next = state.app.spinorama_eq_state.can_advance();
        let is_busy = state.app.spinorama_eq_state.is_optimizing();
        let view = cx.entity().clone();

        let back_label = match current_step {
            SpinoramaStep::SelectSpeaker => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            SpinoramaStep::Review => "Finish",
            _ => "Next",
        };

        HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Button::new("back", back_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .disabled(is_busy)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    match state.app.spinorama_eq_state.step {
                                        SpinoramaStep::SelectSpeaker => {
                                            // Go back to previous screen
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go back to previous step
                                            if let Some(prev) =
                                                state.app.spinorama_eq_state.step.previous()
                                            {
                                                state.app.spinorama_eq_state.step = prev;
                                            }
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Button::new("next", next_label)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Md)
                    .disabled(!can_go_next || is_busy)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    match state.app.spinorama_eq_state.step {
                                        SpinoramaStep::Review => {
                                            // Finish - go back
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go to next step
                                            if let Some(next) =
                                                state.app.spinorama_eq_state.step.next()
                                            {
                                                state.app.spinorama_eq_state.step = next;
                                            }
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
    }

    // ========================================================================
    // Step 1: Select Speaker
    // ========================================================================

    fn render_spinorama_select_speaker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;
        let is_searching = state.app.input_mode == crate::app::InputMode::SpinoramaSpeakerSearch;

        let search_query = spinorama.speaker_search.clone();
        let selected_speaker = spinorama.selected_speaker.clone();
        let suggestions = spinorama.speaker_suggestions.clone();
        let is_loading = spinorama.loading_speakers;

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Select Speaker")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Search for your speaker model from spinorama.org measurements.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Speaker Search").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(
                                    "Type your speaker brand and model to search the database.",
                                )
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child({
                                let state = self.state.clone();
                                let state_for_edit_start = self.state.clone();
                                let accent = theme.accent;
                                let border = theme.border;
                                Input::new("speaker-search")
                                    .placeholder("Click to search speakers...")
                                    .value(SharedString::from(search_query.clone()))
                                    .edit_text(SharedString::from(search_query.clone()))
                                    .size(InputSize::Md)
                                    .bg_color(theme.surface)
                                    .text_color(theme.text_primary)
                                    .placeholder_color(theme.text_muted)
                                    .border_color(if is_searching { accent } else { border })
                                    .editing(is_searching)
                                    .on_edit_start(move |_window, cx| {
                                        state_for_edit_start.update(cx, |state, cx| {
                                            state.app.input_mode =
                                                crate::app::InputMode::SpinoramaSpeakerSearch;
                                            cx.notify();
                                        });
                                    })
                                    .on_text_change(move |text, _window, cx| {
                                        state.update(cx, |state, _| {
                                            state.app.spinorama_eq_state.speaker_search = text;
                                            state.app.spinorama_eq_state.update_suggestions();
                                        });
                                    })
                            })
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Button::new("refresh-speakers", "⟳ Refresh")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Sm)
                                            .disabled(is_loading)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.fetch_spinorama_speakers(cx);
                                                }),
                                            ),
                                    )
                                    .when(is_loading, |hstack| {
                                        hstack.child(
                                            Text::new("Loading...")
                                                .size(TextSize::Sm)
                                                .color(theme.text_muted),
                                        )
                                    })
                                    .when(
                                        !is_loading && !spinorama.available_speakers.is_empty(),
                                        |hstack| {
                                            hstack.child(
                                                Text::new(format!(
                                                    "{} speakers",
                                                    spinorama.available_speakers.len()
                                                ))
                                                .size(TextSize::Sm)
                                                .color(theme.text_muted),
                                            )
                                        },
                                    ),
                            ),
                    ),
            )
            // Speaker suggestions list
            .child(
                Card::new()
                    .header(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Available Speakers").weight(TextWeight::Semibold))
                            .child(
                                Text::new(format!("({} matches)", suggestions.len()))
                                    .size(TextSize::Sm)
                                    .color(theme.text_muted),
                            ),
                    )
                    .content(
                        div()
                            .id("speaker-suggestions-scroll")
                            .flex()
                            .flex_col()
                            .gap_1()
                            .max_h(px(300.0))
                            .overflow_y_scroll()
                            .when(suggestions.is_empty() && is_loading, |d| {
                                d.child(
                                    Text::new("Loading speakers from spinorama.org...")
                                        .size(TextSize::Sm)
                                        .color(theme.text_muted),
                                )
                            })
                            .when(suggestions.is_empty() && !is_loading, |d| {
                                d.child(
                                    Text::new(if search_query.is_empty() {
                                        "No speakers loaded. Click Refresh to load."
                                    } else {
                                        "No matching speakers found."
                                    })
                                    .size(TextSize::Sm)
                                    .color(theme.text_muted),
                                )
                            })
                            .children(suggestions.iter().map(|speaker| {
                                let is_selected = selected_speaker.as_ref() == Some(speaker);
                                let speaker_name = speaker.clone();
                                let accent = theme.accent;
                                let surface = theme.surface;
                                let text_primary = theme.text_primary;
                                let text_on_accent = theme.text_on_accent;

                                div()
                                    .id(SharedString::from(format!(
                                        "speaker-option-{}",
                                        speaker_name
                                    )))
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if is_selected { accent } else { surface })
                                    .text_color(if is_selected {
                                        text_on_accent
                                    } else {
                                        text_primary
                                    })
                                    .text_sm()
                                    .hover(|s| {
                                        s.bg(if is_selected {
                                            accent
                                        } else {
                                            theme.surface_hover
                                        })
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            let speaker_name = speaker_name.clone();
                                            move |view, _, _, cx| {
                                                view.select_spinorama_speaker(&speaker_name, cx);
                                            }
                                        }),
                                    )
                                    .child(speaker_name)
                            })),
                    ),
            )
            // Selected speaker display
            .when_some(selected_speaker.clone(), |vstack, speaker| {
                vstack.child(
                    Card::new()
                        .header(Text::new("Selected Speaker").weight(TextWeight::Semibold))
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    Text::new(speaker)
                                        .size(TextSize::Lg)
                                        .weight(TextWeight::Bold)
                                        .color(theme.accent),
                                )
                                .child(
                                    Text::new("Click 'Next' to configure optimization parameters.")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                ),
                        ),
                )
            })
    }

    // ========================================================================
    // Step 2: Configure
    // ========================================================================

    fn render_spinorama_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;

        // Build AutoEqConfig from our SpinoramaOptimizerConfig
        let config = &spinorama.optimizer_config;
        let autoeq_config = AutoEqConfig {
            num_filters: config.num_filters,
            sample_rate: 48000,
            min_db: config.min_db,
            max_db: config.max_db,
            min_q: config.min_q,
            max_q: config.max_q,
            min_freq: config.min_freq,
            max_freq: config.max_freq,
            peq_model: "pk".to_string(),
            algo: match config.algorithm {
                crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
            }
            .to_string(),
            population: 100,
            maxeval: config.max_iter,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            ..Default::default()
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
            opt_mode_open: spinorama.dropdowns.opt_mode_open,
            peq_model_open: spinorama.dropdowns.peq_model_open,
            algo_open: spinorama.dropdowns.algorithm_open,
            strategy_open: false,
            local_algo_open: false,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("spinorama-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .show_goals(false) // Hide Goals section (System Type, Loss Type, Target Curve)
            .show_optimization_tuning(false) // Hide Optimization Tuning section
            .on_opt_mode_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.spinorama_eq_state.dropdowns.opt_mode_open = open;
                        cx.notify();
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.spinorama_eq_state.dropdowns.peq_model_open = open;
                        cx.notify();
                    });
                }
            })
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    use crate::app::types::RoomEqAlgorithm;
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.algorithm = match algo {
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
                        state.app.spinorama_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.spinorama_eq_state.dropdowns.algorithm_open = open;
                        cx.notify();
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.max_freq = value;
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.max_iter = value;
                    });
                }
            });

        // Optimization mode selection
        let current_mode = spinorama.optimizer_config.mode;

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Set the optimization parameters for your speaker EQ.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Optimization Mode").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Choose what the optimizer should optimize for.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(HStack::new().spacing(StackSpacing::Sm).children(
                                SpinoramaOptimizationMode::all().iter().map(|mode| {
                                    let is_selected = current_mode == *mode;
                                    let mode_value = *mode;

                                    Button::new(
                                        SharedString::from(format!("spinorama-mode-{:?}", mode)),
                                        mode.as_str(),
                                    )
                                    .variant(if is_selected {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    })
                                    .size(ButtonSize::Md)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state
                                                    .app
                                                    .spinorama_eq_state
                                                    .optimizer_config
                                                    .mode = mode_value;
                                            });
                                            cx.notify();
                                        }),
                                    )
                                }),
                            ))
                            .child(
                                Text::new(current_mode.description())
                                    .size(TextSize::Xs)
                                    .color(theme.text_muted),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .header(Text::new("EQ Parameters").weight(TextWeight::Semibold))
                    .content(autoeq_form),
            )
    }

    // ========================================================================
    // Step 3: Optimize
    // ========================================================================

    fn render_spinorama_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;

        let progress = spinorama.progress;
        let status_msg = spinorama.status_message.clone();
        let error_msg = spinorama.error_message.clone();
        let is_optimizing = spinorama.is_optimizing();
        let selected_speaker = spinorama.selected_speaker.clone().unwrap_or_default();
        let mode = spinorama.optimizer_config.mode;

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Run Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Generate optimized EQ filters for your speaker.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Configuration Summary").weight(TextWeight::Semibold))
                    .content(
                        VStack::new().spacing(StackSpacing::Sm).child(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("Speaker")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(
                                            Text::new(selected_speaker)
                                                .weight(TextWeight::Bold)
                                                .color(theme.accent),
                                        ),
                                )
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("Mode")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(Text::new(mode.as_str()).weight(TextWeight::Bold)),
                                )
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("Filters")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(
                                            Text::new(format!(
                                                "{}",
                                                spinorama.optimizer_config.num_filters
                                            ))
                                            .weight(TextWeight::Bold),
                                        ),
                                ),
                        ),
                    ),
            )
            .child(
                Card::new()
                    .header(Text::new("Generate Speaker EQ").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new(
                                    "start_spinorama_optimization",
                                    if is_optimizing {
                                        "Optimizing..."
                                    } else {
                                        "Generate Speaker EQ"
                                    },
                                )
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Lg)
                                .full_width(true)
                                .disabled(is_optimizing)
                                .build()
                                .when(!is_optimizing, |btn| {
                                    btn.on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.start_spinorama_optimization(cx);
                                        }),
                                    )
                                }),
                            )
                            .when(is_optimizing || progress > 0.0, |vstack| {
                                vstack
                                    .child(
                                        Text::new(format!("Progress: {:.0}%", progress * 100.0))
                                            .size(TextSize::Sm),
                                    )
                                    .child(Progress::new(progress * 100.0).size(ProgressSize::Md))
                                    .child(
                                        Text::new(status_msg)
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                            })
                            .when_some(error_msg, |vstack, err| {
                                vstack.child(Text::new(err).size(TextSize::Sm).color(theme.error))
                            }),
                    ),
            )
    }

    // ========================================================================
    // Step 4: Review
    // ========================================================================

    fn render_spinorama_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;
        let result = spinorama.result.as_ref();
        let export_format = spinorama.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Review & Apply")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Review the optimized EQ and apply it to your playback.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .when_some(result, |vstack, result| {
                let theme = theme.clone();
                let num_filters = result.biquads.len();
                let biquads = result.biquads.clone();

                vstack
                    .child(
                        Card::new()
                            .header(Text::new("Optimization Results").weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Lg)
                                            .child(
                                                Text::new(format!("Before: {:.2}", result.pre_score)),
                                            )
                                            .child(
                                                Text::new(format!("After: {:.2}", result.post_score)),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "Improvement: {:.2}",
                                                    result.pre_score - result.post_score
                                                ))
                                                .color(if result.post_score < result.pre_score {
                                                    theme.success
                                                } else {
                                                    theme.error
                                                }),
                                            ),
                                    )
                                    .child(
                                        Text::new(format!("{} filters generated", num_filters))
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    ),
                            ),
                    )
                    .child(
                        Card::new()
                            .header(Text::new("EQ Filters").weight(TextWeight::Semibold))
                            .content(
                                div()
                                    .id("spinorama-filter-list-scroll")
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .p_2()
                                    .rounded_md()
                                    .bg(theme.surface)
                                    .max_h(px(200.0))
                                    .overflow_y_scroll()
                                    .children(biquads.iter().enumerate().map(|(i, biquad)| {
                                        let filter_type = biquad.filter_type.clone();
                                        let freq = biquad.freq;
                                        let q = biquad.q;
                                        let gain = biquad.db_gain;

                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .px_2()
                                            .py_1()
                                            .rounded(px(4.0))
                                            .bg(theme.background)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.accent)
                                                            .child(format!("#{}", i + 1)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_secondary)
                                                            .child(filter_type),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_primary)
                                                            .child(format!("{:.0} Hz", freq)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_muted)
                                                            .child(format!("Q {:.2}", q)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(if gain >= 0.0 {
                                                                theme.success
                                                            } else {
                                                                theme.error
                                                            })
                                                            .child(format!("{:+.1} dB", gain)),
                                                    ),
                                            )
                                    })),
                            ),
                    )
                    .child(
                        Card::new()
                            .header(Text::new("Apply to Playback").weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new(
                                            "Apply the EQ to your current playback to hear the difference.",
                                        )
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .child(
                                                Button::new(
                                                    "apply-spinorama-eq",
                                                    "Apply to Playback",
                                                )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Md)
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.apply_spinorama_eq_result(cx);
                                                    }),
                                                ),
                                            )
                                            .child(
                                                Button::new("clear-spinorama-eq", "Clear EQ")
                                                    .variant(ButtonVariant::Secondary)
                                                    .size(ButtonSize::Md)
                                                    .build()
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|view, _, _, cx| {
                                                            view.clear_spinorama_eq_from_playback(cx);
                                                        }),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        Card::new()
                            .header(Text::new("Export").weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new("Select export format and save your EQ.")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .wrap(true)
                                            .children(
                                                sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS.iter().map(
                                                    |(value, label, _ext)| {
                                                        let is_selected = export_format == *value;
                                                        let value = value.to_string();

                                                        Button::new(
                                                            SharedString::from(format!(
                                                                "spinorama-export-format-{}",
                                                                value
                                                            )),
                                                            *label,
                                                        )
                                                        .variant(if is_selected {
                                                            ButtonVariant::Primary
                                                        } else {
                                                            ButtonVariant::Secondary
                                                        })
                                                        .size(ButtonSize::Sm)
                                                        .build()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view, _, _, cx| {
                                                                    view.state.update(
                                                                        cx,
                                                                        |state, _cx| {
                                                                            state
                                                                                .app
                                                                                .spinorama_eq_state
                                                                                .export_format =
                                                                                value.clone();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                    },
                                                ),
                                            ),
                                    )
                                    .child(
                                        Button::new("save-spinorama-eq", "Save EQ File")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Md)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.save_spinorama_eq_result(cx);
                                                }),
                                            ),
                                    ),
                            ),
                    )
            })
            .when(result.is_none(), |vstack| {
                vstack.child(
                    Card::new()
                        .header(Text::new("No Results").weight(TextWeight::Semibold))
                        .content(
                            Text::new("Go back and run optimization to generate an EQ curve.")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        ),
                )
            })
    }

    // ========================================================================
    // Action Handlers
    // ========================================================================

    fn fetch_spinorama_speakers(&mut self, cx: &mut Context<Self>) {
        log::info!("Fetching spinorama speakers from API...");
        // Note: loading_speakers is set to true before spawning to prevent duplicate fetches
        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.loading_speakers = true;
            state.app.spinorama_eq_state.error_message = None;
        });
        cx.notify();

        // Use a global mutex to share results between threads (like optimization does)
        static SPEAKERS_RESULT: std::sync::Mutex<Option<Result<Vec<String>, String>>> =
            std::sync::Mutex::new(None);

        // Clear any previous result
        *SPEAKERS_RESULT.lock().unwrap() = None;

        // Spawn a background thread with its own tokio runtime for the HTTP request
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let result = rt.block_on(async { autoeq::fetch_available_speakers().await });

            let mapped_result = result.map_err(|e| e.to_string());
            *SPEAKERS_RESULT.lock().unwrap() = Some(mapped_result);
        });

        // Poll for results from GPUI's async context
        let state_entity = self.state.clone();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;

                // Check if result is ready
                let result = SPEAKERS_RESULT.lock().unwrap().take();

                if let Some(result) = result {
                    match result {
                        Ok(speakers) => {
                            log::info!("Fetched {} speakers from spinorama.org", speakers.len());
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.available_speakers = speakers;
                                state.app.spinorama_eq_state.loading_speakers = false;
                                state.app.spinorama_eq_state.speakers_cached_at =
                                    Some(std::time::Instant::now());
                                state.app.spinorama_eq_state.update_suggestions();
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to fetch speakers: {}", e);
                            let _ = state_entity.update(cx, |state, cx| {
                                state.app.spinorama_eq_state.loading_speakers = false;
                                state.app.spinorama_eq_state.error_message =
                                    Some(format!("Failed to fetch speakers: {}", e));
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
            }
        })
        .detach();
    }

    fn select_spinorama_speaker(&mut self, speaker: &str, cx: &mut Context<Self>) {
        log::info!("Selected speaker: {}", speaker);
        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.selected_speaker = Some(speaker.to_string());
        });
        cx.notify();
    }

    fn start_spinorama_optimization(&mut self, cx: &mut Context<Self>) {
        log::info!("Starting spinorama optimization...");

        // Gather config from state
        let (speaker_name, optimizer_config, mode) = {
            let state = self.state.read(cx);
            let spinorama = &state.app.spinorama_eq_state;
            let speaker = spinorama.selected_speaker.clone().unwrap_or_default();
            let config = spinorama.optimizer_config.clone();
            let mode = spinorama.optimizer_config.mode;
            (speaker, config, mode)
        };

        if speaker_name.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.spinorama_eq_state.error_message =
                    Some("No speaker selected".to_string());
            });
            cx.notify();
            return;
        }

        self.state.update(cx, |state, _cx| {
            state.app.spinorama_eq_state.optimization_status =
                crate::app::types::OptimizationStatus::Running;
            state.app.spinorama_eq_state.status_message = "Loading measurement data...".to_string();
            state.app.spinorama_eq_state.progress = 0.0;
            state.app.spinorama_eq_state.progress_history.clear();
            state.app.spinorama_eq_state.error_message = None;
        });
        cx.notify();

        let state_entity = self.state.clone();

        // Build optimization params
        let loss = mode.to_loss_string().to_string();
        let algo = match optimizer_config.algorithm {
            crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
            crate::app::types::RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
            crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
        }
        .to_string();

        let params = sotf_audio_player::autoeq::params::OptimizationParams {
            num_filters: optimizer_config.num_filters,
            sample_rate: 48000,
            min_db: optimizer_config.min_db,
            max_db: optimizer_config.max_db,
            min_q: optimizer_config.min_q,
            max_q: optimizer_config.max_q,
            min_freq: optimizer_config.min_freq,
            max_freq: optimizer_config.max_freq,
            maxeval: optimizer_config.max_iter,
            loss,
            algo,
            curve_name: "Estimated In-Room Response".to_string(),
            ..Default::default()
        };

        // Run optimization in background thread (blocking tokio runtime)
        std::thread::spawn(move || {
            // Build the optimization config
            // Use "Estimated In-Room Response" as measurement to trigger the special
            // handling in load_spinorama_measurement which fetches CEA2034 and computes PIR
            let config = SpeakerOptimizationConfig {
                config_type: SpeakerConfigType::Single,
                main_measurement: Some(MeasurementInput::Spinorama {
                    speaker: speaker_name.clone(),
                    version: "asr".to_string(),
                    measurement: "Estimated In-Room Response".to_string(),
                    curve_name: "Estimated In-Room Response".to_string(),
                }),
                driver_measurements: Vec::new(),
                crossover_type: None,
                crossover_freq_hints: Vec::new(),
                params: params.clone(),
                callback_config: Some(CallbackConfig {
                    interval: 25,
                    include_biquads: true,
                    include_filter_response: true,
                }),
                target: None,
            };

            // Create callback for progress updates
            let _state_for_callback = state_entity.clone();
            let max_iter = params.maxeval;
            let callback: SpeakerOptimizationCallback =
                Box::new(move |progress: &SpeakerOptimizationProgress| {
                    let progress_pct = progress.iteration as f32 / max_iter as f32;
                    let iter = progress.iteration;
                    let loss = progress.loss;

                    // We can't directly update GPUI state from here, so we use a channel or atomic
                    // For now, just log the progress
                    log::debug!(
                        "Spinorama optimization: iter={}, loss={:.4}, progress={:.1}%",
                        iter,
                        loss,
                        progress_pct * 100.0
                    );

                    // Continue optimization
                    sotf_audio_player::autoeq::speaker::CallbackAction::Continue
                });

            // Run the actual optimization
            log::info!("Running speaker optimization for: {}", speaker_name);
            let result = sotf_audio_player::autoeq::speaker::run_speaker_optimization_with_callback(
                &config,
                Some(callback),
            );

            // Update state with result (need to use smol to get back to GPUI context)
            smol::block_on(async {
                match result {
                    Ok(opt_result) => {
                        log::info!(
                            "Optimization complete: {} filters, loss {:.4} -> {:.4}",
                            opt_result.biquads.len(),
                            opt_result.initial_loss,
                            opt_result.final_loss
                        );

                        // Convert biquads to our result format
                        let biquads: Vec<crate::app::types::SpinoramaBiquad> = opt_result
                            .biquads
                            .iter()
                            .map(|b| crate::app::types::SpinoramaBiquad {
                                filter_type: format!("{:?}", b.filter_type),
                                freq: b.freq,
                                q: b.q,
                                db_gain: b.db_gain,
                            })
                            .collect();

                        // Convert curves for plotting
                        let original_response: Vec<(f64, f64)> = opt_result
                            .frequencies
                            .iter()
                            .zip(opt_result.input_curve.iter())
                            .map(|(&f, &db)| (f, db))
                            .collect();

                        let corrected_response: Vec<(f64, f64)> = opt_result
                            .frequencies
                            .iter()
                            .zip(opt_result.corrected_curve.iter())
                            .map(|(&f, &db)| (f, db))
                            .collect();

                        let target_response: Vec<(f64, f64)> = opt_result
                            .frequencies
                            .iter()
                            .zip(opt_result.target_curve.iter())
                            .map(|(&f, &db)| (f, db))
                            .collect();

                        // Note: We can't directly call state_entity.update() from a std::thread
                        // We need to use a channel or store in a shared Arc<Mutex<>>
                        // For now, we'll store in a temporary and poll from GPUI
                        // This is a limitation - ideally we'd use cx.spawn() but that requires async
                        log::info!("Storing optimization result with {} filters", biquads.len());

                        // Store result in a global for pickup (temporary hack)
                        let result = crate::app::types::SpinoramaEqResult {
                            biquads,
                            pre_score: opt_result.initial_loss,
                            post_score: opt_result.final_loss,
                            original_response: Some(original_response),
                            corrected_response: Some(corrected_response),
                            target_response: Some(target_response),
                        };

                        // Use parking_lot or std Mutex to share result
                        SPINORAMA_RESULT
                            .lock()
                            .unwrap()
                            .replace((true, Some(result), None));
                    }
                    Err(e) => {
                        log::error!("Optimization failed: {}", e);
                        SPINORAMA_RESULT
                            .lock()
                            .unwrap()
                            .replace((false, None, Some(e)));
                    }
                }
            });
        });

        // Start a polling timer to check for results
        let state_for_poll = self.state.clone();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;

                // Check if result is ready
                let result_ready = SPINORAMA_RESULT.lock().unwrap().take();

                if let Some((success, result, error)) = result_ready {
                    let _ = state_for_poll.update(cx, |state, cx| {
                        if success {
                            state.app.spinorama_eq_state.optimization_status =
                                crate::app::types::OptimizationStatus::Completed;
                            state.app.spinorama_eq_state.status_message = "Complete!".to_string();
                            state.app.spinorama_eq_state.progress = 1.0;
                            state.app.spinorama_eq_state.result = result;
                            state.app.spinorama_eq_state.step =
                                crate::app::types::SpinoramaStep::Review;
                        } else {
                            state.app.spinorama_eq_state.optimization_status =
                                crate::app::types::OptimizationStatus::Failed;
                            state.app.spinorama_eq_state.error_message = error;
                        }
                        cx.notify();
                    });
                    break;
                }

                // Update progress message
                let _ = state_for_poll.update(cx, |state, cx| {
                    if state.app.spinorama_eq_state.optimization_status
                        == crate::app::types::OptimizationStatus::Running
                    {
                        // Cycle through messages
                        let dots = match (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            / 500)
                            % 4
                        {
                            0 => "",
                            1 => ".",
                            2 => "..",
                            _ => "...",
                        };
                        state.app.spinorama_eq_state.status_message = format!("Optimizing{}", dots);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn apply_spinorama_eq_result(&mut self, cx: &mut Context<Self>) {
        log::info!("Applying spinorama EQ result to playback...");

        // Get the result biquads
        let biquads = {
            let state = self.state.read(cx);
            state
                .app
                .spinorama_eq_state
                .result
                .as_ref()
                .map(|r| r.biquads.clone())
        };

        let Some(biquads) = biquads else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message =
                    Some(crate::app::ToastMessage::error("No EQ result to apply"));
            });
            cx.notify();
            return;
        };

        if biquads.is_empty() {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message =
                    Some(crate::app::ToastMessage::warning("No filters in EQ result"));
            });
            cx.notify();
            return;
        }

        // Convert to EQFilter instances
        let eq_filters: Vec<sotf_audio_player::EQFilter> = biquads
            .iter()
            .map(|b| {
                sotf_audio_player::EQFilter::new(
                    autoeq_iir::BiquadFilterType::Peak,
                    b.freq,
                    b.q,
                    b.db_gain,
                )
            })
            .collect();

        let num_filters = eq_filters.len();

        // Update the plugin chain
        self.state.update(cx, |state, _| {
            let plugin_chain = &mut state.app.plugin_chain;

            // Check if there's an existing EQ plugin
            if let Some(eq_idx) = plugin_chain.find_plugin_index(&sotf_audio_player::PluginType::EQ)
            {
                // Update existing EQ plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(eq_idx) {
                    eq_plugin.settings = sotf_audio_player::PluginSettings::EQ {
                        filters: eq_filters.clone(),
                    };
                    log::info!("Updated existing EQ plugin at index {}", eq_idx);
                }
            } else {
                // No EQ plugin exists, add one before monitoring plugins
                let insert_idx = plugin_chain.find_processing_insert_index();
                plugin_chain.insert_plugin(insert_idx, &sotf_audio_player::PluginType::EQ);

                // Configure the newly inserted plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(insert_idx) {
                    eq_plugin.settings = sotf_audio_player::PluginSettings::EQ {
                        filters: eq_filters.clone(),
                    };
                }
                log::info!("Inserted new EQ plugin at index {}", insert_idx);
            }

            // Mark that plugin chain was modified and needs sync
            state.app.plugin_chain_modified = true;
            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(&format!(
                "Applied {} filter Spinorama EQ",
                num_filters
            )));
        });
        cx.notify();
    }

    fn save_spinorama_eq_result(&mut self, cx: &mut Context<Self>) {
        log::info!("Saving spinorama EQ result...");

        // Get the result and export format
        let (result, export_format, speaker_name) = {
            let state = self.state.read(cx);
            let result = state.app.spinorama_eq_state.result.clone();
            let format = state.app.spinorama_eq_state.export_format.clone();
            let speaker = state
                .app
                .spinorama_eq_state
                .selected_speaker
                .clone()
                .unwrap_or_else(|| "speaker".to_string());
            (result, format, speaker)
        };

        let Some(result) = result else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message =
                    Some(crate::app::ToastMessage::error("No EQ result to save"));
            });
            cx.notify();
            return;
        };

        // Convert biquads to autoeq_iir::Peq for export (Vec<(f64, Biquad)> with preamp gains)
        let peq: autoeq_iir::Peq = result
            .biquads
            .iter()
            .map(|b| {
                (
                    0.0, // preamp gain
                    autoeq_iir::Biquad::new(
                        autoeq_iir::BiquadFilterType::Peak,
                        b.freq,
                        48000.0,
                        b.q,
                        b.db_gain,
                    ),
                )
            })
            .collect();

        // Get file extension for format
        let extension = sotf_audio_player::autoeq::get_export_extension(&export_format);

        let safe_speaker_name = speaker_name
            .replace(' ', "_")
            .replace('/', "_")
            .replace('\\', "_");
        let default_filename = format!("spinorama_eq_{}.{}", safe_speaker_name, extension);

        let state_entity = self.state.clone();
        cx.spawn(async move |_, cx| {
            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter(extension.to_uppercase(), &[extension])
                .set_title("Save Spinorama EQ")
                .set_file_name(&default_filename)
                .save_file()
                .await;

            if let Some(file) = file {
                // Export using the appropriate format function
                let comment = format!(
                    "# Spinorama EQ for {}\n# Generated: {}",
                    speaker_name,
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                );
                let content = match export_format.as_str() {
                    "apo" => autoeq_iir::peq_format_apo(&comment, &peq),
                    "rme-channel" => autoeq_iir::peq_format_rme_channel(&peq),
                    "rme-room" => autoeq_iir::peq_format_rme_room(&peq, &peq),
                    "aupreset" => autoeq_iir::peq_format_aupreset(
                        &peq,
                        &format!("Spinorama EQ {}", speaker_name),
                    ),
                    _ => {
                        // JSON format - serialize the biquads directly
                        serde_json::to_string_pretty(&result.biquads).unwrap_or_default()
                    }
                };

                match std::fs::write(file.path(), content) {
                    Ok(()) => {
                        log::info!("Saved Spinorama EQ to {:?}", file.path());
                        let _ = state_entity.update(cx, |state, cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::success(
                                &format!("Saved to {}", file.path().display()),
                            ));
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to save Spinorama EQ: {}", e);
                        let _ = state_entity.update(cx, |state, cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                &format!("Failed to save: {}", e),
                            ));
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();
    }
}
