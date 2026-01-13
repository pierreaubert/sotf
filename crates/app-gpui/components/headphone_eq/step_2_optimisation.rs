use crate::app::types::OptimizationStatus;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Badge, BadgeVariant, Button, ButtonSize,
    ButtonTheme, ButtonVariant, Card, OptimizationType, Progress, ProgressSize, ProgressVariant,
    StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 2: Optimization (EQ Design, Fine Tuning, Generate)
    // ========================================================================

    pub(crate) fn render_headphone_eq_optimization(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id));
        let headphone_eq = &state.app.measurement_state.headphone_eq_state;

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
            peq_model: config.peq_model.clone(),
            algo: match config.algorithm {
                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
            }
            .to_string(),
            population: config.population,
            maxeval: config.max_iter,
            de_f: config.de_f,
            de_cr: config.de_cr,
            strategy: config.strategy.clone(),
            tolerance: config.tolerance,
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            smooth: config.smooth,
            // Goals - use headphone_eq_state values
            loss_type: headphone_eq.loss_type.clone(),
            target_curve: headphone_eq.target_preset.clone(),
            ..Default::default()
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
            algo_open: headphone_eq.dropdowns.algorithm_open,
            peq_model_open: headphone_eq.dropdowns.peq_model_open,
            loss_type_open: headphone_eq.dropdowns.loss_type_open,
            target_curve_open: headphone_eq.dropdowns.target_curve_open,
            strategy_open: headphone_eq.dropdowns.strategy_open,
            local_algo_open: headphone_eq.dropdowns.local_algo_open,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("headphone-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .optimization_type(OptimizationType::Headphone)
            .show_goals(true)
            .show_optimization_tuning(true)
            .on_loss_type_change({
                let state = self.state.clone();
                move |loss_type, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.loss_type = loss_type.to_string();
                        state.app.measurement_state.headphone_eq_state.dropdowns.loss_type_open = false;
                    });
                }
            })
            .on_loss_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.dropdowns.loss_type_open = open;
                    });
                }
            })
            .on_target_curve_change({
                let state = self.state.clone();
                move |target, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.target_preset = target.to_string();
                        state.app.measurement_state.headphone_eq_state.dropdowns.target_curve_open = false;
                    });
                }
            })
            .on_target_curve_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.dropdowns.target_curve_open = open;
                    });
                }
            })
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    use crate::app::types::RoomEqAlgorithm;
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.algorithm = match algo {
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
                        state.app.measurement_state.headphone_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.dropdowns.algorithm_open = open;
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |model, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.peq_model = model.to_string();
                        state.app.measurement_state.headphone_eq_state.dropdowns.peq_model_open = false;
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.dropdowns.peq_model_open = open;
                    });
                }
            })
            .on_population_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.population = value;
                    });
                }
            })
            .on_de_f_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.de_f = value;
                    });
                }
            })
            .on_de_cr_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.de_cr = value;
                    });
                }
            })
            .on_strategy_change({
                let state = self.state.clone();
                move |strategy, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.strategy =
                            strategy.to_string();
                        state.app.measurement_state.headphone_eq_state.dropdowns.strategy_open = false;
                    });
                }
            })
            .on_strategy_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.dropdowns.strategy_open = open;
                    });
                }
            })
            .on_tolerance_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.tolerance = value;
                    });
                }
            })
            .on_refine_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.refine = value;
                    });
                }
            })
            .on_local_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.local_algo = algo.to_string();
                        state.app.measurement_state.headphone_eq_state.dropdowns.local_algo_open = false;
                    });
                }
            })
            .on_local_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.dropdowns.local_algo_open = open;
                    });
                }
            })
            .on_smooth_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.smooth = value;
                    });
                }
            })
            .on_smooth_n_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.smooth_n = value;
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.max_freq = value;
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.headphone_eq_state.optimizer_config.max_iter = value;
                    });
                }
            });

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .color(theme.text_primary)
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
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("EQ Parameters")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(autoeq_form),
            )
            // Generate EQ section
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Generate Headphone EQ")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let progress = headphone_eq.progress;
                        let status_msg = headphone_eq.status_message.clone();
                        let optimization_status = headphone_eq.optimization_status.clone();
                        let is_optimizing = headphone_eq.is_optimizing();
                        let is_completed = optimization_status == OptimizationStatus::Completed;
                        let is_failed = optimization_status == OptimizationStatus::Failed;
                        let show_progress = is_optimizing || is_completed || is_failed;

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
                                .theme(button_theme.clone())
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
                            .when(show_progress, |vstack| {
                                let display_progress = if is_completed {
                                    100.0
                                } else if is_optimizing {
                                    // Show indeterminate progress while optimizing
                                    // Using a small non-zero value to show activity
                                    (progress * 100.0).max(5.0)
                                } else {
                                    progress * 100.0
                                };

                                vstack
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Text::new(if is_optimizing {
                                                    "Optimizing...".to_string()
                                                } else {
                                                    format!("Progress: {:.0}%", display_progress)
                                                })
                                                .size(TextSize::Sm)
                                                .color(theme.text_primary),
                                            )
                                            .when(is_completed, |el| {
                                                el.child(
                                                    Badge::new("Success")
                                                        .variant(BadgeVariant::Success),
                                                )
                                            })
                                            .when(is_failed, |el| {
                                                el.child(
                                                    Badge::new("Failed")
                                                        .variant(BadgeVariant::Error),
                                                )
                                            }),
                                    )
                                    .child(
                                        Progress::new(display_progress)
                                            .size(ProgressSize::Md)
                                            .variant(if is_completed {
                                                ProgressVariant::Success
                                            } else if is_failed {
                                                ProgressVariant::Error
                                            } else {
                                                ProgressVariant::Default
                                            }),
                                    )
                                    .child(Text::new(status_msg).size(TextSize::Sm).color(
                                        if is_completed {
                                            theme.success
                                        } else if is_failed {
                                            theme.error
                                        } else {
                                            theme.text_secondary
                                        },
                                    ))
                            })
                    }),
            )
    }
}
