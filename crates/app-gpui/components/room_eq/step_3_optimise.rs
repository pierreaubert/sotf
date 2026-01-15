use crate::app::types::room_eq::InteractiveChartStateWrapper;
use crate::components::graphs::common::{rgba_to_u32, theme_to_chart_theme};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::line;
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, HStack, Progress, ProgressSize,
    ProgressVariant, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
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
            // Optimization completed success card
            .when(is_completed, |div| {
                div.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.success)
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new("✓")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Md)
                                                .color(theme.success),
                                        )
                                        .child(
                                            Text::new("Optimization Completed")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Md)
                                                .color(theme.text_primary),
                                        ),
                                )
                                .child(
                                    Text::new(status_msg.clone())
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    Text::new("Click Next to review the results.")
                                        .size(TextSize::Sm)
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
                            .spacing(StackSpacing::Lg)
                            .child(
                                Text::new("Optimization Progress")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .when_some(current_channel.clone(), |hstack, ch| {
                                hstack.child(
                                    Text::new(format!("Channel: {}", ch))
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
                                    .child(Text::new(status_msg.clone()).size(TextSize::Sm).color(
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
                                vstack.child(Text::new(err).size(TextSize::Sm).color(theme.error))
                            }),
                    ),
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

                let iterations: Vec<f64> = history.iter().map(|&(i, _, _)| i as f64).collect();
                let losses: Vec<f64> = history.iter().map(|&(_, loss, _)| loss).collect();

                let current_loss_val = losses.last().copied().unwrap_or(0.0);
                let best_loss = losses.iter().copied().fold(f64::INFINITY, f64::min);

                // Calculate Y range from data
                let (loss_min, loss_max) = losses
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

                // Get domain bounds - use interactive state only when zoomed, otherwise use computed range
                let x_max_data = iterations.last().copied().unwrap_or(100.0);
                let (x_min, x_max) = chart_state
                    .filter(|s| s.is_zoomed())
                    .map(|s| s.x_domain())
                    .unwrap_or((0.0, x_max_data));
                let (y_min_domain, y_max_domain) = chart_state
                    .filter(|s| s.is_zoomed())
                    .map(|s| s.y_domain())
                    .unwrap_or((y_min, y_max));

                let chart = line(&iterations, &losses)
                    .title("Optimization Process")
                    .x_label("Iteration")
                    .y_label("Loss")
                    .label("Loss")
                    .x_range(x_min, x_max)
                    .y_range(y_min_domain, y_max_domain)
                    .color(rgba_to_u32(theme.graph_colors.filter_response))
                    .stroke_width(2.0)
                    .theme(chart_theme)
                    .size(700.0, 250.0)
                    .build();

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
                                .spacing(StackSpacing::Lg)
                                .child(
                                    Text::new("Optimization Process")
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .child(
                                    Text::new(format!("Current: {:.4}", current_loss_val))
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
                                .when_some(chart_element, |el, c| el.child(c)),
                        ),
                )
            })
    }

    fn start_room_eq_optimization(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{
            ChannelOptResult, EqFilterConfig, OptimizationStatus, SpeakerConfigType,
        };
        use sotf_audio_player::room_eq::{
            CallbackAction, CallbackConfig, MeasurementInput, SpeakerOptimizationConfig,
            SpeakerOptimizationProgress, run_speaker_optimization_with_callback,
        };

        log::info!("Starting room EQ optimization with new speaker optimization");

        // Collect configurations from state
        let (channel_configs, _optimizer_params) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.measurement_state.room_eq_state;

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

                    // Build args using library defaults
                    let mut args = autoeq::Args::speaker_defaults();
                    args.num_filters = room_eq.optimizer_config.num_filters;
                    args.sample_rate = 48000.0;
                    args.min_db = room_eq.optimizer_config.min_db;
                    args.max_db = room_eq.optimizer_config.max_db;
                    args.min_q = room_eq.optimizer_config.min_q;
                    args.max_q = room_eq.optimizer_config.max_q;
                    args.min_freq = room_eq.optimizer_config.min_freq;
                    args.max_freq = room_eq.optimizer_config.max_freq;
                    args.algo = match room_eq.optimizer_config.algorithm {
                        crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla".to_string(),
                        crate::app::types::RoomEqAlgorithm::DifferentialEvolution => {
                            "autoeq:de".to_string()
                        }
                        crate::app::types::RoomEqAlgorithm::NelderMead => {
                            "nlopt:neldermead".to_string()
                        }
                    };
                    args.maxeval = room_eq.optimizer_config.max_iter;
                    args.loss = autoeq::LossType::SpeakerFlat;

                    let speaker_config = SpeakerOptimizationConfig {
                        config_type,
                        main_measurement: Some(MeasurementInput::Curve(main_curve)),
                        driver_measurements,
                        crossover_type: Some(crossover_type),
                        crossover_freq_hints: Vec::new(),
                        args,
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

            let mut opt_params = autoeq::Args::speaker_defaults();
            opt_params.num_filters = room_eq.optimizer_config.num_filters;
            opt_params.sample_rate = 48000.0;
            opt_params.min_db = room_eq.optimizer_config.min_db;
            opt_params.max_db = room_eq.optimizer_config.max_db;
            opt_params.min_q = room_eq.optimizer_config.min_q;
            opt_params.max_q = room_eq.optimizer_config.max_q;
            opt_params.min_freq = room_eq.optimizer_config.min_freq;
            opt_params.max_freq = room_eq.optimizer_config.max_freq;
            opt_params.algo = match room_eq.optimizer_config.algorithm {
                crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla".to_string(),
                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => {
                    "autoeq:de".to_string()
                }
                crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead".to_string(),
            };
            opt_params.maxeval = room_eq.optimizer_config.max_iter;
            opt_params.loss = autoeq::LossType::SpeakerFlat;

            (configs, opt_params)
        };

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
        });

        if channel_configs.is_empty() {
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

        let total_channels = channel_configs.len();
        let state_clone = self.state.clone();

        // Create async channel for progress updates from blocking thread
        let (progress_tx, progress_rx) = smol::channel::bounded::<(usize, f64, f32)>(100);

        // Clone state for progress receiver task
        let state_for_progress = self.state.clone();

        // Spawn a task to receive progress updates and update UI
        cx.spawn({
            async move |_, cx| {
                while let Ok((iteration, loss, overall_progress)) = progress_rx.recv().await {
                    let _ = state_for_progress.update(&mut cx.clone(), |state, cx| {
                        state.app.measurement_state.room_eq_state.current_iteration = iteration;
                        state.app.measurement_state.room_eq_state.current_loss = loss;
                        state.app.measurement_state.room_eq_state.overall_progress =
                            overall_progress;
                        // Add to progress history (limit to avoid memory issues)
                        if state
                            .app
                            .measurement_state
                            .room_eq_state
                            .progress_history
                            .len()
                            < 10000
                        {
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .progress_history
                                .push((iteration, loss, None));
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        // Spawn the optimization task
        cx.spawn(async move |_, cx| {
            let mut all_results: Vec<ChannelOptResult> = Vec::new();
            let mut total_pre_score = 0.0;
            let mut total_post_score = 0.0;
            let mut global_iteration_offset = 0usize;

            for (channel_idx, (channel_name, config, _ui_config_type)) in
                channel_configs.into_iter().enumerate()
            {
                // Update status for current channel
                let channel_name_for_status = channel_name.clone();
                let _ = state_clone.update(&mut cx.clone(), |state, cx| {
                    state.app.measurement_state.room_eq_state.current_channel =
                        Some(channel_name_for_status.clone());
                    state.app.measurement_state.room_eq_state.status_message = format!(
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
                let progress_tx_clone = progress_tx.clone();
                let iteration_offset = global_iteration_offset;

                // Create callback that sends progress to UI via channel
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
                        let overall = (channel_idx_f + channel_progress) / total_channels_f;

                        let stage_str = match stage {
                            sotf_audio_player::room_eq::OptimizationStage::Crossover => "crossover",
                            sotf_audio_player::room_eq::OptimizationStage::Eq => "EQ",
                            sotf_audio_player::room_eq::OptimizationStage::Refinement => {
                                "refinement"
                            }
                        };

                        log::debug!(
                            "Channel {}: iter {}/{} ({}) loss={:.4} filters={}",
                            channel_name_cb,
                            iteration,
                            max_iter,
                            stage_str,
                            loss,
                            num_biquads
                        );

                        // Send progress update to UI (ignore errors if channel full or receiver dropped)
                        let _ = progress_tx_clone.try_send((
                            iteration_offset + iteration,
                            loss,
                            overall,
                        ));

                        CallbackAction::Continue
                    });

                // Run optimization in blocking task (optimization is CPU-bound)
                let config_clone = config.clone();
                let max_iter = config.args.maxeval;
                let result = smol::unblock(move || {
                    run_speaker_optimization_with_callback(&config_clone, Some(callback))
                })
                .await;

                // Update iteration offset for next channel
                global_iteration_offset += max_iter;

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
                            normalized_response: Some(
                                speaker_result
                                    .frequencies
                                    .iter()
                                    .zip(speaker_result.normalized_curve.iter())
                                    .map(|(&f, &db)| (f, db))
                                    .collect(),
                            ),
                        };

                        all_results.push(channel_result);

                        // Update progress
                        let progress = (channel_idx + 1) as f32 / total_channels as f32;
                        let _ = state_clone.update(&mut cx.clone(), |state, cx| {
                            state.app.measurement_state.room_eq_state.overall_progress = progress;
                            state.app.measurement_state.room_eq_state.status_message = format!(
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
                        let _ = state_clone.update(&mut cx.clone(), |state, cx| {
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .optimization_status = OptimizationStatus::Failed;
                            state.app.measurement_state.room_eq_state.error_message =
                                Some(format!("Task error for {}: {}", channel_name, e));
                            cx.notify();
                        });
                        return;
                    }
                }
            }

            // Drop the progress sender to signal the receiver task to stop
            drop(progress_tx);

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

            let _ = state_clone.update(&mut cx.clone(), |state, cx| {
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

                // Build DSP output from results
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
                            iterations: state
                                .app
                                .measurement_state
                                .room_eq_state
                                .optimizer_config
                                .max_iter,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        }),
                    });

                // Advance to review step
                state.app.measurement_state.room_eq_state.step =
                    crate::app::types::RoomEqStep::Review;
                cx.notify();
            });
        })
        .detach();
    }
}
