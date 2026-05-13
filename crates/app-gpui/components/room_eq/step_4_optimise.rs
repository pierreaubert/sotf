use crate::app::types::room_eq::InteractiveChartStateWrapper;
use crate::components::design::Ds;
use crate::components::graphs::common::theme_to_chart_theme;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{StrokeDashArray, line};
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, Column, HStack, Progress, ProgressSize,
    ProgressVariant, Spinner, SpinnerSize, StackAlign, StackSpacing, Table, TableTheme, Text,
    TextSize, TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_room_eq_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        let progress = room_eq.overall_progress;
        let status_msg = room_eq.status_message.clone();
        let error_msg = room_eq.error_message.clone();
        let is_running = room_eq.is_optimizing();
        let is_completed = room_eq.is_optimization_complete();
        let is_failed =
            room_eq.optimization_status == crate::app::types::OptimizationStatus::Failed;
        // Warn the user when a prior Delay Detection run failed so they
        // know the optimizer is about to silently fall back to WAV-onset
        // detection. Surfacing this here (rather than only on Step 2)
        // prevents the user from scratching their head when alignment
        // delays look worse than expected.
        let dd_failed_reason = match &room_eq.delay_detection.status {
            crate::app::types::room_eq::DelayDetectionStatus::Failed(msg) => Some(msg.clone()),
            _ => None,
        };
        let show_progress = is_running || is_completed || is_failed;
        let progress_history = room_eq.progress_history.clone();
        let current_channel = room_eq.current_channel.clone();
        let current_iteration = room_eq.current_iteration;
        let current_loss = room_eq.current_loss;
        // Pipeline step strip — stable left-to-right render order,
        // colored by the latest status the optimizer has reported for
        // each step.
        let current_step = room_eq.current_step;
        let step_history = room_eq.step_history.clone();

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
        let param_bo_initial_samples = opt_config.bo_initial_samples;
        let param_bo_batch_size = opt_config.bo_batch_size;
        let param_bo_acquisition = opt_config.bo_acquisition.clone();
        let param_bo_ehvi = opt_config.bo_ehvi;
        let param_peq_model = opt_config.peq_model.clone();
        let param_refine = opt_config.refine;
        let param_local_algo = opt_config.local_algo.clone();
        let param_psychoacoustic = opt_config.psychoacoustic;
        let param_asymmetric_loss = opt_config.asymmetric_loss;
        let param_target_tilt = opt_config.target_response.enabled;
        let param_excursion = opt_config.excursion_protection.enabled;
        let param_schroeder = opt_config.schroeder_split.enabled;
        let param_phase_alignment = opt_config.phase_alignment.enabled;

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.roomeq_run_optimization)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.roomeq_run_optimization_desc)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            // Delay-detection failure banner: the optimizer silently
            // falls back to WAV-onset detection when the probe didn't
            // complete, so we surface the reason up-front.
            .when_some(dd_failed_reason, |div, reason| {
                div.child(
                    Card::new()
                        .background(theme.surface)
                        .border(theme.warning)
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new(translations.roomeq_delay_detection_incomplete)
                                        .weight(TextWeight::Bold)
                                        .size(TextSize::Sm)
                                        .color(theme.warning),
                                )
                                .child(
                                    Text::new(format!(
                                        "{} — the optimizer will use WAV-onset \
                                         detection for per-channel arrival times.",
                                        reason
                                    ))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                                ),
                        ),
                )
            })
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
                                            Icon::new(IconName::Check)
                                                .size(IconSize::Sm)
                                                .color(theme.success),
                                        )
                                        .child(
                                            Text::new(translations.roomeq_optimization_completed)
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
                                    Text::label(translations.roomeq_click_next_to_review)
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
                                Text::new(translations.roomeq_optimization_progress)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .when_some(current_channel.clone(), |hstack, ch| {
                                hstack.child(
                                    Text::new(format!("{}: {}", translations.roomeq_channel, ch))
                                        .size(TextSize::Xs)
                                        .color(theme.accent),
                                )
                            }),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(if is_running {
                                Button::new("cancel_optimization", translations.general_cancel)
                                    .variant(ButtonVariant::Secondary)
                                    .full_width(true)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(cx.listener(|view, _, _, cx| {
                                        view.cancel_room_eq_optimization(cx);
                                    }))
                            } else {
                                Button::new(
                                    "start_optimization",
                                    translations.roomeq_start_optimization,
                                )
                                .variant(ButtonVariant::Primary)
                                .full_width(true)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.start_room_eq_optimization(cx);
                                }))
                            })
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
                                            .gap(d.gap)
                                            .when(is_running, |el| {
                                                el.child(Spinner::new().size(SpinnerSize::Sm))
                                            })
                                            .child(
                                                Text::new(if is_running {
                                                    if current_iteration == 0
                                                        && !status_msg.is_empty()
                                                        && current_loss == 0.0
                                                    {
                                                        status_msg.clone()
                                                    } else {
                                                        format!(
                                                            "Iteration: {} | Loss: {:.4}",
                                                            current_iteration, current_loss
                                                        )
                                                    }
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
                                    .child(render_pipeline_step_strip(
                                        &theme,
                                        &d,
                                        current_step,
                                        &step_history,
                                        is_completed,
                                        is_failed,
                                    ))
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
                        Text::new(translations.roomeq_configuration_summary)
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let label_color = theme.text_secondary;
                        let value_color = theme.text_primary;
                        let accent = theme.accent;

                        // Inline compact pair: "label: value" on one line
                        let pair = |label: &str, value: String| {
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new(format!("{}:", label))
                                        .size(TextSize::Xs)
                                        .color(label_color),
                                )
                                .child(Text::label(value).color(value_color))
                        };

                        // Show only enabled toggles as accent-tinted chips —
                        // disabled flags add noise, not signal.
                        let chip = |label: &'static str| Text::label(label).color(accent);

                        // Collapse channel list to a single compact line:
                        // "3: FL, FR, C" rather than "3 (FL, FR, C)".
                        let channels_text = if channel_names.is_empty() {
                            "None".to_string()
                        } else {
                            format!("{}: {}", channel_names.len(), channel_names.join(", "))
                        };

                        let refine_text = if param_refine {
                            format!("ON ({})", param_local_algo)
                        } else {
                            "OFF".to_string()
                        };

                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            // Row 1: algorithm identity + channels.
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(pair("Channels", channels_text))
                                    .child(pair("Mode", param_mode))
                                    .child(pair("Algo", param_algorithm.clone()))
                                    .child(pair("Model", param_peq_model)),
                            )
                            // Row 2: numeric parameters packed together.
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(pair("Filters", param_num_filters.to_string()))
                                    .child(pair("Iter", param_max_iter.to_string()))
                                    .child(pair("Pop", param_population.to_string()))
                                    .child(pair(
                                        "Freq",
                                        format!("{:.0}-{:.0} Hz", param_min_freq, param_max_freq),
                                    ))
                                    .child(pair(
                                        "Q",
                                        format!("{:.1}-{:.1}", param_min_q, param_max_q),
                                    ))
                                    .child(pair(
                                        "dB",
                                        format!("{:.1}/{:.1}", param_min_db, param_max_db),
                                    )),
                            )
                            // Row 3: feature flags — only show the enabled
                            // ones (plus Refine which carries local algo).
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(pair("Refine", refine_text))
                                    .when(param_algorithm == "autoeq:bo", |h| {
                                        h.child(pair(
                                            "BO",
                                            format!(
                                                "init {} / batch {} / {}",
                                                param_bo_initial_samples,
                                                param_bo_batch_size,
                                                param_bo_acquisition
                                            ),
                                        ))
                                    })
                                    .when(param_algorithm == "autoeq:bo" && param_bo_ehvi, |h| {
                                        h.child(chip("qEHVI"))
                                    })
                                    .when(param_psychoacoustic, |h| h.child(chip("Psychoacoustic")))
                                    .when(param_asymmetric_loss, |h| {
                                        h.child(chip("Asymmetric Loss"))
                                    })
                                    .when(param_target_tilt, |h| h.child(chip("Target Response")))
                                    .when(param_excursion, |h| {
                                        h.child(chip("Excursion Protection"))
                                    })
                                    .when(param_schroeder, |h| h.child(chip("Schroeder Split")))
                                    .when(param_phase_alignment, |h| {
                                        h.child(chip("Phase Alignment"))
                                    }),
                            )
                    }),
            )
            // Optimization Process graph — placed between Configuration
            // Summary and Full Parameters so the user sees live progress
            // without scrolling past the JSON dump.
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
                let translations = state.app.ui_state.translations.clone();
                let room_eq = &state.app.measurement_state.room_eq_state;
                let theme = state.app.ui_state.theme.clone();
                let history = room_eq.progress_history.clone();
                let chart_state = room_eq.progress_chart_state.as_ref().map(|w| w.inner());
                let chart_theme = theme_to_chart_theme(&theme);

                let (channel_order, progress_series, all_losses) =
                    room_eq_progress_chart_series(&history);
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

                // X range: each optimizer pass uses its own iteration counter.
                let x_max_data = progress_series
                    .iter()
                    .flat_map(|series| series.iterations.iter().copied())
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

                // Build chart: each channel pass is its own series so a
                // backend iteration reset does not draw a line backwards.
                let chart = if let Some(first_series) = progress_series
                    .iter()
                    .find(|series| !series.iterations.is_empty())
                {
                    let (iters, losses) =
                        downsample_xy(&first_series.iterations, &first_series.losses, 80);
                    let first_color_idx = channel_order
                        .iter()
                        .position(|ch| ch == &first_series.channel)
                        .unwrap_or(0);
                    let mut builder = line(&iters, &losses)
                        .title("Optimization Process")
                        .x_label("Iterations")
                        .y_label("Loss")
                        .label(first_series.loss_label())
                        .x_range(x_min, x_max)
                        .y_range(y_min_domain, y_max_domain)
                        .color(channel_colors[first_color_idx % channel_colors.len()])
                        .stroke_width(2.0)
                        .theme(chart_theme)
                        .size(700.0, 250.0);

                    for series in progress_series
                        .iter()
                        .filter(|series| !series.iterations.is_empty())
                        .skip(1)
                    {
                        let (ch_iters, ch_losses) =
                            downsample_xy(&series.iterations, &series.losses, 80);
                        let color_idx = channel_order
                            .iter()
                            .position(|ch| ch == &series.channel)
                            .unwrap_or(0);
                        let color = channel_colors[color_idx % channel_colors.len()];
                        builder = builder.add_series_with_x(
                            &ch_iters,
                            &ch_losses,
                            Some(&series.loss_label()),
                            color,
                            2.0,
                            1.0,
                        );
                    }

                    // Add EPA preference on secondary (right) Y-axis.
                    // Same color as the channel's loss series, thinner stroke,
                    // dashed so it's visually distinct from the loss trace.
                    //
                    // Auto-scale Y2 from the actual EPA values. The DE
                    // optimizer only re-evaluates EPA when it finds an
                    // improved candidate, so the raw values sit in a narrow
                    // band (typically ~3-6). A fixed [0, 10] range would
                    // flatten these out and look like a constant line.
                    let has_epa = progress_series
                        .iter()
                        .any(|series| !series.epa_iterations.is_empty());
                    if has_epa {
                        let (epa_min, epa_max) = progress_series
                            .iter()
                            .flat_map(|series| series.epa_values.iter().copied())
                            .filter(|v| v.is_finite())
                            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
                                (min.min(v), max.max(v))
                            });
                        // Pad by 10% of the span (or 0.5 if degenerate)
                        // so extremes don't hug the chart edge.
                        let (y2_lo, y2_hi) = if epa_min.is_finite() && epa_max.is_finite() {
                            let span = (epa_max - epa_min).max(0.5);
                            let pad = span * 0.1;
                            (epa_min - pad, epa_max + pad)
                        } else {
                            (0.0, 10.0)
                        };
                        builder = builder.y2_label("EPA Preference").y2_range(y2_lo, y2_hi);
                        for series in progress_series
                            .iter()
                            .filter(|series| !series.epa_iterations.is_empty())
                        {
                            let (ep_iters, ep_vals) =
                                downsample_xy(&series.epa_iterations, &series.epa_values, 80);
                            let color_idx = channel_order
                                .iter()
                                .position(|ch| ch == &series.channel)
                                .unwrap_or(0);
                            let color = channel_colors[color_idx % channel_colors.len()];
                            builder = builder
                                .add_series_y2_with_x(
                                    &ep_iters,
                                    &ep_vals,
                                    Some(&series.epa_label()),
                                    color,
                                    1.0,
                                    0.6,
                                )
                                .series_dash_array(StrokeDashArray::Dashed);
                        }
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
                                    Text::new(translations.roomeq_optimization_process)
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
                                .w(px(700.0)) // intentional: fixed chart container width
                                .flex()
                                .flex_col()
                                .when_some(chart_element, |el, c| el.child(c)),
                        ),
                )
            })
            // Full RoomConfig key-value table (expandable, below the
            // graph so it doesn't push live progress off-screen)
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new(translations.roomeq_full_parameters)
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let mut pairs: Vec<(String, String)> = Vec::new();
                        match serde_json::from_str::<serde_json::Value>(&room_config_json) {
                            Ok(json_val) => flatten_json(&json_val, String::new(), &mut pairs),
                            Err(e) => log::error!("Failed to parse optimizer config JSON: {}", e),
                        }
                        let total_pairs = pairs.len();
                        pairs.truncate(64);
                        if total_pairs > pairs.len() {
                            pairs.push((
                                "...".to_string(),
                                format!(
                                    "{} additional parameters hidden",
                                    total_pairs - pairs.len()
                                ),
                            ));
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
                            ..Default::default()
                        };

                        div()
                            .id("room-config-params")
                            .overflow_y_scroll()
                            .max_h(px(400.0)) // intentional: fixed params table max height
                            .child(
                                Table::new("optimizer-params-table", pairs)
                                    .column(
                                        Column::new("key", "Parameter")
                                            .width(px(380.0)) // intentional: fixed table column width
                                            .sortable(false)
                                            .resizable(true)
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
                                                    Text::label(pair.1.clone()).color(value_color)
                                                },
                                            ),
                                    )
                                    .alternating_rows(true)
                                    .theme(table_theme),
                            )
                    }),
            )
    }

    fn start_room_eq_optimization(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{ChannelOptResult, EqFilterConfig, OptimizationStatus};
        use autoeq::roomeq::CallbackAction;
        use sotf_audio_player::autoeq::{
            PipelineStepId, PipelineStepStatus, RoomOptimizationCallback, RoomOptimizationProgress,
            run_room_optimization, run_room_optimization_with_probe_arrivals,
        };
        use sotf_audio_player::room_eq_types::RoomEqWizardMode;

        log::info!("Starting room EQ optimization using roomeq");

        // When the user went through the Simple Wizard, apply their
        // preset choices to the optimizer config BEFORE building the
        // RoomConfig. This must happen via state.update (mutating) so
        // subsequent read sees the updated values.
        {
            let wizard_mode = self
                .state
                .read(cx)
                .app
                .measurement_state
                .room_eq_state
                .wizard_mode;
            if wizard_mode == RoomEqWizardMode::Simple {
                self.state.update(cx, |state, _| {
                    let room_eq = &mut state.app.measurement_state.room_eq_state;
                    let preset = room_eq.simple_preset.clone();
                    sotf_audio_player::room_eq_types::apply_simple_preset(
                        &preset,
                        &mut room_eq.optimizer_config,
                    );
                });
            }
        }

        // Build RoomConfig from state using the unified helper. Also read
        // any probe-based per-channel arrival times that the Delay Detection
        // step measured so we can feed them into the optimizer instead of
        // letting it fall back to WAV-onset detection.
        let (room_config, channel_names, max_iter, probe_arrivals) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.measurement_state.room_eq_state;
            let cfg = &room_eq.optimizer_config;
            log::info!(
                "Room EQ optimizer config: algo={}, filters={}, freq=[{:.1}, {:.1}], \
                 db=[{:.1}, {:.1}], q=[{:.2}, {:.2}], pop={}, maxiter={}, \
                 tol={:.2e}, atol={:.2e}, refine={}, psychoacoustic={}, asymmetric={}, \
                 bo_initial={}, bo_batch={}, bo_std_stop={:.3}, bo_acquisition={}, bo_ehvi={}",
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
                cfg.bo_initial_samples,
                cfg.bo_batch_size,
                cfg.bo_posterior_std_threshold,
                cfg.bo_acquisition,
                cfg.bo_ehvi,
            );

            let channel_names: Vec<String> = room_eq
                .channel_measurements
                .iter()
                .map(|m| m.channel_name.clone())
                .collect();

            // Diagnostic: surface input-side state used to build the RoomConfig.
            // Multi-speaker regression hunt — log the UI's declared channel
            // list vs the speaker_configs / channel_measurements that
            // `to_room_config` iterates over. A mismatch here means the
            // RoomConfig will silently carry fewer speakers than the UI
            // thinks it asked for.
            let speaker_config_names: Vec<String> = room_eq
                .speaker_configs
                .iter()
                .map(|sc| sc.channel_name.clone())
                .collect();
            log::info!(
                "Room EQ pre-build: channel_measurements={}, speaker_configs={}, \
                 channel_names={:?}, speaker_config_names={:?}",
                room_eq.channel_measurements.len(),
                room_eq.speaker_configs.len(),
                channel_names,
                speaker_config_names,
            );

            let built_config = room_eq.to_room_config();
            log::info!(
                "Room EQ built RoomConfig: speakers.len()={}, speaker keys={:?}, system={:?}",
                built_config.speakers.len(),
                built_config.speakers.keys().collect::<Vec<_>>(),
                built_config.system.as_ref().map(|_| "Some(System)"),
            );

            (
                built_config,
                channel_names,
                room_eq.optimizer_config.max_iter,
                room_eq.delay_detection.probe_arrival_map(),
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
        let cancel_flag = self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .room_eq_state
                .cancel_requested
                .store(false, std::sync::atomic::Ordering::Relaxed);
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
            state.app.measurement_state.room_eq_state.current_channel = None;
            state.app.measurement_state.room_eq_state.error_message = None;
            state.app.measurement_state.room_eq_state.current_step = None;
            state
                .app
                .measurement_state
                .room_eq_state
                .step_history
                .clear();

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

            state
                .app
                .measurement_state
                .room_eq_state
                .cancel_requested
                .clone()
        });

        let state_clone = self.state.clone();

        // Create async channel for progress updates from blocking thread.
        // Tuple: (iteration, loss, overall, speaker, message, epa,
        //         step_id, step_status).
        type ProgressMsg = (
            usize,
            f64,
            f32,
            String,
            Option<String>,
            Option<f64>,
            Option<PipelineStepId>,
            Option<PipelineStepStatus>,
        );
        let (progress_tx, progress_rx) = smol::channel::bounded::<ProgressMsg>(100);

        // Clone state for progress receiver task
        let state_for_progress = self.state.clone();

        // Spawn a task to receive progress updates and update UI.
        //
        // The optimizer can fire thousands of callbacks per second. If we
        // call state.update() + cx.notify() for every single one, the GPUI
        // event loop spends all its time re-rendering and never polls the
        // smol::unblock future that drives the optimization itself — making
        // the second run appear stuck after the first speaker finishes.
        //
        // Fix: drain all pending messages on each wakeup and coalesce them
        // into a single state update + notify. A small sleep between
        // iterations caps UI refresh at ~20 fps for progress, which is
        // plenty for a progress bar / chart.
        cx.spawn({
            async move |_, cx| {
                loop {
                    // Block until at least one message arrives (or channel closes).
                    let first = progress_rx.recv().await;
                    let Ok((
                        first_iteration,
                        first_loss,
                        first_overall,
                        first_speaker,
                        first_message,
                        first_epa,
                        first_step_id,
                        first_step_status,
                    )) = first
                    else {
                        break;
                    };

                    // Aggregate state from the batch. Two message
                    // shapes flow through this channel:
                    //
                    //   (A) Per-iteration data — emitted from the
                    //       inner optimizer loop. Always carries
                    //       `current_speaker`, `iteration`, `loss`,
                    //       `epa`. Step ids may also be set
                    //       (`InProgress`) but conceptually this is
                    //       iteration data.
                    //
                    //   (B) Step transition — emitted at the
                    //       boundaries of pipeline steps with
                    //       `iteration = 0`, `loss = 0.0`, often
                    //       `current_speaker == ""`. Always carries
                    //       a `step_id` + `step_status`.
                    //
                    // We classify a message as a transition-only
                    // event when it has step info AND no usable
                    // per-channel iteration payload (empty speaker).
                    // This lets a real iter-0 message with `loss = 0`
                    // (e.g. DE optimum hit immediately, or a flat
                    // target test harness) still update the readout —
                    // the previous filter "iter is zero" silently
                    // dropped those.
                    fn is_transition_only(speaker: &str, sid: Option<PipelineStepId>) -> bool {
                        sid.is_some() && speaker.is_empty()
                    }

                    let mut latest_iteration = first_iteration;
                    let mut latest_loss = first_loss;
                    let mut latest_speaker = first_speaker.clone();
                    let mut latest_overall_progress = first_overall;
                    let mut latest_message = first_message;
                    let mut latest_step_id = first_step_id;
                    let mut latest_step_status = first_step_status;
                    let mut batch: Vec<(usize, f64, String, Option<f64>)> = Vec::new();
                    let mut step_transitions: Vec<(PipelineStepId, PipelineStepStatus)> =
                        Vec::new();
                    if !is_transition_only(&first_speaker, first_step_id) {
                        batch.push((first_iteration, first_loss, first_speaker, first_epa));
                    }
                    if let (Some(sid), Some(sst)) = (first_step_id, first_step_status) {
                        step_transitions.push((sid, sst));
                    }
                    while let Ok((it, l, op, sp, msg, ep, sid, sst)) = progress_rx.try_recv() {
                        latest_overall_progress = op;
                        if !is_transition_only(&sp, sid) {
                            batch.push((it, l, sp.clone(), ep));
                            latest_iteration = it;
                            latest_loss = l;
                            latest_speaker = sp;
                        }
                        if msg.is_some() {
                            latest_message = msg;
                        }
                        if let (Some(s), Some(st)) = (sid, sst) {
                            step_transitions.push((s, st));
                            latest_step_id = Some(s);
                            latest_step_status = Some(st);
                        }
                    }

                    state_for_progress.update(&mut cx.clone(), |state, cx| {
                        let room_eq = &mut state.app.measurement_state.room_eq_state;
                        // Iteration / loss only advance when a real
                        // per-iteration message arrived in this batch;
                        // otherwise we keep the previous values so the
                        // readout doesn't flicker on step boundaries.
                        if !batch.is_empty() {
                            room_eq.current_iteration = latest_iteration;
                            room_eq.current_loss = latest_loss;
                            room_eq.current_channel = Some(latest_speaker);
                        }
                        room_eq.overall_progress = latest_overall_progress;
                        if let Some(msg) = latest_message {
                            room_eq.status_message = msg;
                        }

                        // Apply step transitions in order so the
                        // history reflects what actually happened
                        // (e.g. Started → Completed → next Started).
                        for (sid, sst) in &step_transitions {
                            room_eq.step_history.insert(*sid, *sst);
                        }
                        if let Some(sid) = latest_step_id
                            && latest_step_status.map(is_active_step).unwrap_or(false)
                        {
                            room_eq.current_step = Some(sid);
                        }

                        let history = &mut room_eq.progress_history;
                        for (it, l, sp, ep) in batch {
                            if history.len() < 10000 {
                                history.push((it, l, sp, ep));
                            }
                        }
                        cx.notify();
                    });

                    // Yield to the executor so it can poll other futures
                    // (critically, the smol::unblock that drives the
                    // optimizer). 50 ms ≈ 20 fps progress refresh.
                    smol::Timer::after(std::time::Duration::from_millis(50)).await;
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
            let cancel_for_cb = cancel_flag.clone();
            let callback: RoomOptimizationCallback =
                Box::new(move |progress: &RoomOptimizationProgress| {
                    if cancel_for_cb.load(std::sync::atomic::Ordering::Relaxed) {
                        return CallbackAction::Stop;
                    }

                    let iteration = progress.iteration;
                    let loss = progress.loss;
                    let speaker = progress.current_speaker.clone();
                    let message = progress.message.clone();

                    // Use the backend's overall_progress directly — it already
                    // accounts for which channel we're on (base + speaker/total).
                    let overall = progress.overall_progress as f32;

                    // Send progress update (non-blocking)
                    let epa = progress.epa_preference;
                    let step_id = progress.step_id;
                    let step_status = progress.step_status;
                    let _ = progress_tx_clone.try_send((
                        iteration,
                        loss,
                        overall,
                        speaker,
                        message,
                        epa,
                        step_id,
                        step_status,
                    ));
                    CallbackAction::Continue
                });

            // Run room optimization (parallel via rayon internally). Use
            // the probe-arrivals entry point when the user measured
            // per-channel delays in the Delay Detection step; otherwise
            // fall back to WAV-onset detection.
            let result = smol::unblock(move || {
                if let Some(arrivals) = probe_arrivals.as_ref() {
                    run_room_optimization_with_probe_arrivals(
                        &room_config,
                        48000.0,
                        Some(callback),
                        arrivals,
                    )
                } else {
                    run_room_optimization(&room_config, 48000.0, Some(callback))
                }
            })
            .await;

            // Drop progress sender to close channel and stop receiver
            drop(progress_tx);

            // If the user cancelled, short-circuit before mapping partial
            // results into the UI. The optimizer returns Ok(..) with whatever
            // it had finished when the callback returned Stop, but we want
            // Cancelled status and to stay on the Optimise step.
            let was_cancelled = cancel_flag.load(std::sync::atomic::Ordering::Relaxed);
            if was_cancelled {
                state_clone.update(&mut cx.clone(), |state, cx| {
                    let room_eq = &mut state.app.measurement_state.room_eq_state;
                    room_eq.optimization_status = OptimizationStatus::Cancelled;
                    room_eq.status_message = "Optimization cancelled".to_string();
                    room_eq.current_channel = None;
                    finalize_pipeline_step_state(room_eq, false);
                    cx.notify();
                });
                return;
            }

            match result {
                Ok(room_result) => {
                    log::info!(
                        "Room optimization completed: {:.4} -> {:.4}",
                        room_result.combined_pre_score,
                        room_result.combined_post_score
                    );

                    // Diagnostic: surface the channel_results keys against
                    // the UI's channel_names list. If any name from
                    // channel_names is missing from channel_results, the
                    // filter_map below will silently drop it and the UI
                    // will show "only the first speaker" even though the
                    // backend may have completed all of them.
                    let result_keys: Vec<&String> = room_result.channel_results.keys().collect();
                    log::info!(
                        "Room EQ result keys: channel_names={:?}, channel_results={:?}, channels={:?}",
                        channel_names,
                        result_keys,
                        room_result.channels.keys().collect::<Vec<_>>(),
                    );
                    let missing: Vec<&String> = channel_names
                        .iter()
                        .filter(|n| !room_result.channel_results.contains_key(n.as_str()))
                        .collect();
                    if !missing.is_empty() {
                        log::error!(
                            "Room EQ regression signal — {} channel(s) in channel_names have no \
                             entry in room_result.channel_results and will be dropped by \
                             filter_map: {:?}",
                            missing.len(),
                            missing,
                        );
                    }

                    let mut display_channel_names = channel_names.clone();
                    for name in room_result.channel_results.keys() {
                        if !display_channel_names.iter().any(|existing| existing == name) {
                            display_channel_names.push(name.clone());
                        }
                    }
                    for name in room_result.channels.keys() {
                        if !display_channel_names.iter().any(|existing| existing == name) {
                            display_channel_names.push(name.clone());
                        }
                    }

                    // Build UI results from the serialized DSP channels first.
                    // Those `initial_curve`/`final_curve` fields are the same
                    // precomputed curves written to the roomeq JSON and plotted
                    // by display-roomeq.py.
                    let all_results: Vec<ChannelOptResult> = display_channel_names
                        .iter()
                        .filter_map(|name| {
                            let chain = room_eq_channel_chain_by_name(&room_result.channels, name);
                            let channel_res = room_eq_channel_result_by_name(
                                &room_result.channel_results,
                                name,
                                chain,
                            );
                            if chain.is_none() && channel_res.is_none() {
                                return None;
                            }
                            Some({
                                // Extract broadband filters from the DSP chain
                                // (labeled "broadband" EQ plugins)
                                let bb_filters: Vec<EqFilterConfig> = chain
                                    .map(|chain| {
                                        chain
                                            .plugins
                                            .iter()
                                            .filter(|p| {
                                                p.plugin_type.eq_ignore_ascii_case("eq")
                                                    && p.parameters
                                                        .get("label")
                                                        .and_then(|l| l.as_str())
                                                        == Some("broadband")
                                            })
                                            .flat_map(|p| {
                                                p.parameters
                                                    .get("filters")
                                                    .and_then(|f| f.as_array())
                                                    .unwrap_or(&vec![])
                                                    .iter()
                                                    .map(|fj| EqFilterConfig {
                                                        filter_type: fj
                                                            .get("filter_type")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("peak")
                                                            .to_string(),
                                                        frequency: fj
                                                            .get("freq")
                                                            .or(fj.get("frequency"))
                                                            .and_then(|v| v.as_f64())
                                                            .unwrap_or(1000.0),
                                                        q: fj
                                                            .get("q")
                                                            .and_then(|v| v.as_f64())
                                                            .unwrap_or(0.707),
                                                        gain_db: fj
                                                            .get("db_gain")
                                                            .or(fj.get("gain_db"))
                                                            .and_then(|v| v.as_f64())
                                                            .unwrap_or(0.0),
                                                    })
                                                    .collect::<Vec<_>>()
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                let preamp_gain_db: f64 = chain
                                    .map(|chain| {
                                        chain
                                            .plugins
                                            .iter()
                                            .filter(|p| p.plugin_type.eq_ignore_ascii_case("gain"))
                                            .filter_map(|p| {
                                                p.parameters
                                                    .get("gain_db")
                                                    .and_then(|v| v.as_f64())
                                            })
                                            .sum()
                                    })
                                    .unwrap_or(0.0);

                                ChannelOptResult {
                                    channel_name: name.clone(),
                                    pre_score: channel_res.map(|r| r.pre_score).unwrap_or(0.0),
                                    post_score: channel_res.map(|r| r.post_score).unwrap_or(0.0),
                                    eq_filters: channel_res
                                        .map(|r| {
                                            r.biquads
                                                .iter()
                                                .map(|b| EqFilterConfig {
                                                    filter_type: format!("{:?}", b.filter_type),
                                                    frequency: b.freq,
                                                    q: b.q,
                                                    gain_db: b.db_gain,
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    broadband_filters: bb_filters,
                                    preamp_gain_db,
                                    crossover_freqs: None,
                                    driver_gains: None,
                                    original_response: room_eq_initial_response_points(
                                        chain,
                                        channel_res.map(|r| &r.initial_curve),
                                    ),
                                    corrected_response: room_eq_display_response_points(
                                        chain,
                                        channel_res.map(|r| &r.final_curve),
                                    ),
                                    normalized_response: room_eq_display_response_points(
                                        chain,
                                        channel_res.map(|r| &r.final_curve),
                                    ),
                                    target_curve: chain
                                        .and_then(|chain| chain.target_curve.as_ref())
                                        .map(|tc| {
                                            tc.freq
                                                .iter()
                                                .zip(tc.spl.iter())
                                                .map(|(&f, &db)| (f, db))
                                                .collect()
                                        }),
                                    group_delay_before: channel_res.and_then(|r| {
                                        compute_group_delay_from_curve(&r.initial_curve)
                                    }),
                                    group_delay_after: channel_res
                                        .and_then(|r| compute_group_delay_from_curve(&r.final_curve)),
                                    phase_response_before: channel_res.and_then(|r| {
                                        compute_phase_response_from_curve(&r.initial_curve)
                                    }),
                                    phase_response_after: channel_res.and_then(|r| {
                                        compute_phase_response_from_curve(&r.final_curve)
                                    }),
                                    impulse_response: chain
                                        .and_then(|c| c.post_ir.as_ref())
                                        .map(|ir| {
                                            ir.time_ms
                                                .iter()
                                                .zip(ir.amplitude.iter())
                                                .filter(|pair| *pair.0 <= 100.0)
                                                .map(|pair| (*pair.0, *pair.1))
                                                .collect()
                                        }),
                                }
                            })
                        })
                        .collect();

                    let avg_pre = room_result.combined_pre_score;
                    let avg_post = room_result.combined_post_score;

                    // Hand the rich autoeq `DspChainOutput` straight through —
                    // `crate::app::types::DspChainOutput` is now just a
                    // re-export, so no lossy field-by-field copy, no
                    // dropped curves/IRs.
                    let dsp_output = room_result.to_dsp_chain_output();

                    // Update final state
                    state_clone.update(&mut cx.clone(), |state, cx| {
                        let room_eq = &mut state.app.measurement_state.room_eq_state;
                        room_eq.optimization_status = OptimizationStatus::Completed;
                        room_eq.status_message = format!(
                            "Optimization complete! Score: {:.2} -> {:.2}",
                            avg_pre, avg_post
                        );
                        room_eq.channel_results = all_results;
                        room_eq.overall_progress = 1.0;
                        room_eq.current_channel = None;
                        room_eq.dsp_output = Some(dsp_output);
                        room_eq.step = crate::app::types::RoomEqStep::Review;
                        finalize_pipeline_step_state(room_eq, true);
                        cx.notify();
                    });
                }
                Err(e) => {
                    log::error!("Room optimization failed: {}", e);
                    state_clone.update(&mut cx.clone(), |state, cx| {
                        let room_eq = &mut state.app.measurement_state.room_eq_state;
                        room_eq.optimization_status = OptimizationStatus::Failed;
                        room_eq.error_message =
                            Some(format!("Room optimization error: {}", e));
                        finalize_pipeline_step_state(room_eq, false);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn cancel_room_eq_optimization(&mut self, cx: &mut Context<Self>) {
        log::info!("Cancel requested for Room EQ optimization");
        self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .room_eq_state
                .cancel_requested
                .store(true, std::sync::atomic::Ordering::Relaxed);
            state.app.measurement_state.room_eq_state.status_message =
                "Cancelling — finishing current iteration...".to_string();
            cx.notify();
        });
    }
}

/// Recursively flatten a JSON value into dotted key-value pairs.
/// Skips large arrays (e.g. measurement data) — only includes scalars and small objects.
/// `true` when a pipeline step status implies the step is the
/// optimizer's current focus. Started/InProgress events update
/// `current_step`; Completed/Skipped events only land in
/// `step_history` so the previous step doesn't keep claiming
/// "current" once the next step has taken over.
fn is_active_step(status: sotf_audio_player::autoeq::PipelineStepStatus) -> bool {
    use sotf_audio_player::autoeq::PipelineStepStatus;
    matches!(
        status,
        PipelineStepStatus::Started | PipelineStepStatus::InProgress
    )
}

/// Finalize the pipeline-step indicators when an optimization run
/// reaches a terminal state. Clears `current_step` (so no chip stays
/// in "active" colour) and, on success, promotes any in-flight
/// (`Started`/`InProgress`) entries in `step_history` to `Completed`
/// so the strip reads as a fully-green summary instead of leaving
/// "what was running when the run ended" half-coloured.
fn finalize_pipeline_step_state(room_eq: &mut crate::app::types::RoomEqState, succeeded: bool) {
    use sotf_audio_player::autoeq::PipelineStepStatus;
    room_eq.current_step = None;
    if succeeded {
        for status in room_eq.step_history.values_mut() {
            if matches!(
                status,
                PipelineStepStatus::Started | PipelineStepStatus::InProgress
            ) {
                *status = PipelineStepStatus::Completed;
            }
        }
    }
}

/// Render the pipeline-step strip: one chip per `PipelineStepId::ALL`
/// in canonical execution order. Status colors:
/// - Pending (unseen)         → muted background + muted text
/// - Started/InProgress       → accent background, scale-up + ring
/// - Completed                → success background
/// - Skipped                  → muted background, dashed border feel
///
/// On global Completed / Failed we paint every reached step in the
/// global outcome color so the strip reads as a final-state summary.
fn render_pipeline_step_strip(
    theme: &crate::app::theme::Theme,
    d: &Ds,
    current_step: Option<sotf_audio_player::autoeq::PipelineStepId>,
    step_history: &std::collections::HashMap<
        sotf_audio_player::autoeq::PipelineStepId,
        sotf_audio_player::autoeq::PipelineStepStatus,
    >,
    is_completed: bool,
    is_failed: bool,
) -> impl IntoElement {
    use sotf_audio_player::autoeq::{PipelineStepId, PipelineStepStatus};

    let mut row = div().flex().flex_row().flex_wrap().gap(d.gap);

    for &step in PipelineStepId::ALL {
        let status = step_history.get(&step).copied();
        let is_current = current_step == Some(step) && !is_completed && !is_failed;

        let (bg, fg, border) = if is_current {
            (theme.accent, theme.text_primary, theme.accent)
        } else {
            match status {
                Some(PipelineStepStatus::Completed) => {
                    (theme.success, theme.text_primary, theme.success)
                }
                Some(PipelineStepStatus::Skipped) => {
                    (theme.surface_hover, theme.text_muted, theme.border)
                }
                Some(PipelineStepStatus::Started) | Some(PipelineStepStatus::InProgress) => {
                    (theme.accent_muted, theme.text_primary, theme.accent)
                }
                None => (theme.surface_hover, theme.text_muted, theme.border),
            }
        };

        row = row.child(
            div()
                .flex()
                .items_center()
                .px(d.pad_x)
                .py(d.gap)
                .rounded(d.r_sm)
                .bg(bg)
                .border_1()
                .border_color(border)
                .child(Text::new(step.label()).size(TextSize::Xs).color(fg)),
        );
    }

    row
}

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

fn downsample_xy(x: &[f64], y: &[f64], max_points: usize) -> (Vec<f64>, Vec<f64>) {
    if max_points == 0 || x.len() <= max_points || y.len() <= max_points {
        return (x.to_vec(), y.to_vec());
    }

    let len = x.len().min(y.len());
    if len <= max_points {
        return (x[..len].to_vec(), y[..len].to_vec());
    }

    let last = len - 1;
    let denom = max_points - 1;
    let mut xs = Vec::with_capacity(max_points);
    let mut ys = Vec::with_capacity(max_points);
    let mut previous = usize::MAX;
    for i in 0..max_points {
        let idx = (i * last + denom / 2) / denom;
        if idx != previous {
            xs.push(x[idx]);
            ys.push(y[idx]);
            previous = idx;
        }
    }
    (xs, ys)
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqProgressChartSeries {
    pub channel: String,
    pub pass: usize,
    pub iterations: Vec<f64>,
    pub losses: Vec<f64>,
    pub epa_iterations: Vec<f64>,
    pub epa_values: Vec<f64>,
}

impl RoomEqProgressChartSeries {
    fn new(channel: String, pass: usize) -> Self {
        Self {
            channel,
            pass,
            iterations: Vec::new(),
            losses: Vec::new(),
            epa_iterations: Vec::new(),
            epa_values: Vec::new(),
        }
    }

    fn loss_label(&self) -> String {
        if self.pass == 1 {
            format!("Loss {}", self.channel)
        } else {
            format!("Loss {} pass {}", self.channel, self.pass)
        }
    }

    fn epa_label(&self) -> String {
        if self.pass == 1 {
            format!("EPA {}", self.channel)
        } else {
            format!("EPA {} pass {}", self.channel, self.pass)
        }
    }
}

pub fn room_eq_progress_chart_series(
    history: &[(usize, f64, String, Option<f64>)],
) -> (Vec<String>, Vec<RoomEqProgressChartSeries>, Vec<f64>) {
    let mut channel_order = Vec::new();
    let mut active_by_channel: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut pass_count_by_channel: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut series = Vec::new();
    let mut all_losses = Vec::new();

    for (iteration, loss, channel, epa) in history {
        if !loss.is_finite() || *loss <= 0.0 {
            continue;
        }

        let active_idx = match active_by_channel.get(channel).copied() {
            Some(idx) => idx,
            None => {
                if !channel_order.iter().any(|existing| existing == channel) {
                    channel_order.push(channel.clone());
                }
                let pass = pass_count_by_channel
                    .entry(channel.clone())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);
                series.push(RoomEqProgressChartSeries::new(channel.clone(), *pass));
                let idx = series.len() - 1;
                active_by_channel.insert(channel.clone(), idx);
                idx
            }
        };

        // Completion/status records sometimes reuse iteration 0 after a
        // channel already emitted real progress. Keep them out of the chart.
        if *iteration == 0 && !series[active_idx].iterations.is_empty() {
            continue;
        }

        let active_idx = if series[active_idx]
            .iterations
            .last()
            .is_some_and(|last| (*iteration as f64) < *last)
        {
            let pass = pass_count_by_channel
                .entry(channel.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            series.push(RoomEqProgressChartSeries::new(channel.clone(), *pass));
            let idx = series.len() - 1;
            active_by_channel.insert(channel.clone(), idx);
            idx
        } else {
            active_idx
        };

        let series = &mut series[active_idx];
        series.iterations.push(*iteration as f64);
        series.losses.push(*loss);
        if let Some(epa) = epa
            && epa.is_finite()
        {
            series.epa_iterations.push(*iteration as f64);
            series.epa_values.push(*epa);
        }
        all_losses.push(*loss);
    }

    (channel_order, series, all_losses)
}

/// Compute group delay from a Curve's phase data.
/// Returns None if no phase data is available.
/// Group delay is always positive (physical delay cannot be negative).
fn compute_group_delay_from_curve(curve: &autoeq::Curve) -> Option<Vec<(f64, f64)>> {
    let phase = curve.phase.as_ref()?;
    let unwrapped = autoeq::loss::phase_aware::unwrap_phase_degrees(phase);
    let gd = autoeq::loss::phase_aware::compute_group_delay(&curve.freq, &unwrapped);
    Some(
        curve
            .freq
            .iter()
            .zip(gd.iter())
            .map(|(&f, &d)| (f, d.abs()))
            .collect(),
    )
}

/// Compute phase response from a Curve's phase data (in degrees, -180 to 180).
/// Returns None if no phase data is available.
fn compute_phase_response_from_curve(curve: &autoeq::Curve) -> Option<Vec<(f64, f64)>> {
    let phase = curve.phase.as_ref()?;
    let unwrapped = autoeq::loss::phase_aware::unwrap_phase_degrees(phase);
    let phase_deg: Vec<f64> = unwrapped
        .iter()
        .map(|&d| (d + 180.0).rem_euclid(360.0) - 180.0)
        .collect();
    Some(
        curve
            .freq
            .iter()
            .zip(phase_deg.iter())
            .map(|(&f, &p)| (f, p))
            .collect(),
    )
}

/// Pick the curve GPUI should display for a RoomEQ channel.
///
/// `ChannelDspChain.final_curve` is the same post-DSP curve written to the
/// roomeq JSON and used by `display-roomeq.py`; it includes route-owned stages
/// such as bass-management crossovers. `ChannelOptimizationResult.final_curve`
/// remains the fallback for older/incomplete backend results.
pub fn room_eq_display_response_points(
    chain: Option<&autoeq::roomeq::ChannelDspChain>,
    fallback_final_curve: Option<&autoeq::Curve>,
) -> Option<Vec<(f64, f64)>> {
    chain
        .and_then(|chain| chain.final_curve.as_ref())
        .map(room_eq_curve_data_response_points)
        .or_else(|| fallback_final_curve.map(room_eq_curve_response_points))
        .filter(|points| !points.is_empty())
}

pub fn room_eq_initial_response_points(
    chain: Option<&autoeq::roomeq::ChannelDspChain>,
    fallback_initial_curve: Option<&autoeq::Curve>,
) -> Option<Vec<(f64, f64)>> {
    chain
        .and_then(|chain| chain.initial_curve.as_ref())
        .map(room_eq_curve_data_response_points)
        .or_else(|| fallback_initial_curve.map(room_eq_curve_response_points))
        .filter(|points| !points.is_empty())
}

pub fn room_eq_channel_chain_by_name<'a>(
    channels: &'a std::collections::HashMap<String, autoeq::roomeq::ChannelDspChain>,
    name: &str,
) -> Option<&'a autoeq::roomeq::ChannelDspChain> {
    channels
        .get(name)
        .or_else(|| channels.values().find(|chain| chain.channel == name))
}

fn room_eq_channel_result_by_name<'a>(
    results: &'a std::collections::HashMap<String, autoeq::roomeq::ChannelOptimizationResult>,
    name: &str,
    chain: Option<&autoeq::roomeq::ChannelDspChain>,
) -> Option<&'a autoeq::roomeq::ChannelOptimizationResult> {
    results
        .get(name)
        .or_else(|| chain.and_then(|chain| results.get(&chain.channel)))
}

fn room_eq_curve_data_response_points(curve: &autoeq::roomeq::CurveData) -> Vec<(f64, f64)> {
    curve
        .freq
        .iter()
        .zip(curve.spl.iter())
        .filter(|(_, db)| db.is_finite() && **db > -150.0)
        .map(|(&f, &db)| (f, db))
        .collect()
}

fn room_eq_curve_response_points(curve: &autoeq::Curve) -> Vec<(f64, f64)> {
    curve
        .freq
        .iter()
        .zip(curve.spl.iter())
        .filter(|(_, db)| db.is_finite() && **db > -150.0)
        .map(|(&f, &db)| (f, db))
        .collect()
}
