use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};
use sotf_audio_player::room_eq_types::DspChainOutputExt;

/// Export format options shown in the dropdown. Index 0 is the native
/// SotF JSON; indices 1–6 map to `autoeq::roomeq::ExportFormat` variants.
const EXPORT_FORMATS: &[(&str, &str)] = &[
    ("SotF JSON", "json"),
    ("CamillaDSP", "yaml"),
    ("Equalizer APO", "txt"),
    ("EasyEffects", "json"),
    ("Wavelet / GraphicEQ", "txt"),
    ("PipeWire filter-chain", "conf"),
    ("Roon DSP", "json"),
];

impl PlayerView {
    pub(crate) fn render_room_eq_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let has_eq_in_rack = state
            .app
            .plugin_state
            .graph
            .find_plugin_index(&sotf_audio_player::PluginType::EQ)
            .is_some();
        let is_rack_compatible = state
            .app
            .measurement_state
            .room_eq_state
            .dsp_output
            .as_ref()
            .is_none_or(|dsp| dsp.is_rack_compatible());
        let selected_format_idx = state
            .app
            .measurement_state
            .room_eq_state
            .export_format_index;
        let format_dropdown_open = state
            .app
            .measurement_state
            .room_eq_state
            .dropdowns
            .export_format_open;

        let (format_name, format_ext) = EXPORT_FORMATS
            .get(selected_format_idx)
            .copied()
            .unwrap_or(EXPORT_FORMATS[0]);

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
                    .header(
                        Text::new("Backup Current Rack")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
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
            // ── Export Options with format selector ──────────────────
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Export Options")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let mut export_content = VStack::new().spacing(StackSpacing::Sm);

                        // Format selector — click to toggle dropdown
                        export_content = export_content.child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new("Format:")
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    div()
                                        .id("export-format-selector")
                                        .px(d.pad_x)
                                        .py(px(6.0)) // intentional: between pad_y_half (4) and pad_y (8), matches rounded(6) selector pill
                                        .rounded(d.r_md)
                                        .border_1()
                                        .border_color(theme.accent)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.surface_hover))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|view, _, _, cx| {
                                                view.state.update(cx, |state, _| {
                                                    let open = &mut state
                                                        .app
                                                        .measurement_state
                                                        .room_eq_state
                                                        .dropdowns
                                                        .export_format_open;
                                                    *open = !*open;
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .child(
                                            Text::new(format!("{} (.{})", format_name, format_ext))
                                                .size(TextSize::Xs)
                                                .weight(TextWeight::Semibold)
                                                .color(theme.text_primary),
                                        ),
                                )
                                .child(
                                    Button::new("export_file", "Export...")
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _, _, cx| {
                                                view.export_room_eq_format(cx);
                                            }),
                                        ),
                                ),
                        );

                        // Dropdown list (visible when open)
                        if format_dropdown_open {
                            export_content = export_content.child(
                                div()
                                    .id("export-format-dropdown")
                                    .w(px(300.0)) // intentional: dropdown layout width
                                    .max_h(px(250.0)) // intentional: dropdown layout height
                                    .overflow_y_scroll()
                                    .bg(theme.surface)
                                    .rounded(d.r_md)
                                    .border_1()
                                    .border_color(theme.border)
                                    .children(EXPORT_FORMATS.iter().enumerate().map(
                                        |(i, (name, ext))| {
                                            let is_selected = i == selected_format_idx;
                                            let name = name.to_string();
                                            let ext = ext.to_string();
                                            div()
                                                .id(SharedString::from(format!(
                                                    "export-fmt-{}",
                                                    i
                                                )))
                                                .px(px(10.0)) // intentional: compact dropdown item padding, no matching token
                                                .py(px(5.0)) // intentional: compact dropdown item padding, no matching token
                                                .cursor_pointer()
                                                .bg(if is_selected {
                                                    theme.accent_muted
                                                } else {
                                                    theme.surface
                                                })
                                                .hover(|s| s.bg(theme.surface_hover))
                                                .on_mouse_down(MouseButton::Left, {
                                                    cx.listener(move |view, _, _, cx| {
                                                        view.state.update(cx, |state, _| {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .export_format_index = i;
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .dropdowns
                                                                .export_format_open = false;
                                                        });
                                                        cx.notify();
                                                    })
                                                })
                                                .child(
                                                    Text::new(format!("{} (.{})", name, ext))
                                                        .size(TextSize::Xs)
                                                        .color(if is_selected {
                                                            theme.accent
                                                        } else {
                                                            theme.text_primary
                                                        }),
                                                )
                                        },
                                    )),
                            );
                        }

                        export_content
                    }),
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

    /// Export DSP chain in the user-selected format (JSON, CamillaDSP,
    /// APO, EasyEffects, Wavelet, PipeWire, Roon). Index 0 = SotF JSON
    /// (pretty-printed DspChainOutput); indices 1–6 delegate to
    /// `autoeq::roomeq::export::export_dsp_chain`.
    fn export_room_eq_format(&mut self, cx: &mut Context<Self>) {
        let (dsp_output, format_idx) = {
            let state = self.state.read(cx);
            (
                state.app.measurement_state.room_eq_state.dsp_output.clone(),
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .export_format_index,
            )
        };

        let Some(dsp_output) = dsp_output else {
            log::warn!("No DSP output to export");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No optimization results to export".to_string());
            });
            return;
        };

        if format_idx == 0 {
            // SotF JSON — use existing JSON export path
            self.export_room_eq_json(cx);
            return;
        }

        // External format via autoeq::roomeq::export
        let format = match format_idx {
            1 => autoeq::roomeq::ExportFormat::CamillaDsp,
            2 => autoeq::roomeq::ExportFormat::EqualizerApo,
            3 => autoeq::roomeq::ExportFormat::EasyEffects,
            4 => autoeq::roomeq::ExportFormat::Wavelet,
            5 => autoeq::roomeq::ExportFormat::PipeWire,
            6 => autoeq::roomeq::ExportFormat::RoonDsp,
            _ => {
                log::warn!("Unknown export format index {}", format_idx);
                return;
            }
        };
        let ext = format.default_extension();
        let (format_name, _) = EXPORT_FORMATS
            .get(format_idx)
            .copied()
            .unwrap_or(("Unknown", "bin"));

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let state_entity = self.state.clone();
            // Convert DspChainOutput (player type) to the autoeq output
            // type expected by export_dsp_chain. They're the same shape
            // — the player re-exports from autoeq. Use serde round-trip
            // for type conversion since they may be different crate types.
            let dsp_json = match serde_json::to_string(&dsp_output) {
                Ok(j) => j,
                Err(e) => {
                    log::error!("Failed to serialize DSP output: {}", e);
                    return;
                }
            };

            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter(format_name, &[ext])
                    .set_title(format!("Export Room EQ — {}", format_name))
                    .set_file_name(format!("room_eq.{}", ext))
                    .save_file()
                    .await;

                if let Some(file) = file {
                    // Parse back into autoeq DspChainOutput
                    match serde_json::from_str::<autoeq::roomeq::DspChainOutput>(&dsp_json) {
                        Ok(autoeq_output) => {
                            match autoeq::roomeq::export_dsp_chain(
                                &autoeq_output,
                                format,
                                file.path(),
                                48000.0,
                            ) {
                                Ok(()) => {
                                    log::info!(
                                        "Exported room EQ as {} to {:?}",
                                        format_name,
                                        file.path()
                                    );
                                    state_entity.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .room_eq_state
                                            .status_message = format!(
                                            "Exported {} to {}",
                                            format_name,
                                            file.path().display()
                                        );
                                    });
                                }
                                Err(e) => {
                                    log::error!("Export failed: {}", e);
                                    state_entity.update(cx, |state, _| {
                                        state.app.measurement_state.room_eq_state.error_message =
                                            Some(format!("Export failed: {}", e));
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to parse DSP output for export: {}", e);
                            state_entity.update(cx, |state, _| {
                                state.app.measurement_state.room_eq_state.error_message =
                                    Some(format!("Internal error: {}", e));
                            });
                        }
                    }
                }
            })
            .detach();
        }
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
            // Get the current plugin graph
            let plugin_graph = {
                let state = self.state.read(cx);
                state.app.plugin_state.graph.clone()
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

                    match plugin_graph.save_to_file(parent_dir, filename) {
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
        use sotf_audio_player::EQFilter;

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
                state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                    "No optimization results to apply. Run the optimizer first.",
                ));
            });
            return;
        };

        // Collect EQ filters per channel in output channel order.
        // channel_result_names preserves the order from the recording config
        // (0=FL, 1=FR, 2=C, 3=LFE, 4=SL, 5=SR for 5.1).
        let mut per_channel_filters: Vec<Vec<EQFilter>> = Vec::new();
        let mut per_channel_broadband: Vec<Vec<EQFilter>> = Vec::new();
        for channel_name in &channel_result_names {
            if let Some(channel_dsp) = dsp_output.channels.get(channel_name) {
                let (channel_eq_filters, channel_bb_filters) =
                    classify_channel_eq_filters(channel_dsp);
                log::info!(
                    "Channel '{}': {} EQ filters, {} broadband filters",
                    channel_name,
                    channel_eq_filters.len(),
                    channel_bb_filters.len(),
                );
                per_channel_filters.push(channel_eq_filters);
                per_channel_broadband.push(channel_bb_filters);
            } else {
                log::info!(
                    "Channel '{}': no DSP output, using empty filters",
                    channel_name
                );
                per_channel_filters.push(Vec::<EQFilter>::new());
                per_channel_broadband.push(Vec::<EQFilter>::new());
            }
        }

        // Check if we have any filters at all
        let total_filters: usize = per_channel_filters.iter().map(|f| f.len()).sum();
        let total_bb: usize = per_channel_broadband.iter().map(|f| f.len()).sum();
        if total_filters == 0 && total_bb == 0 {
            log::warn!("No EQ filters found in optimization results");
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("No EQ filters found in optimization results".to_string());
            });
            return;
        }

        let num_channels = per_channel_filters.len();
        let global_filters = per_channel_filters.first().cloned().unwrap_or_default();
        let global_bb = per_channel_broadband.first().cloned().unwrap_or_default();

        log::info!(
            "Applying room EQ with {} channels, {} total filters (per-channel mode)",
            num_channels,
            total_filters
        );

        // Update the plugin graph AND immediately flush the config to
        // the audio engine. The previous code set `pending_plugin_update`
        // and relied on the 100ms timer to pick it up, but that path
        // sometimes didn't fire (the timer processes pending updates
        // only on its own tick, and if it happened to miss the window
        // the user saw "applied" but heard no change).
        //
        // Calling `update_plugins` directly inside the same state.update
        // closure guarantees the engine sees the new filters before we
        // show the success toast.
        self.state.update(cx, |state, _| {
            let plugin_graph = &mut state.app.plugin_state.graph;
            upsert_named_room_eq_plugins(
                plugin_graph,
                num_channels,
                &global_bb,
                &per_channel_broadband,
                total_bb,
                &global_filters,
                &per_channel_filters,
            );

            // Flush immediately to the engine — don't defer via
            // `pending_plugin_update` which depends on the timer.
            let device_name = state
                .app
                .audio_device_state
                .current_output_device_name
                .as_deref();
            let track_sr = state.app.playback.sample_rate.unwrap_or(48000);
            let sr = sotf_audio::select_output_sample_rate(track_sr, device_name) as f64;
            let plugins = state.app.plugin_state.graph.to_plugin_configs(sr);
            log::info!(
                "Flushing {} plugins to engine at {:.0} Hz",
                plugins.len(),
                sr
            );
            match state.player.lock().update_plugins(plugins) {
                Ok(()) => {
                    state.app.measurement_state.room_eq_state.status_message =
                        "Room EQ applied to player!".to_string();
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                        "Room EQ applied successfully",
                    ));
                }
                Err(e) => {
                    log::error!("Failed to apply room EQ: {}", e);
                    state.app.measurement_state.room_eq_state.error_message =
                        Some(format!("Failed to apply: {}", e));
                }
            }
            state.app.plugin_state.plugin_graph_modified = true;
            // Invalidate the workflow canvas so the graph view rebuilds
            state.app.plugin_state.workflow_canvas = None;
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
            state.app.measurement_state.room_eq_state.dsp_output.clone()
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

        // Build a matching PluginGraph for the UI so the graph view reflects
        // the topology we're sending to the engine.
        let ui_graph = Self::build_ui_graph_from_config(&graph_config);

        // Send the graph config to the engine AND update the UI graph
        self.state.update(cx, |state, _| {
            match state.player.lock().update_plugin_graph(graph_config) {
                Ok(()) => {
                    // Update the UI graph and invalidate canvas
                    state.app.plugin_state.graph = ui_graph;
                    state.app.plugin_state.workflow_canvas = None;
                    state.app.plugin_state.plugin_graph_modified = true;
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

    /// Build a `PluginGraph` (UI-level) from a `PluginGraphConfig` (engine-level).
    ///
    /// Creates plugin nodes with default settings for each engine node, adds
    /// Input/Output special nodes, wires connections, and auto-lays-out
    /// positions left-to-right.
    fn build_ui_graph_from_config(
        config: &sotf_audio::engine::PluginGraphConfig,
    ) -> sotf_audio_player::PluginGraph {
        use sotf_audio_player::{NodePosition, PluginGraph, SpecialNodeType};

        let mut graph = PluginGraph::new();

        // Add Input special node at the left
        let input_id =
            graph.add_special_node(SpecialNodeType::Input, NodePosition::new(50.0, 200.0), 2);

        // Map engine node IDs (usize) to graph node IDs (Uuid)
        let mut id_map = std::collections::HashMap::new();

        // Auto-layout: position nodes in columns
        let x_spacing = 200.0;
        let y_spacing = 120.0;

        // Simple topological layout: assign each node an x based on its longest
        // incoming path, and stack siblings vertically.
        let mut node_depth: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        // BFS to compute depth
        for node in &config.nodes {
            node_depth.entry(node.id).or_insert(0);
        }
        for edge in &config.edges {
            let from_depth = node_depth.get(&edge.from_node).copied().unwrap_or(0);
            let to_entry = node_depth.entry(edge.to_node).or_insert(0);
            *to_entry = (*to_entry).max(from_depth + 1);
        }
        // Multiple passes for longer chains
        for _ in 0..config.nodes.len() {
            for edge in &config.edges {
                let from_depth = node_depth.get(&edge.from_node).copied().unwrap_or(0);
                let to_entry = node_depth.entry(edge.to_node).or_insert(0);
                *to_entry = (*to_entry).max(from_depth + 1);
            }
        }

        // Group nodes by depth for vertical stacking
        let mut depth_counts: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();

        for node_config in &config.nodes {
            let depth = node_depth.get(&node_config.id).copied().unwrap_or(0);
            let y_index = depth_counts.entry(depth).or_insert(0);
            let x = 250.0 + (depth as f32) * x_spacing;
            let y = 100.0 + (*y_index as f32) * y_spacing;
            *y_index += 1;

            // Parse plugin type from the string name
            let plugin_type = sotf_audio::plugins::PluginType::from_name(&node_config.plugin_type)
                .unwrap_or(sotf_audio::plugins::PluginType::EQ);

            let node_id = graph.add_plugin_node(&plugin_type, NodePosition::new(x, y));

            // Derive a user-facing name for EQ plugins from the `label`
            // metadata carried in the DSP params — this is how the optimizer
            // tells us which EQ is which (broadband vs main room correction).
            let derived_name = derive_plugin_name(&node_config.plugin_type, &node_config.parameters);

            // Apply actual parameters from the DSP output to the plugin settings
            // so the modal shows the real optimized values, not defaults.
            if let Some(node) = graph.nodes.get_mut(&node_id) {
                apply_dsp_params_to_settings(
                    &mut node.plugin.settings,
                    &node_config.plugin_type,
                    &node_config.parameters,
                );
                node.plugin.name = derived_name;
            }

            id_map.insert(node_config.id, node_id);
        }

        // Add Output special node at the right
        let max_depth = node_depth.values().max().copied().unwrap_or(0);
        let output_x = 250.0 + ((max_depth + 1) as f32) * x_spacing;
        let output_id = graph.add_special_node(
            SpecialNodeType::Output,
            NodePosition::new(output_x, 200.0),
            2,
        );

        // Wire connections between plugin nodes
        for edge in &config.edges {
            if let (Some(&from), Some(&to)) = (id_map.get(&edge.from_node), id_map.get(&edge.to_node))
            {
                let _ = graph.add_connection(from, 0, to, 0);
                let _ = graph.add_connection(from, 1, to, 1);
            }
        }

        // Connect Input to first-depth nodes (nodes with no incoming edges)
        let nodes_with_incoming: std::collections::HashSet<usize> =
            config.edges.iter().map(|e| e.to_node).collect();
        for node_config in &config.nodes {
            if !nodes_with_incoming.contains(&node_config.id) {
                if let Some(&graph_id) = id_map.get(&node_config.id) {
                    let _ = graph.add_connection(input_id, 0, graph_id, 0);
                    let _ = graph.add_connection(input_id, 1, graph_id, 1);
                }
            }
        }

        // Connect last-depth nodes (nodes with no outgoing edges) to Output
        let nodes_with_outgoing: std::collections::HashSet<usize> =
            config.edges.iter().map(|e| e.from_node).collect();
        for node_config in &config.nodes {
            if !nodes_with_outgoing.contains(&node_config.id) {
                if let Some(&graph_id) = id_map.get(&node_config.id) {
                    let _ = graph.add_connection(graph_id, 0, output_id, 0);
                    let _ = graph.add_connection(graph_id, 1, output_id, 1);
                }
            }
        }

        graph
    }
}

/// Split a channel's DSP plugins into main-room-EQ filters and broadband
/// pre-correction filters based on the `parameters.label` tag each plugin
/// carries.
///
/// The optimizer emits multiple EQ plugins per channel — main room
/// correction is **unlabeled**, broadband pre-correction is labeled
/// `"broadband"` (see `autoeq::roomeq::spectral_align::create_alignment_plugins`),
/// and other stages (`cea2034`, `user_preference`, `channel_matching`) are
/// not user-editable and are filtered out.
///
/// Regression guard for Issue 6: the emitter used to produce an
/// **unlabeled** broadband plugin, which got merged into the main bucket
/// here. Downstream `upsert_named_room_eq_plugins` then inserted a single
/// merged EQ instead of the two expected named plugins. Keeping the
/// classifier as a pure function lets tests exercise the full flow
/// from real optimizer output to filter lists.
pub fn classify_channel_eq_filters(
    channel_dsp: &sotf_audio_player::room_eq_types::ChannelDspChain,
) -> (
    Vec<sotf_audio_player::EQFilter>,
    Vec<sotf_audio_player::EQFilter>,
) {
    use sotf_audio_player::room_eq_types::parse_eq_filters_from_json;

    let mut main_filters: Vec<sotf_audio_player::EQFilter> = Vec::new();
    let mut bb_filters: Vec<sotf_audio_player::EQFilter> = Vec::new();

    for plugin in &channel_dsp.plugins {
        if !plugin.plugin_type.eq_ignore_ascii_case("eq") {
            continue;
        }
        let Some(filters) = plugin.parameters.get("filters").and_then(|f| f.as_array()) else {
            continue;
        };
        let label = plugin.parameters.get("label").and_then(|l| l.as_str());
        match label {
            Some("broadband") => {
                bb_filters.extend(parse_eq_filters_from_json(filters));
            }
            None => {
                // Unlabeled = main room EQ
                main_filters.extend(parse_eq_filters_from_json(filters));
            }
            _ => {
                // Other labels (cea2034, user_preference, channel_matching) — skip
            }
        }
    }

    (main_filters, bb_filters)
}

/// Linear index of the first **user** EQ plugin with no custom name.
///
/// A user who ran "Apply to Rack" in an older build will have an anonymous
/// EQ sitting in the chain. Re-running Apply in the current build needs to
/// reclaim that node as "Room EQ" instead of inserting a third EQ alongside
/// it — otherwise the rack accumulates stale plugins across runs.
fn unnamed_user_eq_index(graph: &sotf_audio_player::PluginGraph) -> Option<usize> {
    use sotf_audio::plugins::PluginType;
    use sotf_audio_player::plugin_graph::NodeRole;
    graph.plugins_linear()?.iter().position(|n| {
        matches!(n.plugin.plugin_type(), PluginType::EQ)
            && n.plugin.name.as_deref().is_none_or(str::is_empty)
            && !n.plugin.permanent
            && n.role == NodeRole::User
    })
}

/// Upsert the two named EQ plugins ("Broadband EQ" + "Room EQ") into the
/// plugin graph. Extracted from `apply_room_eq_to_player` so the logic is
/// unit-testable against a raw `PluginGraph` without a full `Context`.
///
/// Behavior contract (see `room_eq_apply_tests.rs`):
///
/// - When `total_bb > 0`, produces **two** named EQ plugins.
///   Main is "Room EQ" with `max_filters=10`; broadband is "Broadband EQ"
///   with `max_filters=4`. Both run in per-channel mode.
/// - Pre-existing unnamed user EQ plugins (e.g. from an older
///   Apply-to-Rack build) are adopted in-place as "Room EQ" so the rack
///   does not accumulate stale nodes on upgrade.
/// - Second Apply with same names is idempotent: the existing named EQ
///   is updated in place rather than duplicated.
pub fn upsert_named_room_eq_plugins(
    graph: &mut sotf_audio_player::PluginGraph,
    num_channels: usize,
    global_bb: &[sotf_audio_player::EQFilter],
    per_channel_broadband: &[Vec<sotf_audio_player::EQFilter>],
    total_bb: usize,
    global_filters: &[sotf_audio_player::EQFilter],
    per_channel_filters: &[Vec<sotf_audio_player::EQFilter>],
) {
    use sotf_audio_player::{PluginSettings, PluginType};

    // Step 1: migrate stale unnamed EQ (pre-release upgrade path).
    if let Some(existing_idx) = unnamed_user_eq_index(graph)
        && let Some(p) = graph.get_plugin_mut(existing_idx)
    {
        p.name = Some("Room EQ".to_string());
        log::info!(
            "Adopted pre-existing unnamed EQ at index {} as 'Room EQ'",
            existing_idx
        );
    }

    // Step 2: name-keyed upsert helper. Tracks new nodes by stable
    // GraphNodeId so inserts that shift sibling positions don't leave us
    // writing settings into a neighbouring plugin.
    let upsert_eq = |graph: &mut sotf_audio_player::PluginGraph,
                     settings: PluginSettings,
                     name: &str| {
        if let Some(idx) = graph.find_plugin_index_by_name(name) {
            if let Some(p) = graph.get_plugin_mut(idx) {
                p.settings = settings;
                p.name = Some(name.to_string());
                log::info!("Updated existing '{}' EQ at index {}", name, idx);
            }
            return;
        }

        let insert_idx = graph.user_plugin_insert_index();
        match graph.insert_plugin(insert_idx, &PluginType::EQ) {
            Ok(node_id) => {
                if let Some(node) = graph.nodes.get_mut(&node_id) {
                    node.plugin.settings = settings;
                    node.plugin.name = Some(name.to_string());
                }
                log::info!(
                    "Inserted '{}' EQ at linear index {} (node {:?})",
                    name,
                    insert_idx,
                    node_id
                );
            }
            Err(e) => {
                log::error!("Failed to insert '{}' EQ: {}", name, e);
            }
        }
    };

    // Step 3: broadband correction EQ (first in chain)
    if total_bb > 0 {
        let bb_settings = PluginSettings::EQ {
            channels: num_channels,
            filters: global_bb.to_vec(),
            channel_filters: Some(per_channel_broadband.to_vec()),
            per_channel_mode: true,
            max_filters: 4,
            tdf2: false,
            topology: 0.0,
        };
        upsert_eq(graph, bb_settings, "Broadband EQ");
    }

    // Step 4: main room correction EQ (after broadband)
    let main_settings = PluginSettings::EQ {
        channels: num_channels,
        filters: global_filters.to_vec(),
        channel_filters: Some(per_channel_filters.to_vec()),
        per_channel_mode: true,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
    };
    upsert_eq(graph, main_settings, "Room EQ");

    // Step 5: post-condition sanity log — makes it obvious in logs if we
    // ever regress back to the merged-EQ bug.
    let named_eq_count = graph
        .plugins()
        .iter()
        .filter(|p| {
            matches!(p.plugin_type(), PluginType::EQ)
                && p.name
                    .as_deref()
                    .is_some_and(|n| n == "Room EQ" || n == "Broadband EQ")
        })
        .count();
    log::info!(
        "After upsert: {} named room-EQ plugins in graph (expected {}, total EQs {})",
        named_eq_count,
        if total_bb > 0 { 2 } else { 1 },
        graph
            .plugins()
            .iter()
            .filter(|p| matches!(p.plugin_type(), PluginType::EQ))
            .count()
    );
}

/// Derive a user-facing plugin name from the DSP params the optimizer emits.
///
/// Today the only plugin type that carries a semantic label is `EQ`
/// (`"broadband"` for the pre-correction EQ, unlabeled for the main room
/// correction). Returning `None` lets the UI fall back to the generic
/// plugin type display name.
fn derive_plugin_name(plugin_type_str: &str, parameters: &serde_json::Value) -> Option<String> {
    if !plugin_type_str.eq_ignore_ascii_case("eq") {
        return None;
    }
    match parameters.get("label").and_then(|l| l.as_str()) {
        Some("broadband") => Some("Broadband EQ".to_string()),
        Some("cea2034") => Some("Speaker EQ".to_string()),
        Some("user_preference") => Some("Preference EQ".to_string()),
        Some(other) if !other.is_empty() => Some(other.to_string()),
        // Unlabeled = main room correction EQ
        _ => Some("Room EQ".to_string()),
    }
}

/// Apply DSP output parameters to a `PluginSettings` in-place.
///
/// Handles the common plugin types from roomeq: EQ (filters), Gain (gain_db),
/// and Delay (delay_ms). Unknown types are left at their defaults.
fn apply_dsp_params_to_settings(
    settings: &mut sotf_audio::plugins::PluginSettings,
    plugin_type_str: &str,
    parameters: &serde_json::Value,
) {
    use sotf_audio::plugins::PluginSettings;
    use sotf_audio_player::room_eq_types::parse_eq_filters_from_json;

    let lower = plugin_type_str.to_lowercase();
    match lower.as_str() {
        "eq" => {
            if let PluginSettings::EQ { filters, .. } = settings {
                if let Some(filter_arr) = parameters.get("filters").and_then(|v| v.as_array()) {
                    *filters = parse_eq_filters_from_json(filter_arr);
                }
            }
        }
        "gain" => {
            if let PluginSettings::Gain { gain_db, .. } = settings {
                if let Some(v) = parameters.get("gain_db").and_then(|v| v.as_f64()) {
                    *gain_db = v;
                }
            }
        }
        "delay" => {
            if let PluginSettings::Delay { delay_ms, .. } = settings {
                if let Some(v) = parameters.get("delay_ms").and_then(|v| v.as_f64()) {
                    *delay_ms = v;
                }
            }
        }
        _ => {} // Other types keep defaults
    }
}
