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
use crate::app::ToastMessage;
use crate::app::i18n::{PluginCommonTranslations, PluginRackTranslations};
use crate::app::types::Screen;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::theme::plugin_theme_id_for_app_theme;
use crate::i18n::PluginGraphTranslations;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{Position, WorkflowCanvas, WorkflowNodeData};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, IconButtonVariant,
};
use sotf_audio_player::PluginSettings;

impl PlayerView {
    /// Ensure the WorkflowCanvas entity exists, creating it if needed
    pub(crate) fn ensure_workflow_canvas(&self, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, _| state.app.plugin_state.begin_routing_draft());
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
            let (plugin_graph, output_device_name, output_channels, theme, language) = {
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
                    Some(state.app.plugin_state.routing_controller().graph.clone()),
                    output_device_name,
                    output_channels,
                    state.app.ui_state.theme.clone(),
                    state.app.ui_state.language,
                )
            };
            let graph_text = PluginGraphTranslations::for_language(language);

            let workflow_graph =
                build_workflow_graph(&plugin_graph, &output_device_name, output_channels);

            // Create the WorkflowCanvas entity
            let canvas = cx.new(|cx| WorkflowCanvas::with_graph(workflow_graph, cx));

            // Set theme and menu items
            let workflow_theme = create_workflow_theme(&theme);

            // Clone state for the callback
            let state_for_dblclick = self.state.downgrade();
            let canvas_for_dblclick = canvas.downgrade();
            let state_for_change = self.state.downgrade();
            let canvas_for_change = canvas.downgrade();
            let state_for_menu = self.state.downgrade();
            let canvas_for_menu = canvas.downgrade();

            canvas.update(cx, |canvas, _cx| {
                canvas.set_theme(workflow_theme);
                // Plugin creation is provided by the typed palette. The
                // toolkit's generic canvas menu cannot construct PluginGraph
                // nodes and would otherwise create phantom "Node" entries.
                canvas.set_menu_items(Vec::new());

                // Set double-click callback to open node editor modal.
                // We defer the body because this callback runs while the
                // WorkflowCanvas entity is mutably borrowed by
                // handle_double_click.  Reading the canvas inside the
                // callback would trigger a "cannot read while being
                // updated" panic.  cx.defer() schedules the work after
                // the current entity update completes.
                canvas.set_on_node_double_click(move |node_id, _window, cx| {
                    let Some(canvas) = canvas_for_dblclick.upgrade() else {
                        return;
                    };
                    let Some(state) = state_for_dblclick.upgrade() else {
                        return;
                    };
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
                            if state.app.plugin_state.graph_state.editing_plugin_node
                                == Some(node_id)
                            {
                                return;
                            }
                            if let Some(uuid) = graph_node_uuid {
                                // Keep the app-side keyboard/action model in
                                // step with the node opened by pointer input.
                                // The toolkit canvas owns its visual selection;
                                // this state is the source used by graph
                                // actions and the header status below.
                                state
                                    .app
                                    .plugin_state
                                    .graph_state
                                    .graph_selection
                                    .select_node(uuid, false);
                            }
                            let original_plugin = graph_node_uuid.and_then(|uuid| {
                                state
                                    .app
                                    .plugin_state
                                    .routing_controller()
                                    .graph
                                    .nodes
                                    .get(&uuid)
                            });
                            let original_settings = original_plugin
                                .and_then(|node| serde_json::to_string(&node.plugin.settings).ok());
                            let original_enabled = original_plugin.map(|node| node.plugin.enabled);
                            state.app.plugin_state.graph_state.editing_plugin_node = Some(node_id);
                            state.app.plugin_state.graph_state.editing_graph_node_uuid =
                                graph_node_uuid;
                            state
                                .app
                                .plugin_state
                                .graph_state
                                .editing_original_settings_json = original_settings;
                            state.app.plugin_state.graph_state.editing_original_enabled =
                                original_enabled;
                            state.app.plugin_state.graph_state.confirm_close_dirty = false;
                            state.app.plugin_state.graph_state.graph_config_open = false;
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
                    let Some(canvas) = canvas_for_change.upgrade() else {
                        return;
                    };
                    let Some(state) = state_for_change.upgrade() else {
                        return;
                    };
                    cx.defer(move |cx| {
                        // Snapshot the canvas graph (Clone) so we can
                        // mutate state without holding a canvas read.
                        let workflow_graph = canvas.read(cx).graph().clone();
                        state.update(cx, |state, _cx| {
                            reconcile_plugin_graph_with_canvas(state, &workflow_graph);
                        });
                    });
                });

                // Per-node right-click context menu: mirror the rack's
                // per-plugin actions (Edit, Solo, Bypass, Remove). The
                // canvas only fires this for plugin nodes that registered
                // a `plugin_node_id` in user_data; non-plugin nodes
                // (Player / Input / Output) fall through to the default
                // canvas menu so the user can still add/replace devices.
                canvas.set_node_menu_items(move |node_data| {
                    let is_plugin = node_data
                        .user_data
                        .get("node_type")
                        .and_then(|v| v.as_str())
                        == Some(NODE_TYPE_PLUGIN);
                    if is_plugin {
                        Some(plugin_node_menu_items(graph_text))
                    } else {
                        None
                    }
                });
                canvas.set_on_node_menu_select(move |menu_id, node_id, _window, cx| {
                    let Some(canvas) = canvas_for_menu.upgrade() else {
                        return;
                    };
                    let Some(state) = state_for_menu.upgrade() else {
                        return;
                    };
                    let menu_id = menu_id.clone();
                    cx.defer(move |cx| {
                        dispatch_plugin_node_action(&canvas, &state, &menu_id, node_id, cx);
                    });
                });
            });

            // Selection changes drive the inline inspector. Keep the last selection
            // independently of the editor so panning does not reopen a closed panel.
            let mut inspected_selection = None;
            cx.observe(&canvas, move |view, canvas, cx| {
                let canvas = canvas.read(cx);
                let selected = if canvas.selection().selected_nodes.len() == 1 {
                    canvas.selection().selected_nodes.iter().next().copied()
                } else {
                    None
                };
                if selected == inspected_selection {
                    return;
                }
                inspected_selection = selected;
                let app = &view.state.read(cx).app;
                if app.ui_state.current_screen != Screen::PluginGraph
                    || !matches!(
                        app.ui_state.input_mode,
                        crate::app::InputMode::Normal | crate::app::InputMode::EditingPluginNode
                    )
                {
                    return;
                }
                let uuid = selected
                    .and_then(|id| canvas.graph().nodes.get(&id))
                    .and_then(|node| node.user_data.get("plugin_node_id"))
                    .and_then(|value| value.as_str())
                    .and_then(|value| sotf_audio_player::GraphNodeId::parse_str(value).ok());
                // Special endpoint canvas IDs are the model IDs themselves;
                // they deliberately have no plugin parameter-editing identity.
                let selection_uuid = uuid.or_else(|| {
                    selected.filter(|id| {
                        app.plugin_state
                            .routing_controller()
                            .graph
                            .special_nodes
                            .contains_key(id)
                    })
                });
                view.state.update(cx, |state, _| {
                    let plugin = uuid.and_then(|id| {
                        state
                            .app
                            .plugin_state
                            .routing_controller()
                            .graph
                            .nodes
                            .get(&id)
                    });
                    let original =
                        plugin.and_then(|node| serde_json::to_string(&node.plugin.settings).ok());
                    let enabled = plugin.map(|node| node.plugin.enabled);
                    let graph = &mut state.app.plugin_state.graph_state;
                    graph.clear_editing_context();
                    graph.graph_selection.clear();
                    if let Some(uuid) = selection_uuid {
                        graph.graph_selection.select_node(uuid, false);
                    }
                    graph.editing_plugin_node = selected;
                    graph.editing_graph_node_uuid = uuid;
                    graph.editing_original_settings_json = original;
                    graph.editing_original_enabled = enabled;
                    state.app.ui_state.input_mode = if selected.is_some() {
                        crate::app::InputMode::EditingPluginNode
                    } else {
                        crate::app::InputMode::Normal
                    };
                });
                cx.notify();
            })
            .detach();

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

        let state = self.state.read(cx);
        let sizing = crate::ui::resolve_sizing_context(
            state.app.ui_state.window_width,
            state.app.ui_state.window_height,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        );
        let compact = sizing.window_width_rems < 64.0;
        let inspector_open =
            state.app.ui_state.input_mode == crate::app::InputMode::EditingPluginNode;
        let show_palette = state
            .app
            .plugin_state
            .graph_state
            .palette_open
            .unwrap_or(!compact);
        let show_keyboard = state
            .app
            .plugin_state
            .graph_state
            .keyboard_help_open
            .unwrap_or(!compact);

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
            let pc = state.app.plugin_state.routing_controller().graph.len();
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
            .track_scroll(&self.scroll.routing)
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .bg(theme.background)
            // Header
            .child(if compact {
                let open = self
                    .state
                    .read(cx)
                    .app
                    .plugin_state
                    .graph_state
                    .header_details_open;
                let owner = self.state.clone();
                let label = PluginGraphTranslations::for_language(
                    self.state.read(cx).app.ui_state.language,
                )
                .nodes
                .signal;
                div()
                    .child(super::connections::track_routing_button(
                        Button::new("routing-signal-details", label)
                            .icon_left(if open { "▾" } else { "▸" })
                            .selected(open)
                            .expanded(open)
                            .aria_label(label)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(move |_, _, cx| {
                                owner.update(cx, |state, cx| {
                                    let open =
                                        &mut state.app.plugin_state.graph_state.header_details_open;
                                    *open = !*open;
                                    cx.notify();
                                });
                            }),
                        "routing.signal-details",
                    ))
                    .when(open, |el| {
                        el.child(self.render_graph_header(
                            node_count,
                            connection_count,
                            plugin_count,
                            cx,
                        ))
                    })
                    .into_any_element()
            } else {
                self.render_graph_header(node_count, connection_count, plugin_count, cx)
                    .into_any_element()
            })
            .child(self.render_routing_draft_actions(cx))
            .child(self.render_routing_connections(cx))
            .when(show_keyboard, |el| {
                el.child(self.render_graph_keyboard_bar(cx))
            })
            // Main content: sidebar + canvas
            .child(
                div()
                    .flex()
                    .when(compact && inspector_open, |el| el.flex_col())
                    .flex_1()
                    .min_h(rems(24.0))
                    .flex_shrink_0()
                    .min_w_0()
                    .overflow_hidden()
                    // Sidebar palette
                    .when(show_palette, |el| el.child(self.render_graph_palette(cx)))
                    // Canvas area with drop support
                    .child({
                        let drag_highlight = Theme::opacity_8pct(theme.feedback.drag_over_border);
                        let canvas_area = div()
                            .id("graph-canvas-area")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .size_full()
                            .relative()
                            .drag_over::<PaletteDragData>(move |style, _, _, _| {
                                style.bg(drag_highlight)
                            })
                            .on_drag_move::<PaletteDragData>(cx.listener(
                                |view, event: &DragMoveEvent<PaletteDragData>, _window, _cx| {
                                    let x: f32 = event.event.position.x.into();
                                    let y: f32 = event.event.position.y.into();
                                    let origin_x: f32 = event.bounds.origin.x.into();
                                    let origin_y: f32 = event.bounds.origin.y.into();
                                    view.graph_canvas_drop_position =
                                        Some((x - origin_x, y - origin_y));
                                },
                            ))
                            .on_drop(cx.listener(|view, data: &PaletteDragData, window, cx| {
                                view.handle_palette_drop(data, window, cx);
                            }))
                            .when_some(workflow_canvas, |el, canvas| el.child(canvas));
                        #[cfg(feature = "dev-api")]
                        let canvas_area = {
                            use crate::app::dev_api::DevTrackExt;
                            canvas_area.dev_track("routing.canvas")
                        };
                        canvas_area
                    })
                    .when(inspector_open, |el| {
                        el.child(self.render_plugin_node_modal(cx))
                    }),
            )
    }

    /// Handle dropping a palette item onto the canvas.
    ///
    /// Adds the node to both the `WorkflowCanvas` (visual) and the
    /// `PluginGraph` (persistent data model) so the node survives canvas
    /// rebuilds.
    pub(super) fn handle_palette_drop(
        &mut self,
        data: &PaletteDragData,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let canvas = self
            .state
            .read(cx)
            .app
            .plugin_state
            .graph_state
            .workflow_canvas
            .clone();
        if let Some(canvas) = canvas {
            let drop_position = self
                .graph_canvas_drop_position
                .take()
                .map(|(x, y)| canvas.read(cx).viewport().screen_to_canvas(x, y))
                .unwrap_or_else(|| {
                    // A drop normally records its pointer location through
                    // `on_drag_move`. Keep a deterministic center fallback
                    // for synthetic drops and platforms that omit a final
                    // drag-move event.
                    let viewport = canvas.read(cx).viewport();
                    viewport.screen_to_canvas(viewport.size.0 / 2.0, viewport.size.1 / 2.0)
                });
            let drop_x = drop_position.x;
            let drop_y = drop_position.y;

            // Also persist plugin drops into the PluginGraph
            let graph_node_id = match &data.item_type {
                PaletteItemType::Plugin(plugin_type) => {
                    let id = self.state.update(cx, |state, _| {
                        state
                            .app
                            .plugin_state
                            .routing_controller_mut()
                            .graph
                            .add_plugin_node(
                                plugin_type,
                                sotf_audio_player::NodePosition::new(drop_x, drop_y),
                            )
                    });
                    match id {
                        Ok(id) => Some(id),
                        Err(error) => {
                            self.state.update(cx, |state, _| {
                                state.app.ui_state.toast_message = Some(ToastMessage::error(error));
                            });
                            return;
                        }
                    }
                }
            };

            // Create node based on item type
            let node = match &data.item_type {
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
    fn render_routing_connections(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
        let graph = &state.app.plugin_state.routing_controller().graph;
        let open = state.app.plugin_state.graph_state.connection_list_open;
        let toggle = self.state.clone();
        let mut root = div().flex().flex_col().px(d.card).gap(d.grid).child(
            super::connections::track_routing_button(
                Button::new(
                    "routing-connections",
                    format!("{} ({})", text.routing_connections, graph.connections.len()),
                )
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .theme(theme.to_button_theme())
                .on_click_event(move |_, _, cx| {
                    toggle.update(cx, |state, cx| {
                        state.app.plugin_state.graph_state.connection_list_open = !open;
                        cx.notify();
                    });
                }),
                "routing-connections",
            ),
        );
        if open {
            let label = |id| {
                graph
                    .nodes
                    .get(&id)
                    .map(|node| node.plugin.plugin_type().name().to_string())
                    .or_else(|| {
                        graph
                            .special_nodes
                            .get(&id)
                            .map(|node| node.display_name().to_string())
                    })
                    .unwrap_or_else(|| "—".into())
            };
            let mut list = div()
                .id("routing-connections-list")
                .max_h(rems(12.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(d.grid);
            for edge in &graph.connections {
                let description = format!(
                    "{} [{}] → {} [{}]",
                    label(edge.from_node),
                    edge.from_port + 1,
                    label(edge.to_node),
                    edge.to_port + 1
                );
                let edge_id = edge.id;
                let remove = self.state.clone();
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(d.gap)
                        .child(gpui_ui_kit::Text::caption(description.clone()))
                        .child(
                            Button::new(
                                SharedString::from(format!("routing-remove-{edge_id}")),
                                state.app.ui_state.translations.settings_remove,
                            )
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .theme(theme.to_button_theme())
                            .on_click_event(move |_, _, cx| {
                                remove.update(cx, |state, cx| {
                                    state
                                        .app
                                        .plugin_state
                                        .routing_controller_mut()
                                        .graph
                                        .connections
                                        .retain(|edge| edge.id != edge_id);
                                    state.app.plugin_state.graph_state.workflow_canvas = None;
                                    cx.notify();
                                });
                            }),
                        ),
                );
            }
            root = root
                .child(self.render_routing_connection_form(cx))
                .child(list);
        }
        root.into_any_element()
    }

    fn render_routing_draft_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let draft_dirty = state.app.plugin_state.routing_draft_is_dirty();
        let theme = state.app.ui_state.theme.clone();
        let text = crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
        let apply = self.state.clone();
        let discard = self.state.clone();
        let palette = self.state.clone();
        let keyboard = self.state.clone();
        let graph_text = PluginGraphTranslations::for_language(state.app.ui_state.language);
        let sizing = crate::ui::resolve_sizing_context(
            state.app.ui_state.window_width,
            state.app.ui_state.window_height,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        );
        let compact = sizing.window_width_rems < 64.0;
        let validation_error = state
            .app
            .plugin_state
            .routing_controller()
            .graph
            .validate_routing()
            .err();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(d.gap)
            .px(d.card)
            .py(d.pad_y)
            .child(gpui_ui_kit::Text::caption(text.routing_draft))
            .child(
                Button::new("routing-palette-toggle", graph_text.nodes.plugins)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        palette.update(cx, |state, cx| {
                            let open = &mut state.app.plugin_state.graph_state.palette_open;
                            *open = Some(!open.unwrap_or(!compact));
                            cx.notify();
                        });
                    }),
            )
            .child(
                Button::new("routing-keyboard-toggle", graph_text.keyboard_editor)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        keyboard.update(cx, |state, cx| {
                            let open = &mut state.app.plugin_state.graph_state.keyboard_help_open;
                            *open = Some(!open.unwrap_or(!compact));
                            cx.notify();
                        });
                    }),
            )
            .child(super::connections::track_routing_button(
                Button::new("routing-discard", text.discard_routing)
                    .size(ButtonSize::Sm)
                    .disabled(!draft_dirty)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        discard.update(cx, |state, cx| {
                            state.app.plugin_state.discard_routing_draft();
                            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                            cx.notify();
                        });
                    }),
                "routing-discard",
            ))
            .child(super::connections::track_routing_button(
                Button::new("routing-apply", text.apply_routing)
                    .size(ButtonSize::Sm)
                    .disabled(
                        !draft_dirty
                            || validation_error.is_some()
                            || state
                                .app
                                .plugin_state
                                .graph_state
                                .applying_original
                                .is_some(),
                    )
                    .variant(ButtonVariant::Primary)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        apply.update(cx, |state, cx| {
                            if let Err(error) = state.app.plugin_state.apply_routing_draft() {
                                state.app.ui_state.toast_message = Some(ToastMessage::error(error));
                            } else {
                                state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                            }
                            cx.notify();
                        });
                    }),
                "routing-apply",
            ))
            .when_some(validation_error, |element, error| {
                element.child(gpui_ui_kit::Text::caption(error).color(theme.error))
            })
            .into_any_element()
    }

    pub(super) fn render_graph_header(
        &self,
        node_count: usize,
        connection_count: usize,
        plugin_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let graph_text =
            PluginGraphTranslations::for_language(self.state.read(cx).app.ui_state.language);

        let (
            signal_path_source_rate,
            signal_path_output_rate,
            signal_path_resampled,
            signal_path_issues,
        ) = {
            let state = self.state.read(cx);
            state
                .app
                .playback
                .signal_path
                .as_ref()
                .map_or((None, None, false, false), |p| {
                    let source_rate = p.source.as_ref().map(|s| s.sample_rate_hz);
                    let output_rate = u32::try_from(p.output.sample_rate_hz).ok();
                    (
                        source_rate,
                        output_rate,
                        p.is_resampled(),
                        p.has_known_issues(),
                    )
                })
        };

        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;
        let selected_count = self
            .state
            .read(cx)
            .app
            .plugin_state
            .graph_state
            .graph_selection
            .selected_nodes
            .len();
        let runtime_text = crate::app::i18n::RuntimeMessageTranslations::for_language(
            self.state.read(cx).app.ui_state.language,
        );
        let selection_status = if selected_count == 0 {
            graph_text.none_selected.to_string()
        } else {
            format!("{}: {selected_count}", graph_text.selected)
        };

        div()
            .flex()
            .flex_wrap()
            .gap(d.gap)
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
                            state.app.set_screen(Screen::StudioHub, "RoutingClose");
                        });
                    }),
            )
            // Title and stats
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .min_w_0()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(graph_text.nodes.signal),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(
                                runtime_text
                                    .translate(&format!("#{} plugins", plugin_count))
                                    .into_owned(),
                            ),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(
                                runtime_text
                                    .translate(&format!("#{} links", connection_count))
                                    .into_owned(),
                            ),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(selection_status),
                    )
                    .when_some(signal_path_source_rate, |el, rate| {
                        el.child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(theme.text_muted)
                                .child(format!("{}k →", rate / 1000)),
                        )
                    })
                    .when_some(signal_path_output_rate, |el, rate| {
                        el.child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(theme.text_muted)
                                .child(
                                    runtime_text
                                        .translate(&format!("{}k out", rate / 1000))
                                        .into_owned(),
                                ),
                        )
                    })
                    .when(signal_path_resampled, |el| {
                        el.child(
                            div()
                                .px(d.pad_y_half)
                                // intentional: micro-badge optical padding is below the 4px spacing grid
                                .py(px(1.0))
                                .rounded(d.r_sm)
                                .bg(theme.warning)
                                .text_size(d.text_xs)
                                .text_color(theme.text_on_accent)
                                .child(graph_text.nodes.source),
                        )
                    })
                    .when(signal_path_issues, |el| {
                        el.child(
                            div()
                                .px(d.pad_y_half)
                                // intentional: micro-badge optical padding is below the 4px spacing grid
                                .py(px(1.0))
                                .rounded(d.r_sm)
                                .bg(theme.error)
                                .text_size(d.text_xs)
                                .text_color(theme.text_on_accent)
                                .child("!"),
                        )
                    }),
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
                            .child(
                                runtime_text
                                    .translate(&format!("{} nodes", node_count))
                                    .into_owned(),
                            ),
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
                            .child(graph_text.nodes.reset_view),
                    ),
            )
    }

    /// Render the sidebar palette with plugin types (draggable)
    pub(super) fn render_graph_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, language, release_channel) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.language,
                state.app.ui_state.release_channel,
            )
        };
        let graph_text = PluginGraphTranslations::for_language(language);
        let plugin_categories = sotf_audio_player::plugin_categories::CATEGORIES;

        div()
            .id("graph-palette")
            .w(rems(8.75))
            .flex_shrink_0()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .py(d.pad_y)
            .overflow_y_scroll()
            // Plugins section header
            .child(
                div()
                    .px(d.pad_y)
                    .pb(d.pad_y)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child(graph_text.nodes.plugins),
            )
            // Plugin categories
            .children(plugin_categories.iter().filter_map(|category| {
                let plugins = category
                    .plugins
                    .iter()
                    .filter(|plugin_type| release_channel.allows(plugin_type.maturity()))
                    .cloned()
                    .collect::<Vec<_>>();
                if plugins.is_empty() {
                    return None;
                }
                let theme = theme.clone();
                Some(
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
                                .child(category.name),
                        )
                        .children(plugins.into_iter().map(|plugin_type| {
                            let label = plugin_type.name();
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
                        })),
                )
            }))
    }
}

impl PlayerView {
    /// Render the plugin node editor modal
    pub(crate) fn render_plugin_node_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let graph_text = PluginGraphTranslations::for_language(state.app.ui_state.language);

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

        let (plugin_settings, plugin_enabled, plugin_linear_idx) = graph_node_uuid
            .map(|uuid| {
                let graph = &state.app.plugin_state.routing_controller().graph;
                let plugin = graph.nodes.get(&uuid).map(|node| &node.plugin);
                let settings = plugin.map(|plugin| plugin.settings.clone());
                let enabled = plugin.map(|plugin| plugin.enabled);
                let linear_idx = graph.linear_index_of_node(uuid);
                (settings, enabled, linear_idx)
            })
            .unwrap_or((None, None, None));

        // For non-linear graphs, linear_idx is None but the node still exists.
        // Enable editing whenever the plugin exists in the graph — the
        // `editing_graph_node_uuid` (set on double-click) ensures parameter
        // changes are dispatched via GraphNodeId, bypassing linear indices.
        let node_exists_in_graph = graph_node_uuid.is_some_and(|uuid| {
            state
                .app
                .plugin_state
                .routing_controller()
                .graph
                .nodes
                .contains_key(&uuid)
        });

        let graph_state = &state.app.plugin_state.graph_state;
        let settings_are_dirty =
            graph_state.settings_are_dirty(plugin_settings.as_ref(), plugin_enabled);
        let confirm_close_dirty = graph_state.confirm_close_dirty;
        let graph_config_open = graph_state.graph_config_open;
        let has_graph_config = plugin_settings.as_ref().is_some_and(|settings| {
            settings
                .layout()
                .is_some_and(|layout| !layout.config.is_empty() || !layout.output.is_empty())
        });
        let state_for_close = self.state.clone();
        let state_for_keep = self.state.clone();
        let state_for_continue = self.state.clone();
        let state_for_config = self.state.clone();
        let rack_text = PluginRackTranslations::for_language(state.app.ui_state.language);

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
        let window_w = state.app.ui_state.window_width.max(1.0);
        let window_h = state.app.ui_state.window_height.max(1.0);
        let inline = state.app.ui_state.current_screen == Screen::PluginGraph;
        let modal_w = if inline {
            let sizing = crate::ui::resolve_sizing_context(
                window_w,
                window_h,
                state.app.ui_state.font_scale,
                state.app.ui_state.min_font_size_px,
                state.app.ui_state.max_font_size_px,
            );
            if sizing.window_width_rems < 64.0 {
                window_w
            } else {
                (window_w * 0.45).min(800.0)
            }
        } else {
            (window_w * 0.85).min(1600.0)
        };
        let modal_h = (window_h * 0.85).min(1200.0);
        let layout_scale = crate::ui::compute_combined_scale(
            window_w,
            window_h,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        );
        let editor_width = (modal_w - 4.0 * d.card.0 * 16.0 * layout_scale - 4.0).max(1.0);

        // Create the modal
        div()
            .when(!inline, |el| {
                el.absolute().inset_0().bg(theme.feedback.overlay_bg)
            })
            .when(inline, |el| el.min_w_0().flex_shrink_0())
            .flex()
            .when(!inline, |el| el.items_center().justify_center())
            .when(inline, |el| el.items_start().justify_start())
            // Closing is explicit: a backdrop click must not discard an
            // in-progress NumberInput edit.
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
                                    .children(has_graph_config.then(|| {
                                        let state = state_for_config.clone();
                                        IconButton::with_child(
                                            "graph-node-config",
                                            Icon::new(IconName::Settings).small().color(
                                                if graph_config_open {
                                                    theme.text_on_accent
                                                } else {
                                                    theme.text_secondary
                                                },
                                            ),
                                        )
                                        .variant(if graph_config_open {
                                            IconButtonVariant::Filled
                                        } else {
                                            IconButtonVariant::Outline
                                        })
                                        .size(IconButtonSize::Sm)
                                        .theme(theme.to_icon_button_theme())
                                        .aria_label(rack_text.plugin_configuration)
                                        .on_click_event(
                                            move |_event, _window, cx| {
                                                state.update(cx, |state, cx| {
                                                    let graph_state =
                                                        &mut state.app.plugin_state.graph_state;
                                                    graph_state.graph_config_open =
                                                        !graph_state.graph_config_open;
                                                    cx.notify();
                                                });
                                            },
                                        )
                                    }))
                                    .children(confirm_close_dirty.then(|| {
                                        let view = cx.entity().downgrade();
                                        let state = state_for_continue.clone();
                                        let button = Button::new(
                                            "modal-continue-editing",
                                            graph_text.nodes.continue_editing,
                                        )
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .aria_label(graph_text.nodes.continue_editing)
                                        .on_click_event(move |_, window, cx| {
                                            state.update(cx, |state, cx| {
                                                state
                                                    .app
                                                    .plugin_state
                                                    .graph_state
                                                    .confirm_close_dirty = false;
                                                cx.notify();
                                            });
                                            if let Some(view) = view.upgrade() {
                                                view.update(cx, |view, cx| {
                                                    view.focus_handle.focus(window, cx);
                                                    cx.notify();
                                                });
                                            }
                                        });

                                        super::connections::track_routing_button(
                                            button,
                                            "routing.inspector.continue",
                                        )
                                    }))
                                    .children(confirm_close_dirty.then(|| {
                                        let view = cx.entity().downgrade();
                                        let state = state_for_keep.clone();
                                        let button = Button::new(
                                            "modal-keep-changes",
                                            graph_text.nodes.keep_changes,
                                        )
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .aria_label(graph_text.nodes.keep_changes)
                                        .on_click_event(move |_, window, cx| {
                                            state.update(cx, |state, cx| {
                                                state
                                                    .app
                                                    .plugin_state
                                                    .graph_state
                                                    .clear_editing_context();
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                                cx.notify();
                                            });
                                            if let Some(view) = view.upgrade() {
                                                view.update(cx, |view, cx| {
                                                    view.focus_handle.focus(window, cx);
                                                    cx.notify();
                                                });
                                            }
                                        });
                                        super::connections::track_routing_button(
                                            button,
                                            "routing.inspector.keep",
                                        )
                                    }))
                                    // Close button
                                    .child({
                                        let view = cx.entity().downgrade();
                                        let state = state_for_close.clone();
                                        let label = if confirm_close_dirty {
                                            graph_text.nodes.discard_changes
                                        } else {
                                            graph_text.nodes.close
                                        };
                                        let button = Button::new("modal-close", label)
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .aria_label(label)
                                            .on_click_event(move |_, window, cx| {
                                                state.update(cx, |state, cx| {
                                                    if settings_are_dirty && !confirm_close_dirty {
                                                        state
                                                            .app
                                                            .plugin_state
                                                            .graph_state
                                                            .confirm_close_dirty = true;
                                                        cx.notify();
                                                        return;
                                                    }
                                                    if settings_are_dirty
                                                        && let Some(uuid) = graph_node_uuid
                                                    {
                                                        let graph_state = state
                                                            .app
                                                            .plugin_state
                                                            .graph_state
                                                            .clone();
                                                        if let Some(node) = state
                                                            .app
                                                            .plugin_state
                                                            .routing_controller_mut()
                                                            .graph
                                                            .nodes
                                                            .get_mut(&uuid)
                                                        {
                                                            graph_state
                                                                .restore_original(&mut node.plugin);
                                                        }
                                                        state
                                                            .app
                                                            .plugin_state
                                                            .routing_controller_mut()
                                                            .graph
                                                            .update_channel_dependent_plugins();
                                                    }
                                                    state
                                                        .app
                                                        .plugin_state
                                                        .graph_state
                                                        .clear_editing_context();
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::Normal;
                                                    cx.notify();
                                                });
                                                if let Some(view) = view.upgrade() {
                                                    view.update(cx, |view, cx| {
                                                        view.focus_handle.focus(window, cx);
                                                        cx.notify();
                                                    });
                                                }
                                            });

                                        super::connections::track_routing_button(
                                            button,
                                            "routing.inspector.close",
                                        )
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
                            .flex()
                            .flex_col()
                            .gap(d.section)
                            .child(self.render_plugin_node_content(
                                &node_type,
                                plugin_type.as_deref(),
                                plugin_settings.as_ref(),
                                plugin_linear_idx,
                                node_exists_in_graph,
                                editor_width,
                                &theme,
                                cx,
                            ))
                            .when_some(
                                plugin_settings
                                    .as_ref()
                                    .filter(|_| graph_config_open && has_graph_config),
                                |body, settings| {
                                    body.child(self.render_plugin_graph_config(
                                        settings,
                                        plugin_linear_idx.unwrap_or(0),
                                        plugin_linear_idx.is_some() || node_exists_in_graph,
                                        modal_w,
                                        &theme,
                                        cx,
                                    ))
                                },
                            ),
                    ),
            )
    }

    /// Render generic setup/output controls inside the graph node editor.
    ///
    /// Rack mode exposes these controls through its configuration overlay.
    /// The graph modal needs its own disclosure so file pickers, routing
    /// controls, and output meters remain reachable from the graph surface.
    fn render_plugin_graph_config(
        &self,
        settings: &PluginSettings,
        plugin_idx: usize,
        is_editing: bool,
        modal_width: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let common_text =
            PluginCommonTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (plugin_data, layout_scale, plugin_theme) = {
            let state = self.state.read(cx);
            let selected_theme = state
                .app
                .plugin_state
                .rack_theme_state
                .resolved_id(plugin_idx);
            (
                state.app.playback.rack_plugin_data.clone(),
                crate::ui::compute_combined_scale(
                    state.app.ui_state.window_width,
                    state.app.ui_state.window_height,
                    state.app.ui_state.font_scale,
                    state.app.ui_state.min_font_size_px,
                    state.app.ui_state.max_font_size_px,
                ),
                plugin_theme_id_for_app_theme(
                    selected_theme,
                    &state.app.ui_state.theme,
                    state.app.ui_state.theme_id,
                )
                .theme(),
            )
        };
        let selected_param = if is_editing {
            self.state.read(cx).app.plugin_state.plugin_param_selection
        } else {
            0
        };
        let available_width = (modal_width - 64.0).max(240.0);
        let Some(content) = super::super::ui_layout_renderer::render_config_controls_from_layout(
            &d,
            self.state.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            available_width,
            layout_scale,
            plugin_data.as_ref(),
            common_text,
            theme,
            &plugin_theme,
        ) else {
            return div().into_any_element();
        };

        div()
            .id("graph-node-config-panel")
            .flex()
            .flex_col()
            .gap(d.section)
            .p(d.card)
            .bg(theme.background_secondary)
            .border_1()
            .border_color(theme.border)
            .rounded(d.r_lg)
            .child(content)
            .into_any_element()
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
        editor_width: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let graph_text =
            PluginGraphTranslations::for_language(self.state.read(cx).app.ui_state.language);
        match node_type {
            NODE_TYPE_PLUGIN => {
                // If we have actual plugin settings, render the real plugin UI.
                // Enable editing when the node is in the graph (linear or non-linear).
                if let Some(settings) = plugin_settings {
                    let idx = plugin_linear_idx.unwrap_or(0);
                    let editing = plugin_linear_idx.is_some() || node_exists;
                    return self.render_plugin_settings_ui(
                        settings,
                        idx,
                        editing,
                        editor_width,
                        theme,
                        cx,
                    );
                }

                // Fallback: show placeholder based on plugin type
                match plugin_type {
                    Some("EQ") => div()
                        .flex()
                        .flex_col()
                        .gap(d.section)
                        .child(
                            div()
                                .text_size(d.text_base)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child(graph_text.nodes.parametric_eq),
                        )
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(theme.text_muted)
                                .child(graph_text.nodes.no_plugin_data),
                        )
                        .into_any_element(),
                    Some("Gain") => div()
                        .flex()
                        .flex_col()
                        .gap(d.section)
                        .child(
                            div()
                                .text_size(d.text_base)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child(graph_text.nodes.gain),
                        )
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(theme.text_muted)
                                .child(graph_text.nodes.no_plugin_data),
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
                        .text_size(d.text_base)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(graph_text.nodes.audio_player),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(graph_text.nodes.audio_player_description),
                )
                .into_any_element(),
            NODE_TYPE_OUTPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_base)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(graph_text.nodes.output_device),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(graph_text.nodes.output_device_description),
                )
                .into_any_element(),
            NODE_TYPE_INPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_base)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(graph_text.nodes.input_device),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(graph_text.nodes.input_device_description),
                )
                .into_any_element(),
            _ => div()
                .text_size(d.text_sm)
                .text_color(theme.text_muted)
                .child(graph_text.nodes.unknown_node_type)
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
        editor_width: f32,
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
                    .map(|c| c as std::sync::Arc<dyn std::any::Any + Send + Sync>)
                    .or_else(|| state.app.playback.rack_plugin_data.clone()),
                // The rack snapshot carries the currently edited graph node
                // while the graph screen is active. Reuse it for every
                // generic live visualization (limiter, gate, expander,
                // de-esser, and future layout meters) instead of pinning the
                // output meter to its zero fallback.
                _ => state.app.playback.rack_plugin_data.clone(),
            };
            (
                state.app.plugin_state.routing_controller().graph.clone(),
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
            self.eq_chart_focus_handle.clone(),
            self.plugin_exact_entry_focus_handle.clone(),
            cx,
            Some(editor_width),
        )
    }
}
