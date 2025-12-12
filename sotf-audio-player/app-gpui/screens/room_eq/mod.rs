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
    AutoEqFormUiState, Button, ButtonVariant, Card, HStack, StackSpacing, StepStatus, Text,
    TextSize, TextWeight, VStack, WizardHeader, WizardNavigation, WizardStep, WizardTheme,
};

impl PlayerView {
    /// Main Room EQ screen entry point
    pub(crate) fn render_room_eq_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.room_eq_state.step;

        // Map our domain steps to wizard steps
        let steps = vec![
            WizardStep::new("load_data", "Load Data"),
            WizardStep::new("configure", "Configure"),
            WizardStep::new("optimize", "Optimize"),
            WizardStep::new("review", "Review"),
            WizardStep::new("export", "Export"),
        ];

        let current_step_index = self.room_eq_step_index(&current_step);
        let total_steps = steps.len();

        // Build step statuses
        let step_statuses: Vec<StepStatus> = (0..total_steps)
            .map(|i| {
                if i < current_step_index {
                    StepStatus::Completed
                } else if i == current_step_index {
                    StepStatus::Active
                } else {
                    StepStatus::NotVisited
                }
            })
            .collect();

        // Determine navigation state
        let can_go_back = current_step_index > 0;
        let can_go_next = self.room_eq_can_advance(cx);
        let is_busy = state.app.room_eq_state.is_optimizing();

        // Content for current step
        let content = match current_step {
            RoomEqStep::LoadData => self.render_room_eq_load_data(cx).into_any_element(),
            RoomEqStep::Configure => self.render_room_eq_configure(cx).into_any_element(),
            RoomEqStep::Optimize => self.render_room_eq_optimize(cx).into_any_element(),
            RoomEqStep::Review => self.render_room_eq_review(cx).into_any_element(),
            RoomEqStep::Export => self.render_room_eq_export(cx).into_any_element(),
        };

        let wizard_theme = WizardTheme {
            step_bg: theme.background_secondary,
            step_completed_bg: theme.success,
            step_active_bg: theme.accent,
            step_error_bg: theme.error,
            step_text: theme.text_primary,
            label_text: theme.text_secondary,
            label_active_text: theme.text_primary,
            connector_color: theme.border,
            connector_completed_color: theme.success,
            step_border: theme.border,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(
                WizardHeader::new()
                    .steps(steps)
                    .step_statuses(step_statuses)
                    .current_step(current_step_index)
                    .theme(wizard_theme.clone()),
            )
            .child(
                div()
                    .id("room-eq-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
            .child(
                WizardNavigation::new(current_step_index, total_steps)
                    .back_disabled(!can_go_back)
                    .next_disabled(!can_go_next)
                    .is_busy(is_busy)
                    .show_cancel(true)
                    .theme(wizard_theme)
                    .on_back({
                        let state = self.state.clone();
                        move |_step, _window, cx| {
                            state.update(cx, |state, _cx| {
                                if let Some(prev) = state.app.room_eq_state.step.previous() {
                                    state.app.room_eq_state.step = prev;
                                }
                            });
                        }
                    })
                    .on_next({
                        let state = self.state.clone();
                        move |_step, _window, cx| {
                            state.update(cx, |state, _cx| {
                                if let Some(next) = state.app.room_eq_state.step.next() {
                                    state.app.room_eq_state.step = next;
                                }
                            });
                        }
                    })
                    .on_cancel({
                        let state = self.state.clone();
                        move |_window, cx| {
                            state.update(cx, |state, _cx| {
                                state.app.current_screen = crate::app::types::Screen::Library;
                            });
                        }
                    })
                    .on_finish({
                        let state = self.state.clone();
                        move |_window, cx| {
                            // TODO: Actually apply/save the DSP chain
                            state.update(cx, |state, _cx| {
                                state.app.current_screen = crate::app::types::Screen::Library;
                            });
                        }
                    }),
            )
    }

    /// Map RoomEqStep to index
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
                            VStack::new().spacing(StackSpacing::Sm).child(
                                Text::new(format!("{} channel(s) loaded", channel_count))
                                    .size(TextSize::Sm)
                                    .color(theme.success),
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
                    .content(
                        Text::new(
                            "TODO: Per-channel config (single vs multi-driver, crossover settings)",
                        )
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                    ),
            )
    }

    fn render_room_eq_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let progress = state.app.room_eq_state.overall_progress;
        let status_msg = &state.app.room_eq_state.status_message;

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
                                Text::new(format!("Progress: {:.0}%", progress * 100.0))
                                    .size(TextSize::Sm),
                            )
                            .child(
                                Text::new(status_msg.clone())
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("start_optimization", "Start Optimization")
                                    .variant(ButtonVariant::Primary)
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
                    .content(
                        Text::new("TODO: Frequency response plots, EQ filter details")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    ),
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

    fn load_room_eq_from_file(&mut self, _cx: &mut Context<Self>) {
        // TODO: Open file dialog and load JSON
        log::info!("TODO: Load room EQ from file");
    }

    fn start_room_eq_optimization(&mut self, cx: &mut Context<Self>) {
        // TODO: Spawn async optimization task
        log::info!("TODO: Start room EQ optimization");
        self.state.update(cx, |state, _cx| {
            state.app.room_eq_state.optimization_status =
                crate::app::types::OptimizationStatus::Running;
            state.app.room_eq_state.status_message = "Starting optimization...".to_string();
        });
    }

    fn export_room_eq_json(&mut self, _cx: &mut Context<Self>) {
        // TODO: Save DSP chain as JSON
        log::info!("TODO: Export room EQ as JSON");
    }

    fn apply_room_eq_to_player(&mut self, _cx: &mut Context<Self>) {
        // TODO: Convert DSP chain to PluginChain and apply
        log::info!("TODO: Apply room EQ to player");
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
