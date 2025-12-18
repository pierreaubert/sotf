use crate::app::types::SpinoramaOptimizationMode;
use crate::ui::PlayerView;
use d3rs::prelude::{render_line, D3Color, LineConfig, LinePoint, LinearScale, LogScale};
use d3rs::scale::Scale;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack,
    Progress, ProgressSize, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {

    // ========================================================================
    // Step 2: Configure
    // ========================================================================

    pub(crate) fn render_spinorama_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;

        // Preview curves data
        let preview_loading = spinorama.loading_preview;
        let preview_error = spinorama.preview_error.clone();
        let preview_frequencies = spinorama.preview_frequencies.clone();
        let preview_input = spinorama.preview_input_curve.clone();
        let preview_target = spinorama.preview_target_curve.clone();
        let preview_deviation = spinorama.preview_deviation_curve.clone();
        let has_preview = !preview_frequencies.is_empty();

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
            // Preview chart showing input, target, and deviation curves
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Preview")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        if preview_loading {
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new("Loading preview curves...")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(Progress::new(0.0).size(ProgressSize::Md))
                                .into_any_element()
                        } else if let Some(err) = preview_error {
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new("Failed to load preview")
                                        .size(TextSize::Sm)
                                        .color(theme.error),
                                )
                                .child(
                                    Text::new(err)
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
                                )
                                .into_any_element()
                        } else if has_preview {
                            self.render_preview_chart(
                                &preview_frequencies,
                                &preview_input,
                                &preview_target,
                                &preview_deviation,
                                &theme,
                            )
                            .into_any_element()
                        } else {
                            Text::new("Select a speaker to see preview curves")
                                .size(TextSize::Sm)
                                .color(theme.text_muted)
                                .into_any_element()
                        },
                    ),
            )
            .child(autoeq_form)
    }

    /// Render the preview chart with input, target, and deviation curves
    fn render_preview_chart(
        &self,
        frequencies: &[f64],
        input: &[f64],
        target: &[f64],
        deviation: &[f64],
        theme: &crate::app::theme::Theme,
    ) -> impl IntoElement {
        use d3rs::prelude::LogScale;
        use d3rs::scale::Scale;

        const GRAPH_WIDTH: f32 = 550.0;
        const GRAPH_HEIGHT: f32 = 150.0;

        // Create log scale for frequency (x-axis)
        let freq_min = frequencies.first().copied().unwrap_or(20.0).max(20.0);
        let freq_max = frequencies.last().copied().unwrap_or(20000.0);
        let freq_scale = LogScale::new()
            .domain(freq_min, freq_max)
            .range(0.0, GRAPH_WIDTH as f64);

        // Find y-axis range from all curves
        let all_values: Vec<f64> = input
            .iter()
            .chain(target.iter())
            .chain(deviation.iter())
            .copied()
            .collect();
        let y_min = all_values.iter().cloned().fold(f64::INFINITY, f64::min) - 2.0;
        let y_max = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + 2.0;
        let db_scale = LinearScale::new()
            .domain(y_max, y_min) // Inverted for SVG coordinates
            .range(0.0, GRAPH_HEIGHT as f64);

        // Generate line points for each curve (use raw values, scale handles transformation)
        let input_points: Vec<LinePoint> = frequencies
            .iter()
            .zip(input.iter())
            .filter(|(f, _)| **f >= freq_min && **f <= freq_max)
            .map(|(f, db)| LinePoint::new(*f, *db))
            .collect();

        let target_points: Vec<LinePoint> = frequencies
            .iter()
            .zip(target.iter())
            .filter(|(f, _)| **f >= freq_min && **f <= freq_max)
            .map(|(f, db)| LinePoint::new(*f, *db))
            .collect();

        let deviation_points: Vec<LinePoint> = frequencies
            .iter()
            .zip(deviation.iter())
            .filter(|(f, _)| **f >= freq_min && **f <= freq_max)
            .map(|(f, db)| LinePoint::new(*f, *db))
            .collect();

        // Configure line styles
        let input_config = LineConfig::new()
            .stroke_width(1.5)
            .stroke_color(D3Color::from_hex(0x3498db)); // Blue

        let target_config = LineConfig::new()
            .stroke_width(1.5)
            .stroke_color(D3Color::from_hex(0x27ae60)); // Green

        let deviation_config = LineConfig::new()
            .stroke_width(1.0)
            .stroke_color(D3Color::from_hex(0xe74c3c)); // Red

        // Render the lines using d3rs
        let input_line = render_line(&freq_scale, &db_scale, &input_points, &input_config);
        let target_line = render_line(&freq_scale, &db_scale, &target_points, &target_config);
        let deviation_line = render_line(&freq_scale, &db_scale, &deviation_points, &deviation_config);

        // Calculate zero line position
        let zero_y = db_scale.scale(0.0) as f32;

        // Build legend
        let legend = HStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(2.0))
                            .bg(gpui::rgb(0x3498db)),
                    )
                    .child(Text::new("Input").size(TextSize::Xs).color(theme.text_secondary)),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(2.0))
                            .bg(gpui::rgb(0x27ae60)),
                    )
                    .child(Text::new("Target").size(TextSize::Xs).color(theme.text_secondary)),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(2.0))
                            .bg(gpui::rgb(0xe74c3c)),
                    )
                    .child(Text::new("Deviation").size(TextSize::Xs).color(theme.text_secondary)),
            );

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(legend)
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
                    .when(y_min <= 0.0 && y_max >= 0.0, |el| {
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
                    .child(input_line)
                    .child(target_line)
                    .child(deviation_line),
            )
            .child(
                Text::new("Frequency Response (dB)")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
    }

}
