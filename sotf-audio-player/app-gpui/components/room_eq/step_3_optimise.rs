use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {

    pub(crate) fn render_room_eq_optimize(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Optimization Progress")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
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
}
