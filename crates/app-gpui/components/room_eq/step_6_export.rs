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
            .chain
            .find_plugin_index(&sotf_audio_player::PluginType::EQ)
            .is_some();
        let is_rack_compatible = state
            .app
            .measurement_state
            .room_eq_state
            .dsp_output
            .as_ref()
            .map_or(true, |dsp| dsp.is_rack_compatible());

        let mut stack = VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Export & Apply")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new("Export the DSP chain or apply directly to the player.")
                    .size(TextSize::Xs)
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
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new(
                                    "Save a copy of your current plugin rack before applying changes.",
                                )
                                .size(TextSize::Xs)
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
                            .spacing(StackSpacing::Sm)
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
            );

        if is_rack_compatible {
            // Simple case: EQ (+ optional delay/gain) — apply to linear rack
            stack = stack.child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(Text::new("Apply to Rack").color(theme.text_primary).weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new(if has_eq_in_rack {
                                    "An EQ plugin exists in your rack. It will be updated with the new filters."
                                } else {
                                    "No EQ plugin in rack. A new EQ will be added at the end of the processing chain."
                                })
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("apply_to_player", "Apply to Rack")
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
            );
        } else {
            // Complex case: multi-driver crossovers — requires graph
            stack = stack.child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(Text::new("Apply as Graph").color(theme.text_primary).weight(TextWeight::Semibold))
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new(
                                    "This optimization includes crossovers and per-driver processing \
                                     that requires the full graph view."
                                )
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("apply_as_graph", "Apply as Graph")
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.apply_room_eq_as_graph(cx);
                                        }),
                                    ),
                            ),
                    ),
            );
        }

        stack
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

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
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
                                    state_entity.update(cx, |state, _| {
                                        state.app.measurement_state.room_eq_state.status_message =
                                            format!("Saved to {}", file.path().display());
                                    });
                                }
                                Err(e) => {
                                    log::error!("Failed to write room EQ file: {}", e);
                                    state_entity.update(cx, |state, _| {
                                        state.app.measurement_state.room_eq_state.error_message =
                                            Some(format!("Failed to write: {}", e));
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to serialize room EQ JSON: {}", e);
                            state_entity.update(cx, |state, _| {
                                state.app.measurement_state.room_eq_state.error_message =
                                    Some(format!("Failed to serialize: {}", e));
                            });
                        }
                    }
                }
            })
            .detach();
        }
    }

    fn save_rack_backup(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            // Get the current plugin chain
            let plugin_chain = {
                let state = self.state.read(cx);
                state.app.plugin_state.chain.clone()
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
                            state_entity.update(cx, |state, _| {
                                state.app.measurement_state.room_eq_state.status_message =
                                    format!("Backup saved to {}", file_path.display());
                                state.app.ui_state.toast_message =
                                    Some(crate::app::ToastMessage::success("Rack backup saved"));
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to save rack backup: {}", e);
                            state_entity.update(cx, |state, _| {
                                state.app.measurement_state.room_eq_state.error_message =
                                    Some(format!("Failed to save backup: {}", e));
                            });
                        }
                    }
                }
            })
            .detach();
        }
    }

    fn apply_room_eq_to_player(&mut self, cx: &mut Context<Self>) {
        use math_audio_iir_fir::BiquadFilterType;
        use sotf_audio_player::{EQFilter, PluginSettings, PluginType};

        // Get the DSP output and channel results from state.
        // channel_results preserves the output channel order (0=FL, 1=FR, 2=C, etc.)
        // from the recording config — we MUST use this order, not alphabetical sort,
        // because the EQ plugin maps channel_filters[i] to audio channel i.
        let (dsp_output, channel_result_names) = {
            let state = self.state.read(cx);
            let names: Vec<String> = state
                .app
                .measurement_state
                .room_eq_state
                .channel_results
                .iter()
                .map(|r| r.channel_name.clone())
                .collect();
            (
                state.app.measurement_state.room_eq_state.dsp_output.clone(),
                names,
            )
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

        // Collect EQ filters per channel in output channel order.
        // channel_result_names preserves the order from the recording config
        // (0=FL, 1=FR, 2=C, 3=LFE, 4=SL, 5=SR for 5.1).
        let mut per_channel_filters: Vec<Vec<EQFilter>> = Vec::new();
        for channel_name in &channel_result_names {
            if let Some(channel_dsp) = dsp_output.channels.get(channel_name) {
                let mut channel_eq_filters: Vec<EQFilter> = Vec::new();
                for plugin in &channel_dsp.plugins {
                    if plugin.plugin_type == "EQ"
                        && let Some(filters) =
                            plugin.parameters.get("filters").and_then(|f| f.as_array())
                    {
                        channel_eq_filters.extend(parse_filters(filters));
                    }
                }
                log::info!(
                    "Channel '{}': {} EQ filters",
                    channel_name,
                    channel_eq_filters.len()
                );
                per_channel_filters.push(channel_eq_filters);
            } else {
                // Channel has no DSP output (e.g., optimization skipped it)
                log::info!("Channel '{}': no DSP output, using empty filters", channel_name);
                per_channel_filters.push(Vec::new());
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
            let plugin_chain = &mut state.app.plugin_state.chain;

            // Check if there's an existing EQ plugin
            if let Some(eq_idx) = plugin_chain.find_plugin_index(&PluginType::EQ) {
                // Update existing EQ plugin
                if let Some(eq_plugin) = plugin_chain.get_plugin_mut(eq_idx) {
                    eq_plugin.settings = PluginSettings::EQ {
                        channels: num_channels,
                        filters: global_filters.clone(),
                        channel_filters: Some(per_channel_filters.clone()),
                        per_channel_mode: true,
                        max_filters: 10, tdf2: false,
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
                        max_filters: 10, tdf2: false,
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

    /// Apply roomeq results as a graph (for multi-driver crossover setups).
    ///
    /// Builds a `PluginGraphConfig` from the per-channel DSP chains (including
    /// per-driver crossover, gain, delay, and global EQ) and sends it to the
    /// engine via the graph update API.
    fn apply_room_eq_as_graph(&mut self, cx: &mut Context<Self>) {
        use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

        let dsp_output = {
            let state = self.state.read(cx);
            state
                .app
                .measurement_state
                .room_eq_state
                .dsp_output
                .clone()
        };

        let Some(dsp_output) = dsp_output else {
            log::warn!("No DSP output to apply as graph");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No optimization results to apply".to_string());
            });
            return;
        };

        // Build graph: nodes + edges from the multi-channel, potentially multi-driver chains.
        //
        // For each channel's ChannelDspChain:
        //   - If no drivers: chain global plugins linearly
        //   - If drivers: each driver's plugins are chained, then all driver outputs
        //     feed into the global EQ plugins
        //
        // All channels share the same graph since they're processed as interleaved audio.
        // For per-channel processing, we rely on the EQ plugin's per-channel mode
        // and individual crossover/gain/delay plugins processing all channels.
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut next_id: usize = 0;
        let mut prev_node_id: Option<usize> = None;

        // Flatten all channels into a single graph. For multi-driver, we process
        // the first channel with drivers (they all share the same crossover topology).
        let representative_chain = dsp_output
            .channels
            .values()
            .find(|ch| ch.drivers.is_some())
            .or_else(|| dsp_output.channels.values().next());

        if let Some(chain) = representative_chain {
            if let Some(drivers) = &chain.drivers {
                // Multi-driver: add per-driver plugin chains
                let mut driver_last_ids = Vec::new();

                for driver in drivers {
                    let mut driver_prev: Option<usize> = prev_node_id;

                    for plugin in &driver.plugins {
                        let id = next_id;
                        next_id += 1;
                        nodes.push(PluginGraphNodeConfig {
                            id,
                            plugin_type: plugin.plugin_type.clone(),
                            parameters: plugin.parameters.clone(),
                            input_channels: 2, // Default stereo
                        });
                        if let Some(prev) = driver_prev {
                            edges.push(PluginGraphEdgeConfig {
                                from_node: prev,
                                to_node: id,
                            });
                        }
                        driver_prev = Some(id);
                    }

                    if let Some(last) = driver_prev {
                        driver_last_ids.push(last);
                    }
                }

                // Global plugins after drivers (e.g., global EQ)
                let mut global_prev: Option<usize> = None;
                for plugin in &chain.plugins {
                    let id = next_id;
                    next_id += 1;
                    nodes.push(PluginGraphNodeConfig {
                        id,
                        plugin_type: plugin.plugin_type.clone(),
                        parameters: plugin.parameters.clone(),
                        input_channels: 2,
                    });

                    // Connect all driver outputs to the first global plugin
                    if global_prev.is_none() {
                        for &driver_last in &driver_last_ids {
                            edges.push(PluginGraphEdgeConfig {
                                from_node: driver_last,
                                to_node: id,
                            });
                        }
                    }

                    if let Some(prev) = global_prev {
                        edges.push(PluginGraphEdgeConfig {
                            from_node: prev,
                            to_node: id,
                        });
                    }
                    global_prev = Some(id);
                }
            } else {
                // No drivers — just chain the plugins linearly
                for plugin in &chain.plugins {
                    let id = next_id;
                    next_id += 1;
                    nodes.push(PluginGraphNodeConfig {
                        id,
                        plugin_type: plugin.plugin_type.clone(),
                        parameters: plugin.parameters.clone(),
                        input_channels: 2,
                    });
                    if let Some(prev) = prev_node_id {
                        edges.push(PluginGraphEdgeConfig {
                            from_node: prev,
                            to_node: id,
                        });
                    }
                    prev_node_id = Some(id);
                }
            }
        }

        if nodes.is_empty() {
            log::warn!("No graph nodes to apply");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No plugins in DSP output".to_string());
            });
            return;
        }

        let graph_config = PluginGraphConfig { nodes, edges };

        log::info!(
            "Applying room EQ as graph: {} nodes, {} edges",
            graph_config.nodes.len(),
            graph_config.edges.len()
        );

        // Send the graph config to the engine
        self.state.update(cx, |state, _| {
            match state.player.lock().update_plugin_graph(graph_config) {
                Ok(()) => {
                    // Switch to graph view mode
                    state.app.plugin_state.plugin_view_mode =
                        crate::app::state::plugin::PluginViewMode::Graph;
                    state.app.measurement_state.room_eq_state.status_message =
                        "Room EQ applied as graph!".to_string();
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                        "Room EQ graph applied successfully",
                    ));
                }
                Err(e) => {
                    log::error!("Failed to apply room EQ graph: {}", e);
                    state.app.measurement_state.room_eq_state.error_message =
                        Some(format!("Failed to apply graph: {}", e));
                }
            }
        });

        cx.notify();
    }
}
