use crate::app::types::room_eq::InteractiveChartStateWrapper;
use crate::components::graphs::common::theme_to_chart_theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::line;
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, Column, HStack, Progress, ProgressSize,
    ProgressVariant, StackAlign, StackSpacing, Table, TableTheme, Text, TextSize, TextWeight,
    VStack,
};

impl PlayerView {
    pub(crate) fn render_room_eq_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        let progress = room_eq.overall_progress;
        let status_msg = room_eq.status_message.clone();
        let error_msg = room_eq.error_message.clone();
        let is_running = room_eq.is_optimizing();
        let is_completed = room_eq.is_optimization_complete();
        let is_failed =
            room_eq.optimization_status == crate::app::types::OptimizationStatus::Failed;
        let show_progress = is_running || is_completed || is_failed;
        let progress_history = room_eq.progress_history.clone();
        let current_channel = room_eq.current_channel.clone();
        let current_iteration = room_eq.current_iteration;
        let current_loss = room_eq.current_loss;

        // Build the actual RoomConfig that will be sent to the optimizer
        let room_config = room_eq.to_room_config();
        // Serialize only the optimizer config — RoomConfig itself can't serialize
        // because MeasurementSource::InMemory(Curve) has #[serde(skip)]
        let room_config_json =
            serde_json::to_string_pretty(&room_config.optimizer).unwrap_or_default();

        // Extract optimizer config for parameter summary
        let opt_config = &room_eq.optimizer_config;
        let channel_names: Vec<String> = room_eq
            .channel_measurements
            .iter()
            .map(|m| m.channel_name.clone())
            .collect();
        let param_mode = opt_config.mode.to_code().to_string();
        let param_algorithm = opt_config.algorithm.clone();
        let param_num_filters = opt_config.num_filters;
        let param_min_q = opt_config.min_q;
        let param_max_q = opt_config.max_q;
        let param_min_db = opt_config.min_db;
        let param_max_db = opt_config.max_db;
        let param_min_freq = opt_config.min_freq;
        let param_max_freq = opt_config.max_freq;
        let param_max_iter = opt_config.max_iter;
        let param_population = opt_config.population;
        let param_peq_model = opt_config.peq_model.clone();
        let param_refine = opt_config.refine;
        let param_local_algo = opt_config.local_algo.clone();
        let param_psychoacoustic = opt_config.psychoacoustic;
        let param_asymmetric_loss = opt_config.asymmetric_loss;
        let param_target_tilt = opt_config.target_tilt.enabled;
        let param_excursion = opt_config.excursion_protection.enabled;
        let param_schroeder = opt_config.schroeder_split.enabled;
        let param_phase_alignment = opt_config.phase_alignment.enabled;

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Run Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new("Run the optimization process for each channel.")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            // Optimization completed success card
            .when(is_completed, |div| {
                div.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.success)
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new("✓")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Sm)
                                                .color(theme.success),
                                        )
                                        .child(
                                            Text::new("Optimization Completed")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Sm)
                                                .color(theme.text_primary),
                                        ),
                                )
                                .child(
                                    Text::new(status_msg.clone())
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new("Click Next to review the results.")
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(theme.text_secondary),
                                ),
                        )
                        .into_any_element()
                        .into_any(),
                )
            })
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Optimization Progress")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .when_some(current_channel.clone(), |hstack, ch| {
                                hstack.child(
                                    Text::new(format!("Channel: {}", ch))
                                        .size(TextSize::Xs)
                                        .color(theme.accent),
                                )
                            }),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Button::new(
                                    "start_optimization",
                                    if is_running {
                                        "Optimizing..."
                                    } else {
                                        "Start Optimization"
                                    },
                                )
                                .variant(ButtonVariant::Primary)
                                .full_width(true)
                                .theme(theme.to_button_theme())
                                .disabled(is_running)
                                .build()
                                .when(!is_running, |btn| {
                                    btn.on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.start_room_eq_optimization(cx);
                                        }),
                                    )
                                }),
                            )
                            .when(show_progress, |vstack| {
                                let display_progress = if is_completed {
                                    100.0
                                } else if is_running {
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
                                                Text::new(if is_running {
                                                    format!(
                                                        "Iteration: {} | Loss: {:.4}",
                                                        current_iteration, current_loss
                                                    )
                                                } else {
                                                    format!("Progress: {:.0}%", display_progress)
                                                })
                                                .size(TextSize::Xs)
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
                                            .size(ProgressSize::Sm)
                                            .variant(if is_completed {
                                                ProgressVariant::Success
                                            } else if is_failed {
                                                ProgressVariant::Error
                                            } else {
                                                ProgressVariant::Default
                                            }),
                                    )
                                    .child(Text::new(status_msg.clone()).size(TextSize::Xs).color(
                                        if is_completed {
                                            theme.success
                                        } else if is_failed {
                                            theme.error
                                        } else {
                                            theme.text_secondary
                                        },
                                    ))
                            })
                            .when_some(error_msg, |vstack, err| {
                                vstack.child(Text::new(err).size(TextSize::Xs).color(theme.error))
                            }),
                    ),
            )
            // Parameter summary card
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Configuration Summary")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let label_color = theme.text_secondary;
                        let value_color = theme.text_primary;
                        let accent = theme.accent;

                        // Helper: build a label: value row
                        let row = |label: &str, value: String| {
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new(format!("{}:", label))
                                        .size(TextSize::Xs)
                                        .color(label_color),
                                )
                                .child(
                                    Text::new(value)
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(value_color),
                                )
                        };

                        let bool_badge = |label: &str, enabled: bool| {
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new(format!("{}:", label))
                                        .size(TextSize::Xs)
                                        .color(label_color),
                                )
                                .child(
                                    Text::new(if enabled { "ON" } else { "OFF" })
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(if enabled { accent } else { label_color }),
                                )
                        };

                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            // Channels
                            .child(row(
                                "Channels",
                                if channel_names.is_empty() {
                                    "None".to_string()
                                } else {
                                    format!(
                                        "{} ({})",
                                        channel_names.len(),
                                        channel_names.join(", ")
                                    )
                                },
                            ))
                            // Mode & Algorithm
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(row("Mode", param_mode))
                                    .child(row("Algorithm", param_algorithm))
                                    .child(row("PEQ Model", param_peq_model)),
                            )
                            // Filters & Iterations
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(row("Filters", param_num_filters.to_string()))
                                    .child(row("Max Iter", param_max_iter.to_string()))
                                    .child(row("Population", param_population.to_string())),
                            )
                            // Frequency range
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(row(
                                        "Freq Range",
                                        format!("{:.0} - {:.0} Hz", param_min_freq, param_max_freq),
                                    ))
                                    .child(row(
                                        "Q Range",
                                        format!("{:.1} - {:.1}", param_min_q, param_max_q),
                                    ))
                                    .child(row(
                                        "dB Range",
                                        format!("{:.1} - {:.1}", param_min_db, param_max_db),
                                    )),
                            )
                            // Toggles
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(bool_badge("Refine", param_refine))
                                    .when(param_refine, |h| {
                                        h.child(row("Local Algo", param_local_algo))
                                    })
                                    .child(bool_badge("Psychoacoustic", param_psychoacoustic))
                                    .child(bool_badge("Asymmetric Loss", param_asymmetric_loss)),
                            )
                            // Advanced features (only show enabled ones)
                            .when(
                                param_target_tilt
                                    || param_excursion
                                    || param_schroeder
                                    || param_phase_alignment,
                                |vstack| {
                                    vstack.child(
                                        HStack::new()
                                            .spacing(StackSpacing::Md)
                                            .when(param_target_tilt, |h| {
                                                h.child(bool_badge("Target Tilt", true))
                                            })
                                            .when(param_excursion, |h| {
                                                h.child(bool_badge("Excursion Protection", true))
                                            })
                                            .when(param_schroeder, |h| {
                                                h.child(bool_badge("Schroeder Split", true))
                                            })
                                            .when(param_phase_alignment, |h| {
                                                h.child(bool_badge("Phase Alignment", true))
                                            }),
                                    )
                                },
                            )
                    }),
            )
            // Full RoomConfig key-value table
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Full Parameters (Optimizer)")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let mut pairs: Vec<(String, String)> = Vec::new();
                        match serde_json::from_str::<serde_json::Value>(&room_config_json) {
                            Ok(json_val) => flatten_json(&json_val, String::new(), &mut pairs),
                            Err(e) => log::error!("Failed to parse optimizer config JSON: {}", e),
                        }

                        let label_color = theme.text_secondary;
                        let value_color = theme.text_primary;

                        let table_theme = TableTheme {
                            header_bg: theme.background_secondary,
                            header_text: theme.text_primary,
                            header_border: theme.border,
                            row_bg: theme.surface,
                            row_alt_bg: theme.background_secondary,
                            row_hover_bg: theme.surface_hover,
                            row_selected_bg: theme.accent_muted,
                            cell_text: theme.text_secondary,
                            cell_border: theme.border,
                            sort_icon_color: theme.accent,
                            pagination_text: theme.text_muted,
                            footer_bg: theme.background_secondary,
                            footer_text: theme.text_secondary,
                        };

                        div()
                            .id("room-config-params")
                            .overflow_y_scroll()
                            .max_h(px(400.0))
                            .child(
                                Table::new("optimizer-params-table", pairs)
                                    .column(
                                        Column::new("key", "Parameter")
                                            .width(px(250.0))
                                            .sortable(false)
                                            .resizable(false)
                                            .cell_render(
                                                move |pair: &(String, String), _, _, _| {
                                                    Text::new(pair.0.clone())
                                                        .size(TextSize::Xs)
                                                        .color(label_color)
                                                },
                                            ),
                                    )
                                    .column(
                                        Column::new("value", "Value")
                                            .sortable(false)
                                            .resizable(false)
                                            .cell_render(
                                                move |pair: &(String, String), _, _, _| {
                                                    Text::new(pair.1.clone())
                                                        .size(TextSize::Xs)
                                                        .weight(TextWeight::Semibold)
                                                        .color(value_color)
                                                },
                                            ),
                                    )
                                    .alternating_rows(true)
                                    .theme(table_theme),
                            )
                    }),
            )
            // Optimization Process graph (shown when progress history is available)
            .when(!progress_history.is_empty(), |vstack| {
                // Initialize interactive chart state if needed
                {
                    let state = self.state.read(cx);
                    if state
                        .app
                        .measurement_state
                        .room_eq_state
                        .progress_chart_state
                        .is_none()
                    {
                        let _ = state;
                        self.state.update(cx, |state, _| {
                            // X: iteration range (0 to max), Y: loss range (auto-scale)
                            // We use linear scale for iteration, and auto-fit y based on loss values
                            let max_iter = state
                                .app
                                .measurement_state
                                .room_eq_state
                                .optimizer_config
                                .max_iter as f64;
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .progress_chart_state = Some(
                                InteractiveChartStateWrapper::new(
                                    0.0,
                                    max_iter.max(100.0),
                                    0.0,
                                    1.0,
                                )
                                .with_log_x(false)
                                .with_size(700.0, 250.0),
                            );
                        });
                    }
                }

                let state = self.state.read(cx);
                let room_eq = &state.app.measurement_state.room_eq_state;
                let theme = state.app.ui_state.theme.clone();
                let history = room_eq.progress_history.clone();
                let chart_state = room_eq.progress_chart_state.as_ref().map(|w| w.inner());
                let chart_theme = theme_to_chart_theme(&theme);

                // Filter out status messages (loss <= 0.0 or non-finite)
                // Also skip completion messages (iter==0 after real data — draws line back to 0)
                // Group by channel name, preserving insertion order
                let mut channel_order: Vec<String> = Vec::new();
                let mut channel_data: std::collections::HashMap<String, (Vec<f64>, Vec<f64>)> =
                    std::collections::HashMap::new();
                let mut all_losses: Vec<f64> = Vec::new();

                for (iter, loss, speaker) in &history {
                    if !loss.is_finite() || *loss <= 0.0 {
                        continue;
                    }
                    // Skip completion/status messages that would draw a line back to x=0
                    // These have iter==0 after the channel already has progress data
                    if *iter == 0 && channel_data.contains_key(speaker) {
                        continue;
                    }
                    all_losses.push(*loss);
                    if !channel_data.contains_key(speaker) {
                        channel_order.push(speaker.clone());
                        channel_data.insert(speaker.clone(), (Vec::new(), Vec::new()));
                    }
                    let (iters, losses) = channel_data.get_mut(speaker).unwrap();
                    iters.push(*iter as f64);
                    losses.push(*loss);
                }
                let current_loss_val = all_losses.last().copied().unwrap_or(0.0);
                let best_loss = all_losses.iter().copied().fold(f64::INFINITY, f64::min);

                // Calculate Y range from all channel data
                let (loss_min, loss_max) = all_losses
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &v| {
                        (min.min(v), max.max(v))
                    });
                let y_min = if loss_min.is_finite() {
                    (loss_min * 0.95).max(0.0)
                } else {
                    0.0
                };
                let y_max = if loss_max.is_finite() {
                    loss_max * 1.05
                } else {
                    1.0
                };

                // X range: each channel restarts at 0
                let x_max_data = channel_data
                    .values()
                    .map(|(iters, _)| iters.last().copied().unwrap_or(0.0))
                    .fold(0.0_f64, f64::max)
                    .max(100.0);
                let (x_min, x_max) = chart_state
                    .filter(|s| s.is_zoomed())
                    .map(|s| s.x_domain())
                    .unwrap_or((0.0, x_max_data));
                let (y_min_domain, y_max_domain) = chart_state
                    .filter(|s| s.is_zoomed())
                    .map(|s| s.y_domain())
                    .unwrap_or((y_min, y_max));

                // Guard: ensure axis ranges are finite and non-degenerate
                let x_max = if x_max.is_finite() && x_max > x_min {
                    x_max
                } else {
                    x_min + 100.0
                };
                let y_min_domain = if y_min_domain.is_finite() {
                    y_min_domain
                } else {
                    0.0
                };
                let y_max_domain = if y_max_domain.is_finite() && y_max_domain > y_min_domain {
                    y_max_domain
                } else {
                    y_min_domain + 1.0
                };

                // Per-channel colors (cycle through a palette)
                let channel_colors: &[u32] = &[
                    0x1f77b4, // blue
                    0xff7f0e, // orange
                    0x2ca02c, // green
                    0xd62728, // red
                    0x9467bd, // purple
                    0x8c564b, // brown
                    0xe377c2, // pink
                    0x7f7f7f, // gray
                ];

                // Build chart: first channel is the primary series, rest are added
                let chart = if let Some(first_ch) = channel_order.first() {
                    let (iters, losses) = &channel_data[first_ch];
                    let mut builder = line(iters, losses)
                        .title("Optimization Process")
                        .x_label("Iterations")
                        .y_label("Loss")
                        .label(format!("Loss {}", first_ch))
                        .x_range(x_min, x_max)
                        .y_range(y_min_domain, y_max_domain)
                        .color(channel_colors[0])
                        .stroke_width(2.0)
                        .theme(chart_theme)
                        .size(700.0, 250.0);

                    for (idx, ch_name) in channel_order.iter().enumerate().skip(1) {
                        let (ch_iters, ch_losses) = &channel_data[ch_name];
                        let color = channel_colors[idx % channel_colors.len()];
                        // Each channel has its own X (iteration) values, so use
                        // add_series_with_x to avoid misaligning Y against the
                        // primary series X.
                        builder = builder.add_series_with_x(
                            ch_iters,
                            ch_losses,
                            Some(&format!("Loss {}", ch_name)),
                            color,
                            2.0,
                            1.0,
                        );
                    }
                    builder.build()
                } else {
                    // No data yet — build an empty chart
                    line(&[0.0], &[0.0])
                        .title("Optimization Process")
                        .x_label("Iterations")
                        .y_label("Loss")
                        .label("Loss")
                        .x_range(0.0, 100.0)
                        .y_range(0.0, 1.0)
                        .color(channel_colors[0])
                        .stroke_width(2.0)
                        .theme(chart_theme)
                        .size(700.0, 250.0)
                        .build()
                };

                // Build the chart element, wrapping with interactive if state is available
                let chart_element: Option<gpui::AnyElement> = chart.ok().map(|c| {
                    if let Some(state) = chart_state {
                        gpui_px::interaction::interactive(
                            "room-eq-progress-chart",
                            c,
                            state.clone(),
                        )
                        .build()
                        .into_any_element()
                    } else {
                        c.into_any_element()
                    }
                });

                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new("Optimization Process")
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .child(
                                    Text::new(format!("Current: {:.4}", current_loss_val))
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new(format!("Best: {:.4}", best_loss))
                                        .size(TextSize::Xs)
                                        .color(theme.success),
                                ),
                        )
                        .content(
                            div()
                                .w(px(700.0))
                                .flex()
                                .flex_col()
                                .when_some(chart_element, |el, c| el.child(c)),
                        ),
                )
            })
    }

    fn start_room_eq_optimization(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{ChannelOptResult, EqFilterConfig, OptimizationStatus};
        use autoeq::roomeq::CallbackAction;
        use sotf_audio_player::autoeq::{
            RoomOptimizationCallback, RoomOptimizationProgress, run_room_optimization,
        };

        log::info!("Starting room EQ optimization using roomeq");

        // Build RoomConfig from state using the unified helper
        let (room_config, channel_names, max_iter) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.measurement_state.room_eq_state;
            let cfg = &room_eq.optimizer_config;
            log::info!(
                "Room EQ optimizer config: algo={}, filters={}, freq=[{:.1}, {:.1}], \
                 db=[{:.1}, {:.1}], q=[{:.2}, {:.2}], pop={}, maxiter={}, \
                 tol={:.2e}, atol={:.2e}, refine={}, psychoacoustic={}, asymmetric={}",
                cfg.algorithm,
                cfg.num_filters,
                cfg.min_freq,
                cfg.max_freq,
                cfg.min_db,
                cfg.max_db,
                cfg.min_q,
                cfg.max_q,
                cfg.population,
                cfg.max_iter,
                cfg.tolerance,
                cfg.atolerance,
                cfg.refine,
                cfg.psychoacoustic,
                cfg.asymmetric_loss,
            );

            let channel_names: Vec<String> = room_eq
                .channel_measurements
                .iter()
                .map(|m| m.channel_name.clone())
                .collect();

            (
                room_eq.to_room_config(),
                channel_names,
                room_eq.optimizer_config.max_iter,
            )
        };

        if channel_names.is_empty() {
            log::warn!("No channels to optimize");
            self.state.update(cx, |state, _cx| {
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .optimization_status = OptimizationStatus::Failed;
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No channels to optimize".to_string());
            });
            return;
        }

        // Update state to running and clear progress history
        self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .room_eq_state
                .optimization_status = OptimizationStatus::Running;
            state.app.measurement_state.room_eq_state.status_message =
                "Starting optimization...".to_string();
            state
                .app
                .measurement_state
                .room_eq_state
                .channel_results
                .clear();
            state.app.measurement_state.room_eq_state.overall_progress = 0.0;
            state
                .app
                .measurement_state
                .room_eq_state
                .progress_history
                .clear();
            state.app.measurement_state.room_eq_state.current_iteration = 0;
            state.app.measurement_state.room_eq_state.current_loss = 0.0;

            // Initialize progress chart state immediately
            state
                .app
                .measurement_state
                .room_eq_state
                .progress_chart_state = Some(
                InteractiveChartStateWrapper::new(0.0, max_iter.max(100) as f64, 0.0, 1.0)
                    .with_log_x(false)
                    .with_size(700.0, 250.0),
            );
        });

        let state_clone = self.state.clone();

        // Create async channel for progress updates from blocking thread
        let (progress_tx, progress_rx) =
            smol::channel::bounded::<(usize, f64, f32, String)>(100);

        // Clone state for progress receiver task
        let state_for_progress = self.state.clone();

        // Spawn a task to receive progress updates and update UI
        cx.spawn({
            async move |_, cx| {
                while let Ok((iteration, loss, overall_progress, speaker)) =
                    progress_rx.recv().await
                {
                    state_for_progress.update(&mut cx.clone(), |state, cx| {
                        state.app.measurement_state.room_eq_state.current_iteration = iteration;
                        state.app.measurement_state.room_eq_state.current_loss = loss;
                        state.app.measurement_state.room_eq_state.overall_progress =
                            overall_progress;

                        // Add to progress history with channel name.
                        // Each channel's iterations restart from 0 on the X axis.
                        let history = &mut state
                            .app
                            .measurement_state
                            .room_eq_state
                            .progress_history;
                        if history.len() < 10000 {
                            history.push((iteration, loss, speaker));
                        }
                        cx.notify();
                    });
                }
                log::info!("Progress receiver loop finished");
            }
        })
        .detach();

        // Spawn the optimization task
        cx.spawn(async move |_, cx| {
            // Update status
            state_clone.update(&mut cx.clone(), |state, cx| {
                state.app.measurement_state.room_eq_state.status_message =
                    "Optimizing all channels (parallel)...".to_string();
                cx.notify();
            });

            // Create progress callback
            let progress_tx_clone = progress_tx.clone();
            let callback: RoomOptimizationCallback =
                Box::new(move |progress: &RoomOptimizationProgress| {
                    let iteration = progress.iteration;
                    let loss = progress.loss;
                    let max_iterations = progress.max_iterations;
                    let speaker = progress.current_speaker.clone();

                    let overall = if max_iterations > 0 {
                        iteration as f32 / max_iterations as f32
                    } else {
                        0.0
                    };

                    // Send progress update (non-blocking)
                    let _ = progress_tx_clone.try_send((iteration, loss, overall, speaker));
                    CallbackAction::Continue
                });

            // Run room optimization (parallel via rayon internally)
            let result =
                smol::unblock(move || run_room_optimization(&room_config, 48000.0, Some(callback)))
                    .await;

            // Drop progress sender to close channel and stop receiver
            drop(progress_tx);

            match result {
                Ok(room_result) => {
                    log::info!(
                        "Room optimization completed: {:.4} -> {:.4}",
                        room_result.combined_pre_score,
                        room_result.combined_post_score
                    );

                    // Build UI results from RoomOptimizationResult
                    let all_results: Vec<ChannelOptResult> = channel_names
                        .iter()
                        .filter_map(|name| {
                            room_result.channel_results.get(name).map(|channel_res| {
                                ChannelOptResult {
                                    channel_name: name.clone(),
                                    pre_score: channel_res.pre_score,
                                    post_score: channel_res.post_score,
                                    eq_filters: channel_res
                                        .biquads
                                        .iter()
                                        .map(|b| EqFilterConfig {
                                            filter_type: format!("{:?}", b.filter_type),
                                            frequency: b.freq,
                                            q: b.q,
                                            gain_db: b.db_gain,
                                        })
                                        .collect(),
                                    crossover_freqs: None,
                                    driver_gains: None,
                                    original_response: Some(
                                        channel_res
                                            .initial_curve
                                            .freq
                                            .iter()
                                            .zip(channel_res.initial_curve.spl.iter())
                                            .map(|(&f, &db)| (f, db))
                                            .collect(),
                                    ),
                                    corrected_response: Some(
                                        channel_res
                                            .final_curve
                                            .freq
                                            .iter()
                                            .zip(channel_res.final_curve.spl.iter())
                                            .map(|(&f, &db)| (f, db))
                                            .collect(),
                                    ),
                                    normalized_response: Some(
                                        channel_res
                                            .final_curve
                                            .freq
                                            .iter()
                                            .zip(channel_res.final_curve.spl.iter())
                                            .map(|(&f, &db)| (f, db))
                                            .collect(),
                                    ),
                                    target_curve: room_result
                                        .channels
                                        .get(name)
                                        .and_then(|chain| chain.target_curve.as_ref())
                                        .map(|tc| {
                                            tc.freq
                                                .iter()
                                                .zip(tc.spl.iter())
                                                .map(|(&f, &db)| (f, db))
                                                .collect()
                                        }),
                                    group_delay_before: compute_group_delay_from_curve(
                                        &channel_res.initial_curve,
                                    ),
                                    group_delay_after: compute_group_delay_from_curve(
                                        &channel_res.final_curve,
                                    ),
                                }
                            })
                        })
                        .collect();

                    let avg_pre = room_result.combined_pre_score;
                    let avg_post = room_result.combined_post_score;

                    // Update final state
                    state_clone.update(&mut cx.clone(), |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimization_status = OptimizationStatus::Completed;
                        state.app.measurement_state.room_eq_state.status_message = format!(
                            "Optimization complete! Score: {:.2} -> {:.2}",
                            avg_pre, avg_post
                        );
                        state.app.measurement_state.room_eq_state.channel_results = all_results;
                        state.app.measurement_state.room_eq_state.overall_progress = 1.0;
                        state.app.measurement_state.room_eq_state.current_channel = None;

                        // Build DSP output
                        let mut dsp_channels = std::collections::HashMap::new();
                        for result in &state.app.measurement_state.room_eq_state.channel_results {
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

                        state.app.measurement_state.room_eq_state.dsp_output =
                            Some(crate::app::types::DspChainOutput {
                                channels: dsp_channels,
                                metadata: Some(crate::app::types::DspChainMetadata {
                                    pre_score: avg_pre,
                                    post_score: avg_post,
                                    algorithm: state
                                        .app
                                        .measurement_state
                                        .room_eq_state
                                        .optimizer_config
                                        .algorithm
                                        .as_str()
                                        .to_string(),
                                    iterations: max_iter,
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                }),
                            });

                        state.app.measurement_state.room_eq_state.step =
                            crate::app::types::RoomEqStep::Review;
                        cx.notify();
                    });
                }
                Err(e) => {
                    log::error!("Room optimization failed: {}", e);
                    state_clone.update(&mut cx.clone(), |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimization_status = OptimizationStatus::Failed;
                        state.app.measurement_state.room_eq_state.error_message =
                            Some(format!("Room optimization error: {}", e));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

/// Recursively flatten a JSON value into dotted key-value pairs.
/// Skips large arrays (e.g. measurement data) — only includes scalars and small objects.
fn flatten_json(value: &serde_json::Value, prefix: String, pairs: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(v, key, pairs);
            }
        }
        serde_json::Value::Array(arr) => {
            // Skip large arrays (measurement data, etc.)
            if arr.len() <= 8 {
                for (i, v) in arr.iter().enumerate() {
                    let key = format!("{}[{}]", prefix, i);
                    flatten_json(v, key, pairs);
                }
            }
        }
        serde_json::Value::String(s) => {
            pairs.push((prefix, s.clone()));
        }
        serde_json::Value::Number(n) => {
            pairs.push((prefix, n.to_string()));
        }
        serde_json::Value::Bool(b) => {
            pairs.push((prefix, b.to_string()));
        }
        serde_json::Value::Null => {
            pairs.push((prefix, "null".to_string()));
        }
    }
}

/// Compute group delay from a Curve's phase data.
/// Returns None if no phase data is available.
fn compute_group_delay_from_curve(curve: &autoeq::Curve) -> Option<Vec<(f64, f64)>> {
    let phase = curve.phase.as_ref()?;
    let unwrapped = autoeq::loss::phase_aware::unwrap_phase_degrees(phase);
    let gd = autoeq::loss::phase_aware::compute_group_delay(&curve.freq, &unwrapped);
    Some(
        curve
            .freq
            .iter()
            .zip(gd.iter())
            .map(|(&f, &d)| (f, d))
            .collect(),
    )
}
