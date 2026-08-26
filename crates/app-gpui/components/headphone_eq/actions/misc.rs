use crate::components::plugins::editing::PluginEditingManager;
use crate::i18n::HeadphoneEasyTranslations;
use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::autoeq::apply_headphone_easy_chain;
use sotf_audio_player::{EQFilter, PluginSettings, PluginType};

impl PlayerView {
    // ========================================================================
    // Action Handlers
    // ========================================================================

    pub(crate) fn browse_headphone_eq_measurement(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let state_entity = self.state.clone();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("CSV Files", &["csv", "txt"])
                    .set_title("Select Headphone Measurement")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let path = file.path().to_string_lossy().to_string();
                    state_entity.update(&mut cx.clone(), |state, _| {
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .model
                            .measurement_path = path;
                    });
                }
            })
            .detach();
        }
    }

    pub(crate) fn browse_headphone_eq_target(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let state_entity = self.state.clone();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("CSV Files", &["csv", "txt"])
                    .set_title("Select Headphone Target Curve")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let path = file.path().to_string_lossy().to_string();
                    state_entity.update(&mut cx.clone(), |state, _| {
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .model
                            .custom_target_path = path;
                    });
                }
            })
            .detach();
        }
    }

    pub(crate) fn start_headphone_eq_optimization(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let measurement_path = state
            .app
            .measurement_state
            .headphone_eq_state
            .measurement_path
            .clone();
        let target_preset = state
            .app
            .measurement_state
            .headphone_eq_state
            .target_preset
            .clone();
        let custom_target_path = state
            .app
            .measurement_state
            .headphone_eq_state
            .model
            .custom_target_path
            .clone();
        let config = state
            .app
            .measurement_state
            .headphone_eq_state
            .optimizer_config
            .clone();

        if measurement_path.is_empty() {
            log::warn!("No measurement file selected");
            self.state.update(cx, |state, cx| {
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::error(
                    "Please select a measurement file",
                ));
                cx.notify();
            });
            return;
        }

        if target_preset == "custom" && custom_target_path.trim().is_empty() {
            log::warn!("Custom target selected but no target file provided");
            self.state.update(cx, |state, cx| {
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::error(
                    "Please select a custom target curve file",
                ));
                cx.notify();
            });
            return;
        }

        // Update status to running
        let cancel_flag = self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .headphone_eq_state
                .cancel_requested
                .store(false, std::sync::atomic::Ordering::Relaxed);
            state
                .app
                .measurement_state
                .headphone_eq_state
                .optimization_status = crate::app::types::OptimizationStatus::Running;
            state
                .app
                .measurement_state
                .headphone_eq_state
                .status_message = "Starting optimization...".to_string();
            state
                .app
                .measurement_state
                .headphone_eq_state
                .model
                .progress = 0.0;
            state
                .app
                .measurement_state
                .headphone_eq_state
                .progress_history
                .clear();
            state.app.measurement_state.headphone_eq_state.model.result = None;
            state
                .app
                .measurement_state
                .headphone_eq_state
                .model
                .error_message = None;
            cx.notify();
            state
                .app
                .measurement_state
                .headphone_eq_state
                .cancel_requested
                .clone()
        });

        let state_entity = self.state.clone();
        let cancel_for_task = cancel_flag.clone();

        cx.spawn(async move |_, cx| {
            // Run optimization in a blocking task
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Construct params using library defaults and override specific fields
                    let mut params = autoeq::Args::headphone_defaults();
                    params.num_filters = config.num_filters;
                    params.min_q = config.min_q;
                    params.max_q = config.max_q;
                    params.min_db = config.min_db;
                    params.max_db = config.max_db;
                    params.min_freq = config.min_freq;
                    params.max_freq = config.max_freq;
                    params.maxeval = config.max_iter;
                    params.loss = sotf_audio_player::autoeq::parse_loss_type(&config.loss);
                    params.algo = config.algorithm.to_autoeq_string().to_string();
                    params.peq_model =
                        sotf_audio_player::autoeq::parse_peq_model(&config.peq_model);
                    params.population = config.population;
                    params.recombination = config.de_cr;
                    params.strategy = config.strategy.clone();
                    params.tolerance = config.tolerance;
                    params.atolerance = config.atolerance;
                    params.bo_initial_samples = config.bo_initial_samples;
                    params.bo_batch_size = config.bo_batch_size;
                    params.bo_posterior_std_threshold = config.bo_posterior_std_threshold;
                    params.bo_acquisition = config.bo_acquisition.clone();
                    params.bo_ehvi = config.bo_ehvi;
                    params.refine = config.refine;
                    params.local_algo = config.local_algo.clone();
                    params.smooth = config.smooth;
                    params.smooth_n = config.smooth_n;

                    let cancel_for_cb = cancel_for_task.clone();
                    sotf_audio_player::autoeq::headphone::run_headphone_optimization_with_callback(
                        &measurement_path,
                        &target_preset,
                        &custom_target_path,
                        &params,
                        Some(move |_: &autoeq::ProgressUpdate| {
                            if cancel_for_cb.load(std::sync::atomic::Ordering::Relaxed) {
                                autoeq::de::CallbackAction::Stop
                            } else {
                                autoeq::de::CallbackAction::Continue
                            }
                        }),
                    )
                })
                .await;

            // If the user cancelled, surface Cancelled status and skip the
            // success/failure branch — the optimizer may have returned Ok
            // with partial results, but we don't want them.
            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                state_entity.update(&mut cx.clone(), |state, cx| {
                    state
                        .app
                        .measurement_state
                        .headphone_eq_state
                        .optimization_status = crate::app::types::OptimizationStatus::Cancelled;
                    state
                        .app
                        .measurement_state
                        .headphone_eq_state
                        .status_message = "Optimization cancelled".to_string();
                    cx.notify();
                });
                return;
            }

            state_entity.update(&mut cx.clone(), |state, cx| {
                match result {
                    Ok(opt_result) => {
                        // Map result to App state
                        let app_result = crate::app::types::HeadphoneEqResult {
                            biquads: opt_result
                                .biquads
                                .iter()
                                .map(|b| crate::app::types::HeadphoneEqBiquad {
                                    filter_type: b.filter_type.long_name().to_string(),
                                    freq: b.freq,
                                    q: b.q,
                                    db_gain: b.db_gain,
                                })
                                .collect(),
                            pre_score: opt_result.initial_loss,
                            post_score: opt_result.final_loss,
                            original_response: Some(zip_vectors(
                                &opt_result.frequencies,
                                &opt_result.input_curve,
                            )),
                            corrected_response: Some(zip_vectors(
                                &opt_result.frequencies,
                                &opt_result.corrected_curve,
                            )),
                            target_response: Some(zip_vectors(
                                &opt_result.frequencies,
                                &opt_result.target_curve,
                            )),
                            filter_response: Some(zip_vectors(
                                &opt_result.frequencies,
                                &opt_result.filter_response,
                            )),
                            deviation_response: Some(zip_vectors(
                                &opt_result.frequencies,
                                &opt_result.deviation_curve,
                            )),
                            error_response: Some(zip_vectors(
                                &opt_result.frequencies,
                                &opt_result.error_curve,
                            )),
                            individual_responses: Some(
                                opt_result
                                    .individual_filter_responses
                                    .iter()
                                    .map(|response| zip_vectors(&opt_result.frequencies, response))
                                    .collect(),
                            ),
                        };

                        state.app.measurement_state.headphone_eq_state.model.result =
                            Some(app_result);
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .optimization_status = crate::app::types::OptimizationStatus::Completed;
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .status_message = "Optimization completed successfully".to_string();
                    }
                    Err(e) => {
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .optimization_status = crate::app::types::OptimizationStatus::Failed;
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .model
                            .error_message = Some(e.clone());
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .status_message = format!("Optimization failed: {}", e);
                        // Toast in case the user is no longer looking at the
                        // optimisation step's inline banner.
                        state.app.ui_state.toast_message =
                            Some(crate::app::types::ToastMessage::error(format!(
                                "Optimization failed: {}",
                                e
                            )));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cancel_headphone_eq_optimization(&mut self, cx: &mut Context<Self>) {
        log::info!("Cancel requested for headphone EQ optimization");
        self.state.update(cx, |state, cx| {
            state
                .app
                .measurement_state
                .headphone_eq_state
                .cancel_requested
                .store(true, std::sync::atomic::Ordering::Relaxed);
            state
                .app
                .measurement_state
                .headphone_eq_state
                .status_message = "Cancelling — finishing current iteration...".to_string();
            cx.notify();
        });
    }

    pub(crate) fn apply_headphone_eq_result(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(result) = &state.app.measurement_state.headphone_eq_state.result {
                // Convert biquads to EQFilters, filtering out near-zero gain filters
                let filters: Vec<EQFilter> = result
                    .biquads
                    .iter()
                    .filter(|bq| bq.db_gain.abs() >= 0.1) // Skip effectively disabled filters
                    .map(|bq| {
                        EQFilter::new(
                            parse_filter_type(&bq.filter_type),
                            bq.freq,
                            bq.q,
                            bq.db_gain,
                        )
                    })
                    .collect();

                // Create new EQ plugin settings
                let settings = PluginSettings::EQ {
                    channels: 2,
                    filters,
                    channel_filters: None,
                    per_channel_mode: false,
                    max_filters: 10,
                    tdf2: false,
                    topology: 0.0,
                    auto_gain_enabled: false,
                    oversampling: 1.0,
                };

                // Add to chain (insert before Matrix for proper ordering)
                let insert_idx = state.app.plugin_state.graph.user_plugin_insert_index();
                let _ = state
                    .app
                    .plugin_state
                    .graph
                    .insert_plugin(insert_idx, &PluginType::EQ);
                if let Some(plugin) = state.app.plugin_state.graph.get_plugin_mut(insert_idx) {
                    plugin.settings = settings;
                    // Ensure it's enabled
                    plugin.enabled = true;
                }

                // Notify engine
                state.app.plugin_state.update_state.pending_plugin_update =
                    Some(crate::app::types::PluginUpdateType::Structural);
                state.app.sync_spectrum_visible();

                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::success(
                    "Applied Headphone EQ",
                ));
                cx.notify();
            } else {
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::warning(
                    "No optimization result to apply",
                ));
                cx.notify();
            }
        });
    }

    pub(crate) fn apply_headphone_easy_result(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let translations = HeadphoneEasyTranslations::for_language(state.app.ui_state.language);
            let Some(result) = state
                .app
                .measurement_state
                .headphone_eq_state
                .result
                .as_ref()
            else {
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::warning(
                    translations.no_result,
                ));
                cx.notify();
                return;
            };
            let filters: Vec<(String, f64, f64, f64)> = result
                .biquads
                .iter()
                .map(|filter| {
                    (
                        filter.filter_type.clone(),
                        filter.freq,
                        filter.q,
                        filter.db_gain,
                    )
                })
                .collect();
            let previous_graph = state.app.plugin_state.graph.clone();
            let sample_rate = f64::from(
                state
                    .app
                    .audio_device_state
                    .hal_config
                    .sample_rate
                    .max(8_000),
            );

            match apply_headphone_easy_chain(
                &mut state.app.plugin_state.graph,
                &filters,
                sample_rate,
                70.0,
                83.0,
            ) {
                Ok(outcome) => {
                    let headphone_eq = &mut state.app.measurement_state.headphone_eq_state;
                    headphone_eq.easy_mode_undo_graph = Some(previous_graph);
                    headphone_eq.easy_mode_last_apply = Some(outcome);
                    state.app.plugin_state.update_state.pending_plugin_update =
                        Some(crate::app::types::PluginUpdateType::Structural);
                    state.app.sync_spectrum_visible();
                    state.app.ui_state.toast_message =
                        Some(crate::app::types::ToastMessage::success(
                            translations.applied(outcome.active_filters, outcome.preamp_db),
                        ));
                }
                Err(error) => {
                    state.app.ui_state.toast_message =
                        Some(crate::app::types::ToastMessage::error(error));
                }
            }
            cx.notify();
        });
    }

    pub(crate) fn undo_headphone_easy_chain(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let translations = HeadphoneEasyTranslations::for_language(state.app.ui_state.language);
            let previous = state
                .app
                .measurement_state
                .headphone_eq_state
                .easy_mode_undo_graph
                .take();
            let Some(previous) = previous else {
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::warning(
                    translations.no_undo,
                ));
                cx.notify();
                return;
            };
            state.app.plugin_state.graph = previous;
            state
                .app
                .measurement_state
                .headphone_eq_state
                .easy_mode_last_apply = None;
            state.app.plugin_state.update_state.pending_plugin_update =
                Some(crate::app::types::PluginUpdateType::Structural);
            state.app.sync_spectrum_visible();
            state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::success(
                translations.restored,
            ));
            cx.notify();
        });
    }

    pub(crate) fn edit_headphone_easy_chain(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.ui_state.last_screen = crate::app::Screen::HeadphoneEq;
            state.app.ui_state.current_screen = crate::app::Screen::Studio;
            cx.notify();
        });
    }

    pub(crate) fn save_headphone_eq_result(&mut self, cx: &mut Context<Self>) {
        #[cfg(feature = "dev-api")]
        if self.save_headphone_eq_qa_export(cx) {
            return;
        }

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let state = self.state.read(cx);
            if let Some(result) = &state.app.measurement_state.headphone_eq_state.result {
                let export_format = state
                    .app
                    .measurement_state
                    .headphone_eq_state
                    .export_format
                    .clone();
                let ext = sotf_audio_player::autoeq::get_export_extension(&export_format);
                let save_name = state
                    .app
                    .measurement_state
                    .headphone_eq_state
                    .save_name
                    .clone();

                let default_name = if save_name.is_empty() {
                    format!("headphone_eq{ext}")
                } else {
                    format!("{save_name}{ext}")
                };

                let biquads: Vec<math_audio_iir_fir::Biquad> = result
                    .biquads
                    .iter()
                    .map(|b| {
                        let ft = match b.filter_type.as_str() {
                            "peak" => math_audio_iir_fir::BiquadFilterType::Peak,
                            "lowshelf" => math_audio_iir_fir::BiquadFilterType::Lowshelf,
                            "highshelf" => math_audio_iir_fir::BiquadFilterType::Highshelf,
                            "lowpass" => math_audio_iir_fir::BiquadFilterType::Lowpass,
                            "highpass" => math_audio_iir_fir::BiquadFilterType::Highpass,
                            _ => math_audio_iir_fir::BiquadFilterType::Peak,
                        };
                        math_audio_iir_fir::Biquad::new(ft, b.freq, 48000.0, b.q, b.db_gain)
                    })
                    .collect();

                let content = sotf_audio_player::autoeq::format_peq_export(
                    &export_format,
                    "Headphone EQ",
                    &biquads,
                    48000,
                );

                let content = match content {
                    Ok(c) => c,
                    Err(e) => {
                        self.state.update(cx, |state, cx| {
                            state.app.ui_state.toast_message =
                                Some(crate::app::types::ToastMessage::error(format!(
                                    "Format error: {e}"
                                )));
                            cx.notify();
                        });
                        return;
                    }
                };

                let ext_no_dot = ext.trim_start_matches('.');
                let state_entity = self.state.clone();

                cx.spawn(async move |_, cx| {
                    let file = rfd::AsyncFileDialog::new()
                        .add_filter("EQ File", &[ext_no_dot])
                        .set_file_name(&default_name)
                        .set_title("Save Headphone EQ Result")
                        .save_file()
                        .await;

                    if let Some(file) = file {
                        let path = file.path().to_path_buf();
                        let write_res = std::fs::write(&path, content);

                        state_entity.update(&mut cx.clone(), |state, cx| {
                            match write_res {
                                Ok(_) => {
                                    state.app.ui_state.toast_message =
                                        Some(crate::app::types::ToastMessage::success(format!(
                                            "Saved EQ to {}",
                                            path.display()
                                        )));
                                }
                                Err(e) => {
                                    state.app.ui_state.toast_message =
                                        Some(crate::app::types::ToastMessage::error(format!(
                                            "Failed to save: {}",
                                            e
                                        )));
                                }
                            }
                            cx.notify();
                        });
                    }
                })
                .detach();
            } else {
                self.state.update(cx, |state, cx| {
                    state.app.ui_state.toast_message = Some(
                        crate::app::types::ToastMessage::warning("No optimization result to save"),
                    );
                    cx.notify();
                });
            }
        }
    }

    // ========================================================================
    /// Write a deterministic export for the rendered QA build. The normal
    /// desktop path below still presents the native save dialog in production.
    #[cfg(feature = "dev-api")]
    fn save_headphone_eq_qa_export(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(qa_directory) = std::env::var_os("SOTF_QA_DIR") else {
            return false;
        };

        let (biquads, export_format, default_name) = {
            let state = self.state.read(cx);
            let headphone_eq = &state.app.measurement_state.headphone_eq_state;
            let Some(result) = &headphone_eq.result else {
                return true;
            };
            let export_format = headphone_eq.export_format.clone();
            let extension = sotf_audio_player::autoeq::get_export_extension(&export_format);
            let default_name = if headphone_eq.save_name.is_empty() {
                format!("headphone_eq{extension}")
            } else {
                format!("{}{}", headphone_eq.save_name, extension)
            };
            let biquads = result
                .biquads
                .iter()
                .map(|biquad| {
                    let filter_type = match biquad.filter_type.as_str() {
                        "peak" => math_audio_iir_fir::BiquadFilterType::Peak,
                        "lowshelf" => math_audio_iir_fir::BiquadFilterType::Lowshelf,
                        "highshelf" => math_audio_iir_fir::BiquadFilterType::Highshelf,
                        "lowpass" => math_audio_iir_fir::BiquadFilterType::Lowpass,
                        "highpass" => math_audio_iir_fir::BiquadFilterType::Highpass,
                        _ => math_audio_iir_fir::BiquadFilterType::Peak,
                    };
                    math_audio_iir_fir::Biquad::new(
                        filter_type,
                        biquad.freq,
                        48_000.0,
                        biquad.q,
                        biquad.db_gain,
                    )
                })
                .collect::<Vec<_>>();
            (biquads, export_format, default_name)
        };
        let content = match sotf_audio_player::autoeq::format_peq_export(
            &export_format,
            "Headphone EQ",
            &biquads,
            48_000,
        ) {
            Ok(content) => content,
            Err(error) => {
                self.state.update(cx, |state, _cx| {
                    state.app.ui_state.toast_message =
                        Some(crate::app::types::ToastMessage::error(format!(
                            "Failed to format Headphone EQ export: {error}"
                        )));
                });
                cx.notify();
                return true;
            }
        };
        let directory = std::path::PathBuf::from(qa_directory).join("headphone-exports");
        let path = directory.join(default_name);
        let write_result =
            std::fs::create_dir_all(&directory).and_then(|()| std::fs::write(&path, content));
        self.state.update(cx, |state, _cx| match write_result {
            Ok(()) => {
                state
                    .app
                    .measurement_state
                    .headphone_eq_state
                    .qa_last_export_path = Some(path);
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::success(
                    "Headphone EQ export saved for QA",
                ));
            }
            Err(error) => {
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::error(
                    format!("Failed to save Headphone EQ export: {error}"),
                ));
            }
        });
        cx.notify();
        true
    }

    // Headphone API Download (spinorama.org)
    // ========================================================================

    pub(crate) fn fetch_headphone_list(&mut self, cx: &mut Context<Self>) {
        log::info!("Fetching headphone list from API...");
        let request_id = self.state.update(cx, |state, _cx| {
            let headphone_eq = &mut state.app.measurement_state.headphone_eq_state;
            headphone_eq.loading_headphones = true;
            headphone_eq.model.error_message = None;
            headphone_eq.begin_headphone_list_request()
        });
        cx.notify();

        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        #[cfg(feature = "dev-api")]
        let qa_catalog = self
            .state
            .read(cx)
            .app
            .measurement_state
            .headphone_eq_state
            .qa_discovery_fixture
            .as_ref()
            .map(|fixture| fixture.catalog.clone());
        #[cfg(feature = "dev-api")]
        if let Some(catalog) = qa_catalog {
            let _ = sender.send(Ok(catalog));
        } else {
            std::thread::spawn(move || {
                let result = tokio::runtime::Runtime::new()
                    .map_err(|error| format!("Failed to create network runtime: {error}"))
                    .and_then(|runtime| {
                        runtime
                            .block_on(async { autoeq::fetch_available_headphones().await })
                            .map_err(|error| error.to_string())
                    });
                let _ = sender.send(result);
            });
        }
        #[cfg(not(feature = "dev-api"))]
        std::thread::spawn(move || {
            let result = tokio::runtime::Runtime::new()
                .map_err(|error| format!("Failed to create network runtime: {error}"))
                .and_then(|runtime| {
                    runtime
                        .block_on(async { autoeq::fetch_available_headphones().await })
                        .map_err(|error| error.to_string())
                });
            let _ = sender.send(result);
        });

        let weak_state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;
                let result = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
                        "Headphone fetch worker stopped unexpectedly".to_string(),
                    )),
                };
                if let Some(result) = result {
                    let Some(state_entity) = weak_state.upgrade() else {
                        break;
                    };
                    match result {
                        Ok(headphones) => {
                            log::info!(
                                "Fetched {} headphones from spinorama.org",
                                headphones.len()
                            );
                            state_entity.update(cx, |state, cx| {
                                let headphone_eq =
                                    &mut state.app.measurement_state.headphone_eq_state;
                                if headphone_eq.headphone_list_request_id != request_id {
                                    return;
                                }
                                headphone_eq.available_headphones = headphones;
                                headphone_eq.loading_headphones = false;
                                headphone_eq.headphones_cached_at = Some(std::time::Instant::now());
                                headphone_eq.update_headphone_suggestions();
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to fetch headphones: {}", e);
                            state_entity.update(cx, |state, cx| {
                                let headphone_eq =
                                    &mut state.app.measurement_state.headphone_eq_state;
                                if headphone_eq.headphone_list_request_id != request_id {
                                    return;
                                }
                                headphone_eq.loading_headphones = false;
                                let msg = format!("Failed to fetch headphones: {}", e);
                                headphone_eq.model.error_message = Some(msg.clone());
                                state.app.ui_state.toast_message =
                                    Some(crate::app::types::ToastMessage::error(msg));
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
            }
        })
        .detach();
    }

    /// Select a headphone and auto-download its measurement.
    /// Chains: versions → auto-select first → measurements → auto-select first → download curve → save CSV.
    pub(crate) fn select_headphone(&mut self, headphone: &str, cx: &mut Context<Self>) {
        log::info!("Selected headphone: {}", headphone);
        let headphone_name = headphone.to_string();

        let request_id = self.state.update(cx, |state, _cx| {
            let headphone_eq = &mut state.app.measurement_state.headphone_eq_state;
            headphone_eq.model.selected_headphone = Some(headphone_name.clone());
            headphone_eq.model.loading_download = true;
            headphone_eq.model.error_message = None;
            // Clear any previous measurement_path from a previous download
            headphone_eq.model.measurement_path.clear();
            headphone_eq.model.downloaded_curve = None;
            headphone_eq.begin_download_request()
        });
        cx.notify();

        // Single thread: fetch versions → first version → measurements → first measurement → download curve → save CSV
        // Result contains (csv_path, curve_data)
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let headphone_for_thread = headphone_name.clone();
        #[cfg(feature = "dev-api")]
        let qa_download = self.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .headphone_eq_state
                .qa_discovery_fixture
                .as_mut()
                .and_then(|fixture| fixture.downloads.get_mut(&headphone_name))
                .map(|download| {
                    let should_fail = download.failures_remaining > 0;
                    download.failures_remaining = download.failures_remaining.saturating_sub(1);
                    (download.clone(), should_fail)
                })
        });
        #[cfg(feature = "dev-api")]
        if let Some((download, should_fail)) = qa_download {
            std::thread::spawn(move || {
                if download.delay_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(download.delay_ms));
                }
                let result = if should_fail {
                    Err(download.failure_message)
                } else {
                    (|| -> Result<(String, Vec<(f64, f64)>), String> {
                        let file_name = std::path::Path::new(&download.path)
                            .file_name()
                            .filter(|name| !name.is_empty())
                            .ok_or_else(|| {
                                "fixture measurement path needs a file name".to_string()
                            })?;
                        let directory = std::env::var_os("SOTF_QA_DIR")
                            .map(std::path::PathBuf::from)
                            .ok_or_else(|| {
                                "SOTF_QA_DIR is required for fixture measurements".to_string()
                            })?
                            .join("headphone-fixtures");
                        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
                        let path = directory.join(file_name);
                        let csv = std::iter::once("frequency,spl".to_string())
                            .chain(
                                download
                                    .curve
                                    .iter()
                                    .map(|(frequency, spl)| format!("{frequency},{spl}")),
                            )
                            .collect::<Vec<_>>()
                            .join("\n");
                        std::fs::write(&path, csv).map_err(|error| error.to_string())?;
                        Ok((path.to_string_lossy().into_owned(), download.curve))
                    })()
                };
                let _ = sender.send(result);
            });
        } else {
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ =
                            sender.send(Err(format!("Failed to create network runtime: {error}")));
                        return;
                    }
                };
                let result = rt.block_on(async {
                    let (csv_path, curve) =
                        autoeq::fetch_headphone_frequency_response(&headphone_for_thread)
                            .await
                            .map_err(|e| e.to_string())?;

                    let curve_data: Vec<(f64, f64)> = curve
                        .freq
                        .iter()
                        .zip(curve.spl.iter())
                        .map(|(&f, &s)| (f, s))
                        .collect();

                    Ok::<(String, Vec<(f64, f64)>), String>((csv_path, curve_data))
                });
                let _ = sender.send(result);
            });
        }
        #[cfg(not(feature = "dev-api"))]
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sender.send(Err(format!("Failed to create network runtime: {error}")));
                    return;
                }
            };
            let result = rt.block_on(async {
                let (csv_path, curve) =
                    autoeq::fetch_headphone_frequency_response(&headphone_for_thread)
                        .await
                        .map_err(|error| error.to_string())?;
                Ok::<(String, Vec<(f64, f64)>), String>((
                    csv_path,
                    curve
                        .freq
                        .iter()
                        .zip(curve.spl.iter())
                        .map(|(&f, &s)| (f, s))
                        .collect(),
                ))
            });
            let _ = sender.send(result);
        });

        let weak_state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;
                let result = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
                        "Headphone download worker stopped unexpectedly".to_string(),
                    )),
                };
                if let Some(result) = result {
                    let Some(state_entity) = weak_state.upgrade() else {
                        break;
                    };
                    match result {
                        Ok((csv_path, curve_data)) => {
                            log::info!(
                                "Downloaded headphone measurement to: {} ({} points)",
                                csv_path,
                                curve_data.len()
                            );
                            state_entity.update(cx, |state, cx| {
                                if state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .download_request_id
                                    != request_id
                                {
                                    return;
                                }
                                state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .model
                                    .measurement_path = csv_path;
                                state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .model
                                    .downloaded_curve = Some(curve_data);
                                state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .loading_download = false;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::error!("Headphone download failed: {}", e);
                            state_entity.update(cx, |state, cx| {
                                if state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .download_request_id
                                    != request_id
                                {
                                    return;
                                }
                                state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .loading_download = false;
                                let msg = format!("Headphone download failed: {}", e);
                                state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .model
                                    .error_message = Some(e);
                                state.app.ui_state.toast_message =
                                    Some(crate::app::types::ToastMessage::error(msg));
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
            }
        })
        .detach();
    }
}

/// Helper to zip two vectors into a vector of tuples
fn zip_vectors(x: &[f64], y: &[f64]) -> Vec<(f64, f64)> {
    x.iter().zip(y.iter()).map(|(&a, &b)| (a, b)).collect()
}

/// Helper to parse filter type string to enum
fn parse_filter_type(type_str: &str) -> math_audio_iir_fir::BiquadFilterType {
    match type_str.to_lowercase().as_str() {
        "pk" | "peak" => math_audio_iir_fir::BiquadFilterType::Peak,
        "ls" | "lowshelf" => math_audio_iir_fir::BiquadFilterType::Lowshelf,
        "hs" | "highshelf" => math_audio_iir_fir::BiquadFilterType::Highshelf,
        "lp" | "lowpass" => math_audio_iir_fir::BiquadFilterType::Lowpass,
        "hp" | "highpass" => math_audio_iir_fir::BiquadFilterType::Highpass,
        "bp" | "bandpass" => math_audio_iir_fir::BiquadFilterType::Bandpass,
        "no" | "notch" => math_audio_iir_fir::BiquadFilterType::Notch,
        _ => math_audio_iir_fir::BiquadFilterType::Peak, // Default
    }
}
