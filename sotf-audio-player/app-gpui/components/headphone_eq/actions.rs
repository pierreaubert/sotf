use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::autoeq::OptimizationParams;
use sotf_audio_player::plugins::{EQFilter, PluginSettings, PluginType};

impl PlayerView {
    // ========================================================================
    // Action Handlers
    // ========================================================================

    pub(crate) fn browse_headphone_eq_measurement(&mut self, cx: &mut Context<Self>) {
        let state_entity = self.state.clone();
        cx.spawn(async move |_, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("CSV Files", &["csv", "txt"])
                .set_title("Select Headphone Measurement")
                .pick_file()
                .await;

            if let Some(file) = file {
                let path = file.path().to_string_lossy().to_string();
                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                    state.app.headphone_eq_state.measurement_path = Some(path);
                });
            }
        })
        .detach();
    }

    pub(crate) fn browse_headphone_eq_target(&mut self, cx: &mut Context<Self>) {
        let state_entity = self.state.clone();
        cx.spawn(async move |_, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("CSV Files", &["csv", "txt"])
                .set_title("Select Custom Target Curve")
                .pick_file()
                .await;

            if let Some(file) = file {
                let path = file.path().to_string_lossy().to_string();
                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                    state.app.headphone_eq_state.custom_target_path = Some(path);
                    state.app.headphone_eq_state.target_preset = "custom".to_string();
                });
            }
        })
        .detach();
    }

    pub(crate) fn start_headphone_eq_optimization(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let measurement_path = state.app.headphone_eq_state.measurement_path.clone();
        let target_preset = state.app.headphone_eq_state.target_preset.clone();
        let custom_target_path = state
            .app
            .headphone_eq_state
            .custom_target_path
            .clone()
            .unwrap_or_default();
        let config = state.app.headphone_eq_state.optimizer_config.clone();

        if measurement_path.is_none() {
            log::warn!("No measurement file selected");
            self.state.update(cx, |state, cx| {
                state.app.toast_message = Some(crate::app::types::ToastMessage::error(
                    "Please select a measurement file",
                ));
                cx.notify();
            });
            return;
        }
        let measurement_path = measurement_path.unwrap();

        // Update status to running
        self.state.update(cx, |state, cx| {
            state.app.headphone_eq_state.optimization_status =
                crate::app::types::OptimizationStatus::Running;
            state.app.headphone_eq_state.status_message = "Starting optimization...".to_string();
            state.app.headphone_eq_state.progress = 0.0;
            state.app.headphone_eq_state.progress_history.clear();
            state.app.headphone_eq_state.result = None;
            state.app.headphone_eq_state.error_message = None;
            cx.notify();
        });

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Run optimization in a blocking task
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Construct params
                    let params = OptimizationParams {
                        num_filters: config.num_filters,
                        min_q: config.min_q,
                        max_q: config.max_q,
                        min_db: config.min_db,
                        max_db: config.max_db,
                        min_freq: config.min_freq,
                        max_freq: config.max_freq,
                        maxeval: config.max_iter,
                        loss: config.loss,
                        algo: config.algorithm.to_autoeq_string().to_string(),
                        // Fill defaults for others
                        ..OptimizationParams::headphone_defaults()
                    };

                    sotf_audio_player::autoeq::headphone::run_headphone_optimization(
                        &measurement_path,
                        &target_preset,
                        &custom_target_path,
                        &params,
                        "json",
                    )
                })
                .await;

            let _ = state_entity.update(&mut cx.clone(), |state, cx| {
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

                        state.app.headphone_eq_state.result = Some(app_result);
                        state.app.headphone_eq_state.optimization_status =
                            crate::app::types::OptimizationStatus::Completed;
                        state.app.headphone_eq_state.status_message =
                            "Optimization completed successfully".to_string();
                    }
                    Err(e) => {
                        state.app.headphone_eq_state.optimization_status =
                            crate::app::types::OptimizationStatus::Failed;
                        state.app.headphone_eq_state.error_message = Some(e.clone());
                        state.app.headphone_eq_state.status_message =
                            format!("Optimization failed: {}", e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn apply_headphone_eq_result(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(result) = &state.app.headphone_eq_state.result {
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
                let settings = PluginSettings::EQ { filters };

                // Add to chain
                let plugin_id = state.app.plugin_chain.add_plugin(&PluginType::EQ);
                if let Some(plugin) = state.app.plugin_chain.get_plugin_mut(plugin_id) {
                    plugin.settings = settings;
                    // Ensure it's enabled
                    plugin.enabled = true;
                }

                // Notify engine
                state.app.pending_plugin_update =
                    Some(crate::app::types::PluginUpdateType::Structural);

                state.app.toast_message = Some(crate::app::types::ToastMessage::success(
                    "Applied Headphone EQ",
                ));
                cx.notify();
            } else {
                state.app.toast_message = Some(crate::app::types::ToastMessage::warning(
                    "No optimization result to apply",
                ));
                cx.notify();
            }
        });
    }

    pub(crate) fn save_headphone_eq_result(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        if let Some(result) = &state.app.headphone_eq_state.result {
            // Clone data needed for async task
            let result_json = serde_json::to_string_pretty(result).unwrap_or_default();
            let save_name = state.app.headphone_eq_state.save_name.clone();

            let default_name = if save_name.is_empty() {
                "headphone_eq.json".to_string()
            } else {
                format!("{}.json", save_name)
            };

            let state_entity = self.state.clone();

            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name(&default_name)
                    .set_title("Save Headphone EQ Result")
                    .save_file()
                    .await;

                if let Some(file) = file {
                    let path = file.path().to_path_buf();
                    // Write to file
                    let write_res = std::fs::write(&path, result_json);

                    let _ = state_entity.update(&mut cx.clone(), |state, cx| {
                        match write_res {
                            Ok(_) => {
                                state.app.toast_message =
                                    Some(crate::app::types::ToastMessage::success(format!(
                                        "Saved EQ to {}",
                                        path.display()
                                    )));
                            }
                            Err(e) => {
                                state.app.toast_message =
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
                state.app.toast_message = Some(crate::app::types::ToastMessage::warning(
                    "No optimization result to save",
                ));
                cx.notify();
            });
        }
    }
}

/// Helper to zip two vectors into a vector of tuples
fn zip_vectors(x: &[f64], y: &[f64]) -> Vec<(f64, f64)> {
    x.iter().zip(y.iter()).map(|(&a, &b)| (a, b)).collect()
}

/// Helper to parse filter type string to enum
fn parse_filter_type(type_str: &str) -> autoeq_iir::BiquadFilterType {
    match type_str.to_lowercase().as_str() {
        "pk" | "peak" => autoeq_iir::BiquadFilterType::Peak,
        "ls" | "lowshelf" => autoeq_iir::BiquadFilterType::Lowshelf,
        "hs" | "highshelf" => autoeq_iir::BiquadFilterType::Highshelf,
        "lp" | "lowpass" => autoeq_iir::BiquadFilterType::Lowpass,
        "hp" | "highpass" => autoeq_iir::BiquadFilterType::Highpass,
        "bp" | "bandpass" => autoeq_iir::BiquadFilterType::Bandpass,
        "no" | "notch" => autoeq_iir::BiquadFilterType::Notch,
        _ => autoeq_iir::BiquadFilterType::Peak, // Default
    }
}
