use super::build::build_menu_items;
use super::build::build_workflow_graph;
use super::consts::NODE_TYPE_INPUT_DEVICE;
use super::consts::NODE_TYPE_OUTPUT_DEVICE;
use super::consts::NODE_TYPE_PLAYER;
use super::consts::NODE_TYPE_PLUGIN;
use super::consts::dispatch_plugin_node_action;
use super::consts::reconcile_plugin_graph_with_canvas;
use super::create::create_workflow_theme;
use super::palette_drag_data::PaletteDragData;
use super::plugin::plugin_channel_counts;
use super::plugin::plugin_color;
use super::plugin::plugin_max_ports;
use super::plugin::plugin_node_menu_items;
use super::types::PaletteItemType;
use crate::app::types::Screen;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{Position, WorkflowCanvas, WorkflowNodeData};
use sotf_audio_player::{PluginSettings, PluginType};

impl PlayerView {
    /// Ensure the WorkflowCanvas entity exists, creating it if needed
    pub(crate) fn ensure_workflow_canvas(&self, cx: &mut Context<Self>) {
        let has_canvas = self
            .state
            .read(cx)
            .app
            .plugin_state
            .graph_state
            .workflow_canvas
            .is_some();

        if !has_canvas {
            // Build workflow graph from plugin graph, or create a default graph
            let (
                plugin_graph,
                output_device_name,
                output_channels,
                theme,
                input_devices,
                output_devices,
            ) = {
                let state = self.state.read(cx);
                let output_device_name = state
                    .app
                    .audio_device_state
                    .output_devices
                    .get(state.app.audio_device_state.selected_output_device_index)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Default Output".to_string());
                let output_channels = state
                    .app
                    .audio_device_state
                    .output_devices
                    .get(state.app.audio_device_state.selected_output_device_index)
                    .and_then(|d| d.default_config.as_ref())
                    .map(|c| c.channels as usize)
                    .unwrap_or(2);
                (
                    Some(state.app.plugin_state.graph.clone()),
                    output_device_name,
                    output_channels,
                    state.app.ui_state.theme.clone(),
                    state.app.audio_device_state.input_devices.clone(),
                    state.app.audio_device_state.output_devices.clone(),
                )
            };

            let workflow_graph =
                build_workflow_graph(&plugin_graph, &output_device_name, output_channels);

            // Create the WorkflowCanvas entity
            let canvas = cx.new(|cx| WorkflowCanvas::with_graph(workflow_graph, cx));

            // Set theme and menu items
            let workflow_theme = create_workflow_theme(&theme);
            let menu_items = build_menu_items(&input_devices, &output_devices);

            // Clone state for the callback
            let state_for_dblclick = self.state.clone();
            let canvas_for_dblclick = canvas.clone();
            let state_for_change = self.state.clone();
            let canvas_for_change = canvas.clone();
            let state_for_menu = self.state.clone();
            let canvas_for_menu = canvas.clone();

            canvas.update(cx, |canvas, _cx| {
                canvas.set_theme(workflow_theme);
                canvas.set_menu_items(menu_items);

                // Set double-click callback to open node editor modal.
                // We defer the body because this callback runs while the
                // WorkflowCanvas entity is mutably borrowed by
                // handle_double_click.  Reading the canvas inside the
                // callback would trigger a "cannot read while being
                // updated" panic.  cx.defer() schedules the work after
                // the current entity update completes.
                canvas.set_on_node_double_click(move |node_id, _window, cx| {
                    let canvas = canvas_for_dblclick.clone();
                    let state = state_for_dblclick.clone();
                    cx.defer(move |cx| {
                        let graph_node_uuid = canvas
                            .read(cx)
                            .graph()
                            .nodes
                            .get(&node_id)
                            .and_then(|n| n.user_data.get("plugin_node_id"))
                            .and_then(|v| v.as_str())
                            .and_then(|s| sotf_audio_player::GraphNodeId::parse_str(s).ok());

                        state.update(cx, |state, _cx| {
                            state.app.plugin_state.graph_state.editing_plugin_node = Some(node_id);
                            state.app.plugin_state.graph_state.editing_graph_node_uuid =
                                graph_node_uuid;
                            state.app.ui_state.input_mode =
                                crate::app::InputMode::EditingPluginNode;
                        });
                    });
                });

                // Reconcile PluginGraph (data model + engine source of truth)
                // with the WorkflowCanvas after every structural mutation.
                // Same defer pattern as the double-click callback: the
                // canvas is mutably borrowed when the observer fires.
                canvas.set_on_graph_change(move |cx| {
                    let canvas = canvas_for_change.clone();
                    let state = state_for_change.clone();
                    cx.defer(move |cx| {
                        // Snapshot the canvas graph (Clone) so we can
                        // mutate state without holding a canvas read.
                        let workflow_graph = canvas.read(cx).graph().clone();
                        state.update(cx, |state, _cx| {
                            reconcile_plugin_graph_with_canvas(state, &workflow_graph);
                            state.app.plugin_state.update_state.pending_plugin_update =
                                Some(crate::app::types::PluginUpdateType::Structural);
                            state.app.plugin_state.update_state.plugin_graph_modified = true;
                        });
                    });
                });

                // Per-node right-click context menu: mirror the rack's
                // per-plugin actions (Edit, Solo, Bypass, Remove). The
                // canvas only fires this for plugin nodes that registered
                // a `plugin_node_id` in user_data; non-plugin nodes
                // (Player / Input / Output) fall through to the default
                // canvas menu so the user can still add/replace devices.
                canvas.set_node_menu_items(|node_data| {
                    let is_plugin = node_data
                        .user_data
                        .get("node_type")
                        .and_then(|v| v.as_str())
                        == Some(NODE_TYPE_PLUGIN);
                    if is_plugin {
                        Some(plugin_node_menu_items())
                    } else {
                        None
                    }
                });
                canvas.set_on_node_menu_select(move |menu_id, node_id, _window, cx| {
                    let canvas = canvas_for_menu.clone();
                    let state = state_for_menu.clone();
                    let menu_id = menu_id.clone();
                    cx.defer(move |cx| {
                        dispatch_plugin_node_action(&canvas, &state, &menu_id, node_id, cx);
                    });
                });
            });

            // Store the canvas entity
            self.state.update(cx, |state, _cx| {
                state.app.plugin_state.graph_state.workflow_canvas = Some(canvas);
            });
        }
    }

    /// Render the plugin graph screen with workflow canvas
    pub(crate) fn render_plugin_graph_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure the canvas entity exists
        self.ensure_workflow_canvas(cx);

        let (theme, workflow_canvas, node_count, connection_count, plugin_count) = {
            let state = self.state.read(cx);
            let (nc, cc) = state
                .app
                .plugin_state
                .graph_state
                .workflow_canvas
                .as_ref()
                .map(|canvas| {
                    let stats = canvas.read(cx).stats();
                    (stats.0, stats.1)
                })
                .unwrap_or((0, 0));
            let pc = state.app.plugin_state.graph.len();
            (
                state.app.ui_state.theme.clone(),
                state.app.plugin_state.graph_state.workflow_canvas.clone(),
                nc,
                cc,
                pc,
            )
        };

        div()
            .id("plugin-graph-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Header
            .child(self.render_graph_header(node_count, connection_count, plugin_count, cx))
            // Main content: sidebar + canvas
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Sidebar palette
                    .child(self.render_graph_palette(cx))
                    // Canvas area with drop support
                    .child({
                        let drag_highlight = Theme::opacity_8pct(theme.feedback.drag_over_border);
                        div()
                            .id("graph-canvas-area")
                            .flex_1()
                            .size_full()
                            .relative()
                            .drag_over::<PaletteDragData>(move |style, _, _, _| {
                                style.bg(drag_highlight)
                            })
                            .on_drop(cx.listener(|view, data: &PaletteDragData, _window, cx| {
                                view.handle_palette_drop(data, cx);
                            }))
                            .when_some(workflow_canvas, |el, canvas| el.child(canvas))
                    }),
            )
    }

    /// Handle dropping a palette item onto the canvas.
    ///
    /// Adds the node to both the `WorkflowCanvas` (visual) and the
    /// `PluginGraph` (persistent data model) so the node survives canvas
    /// rebuilds.
    pub(super) fn handle_palette_drop(&mut self, data: &PaletteDragData, cx: &mut Context<Self>) {
        let canvas = self
            .state
            .read(cx)
            .app
            .plugin_state
            .graph_state
            .workflow_canvas
            .clone();
        if let Some(canvas) = canvas {
            // Offset drop position by existing node count so nodes don't stack
            let node_count = self.state.read(cx).app.plugin_state.graph.nodes.len()
                + self
                    .state
                    .read(cx)
                    .app
                    .plugin_state
                    .graph
                    .special_nodes
                    .len();
            let drop_x = 300.0 + (node_count as f32 % 4.0) * 180.0;
            let drop_y = 150.0 + (node_count as f32 / 4.0).floor() * 120.0;

            // Also persist plugin drops into the PluginGraph
            let graph_node_id = match &data.item_type {
                PaletteItemType::Plugin(plugin_type) => {
                    let id = self.state.update(cx, |state, _| {
                        state.app.plugin_state.graph.add_plugin_node(
                            plugin_type,
                            sotf_audio_player::NodePosition::new(drop_x, drop_y),
                        )
                    });
                    Some(id)
                }
                PaletteItemType::Player => None, // Player nodes are special, not plugin nodes
            };

            // Create node based on item type
            let node = match &data.item_type {
                PaletteItemType::Player => {
                    WorkflowNodeData::new("Player", Position::new(100.0, 200.0))
                        .with_ports(0, 2) // Output only: stereo
                        .with_size(160.0, 80.0)
                        .with_user_data(serde_json::json!({
                            "node_type": NODE_TYPE_PLAYER,
                            "channels": 2,
                        }))
                }
                PaletteItemType::Plugin(plugin_type) => {
                    let (inputs, outputs) = plugin_channel_counts(plugin_type);
                    let (max_in, max_out) = plugin_max_ports(plugin_type);
                    let mut user_data = serde_json::json!({
                        "node_type": NODE_TYPE_PLUGIN,
                        "plugin_type": format!("{:?}", plugin_type),
                        "enabled": true,
                    });
                    // Store the PluginGraph node ID so the modal can look up settings
                    if let Some(id) = graph_node_id {
                        user_data["plugin_node_id"] = serde_json::Value::String(id.to_string());
                    }
                    WorkflowNodeData::new(plugin_type.name(), Position::new(drop_x, drop_y))
                        .with_ports(inputs, outputs)
                        .with_max_ports(max_in, max_out)
                        .with_size(160.0, 90.0)
                        .with_user_data(user_data)
                }
            };

            canvas.update(cx, |canvas, cx| {
                canvas.add_node_notify(node, cx);
            });

            // Update stats
            cx.notify();
        }
    }

    /// Render the graph header with stats
    pub(super) fn render_graph_header(
        &self,
        node_count: usize,
        connection_count: usize,
        plugin_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .justify_between()
            .items_center()
            .px(d.card)
            .py(d.pad_y)
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            // Home button on the left
            .child(
                div()
                    .id("graph-home-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(rems(2.5))
                    .h(rems(2.0))
                    .cursor_pointer()
                    .rounded(d.r_md)
                    .hover(move |s| s.bg(surface_hover))
                    .child(Icon::new(IconName::Home).color(text_muted))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        state_for_home.update(cx, |state, _cx| {
                            state.app.ui_state.current_screen = Screen::Library;
                        });
                    }),
            )
            // Title and stats
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("SIGNAL"),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("#{} plugins", plugin_count)),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("#{} links", connection_count)),
                    ),
            )
            // Reset view button on the right
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("{} nodes", node_count)),
                    )
                    // Reset view button
                    .child(
                        div()
                            .id("reset-view")
                            .px(d.pad_y)
                            .py(d.pad_y_half)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    if let Some(canvas) = view
                                        .state
                                        .read(cx)
                                        .app
                                        .plugin_state
                                        .graph_state
                                        .workflow_canvas
                                        .clone()
                                    {
                                        canvas.update(cx, |canvas, cx| {
                                            canvas.reset_viewport(cx);
                                        });
                                    }
                                    cx.notify();
                                }),
                            )
                            .child("Reset View"),
                    ),
            )
    }

    /// Render the sidebar palette with plugin types (draggable)
    pub(super) fn render_graph_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        // Input sources
        let input_items = vec![("Player", PaletteItemType::Player, theme.success)];

        // Plugin categories
        let plugin_categories = vec![
            (
                "EQ",
                vec![
                    (PluginType::EQ, "Parametric"),
                    (PluginType::LinearPhaseEq, "Linear Phase"),
                    (PluginType::FirDesigner, "FIR Designer"),
                    (PluginType::DynamicEq, "Dynamic EQ"),
                ],
            ),
            (
                "Dynamics",
                vec![
                    (PluginType::Gain, "Gain"),
                    (PluginType::Compressor, "Comp"),
                    (PluginType::Limiter, "Limiter"),
                    (PluginType::Gate, "Gate"),
                    (PluginType::Expander, "Expander"),
                    (PluginType::DeEsser, "De-Esser"),
                    (PluginType::TransientShaper, "Transient"),
                    (PluginType::SpectralCompressor, "Spectral C."),
                ],
            ),
            (
                "Color",
                vec![
                    (PluginType::Saturation, "Saturation"),
                    (PluginType::Crossfeed, "Crossfeed"),
                ],
            ),
            (
                "Spatial",
                vec![
                    (PluginType::Upmixer, "Upmix"),
                    (PluginType::AAE, "AAE Reverb"),
                    (PluginType::Downmix, "Downmix"),
                    (PluginType::MonoToStereo, "Mono->2.0"),
                    (PluginType::BinauralDecoder, "Binaural"),
                    (PluginType::Convolution, "Convo"),
                    (PluginType::XTC, "XTC"),
                    (PluginType::StereoImager, "Stereo Img"),
                    (PluginType::Delay, "Delay"),
                ],
            ),
            (
                "Monitor",
                vec![
                    (PluginType::LoudnessCompensation, "Loud Comp"),
                    (PluginType::FletcherMunson, "F-Munson"),
                    (PluginType::LoudnessMonitor, "LUFS"),
                    (PluginType::SpectrumAnalyzer, "Spectrum"),
                    (PluginType::ChannelMuteSolo, "M/S"),
                ],
            ),
            (
                "Denoising",
                vec![
                    (PluginType::Denoiser, "Denoise"),
                    (PluginType::Declick, "Declick"),
                    (PluginType::HissReducer, "Hiss"),
                    (PluginType::SpeechDenoiser, "Speech"),
                    (PluginType::Aec, "AEC"),
                    (PluginType::Pnd, "PND"),
                ],
            ),
            (
                "Utility",
                vec![
                    (PluginType::Matrix, "Matrix"),
                    (PluginType::ABCompare, "A/B"),
                    (PluginType::Crossover, "Crossover"),
                ],
            ),
        ];

        div()
            .id("graph-palette")
            .w(rems(8.75))
            .flex_shrink_0()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .py(d.pad_y)
            .overflow_y_scroll()
            // Input sources section
            .child(
                div()
                    .px(d.pad_y)
                    .pb(d.pad_y)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("INPUT"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .px(d.pad_y)
                    .mb(d.gap_md)
                    .children(input_items.into_iter().map(|(label, item_type, color)| {
                        let drag_data = PaletteDragData {
                            item_type,
                            label: label.to_string(),
                            color,
                            text_on_accent: theme.text_on_accent,
                        };
                        div()
                            .id(SharedString::from(format!("palette-{}", label)))
                            .px(d.pad_y)
                            .py(d.pad_y_half)
                            .rounded(d.r_md)
                            .bg(theme.background)
                            .border_l_2()
                            .border_color(color)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .cursor_grab()
                            .hover(|s| s.bg(theme.background_secondary))
                            .on_drag(drag_data, |info, _pos, _window, cx| {
                                cx.new(|_| info.clone())
                            })
                            .child(label)
                    })),
            )
            // Plugins section header
            .child(
                div()
                    .px(d.pad_y)
                    .pb(d.pad_y)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("PLUGINS"),
            )
            // Plugin categories
            .children(plugin_categories.into_iter().map(|(category, plugins)| {
                let theme = theme.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .px(d.pad_y)
                    .mb(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(category),
                    )
                    .children(plugins.into_iter().map(|(plugin_type, label)| {
                        let color = plugin_color(&plugin_type, &theme);
                        let drag_data = PaletteDragData {
                            item_type: PaletteItemType::Plugin(plugin_type.clone()),
                            label: label.to_string(),
                            color,
                            text_on_accent: theme.text_on_accent,
                        };
                        div()
                            .id(SharedString::from(format!("palette-{:?}", plugin_type)))
                            .px(d.pad_y)
                            .py(d.pad_y_half)
                            .rounded(d.r_md)
                            .bg(theme.background)
                            .border_l_2()
                            .border_color(color)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .cursor_grab()
                            .hover(|s| s.bg(theme.background_secondary))
                            .on_drag(drag_data, |info, _pos, _window, cx| {
                                cx.new(|_| info.clone())
                            })
                            .child(label)
                    }))
            }))
    }
}

impl PlayerView {
    /// Render the plugin node editor modal
    pub(crate) fn render_plugin_node_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        // Get the node being edited
        let node_id = state.app.plugin_state.graph_state.editing_plugin_node;
        let node_info = node_id.and_then(|id| {
            state
                .app
                .plugin_state
                .graph_state
                .workflow_canvas
                .as_ref()
                .and_then(|canvas| {
                    let canvas_read = canvas.read(cx);
                    let graph = canvas_read.graph();
                    graph.nodes.get(&id).map(|node| {
                        let name = node.title.clone();
                        let node_type = node
                            .user_data
                            .get("node_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let plugin_type = node
                            .user_data
                            .get("plugin_type")
                            .and_then(|v| v.as_str())
                            .map(|s: &str| s.to_string());
                        let plugin_node_id = node
                            .user_data
                            .get("plugin_node_id")
                            .and_then(|v| v.as_str())
                            .map(|s: &str| s.to_string());
                        (name, node_type, plugin_type, plugin_node_id)
                    })
                })
        });

        let (node_name, node_type, plugin_type, plugin_node_id) =
            node_info.unwrap_or_else(|| ("Unknown".to_string(), "unknown".to_string(), None, None));

        // Look up the actual plugin settings and linear index from the plugin graph.
        // Also resolve the GraphNodeId UUID for the node-ID-based editing path.
        let graph_node_uuid = plugin_node_id
            .as_ref()
            .and_then(|id_str| sotf_audio_player::GraphNodeId::parse_str(id_str).ok());

        let (plugin_settings, plugin_linear_idx) = graph_node_uuid
            .map(|uuid| {
                let graph = &state.app.plugin_state.graph;
                let settings = graph.nodes.get(&uuid).map(|n| n.plugin.settings.clone());
                let linear_idx = graph.linear_index_of_node(uuid);
                (settings, linear_idx)
            })
            .unwrap_or((None, None));

        // For non-linear graphs, linear_idx is None but the node still exists.
        // Enable editing whenever the plugin exists in the graph — the
        // `editing_graph_node_uuid` (set on double-click) ensures parameter
        // changes are dispatched via GraphNodeId, bypassing linear indices.
        let node_exists_in_graph = graph_node_uuid
            .is_some_and(|uuid| state.app.plugin_state.graph.nodes.contains_key(&uuid));

        let state_for_close = self.state.clone();

        // Determine the title based on node type
        let title = if node_type == NODE_TYPE_PLUGIN {
            node_name.clone()
        } else if node_type == NODE_TYPE_PLAYER {
            "Player".to_string()
        } else if node_type == NODE_TYPE_OUTPUT_DEVICE {
            format!("Output: {}", node_name)
        } else if node_type == NODE_TYPE_INPUT_DEVICE {
            format!("Input: {}", node_name)
        } else {
            node_name.clone()
        };

        // Compute modal dimensions: 85% of window, clamped to reasonable bounds
        let window_w = state.app.ui_state.window_width.max(400.0);
        let window_h = state.app.ui_state.window_height.max(300.0);
        let modal_w = (window_w * 0.85).clamp(400.0, 1600.0);
        let modal_h = (window_h * 0.85).clamp(300.0, 1200.0);

        // Create the modal
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.feedback.overlay_bg)
            .on_mouse_down(MouseButton::Left, {
                let state = state_for_close.clone();
                move |_, _, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                        state.app.plugin_state.graph_state.editing_plugin_node = None;
                        state.app.plugin_state.graph_state.editing_graph_node_uuid = None;
                    });
                }
            })
            .child(
                div()
                    .id("plugin-node-modal")
                    .w(px(modal_w))
                    .max_h(px(modal_h))
                    .bg(theme.surface)
                    .rounded(d.r_lg)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Stop propagation so clicking inside doesn't close
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    // Header
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .px(d.card)
                            .py(d.pad_x)
                            .bg(theme.background_secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(d.text_base)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(d.gap)
                                    // Load button
                                    .child(
                                        div()
                                            .id("modal-load")
                                            .px(d.pad_x)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.surface)
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_secondary)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.surface_hover))
                                            .child("Load"),
                                    )
                                    // Save button
                                    .child(
                                        div()
                                            .id("modal-save")
                                            .px(d.pad_x)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.surface)
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_secondary)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.surface_hover))
                                            .child("Save"),
                                    )
                                    // Close button
                                    .child({
                                        let state = state_for_close.clone();
                                        div()
                                            .id("modal-close")
                                            .px(d.pad_x)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.error)
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_on_accent)
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.8))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                cx.stop_propagation();
                                                state.update(cx, |state, _cx| {
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::Normal;
                                                    state
                                                        .app
                                                        .plugin_state
                                                        .graph_state
                                                        .editing_plugin_node = None;
                                                    state
                                                        .app
                                                        .plugin_state
                                                        .graph_state
                                                        .editing_graph_node_uuid = None;
                                                });
                                            })
                                            .child("Close")
                                    }),
                            ),
                    )
                    // Body - plugin content
                    .child(
                        div()
                            .id("plugin-modal-body")
                            .flex_1()
                            .overflow_y_scroll()
                            .p(d.card)
                            .child(self.render_plugin_node_content(
                                &node_type,
                                plugin_type.as_deref(),
                                plugin_settings.as_ref(),
                                plugin_linear_idx,
                                node_exists_in_graph,
                                &theme,
                                cx,
                            )),
                    ),
            )
    }

    /// Render the content for a plugin node based on its type.
    ///
    /// `plugin_linear_idx` is the linear index in the plugin graph when available
    /// (for linear graphs). `node_exists` indicates the node exists in the
    /// `PluginGraph` even if a linear index isn't available (non-linear graph).
    /// Editing is enabled when either is true — the `editing_graph_node_uuid`
    /// context handles dispatching parameter changes via node ID.
    pub(super) fn render_plugin_node_content(
        &self,
        node_type: &str,
        plugin_type: Option<&str>,
        plugin_settings: Option<&PluginSettings>,
        plugin_linear_idx: Option<usize>,
        node_exists: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        match node_type {
            NODE_TYPE_PLUGIN => {
                // If we have actual plugin settings, render the real plugin UI.
                // Enable editing when the node is in the graph (linear or non-linear).
                if let Some(settings) = plugin_settings {
                    let idx = plugin_linear_idx.unwrap_or(0);
                    let editing = plugin_linear_idx.is_some() || node_exists;
                    return self.render_plugin_settings_ui(settings, idx, editing, theme, cx);
                }

                // Fallback: show placeholder based on plugin type
                match plugin_type {
                    Some("EQ") => div()
                        .flex()
                        .flex_col()
                        .gap(d.section)
                        .child(
                            div()
                                .text_size(d.text_lg)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Parametric EQ"),
                        )
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(theme.text_muted)
                                .child("No plugin data available"),
                        )
                        .into_any_element(),
                    Some("Gain") => div()
                        .flex()
                        .flex_col()
                        .gap(d.section)
                        .child(
                            div()
                                .text_size(d.text_lg)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Gain"),
                        )
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(theme.text_muted)
                                .child("No plugin data available"),
                        )
                        .into_any_element(),
                    _ => div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(format!("Plugin type: {}", plugin_type.unwrap_or("Unknown")))
                        .into_any_element(),
                }
            }
            NODE_TYPE_PLAYER => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_lg)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Audio Player"),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("This node represents the audio file playback source."),
                )
                .into_any_element(),
            NODE_TYPE_OUTPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_lg)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Output Device"),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("This node represents the audio output destination."),
                )
                .into_any_element(),
            NODE_TYPE_INPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_lg)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Input Device"),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("This node represents an audio input source."),
                )
                .into_any_element(),
            _ => div()
                .text_size(d.text_sm)
                .text_color(theme.text_muted)
                .child("Unknown node type")
                .into_any_element(),
        }
    }

    /// Render the actual plugin UI based on settings.
    ///
    /// Delegates to `render_plugin_content` which already handles all plugin
    /// types via the custom view registry and layout renderer fallback.
    pub(super) fn render_plugin_settings_ui(
        &self,
        settings: &PluginSettings,
        plugin_idx: usize,
        is_editing: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (
            plugin_graph,
            loudness,
            selected_eq_band,
            spectrum_tilt_open,
            spectrum_ref_open,
            param_selection,
            plugin_data,
        ) = {
            let state = self.state.read(cx);
            let pd: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> = match settings {
                PluginSettings::SpectrumAnalyzer { .. } => state
                    .app
                    .playback
                    .spectrum_info
                    .clone()
                    .map(|s| s as std::sync::Arc<dyn std::any::Any + Send + Sync>),
                PluginSettings::Compressor { .. } => state
                    .app
                    .playback
                    .compressor_info
                    .clone()
                    .map(|c| c as std::sync::Arc<dyn std::any::Any + Send + Sync>),
                _ => None,
            };
            (
                state.app.plugin_state.graph.clone(),
                state.app.playback.loudness_info.clone(),
                state.app.plugin_state.selected_eq_band,
                state.app.plugin_ui.spectrum_tilt_select_open,
                state.app.plugin_ui.spectrum_reference_select_open,
                if is_editing {
                    state.app.plugin_state.plugin_param_selection
                } else {
                    0
                },
                pd,
            )
        };

        super::super::render_plugin_content(
            self.state.clone(),
            plugin_idx,
            settings,
            is_editing,
            param_selection,
            theme,
            false,
            selected_eq_band,
            loudness,
            plugin_data,
            spectrum_tilt_open,
            spectrum_ref_open,
            &plugin_graph,
            None,
            cx,
        )
    }
}
