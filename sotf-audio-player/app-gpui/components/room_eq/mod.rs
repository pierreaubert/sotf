//! Room EQ Screen
//!
//! Multi-step wizard for room EQ optimization:
//! 1. Load Data - Load/import measurement data
//! 2. Configure - Configure channels and optimizer settings
//! 3. Optimize - Run optimization (per-channel, then combined)
//! 4. Review - Review results and visualizations
//! 5. Export - Export DSP chain and apply

use crate::app::types::{RoomEqAlgorithm, RoomEqStep};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack,
    StackAlign, StackSpacing, StepStatus, Text, TextSize, TextWeight, VStack, WizardHeader,
    WizardStep,
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

    /// Render the room EQ screen header with step indicators using WizardHeader
    fn render_room_eq_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.room_eq_state.step;

        // Build wizard steps
        let steps = vec![
            WizardStep::new("load-data", "Load Data"),
            WizardStep::new("configure", "Configure"),
            WizardStep::new("optimize", "Optimize"),
            WizardStep::new("review", "Review"),
            WizardStep::new("export", "Export"),
        ];

        // Build step statuses based on current step
        let step_statuses: Vec<StepStatus> = RoomEqStep::all()
            .iter()
            .map(|step| {
                if step.index() < current_step.index() {
                    StepStatus::Completed
                } else if step.index() == current_step.index() {
                    StepStatus::Active
                } else {
                    StepStatus::NotVisited
                }
            })
            .collect();

        let wizard_header = WizardHeader::new()
            .title("Room EQ")
            .steps(steps)
            .step_statuses(step_statuses)
            .current_step(current_step.index());

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(wizard_header)
            .child(self.render_room_eq_nav_buttons(cx))
    }

    /// Render navigation buttons (Close/Back and Next/Finish)
    fn render_room_eq_nav_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let current_step = state.app.room_eq_state.step;
        let can_go_next = self.room_eq_can_advance(cx);
        let is_busy = state.app.room_eq_state.is_optimizing();
        let _view = cx.entity().clone();

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
                    .theme(theme.to_button_theme())
                    .disabled(is_busy)
                    .build()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.update(cx, |state, _| {
                                match state.app.room_eq_state.step {
                                    RoomEqStep::LoadData => {
                                        // Go back to previous screen
                                        state.app.current_screen = state.app.last_screen;
                                    }
                                    _ => {
                                        // Go back to previous step
                                        if let Some(prev) = state.app.room_eq_state.step.previous()
                                        {
                                            state.app.room_eq_state.step = prev;
                                        }
                                    }
                                }
                            });
                            cx.notify();
                        }),
                    ),
            )
            .child(
                Button::new("next", next_label)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Md)
                    .theme(theme.to_button_theme())
                    .disabled(!can_go_next || is_busy)
                    .build()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.update(cx, |state, _| {
                                match state.app.room_eq_state.step {
                                    RoomEqStep::Export => {
                                        // Finish - apply and go back
                                        state.app.current_screen = state.app.last_screen;
                                    }
                                    _ => {
                                        // Go to next step
                                        if let Some(next) = state.app.room_eq_state.step.next() {
                                            state.app.room_eq_state.step = next;
                                        }
                                    }
                                }
                            });
                            cx.notify();
                        }),
                    ),
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
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
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
                                    .theme(theme.to_button_theme())
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
        let config = &room_eq.optimizer_config;
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
                RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
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
            algo_open: room_eq.dropdowns.algorithm_open,
            peq_model_open: room_eq.dropdowns.peq_model_open,
            strategy_open: false,
            local_algo_open: false,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("room-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.algorithm = match algo {
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
                        state.app.room_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.algorithm_open = open;
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |_model, _window, cx| {
                    state.update(cx, |state, _cx| {
                        // PEQ model is stored in autoeq_config.peq_model which is read-only display
                        // The actual model selection doesn't need to be stored separately
                        state.app.room_eq_state.dropdowns.peq_model_open = false;
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.peq_model_open = open;
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
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_iter = value;
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
                                            Text::new("●").size(TextSize::Sm).color(theme.info),
                                        )
                                    })
                                    .when(is_completed, |stack| {
                                        stack.child(
                                            Text::new("✓").size(TextSize::Sm).color(theme.success),
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
                                    .child(div().w(relative(progress)).h_full().bg(
                                        if is_completed {
                                            theme.success
                                        } else {
                                            theme.info
                                        },
                                    )),
                            )
                            .child(
                                Text::new(status_msg.clone())
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new(
                                    "start_optimization",
                                    if is_running {
                                        "Optimizing..."
                                    } else {
                                        "Start Optimization"
                                    },
                                )
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
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
                    .content(
                        VStack::new().spacing(StackSpacing::Sm).child(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                .child(Text::new(format!("Before: {:.2}", pre_score)))
                                .child(Text::new(format!("After: {:.2}", post_score)))
                                .child(
                                    Text::new(format!(
                                        "Improvement: {:.2}",
                                        pre_score - post_score
                                    ))
                                    .color(
                                        if post_score < pre_score {
                                            theme.success
                                        } else {
                                            theme.error
                                        },
                                    ),
                                ),
                        ),
                    ),
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
        let has_eq_in_rack = state
            .app
            .plugin_chain
            .find_plugin_index(&sotf_audio_player::PluginType::EQ)
            .is_some();

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
                    .header(Text::new("Backup Current Rack").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(
                                    "Save a copy of your current plugin rack before applying changes.",
                                )
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("backup_rack", "Save Rack Backup...")
                                    .variant(ButtonVariant::Secondary)
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.save_rack_backup(cx);
                                        }),
                                    ),
                            ),
                    ),
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
                            ),
                    ),
            )
            .child(
                Card::new()
                    .header(Text::new("Apply to Player").weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(if has_eq_in_rack {
                                    "An EQ plugin exists in your rack. It will be updated with the new filters."
                                } else {
                                    "No EQ plugin in rack. A new EQ will be added before any monitoring plugins."
                                })
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("apply_to_player", "Apply to Player")
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
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
            .children(
                channel_results
                    .iter()
                    .map(|result| render_channel_result_card(result, &theme)),
            )
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
            .map(|(idx, config)| render_channel_config_row(idx, config, &theme, &view))
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
        use crate::app::types::{
            ChannelOptResult, EqFilterConfig, OptimizationStatus, SpeakerConfigType,
        };
        use sotf_audio_player::room_eq::{
            CallbackAction, CallbackConfig, MeasurementInput, OptimizationParams,
            SpeakerOptimizationConfig, SpeakerOptimizationProgress,
            run_speaker_optimization_with_callback,
        };

        log::info!("Starting room EQ optimization with new speaker optimization");

        // Collect configurations from state
        let (channel_configs, _optimizer_params) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.room_eq_state;

            // Build speaker optimization configs for each channel
            let configs: Vec<(String, SpeakerOptimizationConfig, SpeakerConfigType)> = room_eq
                .channel_measurements
                .iter()
                .zip(room_eq.speaker_configs.iter())
                .map(|(meas, speaker_cfg)| {
                    // Convert measurement data to autoeq::Curve
                    let frequencies: Vec<f64> = meas
                        .measurement
                        .frequencies
                        .iter()
                        .map(|&f| f as f64)
                        .collect();
                    let magnitude_db: Vec<f64> = meas
                        .measurement
                        .magnitude_db
                        .iter()
                        .map(|&db| db as f64)
                        .collect();

                    let main_curve = autoeq::Curve {
                        freq: ndarray::Array1::from_vec(frequencies),
                        spl: ndarray::Array1::from_vec(magnitude_db),
                        phase: None,
                    };

                    // Build driver curves for multi-driver config
                    let driver_measurements: Vec<MeasurementInput> =
                        if meas.is_group && !meas.group_drivers.is_empty() {
                            meas.group_drivers
                                .iter()
                                .map(|driver| {
                                    let freq: Vec<f64> =
                                        driver.frequencies.iter().map(|&f| f as f64).collect();
                                    let spl: Vec<f64> =
                                        driver.magnitude_db.iter().map(|&db| db as f64).collect();
                                    MeasurementInput::Curve(autoeq::Curve {
                                        freq: ndarray::Array1::from_vec(freq),
                                        spl: ndarray::Array1::from_vec(spl),
                                        phase: None,
                                    })
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };

                    let config_type = if meas.is_group && !meas.group_drivers.is_empty() {
                        sotf_audio_player::room_eq::SpeakerConfigType::MultiDriver
                    } else {
                        sotf_audio_player::room_eq::SpeakerConfigType::Single
                    };

                    let crossover_type = match speaker_cfg.crossover_type {
                        crate::app::types::CrossoverType::Butterworth12 => {
                            sotf_audio_player::room_eq::CrossoverType::Butterworth12
                        }
                        crate::app::types::CrossoverType::Butterworth24 => {
                            sotf_audio_player::room_eq::CrossoverType::LR24
                        } // Fallback to LR24
                        crate::app::types::CrossoverType::LR12 => {
                            sotf_audio_player::room_eq::CrossoverType::LR12
                        }
                        crate::app::types::CrossoverType::LR24 => {
                            sotf_audio_player::room_eq::CrossoverType::LR24
                        }
                        crate::app::types::CrossoverType::LR48 => {
                            sotf_audio_player::room_eq::CrossoverType::LR48
                        }
                    };

                    let speaker_config = SpeakerOptimizationConfig {
                        config_type,
                        main_measurement: Some(MeasurementInput::Curve(main_curve)),
                        driver_measurements,
                        crossover_type: Some(crossover_type),
                        crossover_freq_hints: Vec::new(),
                        params: OptimizationParams {
                            num_filters: room_eq.optimizer_config.num_filters,
                            sample_rate: 48000,
                            min_db: room_eq.optimizer_config.min_db,
                            max_db: room_eq.optimizer_config.max_db,
                            min_q: room_eq.optimizer_config.min_q,
                            max_q: room_eq.optimizer_config.max_q,
                            min_freq: room_eq.optimizer_config.min_freq,
                            max_freq: room_eq.optimizer_config.max_freq,
                            algo: match room_eq.optimizer_config.algorithm {
                                crate::app::types::RoomEqAlgorithm::Cobyla => {
                                    "nlopt:cobyla".to_string()
                                }
                                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => {
                                    "autoeq:de".to_string()
                                }
                                crate::app::types::RoomEqAlgorithm::NelderMead => {
                                    "nlopt:neldermead".to_string()
                                }
                            },
                            maxeval: room_eq.optimizer_config.max_iter,
                            loss: "speaker-flat".to_string(),
                            ..OptimizationParams::speaker_defaults()
                        },
                        callback_config: Some(CallbackConfig {
                            interval: 25, // Report every 25 iterations
                            include_biquads: true,
                            include_filter_response: true,
                        }),
                        target: None,
                    };

                    (
                        meas.channel_name.clone(),
                        speaker_config,
                        speaker_cfg.config_type,
                    )
                })
                .collect();

            let opt_params = OptimizationParams {
                num_filters: room_eq.optimizer_config.num_filters,
                sample_rate: 48000,
                min_db: room_eq.optimizer_config.min_db,
                max_db: room_eq.optimizer_config.max_db,
                min_q: room_eq.optimizer_config.min_q,
                max_q: room_eq.optimizer_config.max_q,
                min_freq: room_eq.optimizer_config.min_freq,
                max_freq: room_eq.optimizer_config.max_freq,
                algo: match room_eq.optimizer_config.algorithm {
                    crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla".to_string(),
                    crate::app::types::RoomEqAlgorithm::DifferentialEvolution => {
                        "autoeq:de".to_string()
                    }
                    crate::app::types::RoomEqAlgorithm::NelderMead => {
                        "nlopt:neldermead".to_string()
                    }
                },
                maxeval: room_eq.optimizer_config.max_iter,
                loss: "speaker-flat".to_string(),
                ..OptimizationParams::speaker_defaults()
            };

            (configs, opt_params)
        };

        // Update state to running
        self.state.update(cx, |state, _cx| {
            state.app.room_eq_state.optimization_status = OptimizationStatus::Running;
            state.app.room_eq_state.status_message = "Starting optimization...".to_string();
            state.app.room_eq_state.channel_results.clear();
            state.app.room_eq_state.overall_progress = 0.0;
        });

        if channel_configs.is_empty() {
            log::warn!("No channels to optimize");
            self.state.update(cx, |state, _cx| {
                state.app.room_eq_state.optimization_status = OptimizationStatus::Failed;
                state.app.room_eq_state.error_message = Some("No channels to optimize".to_string());
            });
            return;
        }

        let total_channels = channel_configs.len();
        let state_clone = self.state.clone();

        // Spawn the optimization task
        cx.spawn(async move |_, cx| {
            let mut all_results: Vec<ChannelOptResult> = Vec::new();
            let mut total_pre_score = 0.0;
            let mut total_post_score = 0.0;

            for (channel_idx, (channel_name, config, _ui_config_type)) in
                channel_configs.into_iter().enumerate()
            {
                // Update status for current channel
                let channel_name_for_status = channel_name.clone();
                let _ = state_clone.update(&mut cx.clone(), |state, cx| {
                    state.app.room_eq_state.current_channel = Some(channel_name_for_status.clone());
                    state.app.room_eq_state.status_message = format!(
                        "Optimizing {} ({}/{})",
                        channel_name_for_status,
                        channel_idx + 1,
                        total_channels
                    );
                    cx.notify();
                });

                // Create progress tracking for this channel
                let channel_idx_f = channel_idx as f32;
                let total_channels_f = total_channels as f32;
                let channel_name_cb = channel_name.clone();

                // Create callback that updates UI with real-time progress
                let callback: sotf_audio_player::room_eq::SpeakerOptimizationCallback =
                    Box::new(move |progress: &SpeakerOptimizationProgress| {
                        let iteration = progress.iteration;
                        let loss = progress.loss;
                        let max_iter = progress.max_iterations;
                        let stage = progress.stage;
                        let num_biquads = progress.current_biquads.len();

                        // Calculate overall progress
                        let channel_progress = if max_iter > 0 {
                            iteration as f32 / max_iter as f32
                        } else {
                            0.0
                        };
                        let _overall = (channel_idx_f + channel_progress) / total_channels_f;

                        let stage_str = match stage {
                            sotf_audio_player::room_eq::OptimizationStage::Crossover => "crossover",
                            sotf_audio_player::room_eq::OptimizationStage::Eq => "EQ",
                            sotf_audio_player::room_eq::OptimizationStage::Refinement => {
                                "refinement"
                            }
                        };

                        // Update UI state (note: this is sync context, so we can't use async update)
                        // The callback runs in a blocking thread, so we log progress instead
                        log::debug!(
                            "Channel {}: iter {}/{} ({}) loss={:.4} filters={}",
                            channel_name_cb,
                            iteration,
                            max_iter,
                            stage_str,
                            loss,
                            num_biquads
                        );

                        CallbackAction::Continue
                    });

                // Run optimization in blocking task (optimization is CPU-bound)
                let config_clone = config.clone();
                let result = smol::unblock(move || {
                    run_speaker_optimization_with_callback(&config_clone, Some(callback))
                })
                .await;

                match result {
                    Ok(speaker_result) => {
                        log::info!(
                            "Channel {} optimized: {:.4} -> {:.4}",
                            channel_name,
                            speaker_result.initial_loss,
                            speaker_result.final_loss
                        );

                        total_pre_score += speaker_result.initial_loss;
                        total_post_score += speaker_result.final_loss;

                        // Convert to UI result format
                        let channel_result = ChannelOptResult {
                            channel_name: channel_name.clone(),
                            pre_score: speaker_result.initial_loss,
                            post_score: speaker_result.final_loss,
                            eq_filters: speaker_result
                                .biquads
                                .iter()
                                .map(|b| EqFilterConfig {
                                    filter_type: format!("{:?}", b.filter_type),
                                    frequency: b.freq,
                                    q: b.q,
                                    gain_db: b.db_gain,
                                })
                                .collect(),
                            crossover_freqs: speaker_result.crossover_freqs.clone(),
                            driver_gains: speaker_result.driver_gains.clone(),
                            original_response: Some(
                                speaker_result
                                    .frequencies
                                    .iter()
                                    .zip(speaker_result.input_curve.iter())
                                    .map(|(&f, &db)| (f, db))
                                    .collect(),
                            ),
                            corrected_response: Some(
                                speaker_result
                                    .frequencies
                                    .iter()
                                    .zip(speaker_result.corrected_curve.iter())
                                    .map(|(&f, &db)| (f, db))
                                    .collect(),
                            ),
                        };

                        all_results.push(channel_result);

                        // Update progress
                        let progress = (channel_idx + 1) as f32 / total_channels as f32;
                        let _ = state_clone.update(&mut cx.clone(), |state, cx| {
                            state.app.room_eq_state.overall_progress = progress;
                            state.app.room_eq_state.status_message = format!(
                                "Completed {} ({}/{})",
                                channel_name,
                                channel_idx + 1,
                                total_channels
                            );
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        log::error!("Channel {} optimization failed: {}", channel_name, e);
                        let _ = state_clone.update(&mut cx.clone(), |state, _| {
                            state.app.room_eq_state.optimization_status =
                                OptimizationStatus::Failed;
                            state.app.room_eq_state.error_message =
                                Some(format!("Task error for {}: {}", channel_name, e));
                        });
                        return;
                    }
                }
            }

            // All channels completed - update final state
            let avg_pre = if !all_results.is_empty() {
                total_pre_score / all_results.len() as f64
            } else {
                0.0
            };
            let avg_post = if !all_results.is_empty() {
                total_post_score / all_results.len() as f64
            } else {
                0.0
            };

            log::info!(
                "Room EQ optimization completed: avg score {:.4} -> {:.4}",
                avg_pre,
                avg_post
            );

            let _ = state_clone.update(&mut cx.clone(), |state, _| {
                state.app.room_eq_state.optimization_status = OptimizationStatus::Completed;
                state.app.room_eq_state.status_message = format!(
                    "Optimization complete! Score: {:.2} -> {:.2}",
                    avg_pre, avg_post
                );
                state.app.room_eq_state.channel_results = all_results;
                state.app.room_eq_state.overall_progress = 1.0;
                state.app.room_eq_state.current_channel = None;

                // Build DSP output from results
                let mut dsp_channels = std::collections::HashMap::new();
                for result in &state.app.room_eq_state.channel_results {
                    let eq_params = serde_json::json!({
                        "filters": result.eq_filters.iter().map(|f| {
                            serde_json::json!({
                                "filter_type": f.filter_type.to_lowercase(),
                                "frequency": f.frequency,
                                "q": f.q,
                                "gain_db": f.gain_db
                            })
                        }).collect::<Vec<_>>()
                    });

                    dsp_channels.insert(
                        result.channel_name.clone(),
                        crate::app::types::ChannelDspChain {
                            channel: result.channel_name.clone(),
                            plugins: vec![crate::app::types::DspPluginConfig {
                                plugin_type: "EQ".to_string(),
                                parameters: eq_params,
                            }],
                            drivers: None,
                        },
                    );
                }

                state.app.room_eq_state.dsp_output = Some(crate::app::types::DspChainOutput {
                    channels: dsp_channels,
                    metadata: Some(crate::app::types::DspChainMetadata {
                        pre_score: avg_pre,
                        post_score: avg_post,
                        algorithm: state
                            .app
                            .room_eq_state
                            .optimizer_config
                            .algorithm
                            .as_str()
                            .to_string(),
                        iterations: state.app.room_eq_state.optimizer_config.max_iter,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    }),
                });

                // Advance to review step
                state.app.room_eq_state.step = crate::app::types::RoomEqStep::Review;
            });
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

    fn save_rack_backup(&mut self, cx: &mut Context<Self>) {
        // Get the current plugin chain
        let plugin_chain = {
            let state = self.state.read(cx);
            state.app.plugin_chain.clone()
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Generate default filename with timestamp
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let default_name = format!("rack_backup_{}.json", timestamp);

            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Save Rack Backup")
                .set_file_name(&default_name)
                .save_file()
                .await;

            if let Some(file) = file {
                let file_path = file.path().to_path_buf();

                match plugin_chain.save_to_file(file_path.to_str().unwrap_or("backup.json")) {
                    Ok(()) => {
                        log::info!("Saved rack backup to {:?}", file_path);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.room_eq_state.status_message =
                                format!("Backup saved to {}", file_path.display());
                            state.app.toast_message =
                                Some(crate::app::ToastMessage::success("Rack backup saved"));
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to save rack backup: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.room_eq_state.error_message =
                                Some(format!("Failed to save backup: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn apply_room_eq_to_player(&mut self, cx: &mut Context<Self>) {
        use autoeq_iir::BiquadFilterType;
        use sotf_audio_player::{EQFilter, PluginSettings, PluginType};

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

        // Collect all EQ filters from the optimization results
        let mut eq_filters: Vec<EQFilter> = Vec::new();
        for (_channel_name, channel_dsp) in dsp_output.channels.iter() {
            for plugin in &channel_dsp.plugins {
                if plugin.plugin_type == "EQ" {
                    // Extract filters from the parameters
                    if let Some(filters) =
                        plugin.parameters.get("filters").and_then(|f| f.as_array())
                    {
                        for filter in filters {
                            let filter_type_str = filter
                                .get("filter_type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("peak");
                            let filter_type = match filter_type_str.to_lowercase().as_str() {
                                "peak" | "pk" => BiquadFilterType::Peak,
                                "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                                "highshelf" | "hs" => BiquadFilterType::Highshelf,
                                "lowpass" | "lp" => BiquadFilterType::Lowpass,
                                "highpass" | "hp" => BiquadFilterType::Highpass,
                                "notch" => BiquadFilterType::Notch,
                                _ => BiquadFilterType::Peak,
                            };
                            let frequency = filter
                                .get("frequency")
                                .and_then(|f| f.as_f64())
                                .unwrap_or(1000.0);
                            let q = filter.get("q").and_then(|q| q.as_f64()).unwrap_or(1.0);
                            let gain_db = filter
                                .get("gain_db")
                                .and_then(|g| g.as_f64())
                                .unwrap_or(0.0);

                            eq_filters.push(EQFilter::new(filter_type, frequency, q, gain_db));
                        }
                    }
                }
            }
        }

        if eq_filters.is_empty() {
            log::warn!("No EQ filters found in optimization results");
            self.state.update(cx, |state, _| {
                state.app.room_eq_state.error_message =
                    Some("No EQ filters found in optimization results".to_string());
            });
            return;
        }

        log::info!("Applying {} EQ filters from room EQ", eq_filters.len());

        // Update the plugin chain
        self.state.update(cx, |state, _| {
            let plugin_chain = &mut state.app.plugin_chain;

            // Check if there's an existing EQ plugin
            if let Some(eq_idx) = plugin_chain.find_plugin_index(&PluginType::EQ) {
                // Update existing EQ plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(eq_idx) {
                    eq_plugin.settings = PluginSettings::EQ {
                        filters: eq_filters.clone(),
                    };
                    log::info!("Updated existing EQ plugin at index {}", eq_idx);
                }
            } else {
                // No EQ plugin exists, add one before monitoring plugins
                let insert_idx = plugin_chain.find_processing_insert_index();
                plugin_chain.insert_plugin(insert_idx, &PluginType::EQ);

                // Configure the newly inserted plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(insert_idx) {
                    eq_plugin.settings = PluginSettings::EQ {
                        filters: eq_filters.clone(),
                    };
                }
                log::info!("Inserted new EQ plugin at index {}", insert_idx);
            }

            // Mark that plugin chain was modified and needs sync
            state.app.plugin_chain_modified = true;
            state.app.pending_plugin_update = Some(crate::app::types::PluginUpdateType::Structural);
            state.app.room_eq_state.status_message = "Room EQ applied to player!".to_string();
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Room EQ applied successfully",
            ));
        });

        cx.notify();
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
            div().w(px(80.0)).child(
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
                .child(
                    Text::new("Type:")
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                )
                .child(
                    Button::new(SharedString::from(format!("single-{}", idx)), "Single")
                        .variant(if !is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
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
                    Button::new(SharedString::from(format!("multi-{}", idx)), "Multi-Driver")
                        .variant(if is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
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
                    if let Some(cfg) = state.app.room_eq_state.speaker_configs.get_mut(channel_idx)
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
    let has_response_data =
        result.original_response.is_some() && result.corrected_response.is_some();

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
