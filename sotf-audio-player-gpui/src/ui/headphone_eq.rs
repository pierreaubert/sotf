//! Headphone EQ optimization and management

use crate::app::AppState;
use crate::ui::PlayerView;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

impl PlayerView {
    /// Open file dialog to select headphone measurement file
    pub fn browse_headphone_curve(&mut self, cx: &mut Context<Self>) {
        // Use rfd for native file dialog
        let file_dialog = rfd::FileDialog::new()
            .add_filter("CSV Files", &["csv"])
            .add_filter("All Files", &["*"])
            .set_title("Select Headphone Measurement File");

        if let Some(path) = file_dialog.pick_file() {
            let path_str = path.display().to_string();
            self.state.update(cx, |state, _cx| {
                state.app.headphone_curve_path = path_str;
            });
            cx.notify();
        }
    }

    /// Run headphone EQ optimization
    pub fn run_headphone_optimization(&mut self, cx: &mut Context<Self>) {
        let (curve_path, target, params) = {
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

            (
                state.app.headphone_curve_path.clone(),
                state.app.headphone_target.clone(),
                state.app.headphone_params.clone(),
            )
        };

        // Mark optimization as running
        self.state.update(cx, |state, _cx| {
            state.app.headphone_optimization_running = true;
            state.app.headphone_optimization_progress.clear();
        });
        cx.notify();

        // Clone state for background task
        let state_clone = self.state.clone();

        // Spawn background task for optimization
        cx.spawn(async move |_view, cx| {
            match run_optimization_task(curve_path, target, params).await {
                Ok(result_path) => {
                    // Update state with success
                    let _ = state_clone.update(cx, |state, _cx| {
                        state.app.headphone_optimization_running = false;
                        state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                            "EQ optimization complete! Saved to: {}",
                            result_path
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
                // Parse the EQ file (format TBD - could be array of biquad filters)
                // For now, just show success
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::success(format!(
                        "Loaded EQ from: {}",
                        path.display()
                    )));
                });
                cx.notify();
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
}

/// Run optimization in background task
async fn run_optimization_task(
    curve_path: String,
    target: String,
    params: crate::optimization_params::OptimizationParams,
) -> Result<String, String> {
    // Run optimization on background thread (blocking operation)
    smol::unblock(move || {
        use std::path::PathBuf;

        // Load headphone curve from CSV file
        let input_curve = autoeq::read_curve_from_csv(&PathBuf::from(&curve_path))
            .map_err(|e| format!("Failed to read curve file: {}", e))?;

        // Load target curve from bundled data
        let target_curve = load_target_curve(&target)?;

        // Create standard frequency grid
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

        // Run optimization with callback
        // TODO: Add progress updates via channel or shared state
        let filter_params = autoeq::workflow::perform_optimization_with_callback(
            &args,
            &objective_data,
            Box::new(move |_intermediate| {
                // Progress updates would go here
                autoeq::de::CallbackAction::Continue
            }),
        )
        .map_err(|e| format!("Optimization failed: {}", e))?;

        // Convert to biquad filters
        let biquads = autoeq::x2peq::x2peq(&filter_params, args.sample_rate, args.peq_model);

        // Save to EQ directory
        let eq_dir = sotf_audio_player::config::get_eq_dir()
            .ok_or_else(|| "Could not determine EQ directory".to_string())?;

        // Generate filename from target and timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("{}_{}.json", target, timestamp);
        let output_path = eq_dir.join(&filename);

        // Save biquads as JSON
        let json = serde_json::to_string_pretty(&biquads)
            .map_err(|e| format!("Failed to serialize biquads: {}", e))?;
        std::fs::write(&output_path, json)
            .map_err(|e| format!("Failed to write EQ file: {}", e))?;

        Ok(output_path.display().to_string())
    })
    .await
}

/// Load target curve from bundled data
fn load_target_curve(_target: &str) -> Result<autoeq::Curve, String> {
    use ndarray::Array1;

    // For now, return a flat target at 0 dB
    // TODO: Bundle actual Harman target curves
    let freq: Vec<f64> = (20..=20000).step_by(10).map(|f| f as f64).collect();
    let spl: Vec<f64> = vec![0.0; freq.len()];

    Ok(autoeq::Curve {
        freq: Array1::from_vec(freq),
        spl: Array1::from_vec(spl),
    })
}
