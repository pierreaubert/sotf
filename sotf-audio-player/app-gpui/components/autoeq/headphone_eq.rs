//! Headphone EQ optimization and management - GPUI frontend
//!
//! This module provides GPUI-specific UI interactions for headphone EQ optimization.
//! The actual optimization logic is in sotf_audio_player::autoeq::headphone.

use crate::app::types::PluginUpdateType;
use crate::ui::PlayerView;
use gpui::*;
use std::path::PathBuf;

// Re-export the result type from the common library
pub use sotf_audio_player::autoeq::HeadphoneOptimizationResult;

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
            if state.app.headphone_target == "custom"
                && state.app.headphone_target_custom_path.is_empty()
            {
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

        // Clone values needed after the closure
        let target_for_save = target.clone();
        let export_format_for_save = export_format.clone();

        // Spawn background task for optimization
        cx.spawn(async move |_view, cx| {
            // Run optimization using the common library
            let result = smol::unblock(move || {
                sotf_audio_player::autoeq::run_headphone_optimization(
                    &curve_path,
                    &target,
                    &target_custom_path,
                    &params,
                    &export_format,
                )
            })
            .await;

            match result {
                Ok(mut optimization_result) => {
                    // Save to EQ directory
                    if let Some(eq_dir) = sotf_audio_player::config::get_eq_dir() {
                        let _ = std::fs::create_dir_all(&eq_dir);
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let target_name = if target_for_save == "custom" {
                            "custom"
                        } else {
                            &target_for_save
                        };
                        let extension =
                            sotf_audio_player::autoeq::get_export_extension(&export_format_for_save);
                        let filename =
                            format!("headphone_{}_{}{}", target_name, timestamp, extension);
                        let output_path = eq_dir.join(&filename);

                        // Convert biquads to Peq format for export functions
                        let peq: autoeq_iir::Peq = optimization_result
                            .biquads
                            .iter()
                            .map(|b| (b.freq, b.clone()))
                            .collect();

                        // Generate output content based on selected format
                        let content = match export_format_for_save.as_str() {
                            "apo" => {
                                let comment = format!("# Headphone EQ for {}", target_name);
                                autoeq_iir::peq_format_apo(&comment, &peq)
                            }
                            "rme-channel" => autoeq_iir::peq_format_rme_channel(&peq),
                            "rme-room" => autoeq_iir::peq_format_rme_room(&peq, &peq),
                            "aupreset" => {
                                let name = format!("Headphone EQ - {}", target_name);
                                autoeq_iir::peq_format_aupreset(&peq, &name)
                            }
                            _ => serde_json::to_string_pretty(&optimization_result.biquads)
                                .unwrap_or_default(),
                        };

                        if std::fs::write(&output_path, content).is_ok() {
                            optimization_result.output_path = output_path.display().to_string();
                        }
                    }

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
                            state.app.toast_message =
                                Some(crate::app::ToastMessage::success(format!(
                                    "Loaded {} filters from: {}",
                                    biquads.len(),
                                    path.display()
                                )));
                        });
                        cx.notify();
                    }
                    Err(e) => {
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to parse EQ file: {}", e),
                            ));
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
        let extension = sotf_audio_player::autoeq::get_export_extension(&export_format);
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
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            format!("{}{}", sanitized_name.trim(), extension)
        };
        let output_path = eq_dir.join(&filename);

        // Convert biquads to Peq format for export functions
        let peq: autoeq_iir::Peq = result.biquads.iter().map(|b| (b.freq, b.clone())).collect();

        // Generate output content based on selected format
        let content = match export_format.as_str() {
            "apo" => autoeq_iir::peq_format_apo("# Headphone EQ", &peq),
            "rme-channel" => autoeq_iir::peq_format_rme_channel(&peq),
            "rme-room" => autoeq_iir::peq_format_rme_room(&peq, &peq),
            "aupreset" => autoeq_iir::peq_format_aupreset(&peq, "Headphone EQ"),
            _ => {
                // Default to JSON
                match serde_json::to_string_pretty(&result.biquads) {
                    Ok(json) => json,
                    Err(e) => {
                        self.state.update(cx, |state, _cx| {
                            state.app.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to serialize: {}", e),
                            ));
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
            .map(|b| sotf_audio_player::EQFilter::new(b.filter_type, b.freq, b.q, b.db_gain))
            .collect();

        // Add EQ plugin with these filters to the chain
        self.state.update(cx, |state, _cx| {
            // First remove any existing EQ plugin to avoid duplicates
            let plugins = state.app.plugin_chain.plugins();
            let hp_eq_idx = plugins
                .iter()
                .position(|p| matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ));

            // Remove existing EQ if found (we'll add a new one)
            if let Some(idx) = hp_eq_idx {
                state.app.plugin_chain.remove_plugin(idx);
            }

            // Add new EQ plugin
            state
                .app
                .plugin_chain
                .add_plugin(&sotf_audio_player::PluginType::EQ);
            let plugin_count = state.app.plugin_chain.len();

            // Set the EQ settings on the newly added plugin
            if let Some(plugin) = state.app.plugin_chain.get_plugin_mut(plugin_count - 1) {
                plugin.settings = sotf_audio_player::PluginSettings::EQ { filters };
            }

            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
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

            state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }
}
