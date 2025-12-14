//! Room EQ Screen
//!
//! Multi-step wizard for room EQ optimization:
//! 1. Load Data - Load/import measurement data
//! 2. Configure - Configure channels and optimizer settings
//! 3. Optimize - Run optimization (per-channel, then combined)
//! 4. Review - Review results and visualizations
//! 5. Export - Export DSP chain and apply

use crate::app::types::{AutoEqField, MeasureState, RoomEqAlgorithm, RoomEqStep};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqAlgorithm as UiAutoEqAlgorithm, AutoEqConfig, AutoEqField as UiAutoEqField, AutoEqForm,
    AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing,
    Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    /// Main Room EQ screen entry point
    pub(crate) fn render_room_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.room_eq_state.step;

        // Content for current step
        let content = match current_step {
            RoomEqStep::LoadData => self.render_room_eq_load_data(cx).into_any_element(),
            RoomEqStep::Configure => self.render_room_eq_configure(cx).into_any_element(),
            RoomEqStep::Optimize => self.render_room_eq_optimize(cx).into_any_element(),
            RoomEqStep::Review => self.render_room_eq_review(cx).into_any_element(),
            RoomEqStep::Export => self.render_room_eq_export(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_room_eq_header(cx))
            .child(
                div()
                    .id("room-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
    }

    /// Render the room EQ screen header with step indicators
    fn render_room_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.room_eq_state.step;

        // Helper function to build step indicator
        let build_step_indicator =
            |step: RoomEqStep, label: &'static str, number: u8, theme: &crate::theme::Theme| {
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
        let connector = |from: RoomEqStep, theme: &crate::theme::Theme| {
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
                        Text::new("Room EQ")
                            .size(TextSize::Xl)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().w(px(1.0)).h(px(24.0)).bg(theme.border))
                    .child(build_step_indicator(
                        RoomEqStep::LoadData,
                        "Load Data",
                        1,
                        &theme,
                    ))
                    .child(connector(RoomEqStep::LoadData, &theme))
                    .child(build_step_indicator(
                        RoomEqStep::Configure,
                        "Configure",
                        2,
                        &theme,
                    ))
                    .child(connector(RoomEqStep::Configure, &theme))
                    .child(build_step_indicator(
                        RoomEqStep::Optimize,
                        "Optimize",
                        3,
                        &theme,
                    ))
                    .child(connector(RoomEqStep::Optimize, &theme))
                    .child(build_step_indicator(
                        RoomEqStep::Review,
                        "Review",
                        4,
                        &theme,
                    ))
                    .child(connector(RoomEqStep::Review, &theme))
                    .child(build_step_indicator(
                        RoomEqStep::Export,
                        "Export",
                        5,
                        &theme,
                    )),
            )
            .child(self.render_room_eq_nav_buttons(cx))
    }

    /// Render navigation buttons (Close/Back and Next/Finish)
    fn render_room_eq_nav_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_step = state.app.room_eq_state.step;
        let can_go_next = self.room_eq_can_advance(cx);
        let is_busy = state.app.room_eq_state.is_optimizing();
        let view = cx.entity().clone();

        let back_label = match current_step {
            RoomEqStep::LoadData => "Close",
            _ => "Back",
        };
        let next_label = match current_step {
            RoomEqStep::Export => "Finish",
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
                                    match state.app.room_eq_state.step {
                                        RoomEqStep::LoadData => {
                                            // Go back to previous screen
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go back to previous step
                                            if let Some(prev) =
                                                state.app.room_eq_state.step.previous()
                                            {
                                                state.app.room_eq_state.step = prev;
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
                                    match state.app.room_eq_state.step {
                                        RoomEqStep::Export => {
                                            // Finish - apply and go back
                                            // TODO: Apply DSP chain
                                            state.app.current_screen = state.app.last_screen;
                                        }
                                        _ => {
                                            // Go to next step
                                            if let Some(next) = state.app.room_eq_state.step.next()
                                            {
                                                state.app.room_eq_state.step = next;
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

    /// Map RoomEqStep to index
    #[allow(dead_code)]
    fn room_eq_step_index(&self, step: &RoomEqStep) -> usize {
        match step {
            RoomEqStep::LoadData => 0,
            RoomEqStep::Configure => 1,
            RoomEqStep::Optimize => 2,
            RoomEqStep::Review => 3,
            RoomEqStep::Export => 4,
        }
    }

    /// Check if we can advance from current step
    fn room_eq_can_advance(&self, cx: &Context<Self>) -> bool {
        let state = self.state.read(cx);
        let room_eq = &state.app.room_eq_state;

        match room_eq.step {
            RoomEqStep::LoadData => room_eq.has_measurements(),
            RoomEqStep::Configure => !room_eq.speaker_configs.is_empty(),
            RoomEqStep::Optimize => room_eq.is_optimization_complete(),
            RoomEqStep::Review => true,
            RoomEqStep::Export => true,
        }
    }

    // === Step Content Renderers ===

    fn render_room_eq_load_data(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let has_recordings = state.app.room_eq_state.has_measurements();
        let channel_count = state.app.room_eq_state.channel_count();
        let error_message = state.app.room_eq_state.error_message.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Load Measurement Data")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new(
                    "Load measurement data from a previous recording session or import from a JSON file.",
                )
                .size(TextSize::Sm)
                .color(theme.text_secondary),
            )
            // Error message display
            .when(error_message.is_some(), |div| {
                div.child(
                    Card::new()
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new("Error")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Sm)
                                                .color(theme.error),
                                        )
                                        .child(
                                            Text::new(error_message.unwrap_or_default())
                                                .size(TextSize::Sm)
                                                .color(theme.text_primary),
                                        ),
                                )
                                .child(
                                    Button::new("dismiss_error", "Dismiss")
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .build()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _, _, cx| {
                                                view.state.update(cx, |state, _| {
                                                    state.app.room_eq_state.error_message = None;
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        )
                        .into_any_element()
                        .into_any(),
                )
            })
            .child(
                Card::new()
                    .header(Text::new("From Recording Session").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Use measurements from the Recording screen.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("load_from_recording", "Load from Recording")
                                    .variant(ButtonVariant::Primary)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.load_room_eq_from_recording(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .header(Text::new("From JSON File").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Import measurements from a previously saved JSON file.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("load_from_file", "Browse...")
                                    .variant(ButtonVariant::Secondary)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.load_room_eq_from_file(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
            .when(has_recordings, |div| {
                div.child(
                    Card::new()
                        .header(Text::new("Loaded Data").weight(TextWeight::Semibold))
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new("✓")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Lg)
                                                .color(theme.success),
                                        )
                                        .child(
                                            Text::new(format!("{} channel(s) loaded successfully", channel_count))
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold)
                                                .color(theme.success),
                                        ),
                                )
                                .child(
                                    Text::new("You can now click Next to configure the optimization settings.")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Button::new("save_measurements", "Save Measurements...")
                                        .variant(ButtonVariant::Secondary)
                                        .build()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _, _, cx| {
                                                view.save_measurements_to_file(cx);
                                            }),
                                        ),
                                ),
                        ),
                )
            })
    }

    fn render_room_eq_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let room_eq = &state.app.room_eq_state;

        // Build AutoEqConfig from our RoomEqOptimizerConfig
        let autoeq_config = AutoEqConfig {
            algorithm: match room_eq.optimizer_config.algorithm {
                RoomEqAlgorithm::Cobyla => UiAutoEqAlgorithm::Cobyla,
                RoomEqAlgorithm::DifferentialEvolution => UiAutoEqAlgorithm::DifferentialEvolution,
                RoomEqAlgorithm::NelderMead => UiAutoEqAlgorithm::NelderMead,
            },
            num_filters: room_eq.optimizer_config.num_filters,
            min_q: room_eq.optimizer_config.min_q,
            max_q: room_eq.optimizer_config.max_q,
            min_db: room_eq.optimizer_config.min_db,
            max_db: room_eq.optimizer_config.max_db,
            min_freq: room_eq.optimizer_config.min_freq,
            max_freq: room_eq.optimizer_config.max_freq,
            max_iter: room_eq.optimizer_config.max_iter,
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
            algorithm_open: room_eq.dropdowns.algorithm_open,
            editing_field: room_eq.dropdowns.autoeq_editing_field.map(|f| match f {
                AutoEqField::NumFilters => UiAutoEqField::NumFilters,
                AutoEqField::MinQ => UiAutoEqField::MinQ,
                AutoEqField::MaxQ => UiAutoEqField::MaxQ,
                AutoEqField::MinDb => UiAutoEqField::MinDb,
                AutoEqField::MaxDb => UiAutoEqField::MaxDb,
                AutoEqField::MinFreq => UiAutoEqField::MinFreq,
                AutoEqField::MaxFreq => UiAutoEqField::MaxFreq,
                AutoEqField::MaxIter => UiAutoEqField::MaxIter,
            }),
            edit_text: room_eq.dropdowns.autoeq_edit_text.clone(),
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("room-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .on_algorithm_change({
                let state = self.state.clone();
                move |alg, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.algorithm = match alg {
                            UiAutoEqAlgorithm::Cobyla => RoomEqAlgorithm::Cobyla,
                            UiAutoEqAlgorithm::DifferentialEvolution => {
                                RoomEqAlgorithm::DifferentialEvolution
                            }
                            UiAutoEqAlgorithm::NelderMead => RoomEqAlgorithm::NelderMead,
                        };
                        state.app.room_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algorithm_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.algorithm_open = open;
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_freq = value;
                    });
                }
            })
            .on_max_iter_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_iter = value;
                    });
                }
            })
            .on_field_edit_start({
                let state = self.state.clone();
                move |field, _window, cx| {
                    state.update(cx, |state, _cx| {
                        let local_field = match field {
                            UiAutoEqField::NumFilters => AutoEqField::NumFilters,
                            UiAutoEqField::MinQ => AutoEqField::MinQ,
                            UiAutoEqField::MaxQ => AutoEqField::MaxQ,
                            UiAutoEqField::MinDb => AutoEqField::MinDb,
                            UiAutoEqField::MaxDb => AutoEqField::MaxDb,
                            UiAutoEqField::MinFreq => AutoEqField::MinFreq,
                            UiAutoEqField::MaxFreq => AutoEqField::MaxFreq,
                            UiAutoEqField::MaxIter => AutoEqField::MaxIter,
                        };
                        state.app.room_eq_state.dropdowns.autoeq_editing_field = Some(local_field);
                        // Initialize edit text with current value
                        let config = &state.app.room_eq_state.optimizer_config;
                        state.app.room_eq_state.dropdowns.autoeq_edit_text = match field {
                            UiAutoEqField::NumFilters => config.num_filters.to_string(),
                            UiAutoEqField::MinQ => format!("{:.1}", config.min_q),
                            UiAutoEqField::MaxQ => format!("{:.1}", config.max_q),
                            UiAutoEqField::MinDb => format!("{:.1}", config.min_db),
                            UiAutoEqField::MaxDb => format!("{:.1}", config.max_db),
                            UiAutoEqField::MinFreq => format!("{:.0}", config.min_freq),
                            UiAutoEqField::MaxFreq => format!("{:.0}", config.max_freq),
                            UiAutoEqField::MaxIter => config.max_iter.to_string(),
                        };
                    });
                }
            })
            .on_field_edit_end({
                let state = self.state.clone();
                move |_window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.autoeq_editing_field = None;
                        state.app.room_eq_state.dropdowns.autoeq_edit_text.clear();
                    });
                }
            })
            .on_edit_text_change({
                let state = self.state.clone();
                move |text, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.autoeq_edit_text = text;
                    });
                }
            });

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Configure per-channel settings and optimizer parameters.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Optimizer Settings").weight(TextWeight::Semibold))
                    .content(autoeq_form),
            )
            .child(
                Card::new()
                    .header(Text::new("Channel Configuration").weight(TextWeight::Semibold))
                    .content(self.render_channel_config_list(cx)),
            )
    }

    fn render_room_eq_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let progress = state.app.room_eq_state.overall_progress;
        let status_msg = &state.app.room_eq_state.status_message;
        let is_running = state.app.room_eq_state.is_optimizing();
        let is_completed = state.app.room_eq_state.is_optimization_complete();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Run Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Run the optimization process for each channel.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Optimization Progress").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .align(StackAlign::Center)
                                    .child(
                                        Text::new(format!("Progress: {:.0}%", progress * 100.0))
                                            .size(TextSize::Sm)
                                            .weight(TextWeight::Semibold),
                                    )
                                    .when(is_running, |stack| {
                                        stack.child(
                                            Text::new("●")
                                                .size(TextSize::Sm)
                                                .color(theme.info),
                                        )
                                    })
                                    .when(is_completed, |stack| {
                                        stack.child(
                                            Text::new("✓")
                                                .size(TextSize::Sm)
                                                .color(theme.success),
                                        )
                                    }),
                            )
                            .child(
                                // Progress bar
                                div()
                                    .w_full()
                                    .h(px(8.0))
                                    .bg(theme.background_secondary)
                                    .rounded_md()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(relative(progress))
                                            .h_full()
                                            .bg(if is_completed { theme.success } else { theme.info }),
                                    ),
                            )
                            .child(
                                Text::new(status_msg.clone())
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("start_optimization", if is_running { "Optimizing..." } else { "Start Optimization" })
                                    .variant(ButtonVariant::Primary)
                                    .disabled(is_running)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.start_room_eq_optimization(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
    }

    fn render_room_eq_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let pre_score = state.app.room_eq_state.average_pre_score();
        let post_score = state.app.room_eq_state.average_post_score();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Review Results")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Review the optimization results before applying.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Score Summary").weight(TextWeight::Semibold))
                    .content(VStack::new().spacing(StackSpacing::Sm).child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(Text::new(format!("Before: {:.2}", pre_score)))
                            .child(Text::new(format!("After: {:.2}", post_score)))
                            .child(
                                Text::new(format!("Improvement: {:.2}", pre_score - post_score))
                                    .color(if post_score < pre_score {
                                        theme.success
                                    } else {
                                        theme.error
                                    }),
                            ),
                    )),
            )
            .child(
                Card::new()
                    .header(Text::new("Per-Channel Results").weight(TextWeight::Semibold))
                    .content(self.render_channel_results(cx)),
            )
    }

    fn render_room_eq_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Export & Apply")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Export the DSP chain or apply directly to the player.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .header(Text::new("Export Options").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new("export_json", "Export as JSON")
                                    .variant(ButtonVariant::Secondary)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.export_room_eq_json(cx);
                                        }),
                                    ),
                            )
                            .child(
                                Button::new("apply_to_player", "Apply to Player")
                                    .variant(ButtonVariant::Primary)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.apply_room_eq_to_player(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
    }

    // === Review Step UI ===

    /// Render per-channel optimization results
    fn render_channel_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let channel_results = state.app.room_eq_state.channel_results.clone();

        if channel_results.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("No optimization results yet. Run optimization first.")
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }

        VStack::new()
            .spacing(StackSpacing::Lg)
            .children(channel_results.iter().map(|result| {
                render_channel_result_card(result, &theme)
            }))
            .into_any_element()
    }

    // === Channel Configuration UI ===

    /// Render the list of channel configurations
    fn render_channel_config_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let speaker_configs = state.app.room_eq_state.speaker_configs.clone();

        if speaker_configs.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("No channels configured. Load measurement data first.")
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }

        let view = cx.entity().clone();

        // Collect rows before returning to avoid closure lifetime issues
        let rows: Vec<_> = speaker_configs
            .iter()
            .enumerate()
            .map(|(idx, config)| {
                render_channel_config_row(idx, config, &theme, &view)
            })
            .collect();

        VStack::new()
            .spacing(StackSpacing::Md)
            .children(rows)
            .into_any_element()
    }

    // === Action Handlers ===

    fn load_room_eq_from_recording(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .room_eq_state
                .load_from_recording(&state.app.recording_state);
            state.app.room_eq_state.init_speaker_configs();
        });
    }

    fn load_room_eq_from_file(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{
            RoomEqDataSource, RoomEqMeasurementsFile, RoomEqSpeakerConfig, SpeakerConfigType,
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Load Room EQ Measurements")
                .pick_file()
                .await;

            if let Some(file) = file {
                let file_path = file.path().to_path_buf();
                log::info!("Loading measurements from {:?}", file_path);

                // Read file content
                match std::fs::read_to_string(&file_path) {
                    Ok(json) => {
                        log::debug!("File read successfully, size: {} bytes", json.len());

                        // Deserialize measurements file
                        match serde_json::from_str::<RoomEqMeasurementsFile>(&json) {
                            Ok(measurements_file) => {
                                log::info!(
                                    "Successfully parsed {} channel measurements from {:?}",
                                    measurements_file.channels.len(),
                                    file_path
                                );

                                // Validate that we have at least one channel
                                if measurements_file.channels.is_empty() {
                                    log::error!("No channels found in measurements file");
                                    let _ = state_entity.update(cx, |state, _| {
                                        state.app.room_eq_state.error_message =
                                            Some("No channels found in the measurement file".to_string());
                                    });
                                    return;
                                }

                                // Validate each channel has data
                                for (idx, channel) in measurements_file.channels.iter().enumerate() {
                                    if channel.measurement.frequencies.is_empty() {
                                        log::error!("Channel {} '{}' has no frequency data", idx, channel.channel_name);
                                        let _ = state_entity.update(cx, |state, _| {
                                            state.app.room_eq_state.error_message =
                                                Some(format!("Channel '{}' has no frequency data", channel.channel_name));
                                        });
                                        return;
                                    }
                                    log::debug!(
                                        "Channel {}: {} freq points, is_group: {}",
                                        channel.channel_name,
                                        channel.measurement.frequencies.len(),
                                        channel.is_group
                                    );
                                }

                                // Create speaker configs from loaded measurements
                                let speaker_configs: Vec<RoomEqSpeakerConfig> = measurements_file
                                    .channels
                                    .iter()
                                    .map(|m| {
                                        let config_type = if m.is_group {
                                            SpeakerConfigType::MultiDriver
                                        } else {
                                            SpeakerConfigType::Single
                                        };
                                        RoomEqSpeakerConfig {
                                            channel_name: m.channel_name.clone(),
                                            config_type,
                                            driver_names: m
                                                .group_drivers
                                                .iter()
                                                .enumerate()
                                                .map(|(i, _)| format!("Driver {}", i + 1))
                                                .collect(),
                                            ..Default::default()
                                        }
                                    })
                                    .collect();

                                let channel_count = measurements_file.channels.len();
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.channel_measurements =
                                        measurements_file.channels;
                                    state.app.room_eq_state.speaker_configs = speaker_configs;
                                    state.app.room_eq_state.data_source =
                                        RoomEqDataSource::FromFile(file_path.clone());
                                    state.app.room_eq_state.status_message = format!(
                                        "Successfully loaded {} channel(s) from {}",
                                        channel_count,
                                        file_path.display()
                                    );
                                    state.app.room_eq_state.error_message = None;
                                });
                            }
                            Err(e) => {
                                log::error!("JSON parse error: {}", e);
                                // Try to provide more helpful error messages
                                let error_msg = if json.contains("\"channel\"") && !json.contains("\"version\"") {
                                    format!(
                                        "File format error: Missing 'version' field. This may be an old format file. Error: {}",
                                        e
                                    )
                                } else if !json.contains("\"channels\"") {
                                    format!(
                                        "File format error: Missing 'channels' field. This doesn't appear to be a valid measurement file. Error: {}",
                                        e
                                    )
                                } else {
                                    format!("Failed to parse JSON: {}", e)
                                };

                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.error_message = Some(error_msg);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File read error: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.room_eq_state.error_message =
                                Some(format!("Failed to read file: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn save_measurements_to_file(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::RoomEqMeasurementsFile;

        // Get current measurements from state
        let measurements = {
            let state = self.state.read(cx);
            state.app.room_eq_state.channel_measurements.clone()
        };

        if measurements.is_empty() {
            log::warn!("No measurements to save");
            return;
        }

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Save Room EQ Measurements")
                .set_file_name("room_eq_measurements.json")
                .save_file()
                .await;

            if let Some(file) = file {
                let file_path = file.path().to_path_buf();

                // Create measurements file structure
                let measurements_file = RoomEqMeasurementsFile::new(measurements);

                // Serialize to JSON
                match serde_json::to_string_pretty(&measurements_file) {
                    Ok(json) => {
                        // Write to file
                        match std::fs::write(&file_path, json) {
                            Ok(()) => {
                                log::info!("Saved measurements to {:?}", file_path);
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.status_message =
                                        format!("Saved to {}", file_path.display());
                                    state.app.room_eq_state.error_message = None;
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to write measurements file: {}", e);
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.error_message =
                                        Some(format!("Failed to write file: {}", e));
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize measurements: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.room_eq_state.error_message =
                                Some(format!("Failed to serialize: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn start_room_eq_optimization(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{ChannelOptResult, EqFilterConfig, OptimizationStatus};
        use sotf_audio_player::room_eq::{
            ChannelConfig, ChannelMeasurements, Curve, Measurement, OptimizerConfig,
            RoomEqOptimizer,
        };
        use std::collections::HashMap;
        use std::sync::Arc;

        log::info!("Starting room EQ optimization");

        // Collect data from state
        let (channels, configs, optimizer_config) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.room_eq_state;

            // Build channel measurements map
            let mut channels: HashMap<String, ChannelMeasurements> = HashMap::new();
            for meas in &room_eq.channel_measurements {
                // Convert f32 frequencies/magnitudes to f64 curves
                let curve = Curve::new(
                    ndarray::Array1::from_vec(
                        meas.measurement.frequencies.iter().map(|&f| f as f64).collect(),
                    ),
                    ndarray::Array1::from_vec(
                        meas.measurement.magnitude_db.iter().map(|&db| db as f64).collect(),
                    ),
                );
                let measurement = Measurement::new(&meas.channel_name, curve);

                if meas.is_group && !meas.group_drivers.is_empty() {
                    // Multi-driver
                    let driver_measurements: Vec<Measurement> = meas
                        .group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, driver)| {
                            let driver_curve = Curve::new(
                                ndarray::Array1::from_vec(
                                    driver.frequencies.iter().map(|&f| f as f64).collect(),
                                ),
                                ndarray::Array1::from_vec(
                                    driver.magnitude_db.iter().map(|&db| db as f64).collect(),
                                ),
                            );
                            Measurement::new(format!("driver_{}", i + 1), driver_curve)
                        })
                        .collect();
                    channels.insert(
                        meas.channel_name.clone(),
                        ChannelMeasurements::multi_driver(&meas.channel_name, driver_measurements),
                    );
                } else {
                    // Single driver
                    channels.insert(
                        meas.channel_name.clone(),
                        ChannelMeasurements::single(&meas.channel_name, measurement),
                    );
                }
            }

            // Build channel configs map
            let mut configs: HashMap<String, ChannelConfig> = HashMap::new();
            for cfg in &room_eq.speaker_configs {
                configs.insert(
                    cfg.channel_name.clone(),
                    ChannelConfig {
                        name: cfg.channel_name.clone(),
                        config_type: cfg.config_type.into(),
                        crossover_type: Some(cfg.crossover_type.into()),
                        driver_names: Vec::new(),
                        crossover_freq_hints: Vec::new(),
                    },
                );
            }

            // Optimizer config
            let opt_cfg = OptimizerConfig {
                algorithm: room_eq.optimizer_config.algorithm.into(),
                num_filters: room_eq.optimizer_config.num_filters,
                min_q: room_eq.optimizer_config.min_q,
                max_q: room_eq.optimizer_config.max_q,
                min_db: room_eq.optimizer_config.min_db,
                max_db: room_eq.optimizer_config.max_db,
                min_freq: room_eq.optimizer_config.min_freq,
                max_freq: room_eq.optimizer_config.max_freq,
                max_iter: room_eq.optimizer_config.max_iter,
                sample_rate: 48000.0, // Default sample rate
            };

            (channels, configs, opt_cfg)
        };

        // Update state to running
        self.state.update(cx, |state, _cx| {
            state.app.room_eq_state.optimization_status = OptimizationStatus::Running;
            state.app.room_eq_state.status_message = "Starting optimization...".to_string();
            state.app.room_eq_state.channel_results.clear();
            state.app.room_eq_state.overall_progress = 0.0;
        });

        if channels.is_empty() {
            log::warn!("No channels to optimize");
            self.state.update(cx, |state, _cx| {
                state.app.room_eq_state.optimization_status = OptimizationStatus::Failed;
                state.app.room_eq_state.error_message =
                    Some("No channels to optimize".to_string());
            });
            return;
        }

        // Create the optimizer
        let optimizer = Arc::new(RoomEqOptimizer::new(optimizer_config));

        // Clone state for the async task
        let state_clone = self.state.clone();
        let state_for_progress = self.state.clone();
        let crossover_types: HashMap<String, _> = configs
            .iter()
            .map(|(k, v)| (k.clone(), v.crossover_type.unwrap_or_default()))
            .collect();

        // Create progress channel
        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::channel::<sotf_audio_player::room_eq::OptimizationProgress>(100);

        // Spawn a task to listen to progress updates
        cx.spawn(async move |_, cx| {
            while let Some(progress) = progress_rx.recv().await {
                let _ = state_for_progress.update(&mut cx.clone(), |state, cx| {
                    state.app.room_eq_state.overall_progress = progress.overall_progress;
                    state.app.room_eq_state.status_message = progress.message.clone();
                    cx.notify(); // Trigger UI update
                });
            }
        })
        .detach();

        // Spawn the optimization task with progress channel
        cx.spawn(async move |_, cx| {
            // Run the optimization with progress updates
            let result = optimizer
                .optimize_all_channels(channels, configs, Some(progress_tx))
                .await;

            // Process result
            match result {
                Ok(results) => {
                    // Generate DSP output
                    let dsp_output = optimizer.generate_dsp_output(&results, &crossover_types);

                    // Convert results to UI format
                    let channel_results: Vec<ChannelOptResult> = results
                        .into_iter()
                        .map(|(name, r)| ChannelOptResult {
                            channel_name: name,
                            pre_score: r.pre_score,
                            post_score: r.post_score,
                            eq_filters: r
                                .eq_filters
                                .iter()
                                .map(|f| EqFilterConfig {
                                    filter_type: f.filter_type.clone(),
                                    frequency: f.frequency,
                                    q: f.q,
                                    gain_db: f.gain_db,
                                })
                                .collect(),
                            crossover_freqs: r.crossover_freqs,
                            driver_gains: r.driver_gains,
                            original_response: r.original_response.map(|c| {
                                c.freq
                                    .iter()
                                    .zip(c.spl.iter())
                                    .map(|(&f, &db)| (f, db))
                                    .collect()
                            }),
                            corrected_response: r.corrected_response.map(|c| {
                                c.freq
                                    .iter()
                                    .zip(c.spl.iter())
                                    .map(|(&f, &db)| (f, db))
                                    .collect()
                            }),
                        })
                        .collect();

                    // Update state with results
                    let _ = state_clone.update(&mut cx.clone(), |state, _| {
                        state.app.room_eq_state.optimization_status =
                            OptimizationStatus::Completed;
                        state.app.room_eq_state.status_message =
                            "Optimization complete!".to_string();
                        state.app.room_eq_state.channel_results = channel_results;
                        state.app.room_eq_state.overall_progress = 1.0;
                        state.app.room_eq_state.dsp_output =
                            Some(crate::app::types::DspChainOutput {
                                channels: dsp_output
                                    .channels
                                    .into_iter()
                                    .map(|(k, v)| {
                                        (
                                            k,
                                            crate::app::types::ChannelDspChain {
                                                channel: v.channel,
                                                plugins: v
                                                    .plugins
                                                    .iter()
                                                    .map(|p| {
                                                        crate::app::types::DspPluginConfig {
                                                            plugin_type: p.plugin_type.clone(),
                                                            parameters: p.parameters.clone(),
                                                        }
                                                    })
                                                    .collect(),
                                                drivers: v.drivers.map(|d| {
                                                    d.into_iter()
                                                        .map(|dr| {
                                                            crate::app::types::DriverDspChain {
                                                                name: dr.name,
                                                                index: dr.index,
                                                                plugins: dr
                                                                    .plugins
                                                                    .iter()
                                                                    .map(|p| {
                                                                        crate::app::types::DspPluginConfig {
                                                                            plugin_type: p.plugin_type.clone(),
                                                                            parameters: p.parameters.clone(),
                                                                        }
                                                                    })
                                                                    .collect(),
                                                            }
                                                        })
                                                        .collect()
                                                }),
                                            },
                                        )
                                    })
                                    .collect(),
                                metadata: dsp_output.metadata.map(|m| {
                                    crate::app::types::DspChainMetadata {
                                        pre_score: m.pre_score,
                                        post_score: m.post_score,
                                        algorithm: m.algorithm,
                                        iterations: m.iterations,
                                        timestamp: m.timestamp,
                                    }
                                }),
                            });
                        // Advance to review step
                        state.app.room_eq_state.step = crate::app::types::RoomEqStep::Review;
                    });

                    log::info!("Room EQ optimization completed successfully");
                }
                Err(e) => {
                    log::error!("Room EQ optimization failed: {}", e);
                    let _ = state_clone.update(&mut cx.clone(), |state, _| {
                        state.app.room_eq_state.optimization_status = OptimizationStatus::Failed;
                        state.app.room_eq_state.error_message = Some(e);
                    });
                }
            }
        })
        .detach();
    }

    fn export_room_eq_json(&mut self, cx: &mut Context<Self>) {
        // Get the DSP output from state
        let dsp_output = {
            let state = self.state.read(cx);
            state.app.room_eq_state.dsp_output.clone()
        };

        let Some(dsp_output) = dsp_output else {
            log::warn!("No DSP output to export");
            self.state.update(cx, |state, _| {
                state.app.room_eq_state.error_message =
                    Some("No optimization results to export".to_string());
            });
            return;
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Export Room EQ Configuration")
                .set_file_name("room_eq.json")
                .save_file()
                .await;

            if let Some(file) = file {
                // Serialize DSP output
                match serde_json::to_string_pretty(&dsp_output) {
                    Ok(json) => {
                        // Write to file
                        match std::fs::write(file.path(), &json) {
                            Ok(()) => {
                                log::info!("Exported room EQ config to {:?}", file.path());
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.status_message =
                                        format!("Saved to {}", file.path().display());
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to write room EQ file: {}", e);
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.error_message =
                                        Some(format!("Failed to write: {}", e));
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize room EQ JSON: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.room_eq_state.error_message =
                                Some(format!("Failed to serialize: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn apply_room_eq_to_player(&mut self, cx: &mut Context<Self>) {
        use sotf_audio::PluginConfig;

        // Get the DSP output from state
        let dsp_output = {
            let state = self.state.read(cx);
            state.app.room_eq_state.dsp_output.clone()
        };

        let Some(dsp_output) = dsp_output else {
            log::warn!("No DSP output to apply");
            self.state.update(cx, |state, _| {
                state.app.room_eq_state.error_message =
                    Some("No optimization results to apply".to_string());
            });
            return;
        };

        // Convert DSP output to PluginConfigs
        let mut plugins: Vec<PluginConfig> = Vec::new();

        // For each channel, extract the plugins
        for (channel_name, channel_dsp) in dsp_output.channels.iter() {
            log::info!("Applying room EQ for channel: {}", channel_name);

            // Add channel plugins (EQ filters)
            for plugin in &channel_dsp.plugins {
                let config = PluginConfig {
                    plugin_type: plugin.plugin_type.clone(),
                    parameters: plugin.parameters.clone(),
                };
                plugins.push(config);
            }

            // Add driver plugins if multi-driver
            if let Some(ref drivers) = channel_dsp.drivers {
                for driver_dsp in drivers {
                    for plugin in &driver_dsp.plugins {
                        let config = PluginConfig {
                            plugin_type: plugin.plugin_type.clone(),
                            parameters: plugin.parameters.clone(),
                        };
                        plugins.push(config);
                    }
                }
            }
        }

        log::info!("Applied {} plugins from room EQ", plugins.len());

        // Update state with applied plugins and show success message
        self.state.update(cx, |state, _| {
            // Store the plugins for the audio engine to use
            state.app.room_eq_applied_plugins = Some(plugins);
            state.app.room_eq_state.status_message =
                "Room EQ applied to player!".to_string();
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Room EQ applied successfully",
            ));
        });
    }

    // Settings content (legacy, kept for compatibility)
    pub(crate) fn render_roomeq_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Card::new()
                    .header(Text::new("Data Acquisition").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(
                                    "Measure your room impulse response to calculate correction filters.",
                                )
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("meas_btn", "Measure Room Response")
                                    .variant(ButtonVariant::Primary)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.measure_state =
                                                    Some(MeasureState::default());
                                            });
                                        }),
                                    ),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .header(
                        Text::new("Room Correction (Coming Soon)").weight(TextWeight::Semibold),
                    )
                    .content(
                        Text::new(
                            "Optimization logic will be integrated after measurements are available.",
                        )
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                    ),
            )
            .into_any_element()
    }
}

// === Free functions for channel configuration UI ===

/// Render a single channel configuration row
fn render_channel_config_row(
    idx: usize,
    config: &crate::app::types::RoomEqSpeakerConfig,
    theme: &crate::theme::Theme,
    view: &Entity<PlayerView>,
) -> impl IntoElement {
    use crate::app::types::SpeakerConfigType;

    let channel_name = config.channel_name.clone();
    let is_multi = config.config_type == SpeakerConfigType::MultiDriver;
    let crossover_type = config.crossover_type;

    div()
        .flex()
        .gap_4()
        .items_center()
        .w_full()
        .p_3()
        .bg(theme.surface)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        // Channel name
        .child(
            div()
                .w(px(80.0))
                .child(
                    Text::new(channel_name)
                        .weight(TextWeight::Bold)
                        .color(theme.text_primary),
                ),
        )
        // Speaker type toggle
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(Text::new("Type:").size(TextSize::Sm).color(theme.text_secondary))
                .child(
                    Button::new(SharedString::from(format!("single-{}", idx)), "Single")
                        .variant(if !is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Sm)
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) =
                                            state.app.room_eq_state.speaker_configs.get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::Single;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("multi-{}", idx)),
                        "Multi-Driver",
                    )
                    .variant(if is_multi {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Sm)
                    .on_click({
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    if let Some(cfg) =
                                        state.app.room_eq_state.speaker_configs.get_mut(idx)
                                    {
                                        cfg.config_type = SpeakerConfigType::MultiDriver;
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
                ),
        )
        // Crossover type selector (only shown for multi-driver)
        .when(is_multi, |el| {
            el.child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Text::new("Crossover:")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    )
                    .child(render_crossover_dropdown(idx, crossover_type, view)),
            )
        })
}

/// Render crossover type dropdown as a cycling button
fn render_crossover_dropdown(
    channel_idx: usize,
    current: crate::app::types::CrossoverType,
    view: &Entity<PlayerView>,
) -> impl IntoElement {
    use crate::app::types::CrossoverType;

    let crossover_types = CrossoverType::all();
    let current_label = current.as_str();

    Button::new(
        SharedString::from(format!("xover-{}", channel_idx)),
        current_label,
    )
    .variant(ButtonVariant::Secondary)
    .size(ButtonSize::Sm)
    .on_click({
        let view = view.clone();
        let crossover_types = crossover_types.to_vec();
        move |_, cx| {
            view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    if let Some(cfg) = state
                        .app
                        .room_eq_state
                        .speaker_configs
                        .get_mut(channel_idx)
                    {
                        // Find current index and cycle to next
                        let current_idx = crossover_types
                            .iter()
                            .position(|&ct| ct == cfg.crossover_type)
                            .unwrap_or(0);
                        let next_idx = (current_idx + 1) % crossover_types.len();
                        cfg.crossover_type = crossover_types[next_idx];
                    }
                });
                cx.notify();
            });
        }
    })
}

// === Review Step UI Free Functions ===

/// Render a single channel result card with plots and filter details
fn render_channel_result_card(
    result: &crate::app::types::ChannelOptResult,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    let channel_name = result.channel_name.clone();
    let score_improvement = result.pre_score - result.post_score;
    let has_response_data = result.original_response.is_some() && result.corrected_response.is_some();

    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .bg(theme.surface)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        // Header with channel name and scores
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    Text::new(channel_name)
                        .weight(TextWeight::Bold)
                        .size(TextSize::Lg)
                        .color(theme.text_primary),
                )
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            Text::new(format!("Before: {:.2}", result.pre_score))
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("After: {:.2}", result.post_score))
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("{:+.2}", score_improvement))
                                .weight(TextWeight::Bold)
                                .color(if score_improvement > 0.0 {
                                    theme.success
                                } else {
                                    theme.error
                                }),
                        ),
                ),
        )
        // Frequency response plot (if available)
        .when(has_response_data, |div| {
            let original = result.original_response.as_ref().unwrap();
            let corrected = result.corrected_response.as_ref().unwrap();
            div.child(render_response_comparison_graph(original, corrected, theme))
        })
        // EQ Filter details
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("EQ Filters")
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                .child(render_filter_table(&result.eq_filters, theme)),
        )
        // Crossover info (if multi-driver)
        .when(result.crossover_freqs.is_some(), |el| {
            let xover_freqs = result.crossover_freqs.as_ref().unwrap();
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Crossover Frequencies")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Sm)
                            .color(theme.text_primary),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .gap_2()
                            .children(xover_freqs.iter().map(|f| {
                                gpui::div()
                                    .px_2()
                                    .py_1()
                                    .bg(theme.background_secondary)
                                    .rounded_md()
                                    .child(
                                        Text::new(format_frequency(*f))
                                            .size(TextSize::Sm)
                                            .color(theme.text_primary),
                                    )
                            })),
                    ),
            )
        })
}

/// Render the frequency response comparison graph
fn render_response_comparison_graph(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use d3rs::color::D3Color;
    use d3rs::scale::{LinearScale, LogScale, Scale};
    use d3rs::shape::{LineConfig, LinePoint, render_line};

    const GRAPH_WIDTH: f32 = 400.0;
    const GRAPH_HEIGHT: f32 = 150.0;
    const Y_AXIS_WIDTH: f32 = 32.0;
    const X_AXIS_HEIGHT: f32 = 16.0;
    const MIN_FREQ: f64 = 20.0;
    const MAX_FREQ: f64 = 20000.0;

    // Calculate dB range
    let all_values: Vec<f64> = original
        .iter()
        .chain(corrected.iter())
        .map(|(_, db)| *db)
        .collect();
    let min_db = all_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
        .max(-24.0);
    let max_db = all_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .min(24.0);

    // Add padding
    let range = max_db - min_db;
    let padding = range * 0.1;
    let min_db = ((min_db - padding) / 6.0).floor() * 6.0;
    let max_db = ((max_db + padding) / 6.0).ceil() * 6.0;

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, GRAPH_WIDTH as f64);
    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(GRAPH_HEIGHT as f64, 0.0);

    // Create line points
    let original_points: Vec<LinePoint> = original
        .iter()
        .map(|(f, db)| LinePoint::new(*f, *db))
        .collect();
    let corrected_points: Vec<LinePoint> = corrected
        .iter()
        .map(|(f, db)| LinePoint::new(*f, *db))
        .collect();

    let original_config = LineConfig::new()
        .stroke_width(1.5)
        .stroke_color(D3Color::from_rgba(theme.text_muted));
    let corrected_config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(theme.info));

    let original_line = render_line(&freq_scale, &db_scale, &original_points, &original_config);
    let corrected_line = render_line(&freq_scale, &db_scale, &corrected_points, &corrected_config);

    div()
        .w(px(GRAPH_WIDTH + Y_AXIS_WIDTH))
        .h(px(GRAPH_HEIGHT + X_AXIS_HEIGHT + 24.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                // Graph area
                .child(
                    div()
                        .w(px(GRAPH_WIDTH))
                        .h(px(GRAPH_HEIGHT))
                        .bg(theme.background)
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .relative()
                        .overflow_hidden()
                        // Zero line
                        .when(min_db <= 0.0 && max_db >= 0.0, |el| {
                            let zero_y = db_scale.scale(0.0) as f32;
                            el.child(
                                div()
                                    .absolute()
                                    .top(px(zero_y))
                                    .left_0()
                                    .right_0()
                                    .h(px(1.0))
                                    .bg(theme.text_muted)
                                    .opacity(0.3),
                            )
                        })
                        .child(original_line)
                        .child(corrected_line),
                ),
        )
        // Legend
        .child(
            div()
                .flex()
                .gap_4()
                .justify_center()
                .pt_2()
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(div().w(px(12.0)).h(px(2.0)).bg(theme.text_muted))
                        .child(
                            Text::new("Original")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(div().w(px(12.0)).h(px(2.0)).bg(theme.info))
                        .child(
                            Text::new("Corrected")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                ),
        )
}

/// Render the EQ filter table
fn render_filter_table(
    filters: &[crate::app::types::EqFilterConfig],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    if filters.is_empty() {
        return div()
            .child(
                Text::new("No filters")
                    .size(TextSize::Sm)
                    .color(theme.text_muted),
            )
            .into_any_element();
    }

    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let gain_color = if f.gain_db > 0.5 {
                theme.success
            } else if f.gain_db < -0.5 {
                theme.error
            } else {
                theme.text_muted
            };

            div()
                .px_3()
                .py_2()
                .bg(theme.background_secondary)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap_1()
                .min_w(px(80.0))
                // Filter number and type
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .items_center()
                        .child(
                            Text::new(format!("{}", i + 1))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Xs)
                                .color(theme.text_primary),
                        )
                        .child(
                            Text::new(&f.filter_type)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
                // Frequency
                .child(
                    Text::new(format_frequency(f.frequency))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                // Gain and Q
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Text::new(format!("{:+.1}dB", f.gain_db))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Sm)
                                .color(gain_color),
                        )
                        .child(
                            Text::new(format!("Q:{:.1}", f.q))
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        ),
                )
        }))
        .into_any_element()
}
