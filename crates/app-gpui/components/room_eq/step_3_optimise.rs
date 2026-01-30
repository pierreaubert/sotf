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
        use crate::app::types::{ChannelOptResult, EqFilterConfig, OptimizationStatus, SpeakerConfigType};
        use autoeq::roomeq::CallbackAction;
        use sotf_audio_player::autoeq::{
            CrossoverConfig, MeasurementSource, OptimizerConfig, RoomConfig,
            RoomOptimizationCallback, RoomOptimizationProgress, SpeakerConfig, SpeakerGroup,
            run_room_optimization,
        };
        use std::collections::HashMap;

        log::info!("Starting room EQ optimization using roomeq");

        // Build RoomConfig from state
        let (room_config, channel_names, max_iter) = {
            let state = self.state.read(cx);
            let room_eq = &state.app.measurement_state.room_eq_state;

            // Build speakers map and crossover map
            let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
            let mut crossovers: HashMap<String, CrossoverConfig> = HashMap::new();

            // Helper to convert measurement to curve
            let to_curve = |meas: &crate::app::types::ChannelMeasurement| -> autoeq::Curve {
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

                autoeq::Curve {
                    freq: ndarray::Array1::from_vec(frequencies),
                    spl: ndarray::Array1::from_vec(magnitude_db),
                    phase: None,
                }
            };

            // Helper to convert recording result to curve
            let result_to_curve = |res: &crate::app::types::recording::RecordingResult| -> autoeq::Curve {
                let frequencies: Vec<f64> = res.frequencies.iter().map(|&f| f as f64).collect();
                let magnitude_db: Vec<f64> = res.magnitude_db.iter().map(|&db| db as f64).collect();

                autoeq::Curve {
                    freq: ndarray::Array1::from_vec(frequencies),
                    spl: ndarray::Array1::from_vec(magnitude_db),
                    phase: None,
                }
            };

            // Iterate over configured speakers
            for speaker_config in &room_eq.speaker_configs {
                let channel_name = &speaker_config.channel_name;
                
                // Find corresponding measurement
                if let Some(meas) = room_eq.channel_measurements.iter().find(|m| &m.channel_name == channel_name) {
                    match speaker_config.config_type {
                        SpeakerConfigType::Single => {
                            let curve = to_curve(meas);
                            speakers.insert(
                                channel_name.clone(),
                                SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
                            );
                        }
                        SpeakerConfigType::MultiDriver => {
                            // Collect driver measurements
                            let mut driver_measurements = Vec::new();
                            
                            // If measurement has group drivers, use them
                            if meas.is_group && !meas.group_drivers.is_empty() {
                                for driver_res in &meas.group_drivers {
                                    driver_measurements.push(MeasurementSource::InMemory(result_to_curve(driver_res)));
                                }
                            } else {
                                // Fallback if no individual drivers found (should not happen if configured correctly)
                                log::warn!("Multi-driver config for {} but no driver measurements found, using main measurement", channel_name);
                                driver_measurements.push(MeasurementSource::InMemory(to_curve(meas)));
                            }

                            // Create crossover config
                            let xover_id = format!("xover_{}", channel_name);
                            let xover_type = match speaker_config.crossover_type {
                                crate::app::types::CrossoverType::LR12 => "LR12",
                                crate::app::types::CrossoverType::LR24 => "LR24",
                                crate::app::types::CrossoverType::LR48 => "LR48",
                                crate::app::types::CrossoverType::Butterworth12 => "Butterworth12",
                                crate::app::types::CrossoverType::Butterworth24 => "Butterworth24",
                            };

                            crossovers.insert(xover_id.clone(), CrossoverConfig {
                                crossover_type: xover_type.to_string(),
                                frequency: None, // Auto-detect
                                frequencies: None, // Auto-detect
                                frequency_range: None,
                            });

                            speakers.insert(
                                channel_name.clone(),
                                SpeakerConfig::Group(SpeakerGroup {
                                    name: channel_name.clone(),
                                    measurements: driver_measurements,
                                    crossover: Some(xover_id),
                                }),
                            );
                        }
                    }
                }
            }

            let channel_names: Vec<String> = room_eq
                .channel_measurements
                .iter()
                .map(|m| m.channel_name.clone())
                .collect();

            // Build optimizer config
            let algorithm = match room_eq.optimizer_config.algorithm {
                crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla".to_string(),
                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => {
                    "autoeq:de".to_string()
                }
                crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead".to_string(),
            };

            let optimizer = OptimizerConfig {
                loss_type: "flat".to_string(),
                algorithm,
                num_filters: room_eq.optimizer_config.num_filters,
                min_q: room_eq.optimizer_config.min_q,
                max_q: room_eq.optimizer_config.max_q,
                min_db: room_eq.optimizer_config.min_db,
                max_db: room_eq.optimizer_config.max_db,
                min_freq: room_eq.optimizer_config.min_freq,
                max_freq: room_eq.optimizer_config.max_freq,
                max_iter: room_eq.optimizer_config.max_iter,
                population: room_eq.optimizer_config.population,
                peq_model: "pk".to_string(),
                mode: "iir".to_string(),
                fir: None,
                seed: None,
                mixed_config: None,
                ..Default::default()
            };

            let config = RoomConfig {
                version: autoeq::roomeq::default_config_version(),
                speakers,
                crossovers: Some(crossovers),
                target_curve: None,
                group_delay: None,
                optimizer,
                recording_config: None,
            };

            (config, channel_names, room_eq.optimizer_config.max_iter)
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
            state.app.measurement_state.room_eq_state.progress_chart_state = Some(
                InteractiveChartStateWrapper::new(
                    0.0,
                    max_iter.max(100) as f64,
                    0.0,
                    1.0,
                )
                .with_log_x(false)
                .with_size(700.0, 250.0),
            );
        });

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
                log::info!("Progress receiver loop finished");
            }
        })
        .detach();

        // Spawn the optimization task
        cx.spawn(async move |_, cx| {
            // Update status
            let _ = state_clone.update(&mut cx.clone(), |state, cx| {
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

                    let overall = if max_iterations > 0 {
                        iteration as f32 / max_iterations as f32
                    } else {
                        0.0
                    };

                    // Send progress update (non-blocking)
                    let _ = progress_tx_clone.try_send((iteration, loss, overall));
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
                                }
                            })
                        })
                        .collect();

                    let avg_pre = room_result.combined_pre_score;
                    let avg_post = room_result.combined_post_score;

                    // Update final state
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
                    let _ = state_clone.update(&mut cx.clone(), |state, cx| {
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
