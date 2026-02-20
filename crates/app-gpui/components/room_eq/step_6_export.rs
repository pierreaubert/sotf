use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonVariant, Card, StackSpacing, Text, TextSize, TextWeight, VStack};

impl PlayerView {
    pub(crate) fn render_room_eq_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let has_eq_in_rack = state
            .app
            .plugin_state
            .plugin_chain
            .find_plugin_index(&sotf_audio_player::PluginType::EQ)
            .is_some();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Export & Apply")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Export the DSP chain or apply directly to the player.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(Text::new("Backup Current Rack").color(theme.text_primary).weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(
                                    "Save a copy of your current plugin rack before applying changes.",
                                )
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("backup_rack", "Save Rack Backup...")
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.save_rack_backup(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(Text::new("Export Options").color(theme.text_primary).weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new("export_json", "Export as JSON")
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.export_room_eq_json(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(Text::new("Apply to Player").color(theme.text_primary).weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(if has_eq_in_rack {
                                    "An EQ plugin exists in your rack. It will be updated with the new filters."
                                } else {
                                    "No EQ plugin in rack. A new EQ will be added at the end of the processing chain."
                                })
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("apply_to_player", "Apply to Player")
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.apply_room_eq_to_player(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
    }

    fn export_room_eq_json(&mut self, cx: &mut Context<Self>) {
        // Get the DSP output from state
        let dsp_output = {
            let state = self.state.read(cx);
            state.app.measurement_state.room_eq_state.dsp_output.clone()
        };

        let Some(dsp_output) = dsp_output else {
            log::warn!("No DSP output to export");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No optimization results to export".to_string());
            });
            return;
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Export Room EQ Configuration")
                .set_file_name("room_eq.json")
                .save_file()
                .await;

            if let Some(file) = file {
                // Serialize DSP output
                match serde_json::to_string_pretty(&dsp_output) {
                    Ok(json) => {
                        // Write to file
                        match std::fs::write(file.path(), &json) {
                            Ok(()) => {
                                log::info!("Exported room EQ config to {:?}", file.path());
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.status_message =
                                        format!("Saved to {}", file.path().display());
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to write room EQ file: {}", e);
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.error_message =
                                        Some(format!("Failed to write: {}", e));
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize room EQ JSON: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.measurement_state.room_eq_state.error_message =
                                Some(format!("Failed to serialize: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn save_rack_backup(&mut self, cx: &mut Context<Self>) {
        // Get the current plugin chain
        let plugin_chain = {
            let state = self.state.read(cx);
            state.app.plugin_state.plugin_chain.clone()
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Generate default filename with timestamp
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let default_name = format!("rack_backup_{}.json", timestamp);

            // Open save file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Save Rack Backup")
                .set_file_name(&default_name)
                .save_file()
                .await;

            if let Some(file) = file {
                let file_path = file.path().to_path_buf();
                let parent_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("backup.json");

                match plugin_chain.save_to_file(parent_dir, filename) {
                    Ok(()) => {
                        log::info!("Saved rack backup to {:?}", file_path);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.measurement_state.room_eq_state.status_message =
                                format!("Backup saved to {}", file_path.display());
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::success("Rack backup saved"));
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to save rack backup: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.measurement_state.room_eq_state.error_message =
                                Some(format!("Failed to save backup: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn apply_room_eq_to_player(&mut self, cx: &mut Context<Self>) {
        use math_audio_iir_fir::BiquadFilterType;
        use sotf_audio_player::{EQFilter, PluginSettings, PluginType};

        // Get the DSP output from state
        let dsp_output = {
            let state = self.state.read(cx);
            state.app.measurement_state.room_eq_state.dsp_output.clone()
        };

        let Some(dsp_output) = dsp_output else {
            log::warn!("No DSP output to apply");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No optimization results to apply".to_string());
            });
            return;
        };

        // Helper to parse filters from JSON
        let parse_filters = |filters_json: &[serde_json::Value]| -> Vec<EQFilter> {
            filters_json
                .iter()
                .map(|filter| {
                    let filter_type_str = filter
                        .get("filter_type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("peak");
                    let filter_type = match filter_type_str.to_lowercase().as_str() {
                        "peak" | "pk" => BiquadFilterType::Peak,
                        "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                        "highshelf" | "hs" => BiquadFilterType::Highshelf,
                        "lowpass" | "lp" => BiquadFilterType::Lowpass,
                        "highpass" | "hp" => BiquadFilterType::Highpass,
                        "notch" => BiquadFilterType::Notch,
                        _ => BiquadFilterType::Peak,
                    };
                    let frequency = filter
                        .get("frequency")
                        .and_then(|f| f.as_f64())
                        .unwrap_or(1000.0);
                    let q = filter.get("q").and_then(|q| q.as_f64()).unwrap_or(1.0);
                    let gain_db = filter
                        .get("gain_db")
                        .and_then(|g| g.as_f64())
                        .unwrap_or(0.0);
                    EQFilter::new(filter_type, frequency, q, gain_db)
                })
                .collect()
        };

        // Collect EQ filters per channel for proper per-channel room correction
        // Sort channel names to ensure consistent ordering (L, R, C, etc.)
        let mut channel_names: Vec<_> = dsp_output.channels.keys().cloned().collect();
        channel_names.sort();

        let mut per_channel_filters: Vec<Vec<EQFilter>> = Vec::new();
        for channel_name in &channel_names {
            if let Some(channel_dsp) = dsp_output.channels.get(channel_name) {
                let mut channel_eq_filters: Vec<EQFilter> = Vec::new();
                for plugin in &channel_dsp.plugins {
                    if plugin.plugin_type == "EQ" {
                        if let Some(filters) =
                            plugin.parameters.get("filters").and_then(|f| f.as_array())
                        {
                            channel_eq_filters.extend(parse_filters(filters));
                        }
                    }
                }
                per_channel_filters.push(channel_eq_filters);
            }
        }

        // Check if we have any filters at all
        let total_filters: usize = per_channel_filters.iter().map(|f| f.len()).sum();
        if total_filters == 0 {
            log::warn!("No EQ filters found in optimization results");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No EQ filters found in optimization results".to_string());
            });
            return;
        }

        let num_channels = per_channel_filters.len();
        // Use first channel's filters as the global fallback
        let global_filters = per_channel_filters.first().cloned().unwrap_or_default();

        log::info!(
            "Applying room EQ with {} channels, {} total filters (per-channel mode)",
            num_channels,
            total_filters
        );

        // Update the plugin chain
        self.state.update(cx, |state, _| {
            let plugin_chain = &mut state.app.plugin_state.plugin_chain;

            // Check if there's an existing EQ plugin
            if let Some(eq_idx) = plugin_chain.find_plugin_index(&PluginType::EQ) {
                // Update existing EQ plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(eq_idx) {
                    eq_plugin.settings = PluginSettings::EQ {
                        channels: num_channels,
                        filters: global_filters.clone(),
                        channel_filters: Some(per_channel_filters.clone()),
                        per_channel_mode: true,
                        max_filters: 10,
                    };
                    log::info!(
                        "Updated existing EQ plugin at index {} with per-channel room EQ",
                        eq_idx
                    );
                }
            } else {
                // No EQ plugin exists, add one at the end before Matrix and Output Monitor
                let insert_idx = plugin_chain.user_plugin_insert_index();
                plugin_chain.insert_plugin(insert_idx, &PluginType::EQ);

                // Configure the newly inserted plugin with per-channel room EQ
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(insert_idx) {
                    eq_plugin.settings = PluginSettings::EQ {
                        channels: num_channels,
                        filters: global_filters.clone(),
                        channel_filters: Some(per_channel_filters.clone()),
                        per_channel_mode: true,
                        max_filters: 10,
                    };
                }
                log::info!(
                    "Inserted new EQ plugin at index {} with per-channel room EQ",
                    insert_idx
                );
            }

            // Mark that plugin chain was modified and needs sync
            state.app.plugin_state.plugin_chain_modified = true;
            state.app.plugin_state.pending_plugin_update =
                Some(crate::app::types::PluginUpdateType::Structural);
            state.app.measurement_state.room_eq_state.status_message =
                "Room EQ applied to player!".to_string();
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                "Room EQ applied successfully",
            ));
        });

        cx.notify();
    }
}
