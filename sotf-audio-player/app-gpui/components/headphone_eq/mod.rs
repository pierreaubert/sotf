//! Headphone EQ Screen
//!
//! Multi-step wizard for headphone EQ optimization:
//! 1. Measurement & Target - Choose measurement file and target curve
//! 2. Optimization - EQ design, fine tuning, and generate EQ
//! 3. Listen - Preview and apply EQ to playback
//! 4. Save - Export format selection and save

use crate::app::types::{HeadphoneEqStep, PluginUpdateType};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack,
    Progress, ProgressSize, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

/// Target curve options for headphone EQ
pub const TARGET_CURVE_OPTIONS: &[(&str, &str)] = &[
    ("harman-over-ear-2018", "Harman Over-Ear 2018"),
    ("harman-over-ear-2015", "Harman Over-Ear 2015"),
    ("harman-over-ear-2013", "Harman Over-Ear 2013"),
    ("harman-in-ear-2019", "Harman In-Ear 2019"),
    ("custom", "Custom File..."),
];

impl PlayerView {
    // ========================================================================
    // Headphone EQ Wizard Screen
    // ========================================================================

    /// Clear the headphone EQ from the playback chain
    pub fn clear_headphone_eq_from_playback(&mut self, cx: &mut Context<Self>) {
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

    /// Main Headphone EQ screen entry point (wizard)
    pub(crate) fn render_headphone_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.headphone_eq_state.step;

        // Content for current step
        let content = match current_step {
            HeadphoneEqStep::MeasurementTarget => self
                .render_headphone_eq_measurement_target(cx)
                .into_any_element(),
            HeadphoneEqStep::Optimization => {
                self.render_headphone_eq_optimization(cx).into_any_element()
            }
            HeadphoneEqStep::Listen => self.render_headphone_eq_listen(cx).into_any_element(),
            HeadphoneEqStep::Save => self.render_headphone_eq_save(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_headphone_eq_header(cx))
            .child(
                div()
                    .id("headphone-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
    }

    /// Render the headphone EQ screen header with step indicators
    fn render_headphone_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.headphone_eq_state.step;

        // Helper function to build step indicator
        let build_step_indicator =
            |step: HeadphoneEqStep,
             label: &'static str,
             number: u8,
             theme: &crate::theme::Theme| {
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
        let connector = |from: HeadphoneEqStep, theme: &crate::theme::Theme| {
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
                        Text::new("Headphone EQ")
                            .size(TextSize::Xl)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().w(px(1.0)).h(px(24.0)).bg(theme.border))
                    .child(build_step_indicator(
                        HeadphoneEqStep::MeasurementTarget,
                        "Measurement",
                        1,
                        &theme,
                    ))
                    .child(connector(HeadphoneEqStep::MeasurementTarget, &theme))
                    .child(build_step_indicator(
                        HeadphoneEqStep::Optimization,
                        "Optimization",
                        2,
                        &theme,
                    ))
                    .child(connector(HeadphoneEqStep::Optimization, &theme))
                    .child(build_step_indicator(
                        HeadphoneEqStep::Listen,
                        "Listen",
                        3,
                        &theme,
                    ))
                    .child(connector(HeadphoneEqStep::Listen, &theme))
                    .child(build_step_indicator(
                        HeadphoneEqStep::Save,
                        "Save",
                        4,
                        &theme,
                    )),
            )
            .child(self.render_headphone_eq_nav_buttons(cx))
    }

    /// Render navigation buttons (Close/Back and Next/Finish)
    fn render_headphone_eq_nav_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_step = state.app.headphone_eq_state.step;
        let can_go_next = state.app.headphone_eq_state.can_advance();
        let is_busy = state.app.headphone_eq_state.is_optimizing();
        let view = cx.entity().clone();

        let back_label = match current_step {
            HeadphoneEqStep::MeasurementTarget => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            HeadphoneEqStep::Save => "Finish",
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
                                    match state.app.headphone_eq_state.step {
                                        HeadphoneEqStep::MeasurementTarget => {
                                            // Go back to previous screen
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go back to previous step
                                            if let Some(prev) =
                                                state.app.headphone_eq_state.step.previous()
                                            {
                                                state.app.headphone_eq_state.step = prev;
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
                                    match state.app.headphone_eq_state.step {
                                        HeadphoneEqStep::Save => {
                                            // Finish - go back
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go to next step
                                            if let Some(next) =
                                                state.app.headphone_eq_state.step.next()
                                            {
                                                state.app.headphone_eq_state.step = next;
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
    // Step 1: Measurement & Target
    // ========================================================================

    fn render_headphone_eq_measurement_target(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;

        let measurement_path = headphone_eq.measurement_path.clone().unwrap_or_default();
        let target_preset = headphone_eq.target_preset.clone();
        let custom_target_path = headphone_eq.custom_target_path.clone().unwrap_or_default();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Select Measurement & Target")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Choose your headphone measurement file and target curve.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Measurement File").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Select a CSV file with your headphone's frequency response measurement.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(theme.background_secondary)
                                            .text_sm()
                                            .text_color(if measurement_path.is_empty() {
                                                theme.text_muted
                                            } else {
                                                theme.text_primary
                                            })
                                            .child(if measurement_path.is_empty() {
                                                "No file selected".to_string()
                                            } else {
                                                measurement_path.clone()
                                            }),
                                    )
                                    .child(
                                        Button::new("browse-measurement", "Browse...")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Md)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.browse_headphone_eq_measurement(cx);
                                                }),
                                            ),
                                    ),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .header(Text::new("Target Curve").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Select a target curve for your headphone EQ.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .wrap(true)
                                    .children(TARGET_CURVE_OPTIONS.iter().map(|(value, label)| {
                                        let is_selected = target_preset == *value;
                                        let value = value.to_string();
                                        let is_custom = value == "custom";

                                        Button::new(
                                            SharedString::from(format!("hp-target-{}", value)),
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
                                            cx.listener(move |view, _, _, cx| {
                                                if is_custom {
                                                    view.browse_headphone_eq_target(cx);
                                                } else {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.headphone_eq_state.target_preset =
                                                            value.clone();
                                                    });
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                    })),
                            )
                            .when(target_preset == "custom", |vstack| {
                                let theme = theme.clone();
                                vstack.child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            div()
                                                .flex_1()
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(theme.background_secondary)
                                                .text_sm()
                                                .text_color(theme.text_muted)
                                                .child(if custom_target_path.is_empty() {
                                                    "No custom target file selected".to_string()
                                                } else {
                                                    custom_target_path.clone()
                                                }),
                                        )
                                        .child(
                                            Button::new("browse-custom-target", "Change")
                                                .variant(ButtonVariant::Secondary)
                                                .size(ButtonSize::Sm)
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.browse_headphone_eq_target(cx);
                                                    }),
                                                ),
                                        ),
                                )
                            }),
                    ),
            )
    }

    // ========================================================================
    // Step 2: Optimization (EQ Design, Fine Tuning, Generate)
    // ========================================================================

    fn render_headphone_eq_optimization(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;

        // Build AutoEqConfig from our HeadphoneEqOptimizerConfig
        let config = &headphone_eq.optimizer_config;
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
            algo_open: headphone_eq.dropdowns.algorithm_open,
            peq_model_open: headphone_eq.dropdowns.peq_model_open,
            strategy_open: false,
            local_algo_open: false,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("headphone-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .show_optimization_tuning(false) // Only show EQ Design section
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    use crate::app::types::RoomEqAlgorithm;
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.algorithm = match algo {
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
                        state.app.headphone_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.dropdowns.algorithm_open = open;
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |_model, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.dropdowns.peq_model_open = false;
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.dropdowns.peq_model_open = open;
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_freq = value;
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_iter = value;
                    });
                }
            });

        // Loss function selection
        let current_loss = headphone_eq.optimizer_config.loss.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Set the optimization parameters for your headphone EQ.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Optimization Goal").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Choose what the optimizer should optimize for.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child({
                                        let is_selected = current_loss == "headphone-score";
                                        Button::new("loss-score", "Harman Score")
                                            .variant(if is_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Sm)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .headphone_eq_state
                                                            .optimizer_config
                                                            .loss = "headphone-score".to_string();
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                    })
                                    .child({
                                        let is_selected = current_loss == "headphone-flat";
                                        Button::new("loss-flat", "Target Flat")
                                            .variant(if is_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Sm)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .headphone_eq_state
                                                            .optimizer_config
                                                            .loss = "headphone-flat".to_string();
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                    }),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .header(Text::new("EQ Parameters").weight(TextWeight::Semibold))
                    .content(autoeq_form),
            )
            // Generate EQ section
            .child(
                Card::new()
                    .header(Text::new("Generate Headphone EQ").weight(TextWeight::Semibold))
                    .content({
                        let progress = headphone_eq.progress;
                        let status_msg = headphone_eq.status_message.clone();
                        let is_optimizing = headphone_eq.is_optimizing();

                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new(
                                    "start_optimization",
                                    if is_optimizing {
                                        "Optimizing..."
                                    } else {
                                        "Generate Headphone EQ"
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
                                            view.start_headphone_eq_optimization(cx);
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
                    }),
            )
    }

    // ========================================================================
    // Step 3: Listen
    // ========================================================================

    fn render_headphone_eq_listen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;
        let result = headphone_eq.result.as_ref();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Listen & Preview")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Preview the optimized EQ and apply it to your playback.")
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
                                            .child(Text::new(format!(
                                                "Before: {:.2}",
                                                result.pre_score
                                            )))
                                            .child(Text::new(format!(
                                                "After: {:.2}",
                                                result.post_score
                                            )))
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
                                    .id("filter-list-scroll")
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .p_2()
                                    .rounded_md()
                                    .bg(theme.surface)
                                    .max_h(px(200.0))
                                    .overflow_y_scroll()
                                    .children(biquads.iter().enumerate().map(|(i, biquad)| {
                                        let filter_type = format!("{:?}", biquad.filter_type);
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
                            .header(Text::new("Playback Preview").weight(TextWeight::Semibold))
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
                                                    "apply-to-playback",
                                                    "Apply to Playback",
                                                )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Md)
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.apply_headphone_eq_result(cx);
                                                    }),
                                                ),
                                            )
                                            .child(
                                                Button::new("clear-eq", "Clear EQ")
                                                    .variant(ButtonVariant::Secondary)
                                                    .size(ButtonSize::Md)
                                                    .build()
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|view, _, _, cx| {
                                                            view.clear_headphone_eq_from_playback(
                                                                cx,
                                                            );
                                                        }),
                                                    ),
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
    // Step 4: Save
    // ========================================================================

    fn render_headphone_eq_save(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;
        let result = headphone_eq.result.as_ref();
        let export_format = headphone_eq.export_format.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Save EQ")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Choose an export format and save your EQ configuration.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .when_some(result, |vstack, _result| {
                vstack
                    .child(
                        Card::new()
                            .header(Text::new("Export Format").weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new("Select the format for your EQ file.")
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
                                                                "export-format-{}",
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
                                                                                .headphone_eq_state
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
                                    ),
                            ),
                    )
                    .child(
                        Card::new()
                            .header(Text::new("Save").weight(TextWeight::Semibold))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Button::new("save-eq", "Save EQ File")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Lg)
                                            .full_width(true)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.save_headphone_eq_result(cx);
                                                }),
                                            ),
                                    )
                                    .child(
                                        Text::new(
                                            "Your EQ will be saved to ~/Library/Application Support/org.spinorama.sotf/EQ",
                                        )
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
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

    fn browse_headphone_eq_measurement(&mut self, _cx: &mut Context<Self>) {
        // TODO: Open file dialog and load CSV
        log::info!("TODO: Browse headphone EQ measurement file");
    }

    fn browse_headphone_eq_target(&mut self, cx: &mut Context<Self>) {
        // TODO: Open file dialog for custom target
        log::info!("TODO: Browse headphone EQ target file");
        self.state.update(cx, |state, _cx| {
            state.app.headphone_eq_state.target_preset = "custom".to_string();
        });
        cx.notify();
    }

    fn start_headphone_eq_optimization(&mut self, cx: &mut Context<Self>) {
        // TODO: Spawn async optimization task
        log::info!("TODO: Start headphone EQ optimization");
        self.state.update(cx, |state, _cx| {
            state.app.headphone_eq_state.optimization_status =
                crate::app::types::OptimizationStatus::Running;
            state.app.headphone_eq_state.status_message = "Starting optimization...".to_string();
        });
        cx.notify();
    }

    fn apply_headphone_eq_result(&mut self, _cx: &mut Context<Self>) {
        // TODO: Apply result to player's plugin chain
        log::info!("TODO: Apply headphone EQ result to playback");
    }

    fn save_headphone_eq_result(&mut self, _cx: &mut Context<Self>) {
        // TODO: Save result to file
        log::info!("TODO: Save headphone EQ result");
    }
}
