use crate::app::types::{OptimizationStatus, SpinoramaOptimizationMode};
use crate::components::graphs::common::{rgba_to_u32, theme_to_chart_theme};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::line;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Badge, BadgeVariant, Button, ButtonSize,
    ButtonVariant, Card, HStack, Progress, ProgressSize, ProgressVariant, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};

impl PlayerView {

    // ========================================================================
    // Step 2: Configure
    // ========================================================================

    pub(crate) fn render_spinorama_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;

        // Filter available modes based on phase data availability
        // Default is IIR. If phase data is available, add FIR and mixed options.
        let allowed_modes = if spinorama.has_phase_data {
            vec!["iir".to_string(), "fir".to_string(), "mixed".to_string()]
        } else {
            vec!["iir".to_string()] // Only IIR when no phase data
        };

        let mut opt_mode = spinorama.dropdowns.opt_mode.clone();
        // If no phase data and mode requires it (FIR/mixed), fall back to IIR
        if !spinorama.has_phase_data && (opt_mode == "fir" || opt_mode == "mixed") {
            opt_mode = "iir".to_string();
        }

        // Build AutoEqConfig from our SpinoramaOptimizerConfig
        let config = &spinorama.optimizer_config;
        let autoeq_config = AutoEqConfig {
            opt_mode,
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
                crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
            }
            .to_string(),
            population: config.population,
            maxeval: config.max_iter,
            de_f: config.de_f,
            de_cr: config.de_cr,
            strategy: config.strategy.clone(),
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            smooth: config.smooth,
            ..Default::default()
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
            opt_mode_open: spinorama.dropdowns.opt_mode_open,
            peq_model_open: spinorama.dropdowns.peq_model_open,
            algo_open: spinorama.dropdowns.algorithm_open,
            strategy_open: spinorama.dropdowns.strategy_open,
            local_algo_open: spinorama.dropdowns.local_algo_open,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("spinorama-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .allowed_opt_modes(allowed_modes)
            .show_goals(false) // Hide Goals section (System Type, Loss Type, Target Curve)
            .show_optimization_tuning(true) // Show Optimization Fine Tuning section
            .on_opt_mode_change({
                let state = self.state.clone();
                move |mode, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.dropdowns.opt_mode = mode.to_string();
                        state.app.spinorama_eq_state.dropdowns.opt_mode_open = false;
                    });
                }
            })
            .on_opt_mode_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.spinorama_eq_state.dropdowns.opt_mode_open = open;
                        cx.notify();
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |model, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.peq_model = model.to_string();
                        state.app.spinorama_eq_state.dropdowns.peq_model_open = false;
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
            })
            .on_population_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.population = value;
                    });
                }
            })
            .on_de_f_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.de_f = value;
                    });
                }
            })
            .on_de_cr_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.de_cr = value;
                    });
                }
            })
            .on_strategy_change({
                let state = self.state.clone();
                move |strategy, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.strategy = strategy.to_string();
                        state.app.spinorama_eq_state.dropdowns.strategy_open = false;
                    });
                }
            })
            .on_strategy_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.spinorama_eq_state.dropdowns.strategy_open = open;
                        cx.notify();
                    });
                }
            })
            .on_refine_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.refine = value;
                    });
                }
            })
            .on_local_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.local_algo = algo.to_string();
                        state.app.spinorama_eq_state.dropdowns.local_algo_open = false;
                    });
                }
            })
            .on_local_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.spinorama_eq_state.dropdowns.local_algo_open = open;
                        cx.notify();
                    });
                }
            })
            .on_smooth_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.spinorama_eq_state.optimizer_config.smooth = value;
                    });
                }
            });

        // Optimization mode selection
        let current_mode = spinorama.optimizer_config.mode;

        // Optimization status
        let progress = spinorama.progress;
        let status_msg = spinorama.status_message.clone();
        let error_msg = spinorama.error_message.clone();
        let optimization_status = spinorama.optimization_status.clone();
        let is_optimizing = spinorama.is_optimizing();
        let is_completed = optimization_status == OptimizationStatus::Completed;
        let is_failed = optimization_status == OptimizationStatus::Failed;
        let show_progress = is_optimizing || is_completed || is_failed;
        let selected_speaker = spinorama.selected_speaker.clone().unwrap_or_default();
        let progress_history = spinorama.progress_history.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .color(theme.text_primary)
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
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Optimization Mode")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
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
            // Target curve selection (only shown when mode is FlatOnPir/Target)
            .when(current_mode == SpinoramaOptimizationMode::FlatOnPir, |vstack| {
                let current_curve = spinorama.optimizer_config.target_curve;
                let theme = theme.clone();

                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new("Target Curve")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new("Select which measurement curve to flatten.")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .children(
                                            crate::app::types::SpinoramaTargetCurve::all().iter().map(|curve| {
                                                let is_selected = current_curve == *curve;
                                                let curve_value = *curve;

                                                Button::new(
                                                    SharedString::from(format!("spinorama-curve-{:?}", curve)),
                                                    curve.short_name(),
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
                                                                .target_curve = curve_value;
                                                        });
                                                        cx.notify();
                                                    }),
                                                )
                                            }),
                                        ),
                                )
                                .child(
                                    Text::new(current_curve.as_str())
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
                                ),
                        ),
                )
            })
            .child(autoeq_form)
            // Generate Speaker EQ card with progress
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(
                                Text::new("Generate Speaker EQ")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .when(!selected_speaker.is_empty(), |hstack| {
                                hstack.child(
                                    Text::new(selected_speaker.clone())
                                        .size(TextSize::Sm)
                                        .color(theme.accent),
                                )
                            }),
                    )
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
                            .when(show_progress, |vstack| {
                                let display_progress = if is_completed {
                                    100.0
                                } else if is_optimizing {
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
                                                    Badge::new("Failed").variant(BadgeVariant::Error),
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
                                    .child(
                                        Text::new(status_msg)
                                            .size(TextSize::Sm)
                                            .color(if is_completed {
                                                theme.success
                                            } else if is_failed {
                                                theme.error
                                            } else {
                                                theme.text_secondary
                                            }),
                                    )
                            })
                            .when_some(error_msg, |vstack, err| {
                                vstack.child(Text::new(err).size(TextSize::Sm).color(theme.error))
                            }),
                    ),
            )
            // Optimization Process graph (shown when progress history is available)
            .when(!progress_history.is_empty(), |vstack| {
                let theme = theme.clone();
                let history = progress_history.clone();
                let chart_theme = theme_to_chart_theme(&theme);

                let iterations: Vec<f64> = history.iter().map(|&(i, _, _)| i as f64).collect();
                let losses: Vec<f64> = history.iter().map(|&(_, loss, _)| loss).collect();
                let scores: Vec<f64> = history
                    .iter()
                    .filter_map(|&(_, _, score)| score)
                    .collect();
                let has_scores = !scores.is_empty();

                let current_loss = losses.last().copied().unwrap_or(0.0);
                let best_loss = losses.iter().copied().fold(f64::INFINITY, f64::min);

                // Build chart with loss curve (and optionally score curve on secondary axis)
                let mut chart_builder = line(&iterations, &losses)
                    .title("Optimization Process")
                    .x_label("Iteration")
                    .y_label("Loss")
                    .label("Loss")
                    .color(rgba_to_u32(theme.graph_colors.filter_response))
                    .stroke_width(2.0)
                    .theme(chart_theme.clone())
                    .size(700.0, 250.0);

                // Add score series on secondary (right) axis if scores are available
                let chart = if has_scores {
                    let score_iterations: Vec<f64> = history
                        .iter()
                        .filter_map(|&(i, _, score)| score.map(|_| i as f64))
                        .collect();
                    // Set secondary axis label and range for score
                    chart_builder = chart_builder
                        .y2_label("Score")
                        .y2_range(0.0, 10.0);
                    chart_builder
                        .add_series_y2_with_x(
                            &score_iterations,
                            &scores,
                            Some("Score"),
                            rgba_to_u32(theme.graph_colors.target),
                            2.0,
                            1.0,
                        )
                        .build()
                } else {
                    chart_builder.build()
                };

                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                .child(
                                    Text::new("Optimization Process")
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .child(
                                    Text::new(format!("Current: {:.4}", current_loss))
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(format!("Best: {:.4}", best_loss))
                                        .size(TextSize::Sm)
                                        .color(theme.success),
                                ),
                        )
                        .content(
                            div()
                                .w(px(700.0))
                                .flex()
                                .flex_col()
                                .when_some(chart.ok(), |el, c| el.child(c)),
                        ),
                )
            })
    }

}
