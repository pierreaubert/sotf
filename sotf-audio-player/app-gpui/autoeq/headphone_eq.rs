//! Headphone EQ optimization and management

use crate::ui::PlayerView;
use gpui::*;
use std::path::PathBuf;

/// Bundled target curve data
mod target_curves {
    pub const HARMAN_OVER_EAR_2018: &str = include_str!("../../../data_tests/targets/harman-over-ear-2018.csv");
    pub const HARMAN_OVER_EAR_2015: &str = include_str!("../../../data_tests/targets/harman-over-ear-2015.csv");
    pub const HARMAN_OVER_EAR_2013: &str = include_str!("../../../data_tests/targets/harman-over-ear-2013.csv");
    pub const HARMAN_IN_EAR_2019: &str = include_str!("../../../data_tests/targets/harman-in-ear-2019.csv");
}

/// Result of headphone EQ optimization with all curves for visualization
#[derive(Clone, Debug)]
pub struct HeadphoneOptimizationResult {
    /// Optimized biquad filters
    pub biquads: Vec<autoeq_iir::Biquad>,
    /// Frequency points (Hz) - log-spaced
    pub frequencies: Vec<f64>,
    /// Input headphone measurement curve (dB)
    pub input_curve: Vec<f64>,
    /// Target curve (dB)
    pub target_curve: Vec<f64>,
    /// Deviation from target = target - input (dB)
    pub deviation_curve: Vec<f64>,
    /// Combined filter response (dB)
    pub filter_response: Vec<f64>,
    /// Error = deviation - filter_response (dB)
    pub error_curve: Vec<f64>,
    /// Corrected response = input + filter_response (dB)
    pub corrected_curve: Vec<f64>,
    /// Individual filter responses (each filter's dB response)
    pub individual_filter_responses: Vec<Vec<f64>>,
    /// Path where results were saved
    pub output_path: String,
    /// Optimization history (iteration, loss)
    pub optimization_history: Vec<(usize, f64)>,
    /// Initial loss value
    pub initial_loss: f64,
    /// Final loss value
    pub final_loss: f64,
}

impl PlayerView {
    /// Open file dialog to select headphone measurement file
    pub fn browse_headphone_curve(&mut self, cx: &mut Context<Self>) {
        // Use async file dialog to avoid blocking the main thread
        let state_clone = self.state.clone();
        cx.spawn(async move |_view: WeakEntity<PlayerView>, cx| {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .add_filter("CSV Files", &["csv"])
                .add_filter("All Files", &["*"])
                .set_title("Select Headphone Measurement File")
                .pick_file()
                .await
            {
                let path_str = handle.path().display().to_string();
                let _ = state_clone.update(cx, |state, _cx| {
                    state.app.headphone_curve_path = path_str;
                });
            }
        })
        .detach();
    }

    /// Open file dialog to select custom target curve file
    pub fn browse_target_curve(&mut self, cx: &mut Context<Self>) {
        // Use async file dialog to avoid blocking the main thread
        let state_clone = self.state.clone();
        cx.spawn(async move |_view: WeakEntity<PlayerView>, cx| {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .add_filter("CSV Files", &["csv"])
                .add_filter("All Files", &["*"])
                .set_title("Select Custom Target Curve")
                .pick_file()
                .await
            {
                let path_str = handle.path().display().to_string();
                let _ = state_clone.update(cx, |state, _cx| {
                    // Store the custom path and set target to "custom"
                    state.app.headphone_target_custom_path = path_str;
                    state.app.headphone_target = "custom".to_string();
                });
            }
        })
        .detach();
    }

    /// Run headphone EQ optimization
    pub fn run_headphone_optimization(&mut self, cx: &mut Context<Self>) {
        let (curve_path, target, target_custom_path, params, export_format) = {
            let state = self.state.read(cx);

            // Validate inputs
            if state.app.headphone_curve_path.is_empty() {
                let _ = state;
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(
                        "Please select a headphone measurement file",
                    ));
                });
                cx.notify();
                return;
            }

            // Validate custom target path if custom is selected
            if state.app.headphone_target == "custom" && state.app.headphone_target_custom_path.is_empty() {
                let _ = state;
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(
                        "Please select a custom target curve file",
                    ));
                });
                cx.notify();
                return;
            }

            (
                state.app.headphone_curve_path.clone(),
                state.app.headphone_target.clone(),
                state.app.headphone_target_custom_path.clone(),
                state.app.headphone_params.clone(),
                state.app.headphone_export_format.clone(),
            )
        };

        // Mark optimization as running and clear previous results
        self.state.update(cx, |state, _cx| {
            state.app.headphone_optimization_running = true;
            state.app.headphone_optimization_progress.clear();
            state.app.headphone_optimization_result = None;
        });
        cx.notify();

        // Clone state for background task
        let state_clone = self.state.clone();

        // Spawn background task for optimization
        cx.spawn(async move |_view, cx| {
            // Run optimization
            let result = run_optimization_task(
                curve_path,
                target,
                target_custom_path,
                params,
                export_format,
            ).await;

            match result {
                Ok(optimization_result) => {
                    let output_path = optimization_result.output_path.clone();
                    // Update state with success and results
                    let _ = state_clone.update(cx, |state, _cx| {
                        state.app.headphone_optimization_running = false;
                        state.app.headphone_optimization_result = Some(optimization_result);
                        state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                            "EQ optimization complete! Saved to: {}",
                            output_path
                        )));
                    });
                }
                Err(e) => {
                    // Update state with error
                    let _ = state_clone.update(cx, |state, _cx| {
                        state.app.headphone_optimization_running = false;
                        state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                            "Optimization failed: {}",
                            e
                        )));
                    });
                }
            }
        })
        .detach();
    }

    /// List saved EQ files
    pub fn list_saved_eq_files(&self) -> Vec<PathBuf> {
        if let Some(eq_dir) = sotf_audio_player::config::get_eq_dir() {
            if let Ok(entries) = std::fs::read_dir(&eq_dir) {
                return entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry.path().extension().and_then(|s| s.to_str()) == Some("json")
                    })
                    .map(|entry| entry.path())
                    .collect();
            }
        }
        Vec::new()
    }

    /// Load EQ from file and apply to plugin chain
    pub fn load_headphone_eq(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                // Parse the EQ file as array of biquad filters
                match serde_json::from_str::<Vec<autoeq_iir::Biquad>>(&json) {
                    Ok(biquads) => {
                        // TODO: Apply biquads to plugin chain
                        log::info!("Loaded {} biquad filters from {:?}", biquads.len(), path);
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                                "Loaded {} filters from: {}",
                                biquads.len(),
                                path.display()
                            )));
                        });
                        cx.notify();
                    }
                    Err(e) => {
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                                "Failed to parse EQ file: {}",
                                e
                            )));
                        });
                        cx.notify();
                    }
                }
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                        "Failed to load EQ: {}",
                        e
                    )));
                });
                cx.notify();
            }
        }
    }

    /// Save current headphone EQ result to file in selected format
    pub fn save_headphone_eq(&mut self, cx: &mut Context<Self>) {
        let (result, export_format, save_name) = {
            let state = self.state.read(cx);
            (
                state.app.headphone_optimization_result.clone(),
                state.app.headphone_export_format.clone(),
                state.app.headphone_eq_save_name.clone(),
            )
        };

        let Some(result) = result else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message = Some(crate::app::ToastMessage::error(
                    "No optimization result to save",
                ));
            });
            cx.notify();
            return;
        };

        // Get EQ directory
        let Some(eq_dir) = sotf_audio_player::config::get_eq_dir() else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message = Some(crate::app::ToastMessage::error(
                    "Could not determine EQ directory",
                ));
            });
            cx.notify();
            return;
        };

        // Ensure directory exists
        if let Err(e) = std::fs::create_dir_all(&eq_dir) {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                    "Failed to create EQ directory: {}",
                    e
                )));
            });
            cx.notify();
            return;
        }

        // Generate filename - use custom name if provided, otherwise use timestamp
        let extension = crate::autoeq::get_export_extension(&export_format);
        let filename = if save_name.trim().is_empty() {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            format!("headphone_{}{}", timestamp, extension)
        } else {
            // Sanitize the name: replace invalid filename characters
            let sanitized_name: String = save_name
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
                .collect();
            format!("{}{}", sanitized_name.trim(), extension)
        };
        let output_path = eq_dir.join(&filename);

        // Convert biquads to Peq format for export functions
        let peq: autoeq_iir::Peq = result.biquads.iter().map(|b| (b.freq, b.clone())).collect();

        // Generate output content based on selected format
        let content = match export_format.as_str() {
            "apo" => {
                autoeq_iir::peq_format_apo("# Headphone EQ", &peq)
            }
            "rme-channel" => autoeq_iir::peq_format_rme_channel(&peq),
            "rme-room" => {
                autoeq_iir::peq_format_rme_room(&peq, &peq)
            }
            "aupreset" => {
                autoeq_iir::peq_format_aupreset(&peq, "Headphone EQ")
            }
            _ => {
                // Default to JSON
                match serde_json::to_string_pretty(&result.biquads) {
                    Ok(json) => json,
                    Err(e) => {
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                                "Failed to serialize: {}",
                                e
                            )));
                        });
                        cx.notify();
                        return;
                    }
                }
            }
        };

        match std::fs::write(&output_path, content) {
            Ok(_) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                        "Saved EQ to: {}",
                        output_path.display()
                    )));
                });
                cx.notify();
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                        "Failed to save EQ: {}",
                        e
                    )));
                });
                cx.notify();
            }
        }
    }

    /// Delete saved EQ file
    pub fn delete_headphone_eq(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::remove_file(&path) {
            Ok(_) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                        "Deleted EQ file: {}",
                        path.display()
                    )));
                });
                cx.notify();
            }
            Err(e) => {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                        "Failed to delete EQ: {}",
                        e
                    )));
                });
                cx.notify();
            }
        }
    }

    /// Apply the computed headphone EQ to the current playback chain
    pub fn apply_headphone_eq_to_playback(&mut self, cx: &mut Context<Self>) {
        let result = {
            let state = self.state.read(cx);
            state.app.headphone_optimization_result.clone()
        };

        let Some(result) = result else {
            self.state.update(cx, |state, _cx| {
                state.app.toast_message = Some(crate::app::ToastMessage::error(
                    "No optimization result to apply",
                ));
            });
            cx.notify();
            return;
        };

        // Convert biquads to EQ filter settings
        let filters: Vec<sotf_audio_player::EQFilter> = result
            .biquads
            .iter()
            .map(|b| sotf_audio_player::EQFilter::new(
                b.filter_type,
                b.freq,
                b.q,
                b.db_gain,
            ))
            .collect();

        // Add EQ plugin with these filters to the chain
        self.state.update(cx, |state, _cx| {
            // First remove any existing EQ plugin to avoid duplicates
            let plugins = state.app.plugin_chain.plugins();
            let hp_eq_idx = plugins.iter().position(|p| {
                matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ)
            });

            // Remove existing EQ if found (we'll add a new one)
            if let Some(idx) = hp_eq_idx {
                state.app.plugin_chain.remove_plugin(idx);
            }

            // Add new EQ plugin
            state.app.plugin_chain.add_plugin(&sotf_audio_player::PluginType::EQ);
            let plugin_count = state.app.plugin_chain.len();

            // Set the EQ settings on the newly added plugin
            if let Some(plugin) = state.app.plugin_chain.get_plugin_mut(plugin_count - 1) {
                plugin.settings = sotf_audio_player::PluginSettings::EQ {
                    filters,
                };
            }

            state.app.needs_plugin_update = true;
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Applied headphone EQ to playback",
            ));
        });
        cx.notify();
    }

    /// Clear the headphone EQ from the playback chain
    pub fn clear_headphone_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Find and remove EQ plugins
            let plugins = state.app.plugin_chain.plugins();
            let eq_indices: Vec<_> = plugins
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            // Remove in reverse order to maintain correct indices
            for idx in eq_indices.into_iter().rev() {
                state.app.plugin_chain.remove_plugin(idx);
            }

            state.app.needs_plugin_update = true;
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }
}

/// Run optimization in background task
async fn run_optimization_task(
    curve_path: String,
    target: String,
    target_custom_path: String,
    params: crate::optimization_params::OptimizationParams,
    export_format: String,
) -> Result<HeadphoneOptimizationResult, String> {
    // Run optimization on background thread (blocking operation)
    smol::unblock(move || {
        use std::path::PathBuf;

        // Load headphone curve from CSV file
        let input_curve = autoeq::read_curve_from_csv(&PathBuf::from(&curve_path))
            .map_err(|e| format!("Failed to read curve file: {}", e))?;

        // Load target curve
        let target_curve = load_target_curve(&target, &target_custom_path)?;

        // Create standard frequency grid (200 points, 20 Hz to 20 kHz)
        let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

        // Normalize and interpolate curves
        let input_curve_norm =
            autoeq::normalize_and_interpolate_response(&standard_freq, &input_curve);
        let target_curve_norm =
            autoeq::normalize_and_interpolate_response(&standard_freq, &target_curve);

        // Create deviation curve
        let deviation_curve = autoeq::Curve {
            freq: target_curve_norm.freq.clone(),
            spl: &target_curve_norm.spl - &input_curve_norm.spl,
        };

        // Setup optimization arguments from params
        let args = autoeq::Args {
            num_filters: params.num_filters,
            sample_rate: params.sample_rate as f64,
            loss: if params.loss == "headphone-flat" {
                autoeq::LossType::HeadphoneFlat
            } else {
                autoeq::LossType::HeadphoneScore
            },
            algo: params.algo.clone(),
            population: params.population,
            maxeval: params.maxeval,
            strategy: params.strategy.clone(),
            min_db: params.min_db,
            max_db: params.max_db,
            min_q: params.min_q,
            max_q: params.max_q,
            min_freq: params.min_freq,
            max_freq: params.max_freq,
            min_spacing_oct: params.min_spacing_oct,
            spacing_weight: params.spacing_weight,
            smooth: params.smooth,
            smooth_n: params.smooth_n,
            refine: params.refine,
            local_algo: params.local_algo.clone(),
            tolerance: params.tolerance,
            atolerance: params.abs_tolerance,
            recombination: params.de_cr,
            adaptive_weight_f: params.adaptive_weight_f,
            adaptive_weight_cr: params.adaptive_weight_cr,
            peq_model: match params.peq_model.as_str() {
                "hp-pk" => autoeq::cli::PeqModel::HpPk,
                "hp-pk-lp" => autoeq::cli::PeqModel::HpPkLp,
                "ls-pk" => autoeq::cli::PeqModel::LsPk,
                "ls-pk-hs" => autoeq::cli::PeqModel::LsPkHs,
                "free-pk-free" => autoeq::cli::PeqModel::FreePkFree,
                "free" => autoeq::cli::PeqModel::Free,
                _ => autoeq::cli::PeqModel::Pk,
            },
            curve: None,
            target: None,
            output: None,
            speaker: None,
            version: None,
            measurement: None,
            curve_name: params.curve_name.clone(),
            peq_model_list: false,
            algo_list: false,
            strategy_list: false,
            no_parallel: false,
            parallel_threads: 0,
            seed: None,
            qa: None,
            driver1: None,
            driver2: None,
            driver3: None,
            driver4: None,
            crossover_type: "linkwitzriley4".to_string(),
        };

        // Setup objective data
        let (objective_data, _use_cea) = autoeq::workflow::setup_objective_data(
            &args,
            &input_curve_norm,
            &target_curve_norm,
            &deviation_curve,
            &None,
        );

        // Run optimization with history tracking
        let history: Vec<(usize, f64)> = Vec::new();
        let history_ptr = std::sync::Arc::new(std::sync::Mutex::new(history));
        let history_callback = history_ptr.clone();

        let filter_params = autoeq::workflow::perform_optimization_with_callback(
            &args,
            &objective_data,
            Box::new(move |intermediate| {
                 if let Ok(mut h) = history_callback.lock() {
                     // Check fields of intermediate
                     h.push((intermediate.iter, intermediate.fun));
                 }
                autoeq::de::CallbackAction::Continue
            }),
        )
        .map_err(|e| format!("Optimization failed: {}", e))?;

        // Retrieve history
        let history = history_ptr.lock().map_err(|_| "Failed to lock history")?.clone();
        let initial_loss = history.first().map(|x| x.1).unwrap_or(0.0);
        let final_loss = history.last().map(|x| x.1).unwrap_or(0.0);

        // Convert to biquad filters (x2peq returns Vec<(f64, Biquad)>)
        let peq = autoeq::x2peq::x2peq(&filter_params, args.sample_rate, args.peq_model);
        // Extract just the biquads from the (frequency, biquad) tuples
        let biquads: Vec<autoeq_iir::Biquad> = peq.into_iter().map(|(_, b)| b).collect();

        // Calculate all the curves for visualization
        let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
        let input_curve_vec: Vec<f64> = input_curve_norm.spl.iter().copied().collect();
        let target_curve_vec: Vec<f64> = target_curve_norm.spl.iter().copied().collect();
        let deviation_curve_vec: Vec<f64> = deviation_curve.spl.iter().copied().collect();

        // Calculate combined filter response
        let filter_response: Vec<f64> = frequencies
            .iter()
            .map(|&freq| {
                biquads.iter().map(|b| b.log_result(freq)).sum()
            })
            .collect();

        // Calculate individual filter responses
        let individual_filter_responses: Vec<Vec<f64>> = biquads
            .iter()
            .map(|biquad| {
                frequencies
                    .iter()
                    .map(|&freq| biquad.log_result(freq))
                    .collect()
            })
            .collect();

        // Error = deviation - filter_response (what we still need to correct)
        let error_curve: Vec<f64> = deviation_curve_vec
            .iter()
            .zip(filter_response.iter())
            .map(|(d, f)| d - f)
            .collect();

        // Corrected response = input + filter_response
        let corrected_curve: Vec<f64> = input_curve_vec
            .iter()
            .zip(filter_response.iter())
            .map(|(i, f)| i + f)
            .collect();

        // Save to EQ directory
        let eq_dir = sotf_audio_player::config::get_eq_dir()
            .ok_or_else(|| "Could not determine EQ directory".to_string())?;

        // Ensure directory exists
        std::fs::create_dir_all(&eq_dir)
            .map_err(|e| format!("Failed to create EQ directory: {}", e))?;

        // Generate filename from target and timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Use a descriptive name based on target
        let target_name = match target.as_str() {
            "custom" => "custom",
            other => other,
        };

        // Get file extension for the selected format
        let extension = crate::autoeq::get_export_extension(&export_format);
        let filename = format!("headphone_{}_{}{}", target_name, timestamp, extension);
        let output_path = eq_dir.join(&filename);

        // Convert biquads to Peq format for export functions
        let peq: autoeq_iir::Peq = biquads.iter().map(|b| (b.freq, b.clone())).collect();

        // Generate output content based on selected format
        let content = match export_format.as_str() {
            "apo" => {
                let comment = format!("# Headphone EQ for {}", target_name);
                autoeq_iir::peq_format_apo(&comment, &peq)
            }
            "rme-channel" => autoeq_iir::peq_format_rme_channel(&peq),
            "rme-room" => {
                // For room EQ, use same filters for left and right
                autoeq_iir::peq_format_rme_room(&peq, &peq)
            }
            "aupreset" => {
                let name = format!("Headphone EQ - {}", target_name);
                autoeq_iir::peq_format_aupreset(&peq, &name)
            }
            _ => {
                // Default to JSON
                serde_json::to_string_pretty(&biquads)
                    .map_err(|e| format!("Failed to serialize biquads: {}", e))?
            }
        };

        std::fs::write(&output_path, content)
            .map_err(|e| format!("Failed to write EQ file: {}", e))?;

        Ok(HeadphoneOptimizationResult {
            biquads,
            frequencies,
            input_curve: input_curve_vec,
            target_curve: target_curve_vec,
            deviation_curve: deviation_curve_vec,
            filter_response,
            error_curve,
            corrected_curve,
            individual_filter_responses,
            output_path: output_path.display().to_string(),
            optimization_history: history,
            initial_loss,
            final_loss,
        })
    })
    .await
}

/// Load target curve from bundled data or custom file
fn load_target_curve(target: &str, custom_path: &str) -> Result<autoeq::Curve, String> {


    match target {
        "harman-over-ear-2018" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2018),
        "harman-over-ear-2015" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2015),
        "harman-over-ear-2013" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2013),
        "harman-in-ear-2019" => parse_csv_curve(target_curves::HARMAN_IN_EAR_2019),
        "custom" => {
            // Load from custom file path
            autoeq::read_curve_from_csv(&PathBuf::from(custom_path))
                .map_err(|e| format!("Failed to read custom target curve: {}", e))
        }
        _ => {
            // Load from custom file path
            autoeq::read_curve_from_csv(&PathBuf::from(custom_path))
                .map_err(|e| format!("A target curve is required for headphone: {}", e))
        }
    }
}

/// Parse a CSV string into a Curve
fn parse_csv_curve(csv_data: &str) -> Result<autoeq::Curve, String> {
    use ndarray::Array1;

    let mut freq = Vec::new();
    let mut spl = Vec::new();

    for (i, line) in csv_data.lines().enumerate() {
        // Skip header line
        if i == 0 && line.contains("frequency") {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let (Ok(f), Ok(s)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                freq.push(f);
                spl.push(s);
            }
        }
    }

    if freq.is_empty() {
        return Err("No valid data found in CSV".to_string());
    }

    Ok(autoeq::Curve {
        freq: Array1::from_vec(freq),
        spl: Array1::from_vec(spl),
    })
}
