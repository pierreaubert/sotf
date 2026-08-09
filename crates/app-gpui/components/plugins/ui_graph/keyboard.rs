use crate::app::types::PluginUpdateType;
use crate::app::{InputMode, ToastMessage};
use crate::components::design::Ds;
use crate::i18n::PluginGraphTranslations;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{Context, IntoElement, KeyDownEvent, Keystroke, Window, div};
use gpui_ui_kit::{HStack, StackSpacing, Text, TextSize, TextWeight, VStack};
use sotf_audio_player::{GraphNodeId, NodePosition, PluginGraph, PluginType};

fn keyboard_node_ids(graph: &PluginGraph) -> Vec<GraphNodeId> {
    let mut nodes = graph
        .special_nodes
        .iter()
        .map(|(id, node)| (*id, node.position.x, node.position.y))
        .chain(
            graph
                .nodes
                .iter()
                .map(|(id, node)| (*id, node.position.x, node.position.y)),
        )
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        left.2
            .total_cmp(&right.2)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.0.to_string().cmp(&right.0.to_string()))
    });
    nodes.into_iter().map(|(id, _, _)| id).collect()
}

fn selected_node_id(graph: &PluginGraph, selected: &[GraphNodeId]) -> Option<GraphNodeId> {
    selected
        .iter()
        .copied()
        .find(|node_id| graph.node_exists(*node_id))
}

fn node_label(graph: &PluginGraph, node_id: GraphNodeId) -> Option<String> {
    graph
        .nodes
        .get(&node_id)
        .map(|node| node.plugin.plugin_type().name().to_string())
        .or_else(|| {
            graph
                .special_nodes
                .get(&node_id)
                .map(|node| node.display_name().to_string())
        })
}

fn mark_graph_changed(state: &mut crate::app::AppState) {
    state
        .app
        .plugin_state
        .graph
        .update_channel_dependent_plugins();
    state.app.plugin_state.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
    state.app.plugin_state.update_state.plugin_graph_modified = true;
}

macro_rules! graph_action_handler {
    ($method:ident, $action:path, $key:literal) => {
        pub(crate) fn $method(&mut self, _: &$action, _: &mut Window, cx: &mut Context<Self>) {
            self.dispatch_plugin_graph_key($key, cx);
        }
    };
}

impl PlayerView {
    /// Rebuild after keyboard graph edits so stale canvas history cannot apply
    /// commands to a graph that was replaced outside the canvas command model.
    fn refresh_workflow_canvas(&self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            if matches!(
                state.app.plugin_state.update_state.pending_plugin_update,
                Some(PluginUpdateType::Structural)
            ) {
                state.app.plugin_state.graph_state.workflow_canvas = None;
            }
        });
    }

    fn dispatch_plugin_graph_key(&self, key_spec: &str, cx: &mut Context<Self>) {
        let Ok(keystroke) = Keystroke::parse(key_spec) else {
            return;
        };
        let event = KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        };
        debug_assert!(self.handle_plugin_graph_keyboard(&event, cx));
    }

    pub(crate) fn handle_plugin_graph_keyboard(
        &self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        let reverse = event.keystroke.modifiers.shift;

        match key {
            "tab" => {
                self.state.update(cx, |state, _cx| {
                    let order = keyboard_node_ids(&state.app.plugin_state.graph);
                    if order.is_empty() {
                        return;
                    }
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let current = selected_node_id(&state.app.plugin_state.graph, &selected);
                    let next_index = current
                        .and_then(|id| order.iter().position(|candidate| *candidate == id))
                        .map(|index| {
                            if reverse {
                                index.checked_sub(1).unwrap_or(order.len() - 1)
                            } else {
                                (index + 1) % order.len()
                            }
                        })
                        .unwrap_or_else(|| if reverse { order.len() - 1 } else { 0 });
                    state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .select_node(order[next_index], false);
                });
                cx.notify();
                true
            }
            "[" | "]" => {
                self.state.update(cx, |state, _cx| {
                    let plugin_count = PluginType::all().len();
                    if plugin_count == 0 {
                        return;
                    }
                    let index =
                        state.app.plugin_state.graph_state.keyboard_palette_index % plugin_count;
                    state.app.plugin_state.graph_state.keyboard_palette_index = if key == "[" {
                        index.checked_sub(1).unwrap_or(plugin_count - 1)
                    } else {
                        (index + 1) % plugin_count
                    };
                });
                cx.notify();
                true
            }
            "-" | "=" => {
                self.state.update(cx, |state, _cx| {
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let Some(node_id) = selected_node_id(&state.app.plugin_state.graph, &selected)
                    else {
                        return;
                    };
                    let channel_count = if state
                        .app
                        .plugin_state
                        .graph_state
                        .keyboard_connect_source
                        .is_some()
                    {
                        state.app.plugin_state.graph.node_input_channels(node_id)
                    } else {
                        state.app.plugin_state.graph.node_output_channels(node_id)
                    };
                    if channel_count == 0 {
                        state.app.plugin_state.graph_state.keyboard_target_port = 0;
                        return;
                    }
                    let port =
                        state.app.plugin_state.graph_state.keyboard_target_port % channel_count;
                    state.app.plugin_state.graph_state.keyboard_target_port = if key == "-" {
                        port.checked_sub(1).unwrap_or(channel_count - 1)
                    } else {
                        (port + 1) % channel_count
                    };
                });
                cx.notify();
                true
            }
            "a" => {
                self.state.update(cx, |state, _cx| {
                    let language = state.app.ui_state.language;
                    let text = PluginGraphTranslations::for_language(language);
                    let all = PluginType::all();
                    let Some(plugin_type) = all
                        .get(
                            state.app.plugin_state.graph_state.keyboard_palette_index
                                % all.len().max(1),
                        )
                        .cloned()
                    else {
                        return;
                    };
                    let order = keyboard_node_ids(&state.app.plugin_state.graph);
                    let x = order
                        .iter()
                        .filter_map(|id| {
                            state
                                .app
                                .plugin_state
                                .graph
                                .nodes
                                .get(id)
                                .map(|node| node.position.x)
                                .or_else(|| {
                                    state
                                        .app
                                        .plugin_state
                                        .graph
                                        .special_nodes
                                        .get(id)
                                        .map(|node| node.position.x)
                                })
                        })
                        .fold(100.0_f32, f32::max)
                        + 220.0;
                    let y = 100.0 + (state.app.plugin_state.graph.nodes.len() % 5) as f32 * 110.0;
                    let node_id = match state
                        .app
                        .plugin_state
                        .graph
                        .add_plugin_node(&plugin_type, NodePosition::new(x, y))
                    {
                        Ok(node_id) => node_id,
                        Err(error) => {
                            state.app.ui_state.toast_message = Some(ToastMessage::error(error));
                            return;
                        }
                    };
                    state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .select_node(node_id, false);
                    mark_graph_changed(state);
                    state.app.ui_state.toast_message =
                        Some(ToastMessage::success(text.plugin_added));
                });
                self.refresh_workflow_canvas(cx);
                cx.notify();
                true
            }
            "enter" | "e" => {
                let (selected, workflow_node) = {
                    let state = self.state.read(cx);
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .find(|id| state.app.plugin_state.graph.nodes.contains_key(id));
                    let workflow_node = selected.and_then(|selected| {
                        state
                            .app
                            .plugin_state
                            .graph_state
                            .workflow_canvas
                            .as_ref()
                            .and_then(|canvas| {
                                canvas.read(cx).graph().nodes.iter().find_map(|(id, node)| {
                                    (node
                                        .user_data
                                        .get("plugin_node_id")
                                        .and_then(|value| value.as_str())
                                        .and_then(|value| GraphNodeId::parse_str(value).ok())
                                        == Some(selected))
                                    .then_some(*id)
                                })
                            })
                    });
                    (selected, workflow_node)
                };
                self.state.update(cx, |state, _cx| {
                    let text = PluginGraphTranslations::for_language(state.app.ui_state.language);
                    if let (Some(selected), Some(workflow_node)) = (selected, workflow_node) {
                        let original_plugin = state.app.plugin_state.graph.nodes.get(&selected);
                        let original_settings = original_plugin
                            .and_then(|node| serde_json::to_string(&node.plugin.settings).ok());
                        let original_enabled = original_plugin.map(|node| node.plugin.enabled);
                        state.app.plugin_state.graph_state.editing_graph_node_uuid = Some(selected);
                        state.app.plugin_state.graph_state.editing_plugin_node =
                            Some(workflow_node);
                        state
                            .app
                            .plugin_state
                            .graph_state
                            .editing_original_settings_json = original_settings;
                        state.app.plugin_state.graph_state.editing_original_enabled =
                            original_enabled;
                        state.app.plugin_state.graph_state.confirm_close_dirty = false;
                        state.app.ui_state.input_mode = InputMode::EditingPluginNode;
                    } else {
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::info(text.select_node_first));
                    }
                });
                cx.notify();
                true
            }
            "b" => {
                self.state.update(cx, |state, _cx| {
                    let text = PluginGraphTranslations::for_language(state.app.ui_state.language);
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let Some(node_id) = selected_node_id(&state.app.plugin_state.graph, &selected)
                    else {
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::info(text.select_node_first));
                        return;
                    };
                    if let Err(error) = state.app.plugin_state.graph.toggle_plugin(node_id) {
                        state.app.ui_state.toast_message = Some(ToastMessage::error(error));
                        return;
                    }
                    mark_graph_changed(state);
                });
                self.refresh_workflow_canvas(cx);
                cx.notify();
                true
            }
            "c" => {
                self.state.update(cx, |state, _cx| {
                    let text = PluginGraphTranslations::for_language(state.app.ui_state.language);
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let Some(node_id) = selected_node_id(&state.app.plugin_state.graph, &selected)
                    else {
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::info(text.select_node_first));
                        return;
                    };
                    let requested_port = state.app.plugin_state.graph_state.keyboard_target_port;
                    if let Some((source, source_port)) =
                        state.app.plugin_state.graph_state.keyboard_connect_source
                    {
                        let input_count = state.app.plugin_state.graph.node_input_channels(node_id);
                        if input_count == 0 {
                            state.app.ui_state.toast_message =
                                Some(ToastMessage::error(text.connection_failed));
                            return;
                        }
                        let target_port = requested_port.min(input_count - 1);
                        if state
                            .app
                            .plugin_state
                            .graph
                            .add_connection(source, source_port, node_id, target_port)
                            .is_ok()
                        {
                            state.app.plugin_state.graph_state.keyboard_connect_source = None;
                            state.app.plugin_state.graph_state.keyboard_target_port = 0;
                            mark_graph_changed(state);
                            state.app.ui_state.toast_message =
                                Some(ToastMessage::success(text.connection_created));
                        } else {
                            state.app.ui_state.toast_message =
                                Some(ToastMessage::error(text.connection_failed));
                        }
                    } else {
                        let output_count =
                            state.app.plugin_state.graph.node_output_channels(node_id);
                        if output_count == 0 {
                            state.app.ui_state.toast_message =
                                Some(ToastMessage::error(text.connection_failed));
                            return;
                        }
                        state.app.plugin_state.graph_state.keyboard_connect_source =
                            Some((node_id, requested_port.min(output_count - 1)));
                        state.app.plugin_state.graph_state.keyboard_target_port = 0;
                    }
                });
                self.refresh_workflow_canvas(cx);
                cx.notify();
                true
            }
            "x" => {
                self.state.update(cx, |state, _cx| {
                    let text = PluginGraphTranslations::for_language(state.app.ui_state.language);
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let Some(node_id) = selected_node_id(&state.app.plugin_state.graph, &selected)
                    else {
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::info(text.select_node_first));
                        return;
                    };
                    let before = state.app.plugin_state.graph.connections.len();
                    state
                        .app
                        .plugin_state
                        .graph
                        .connections
                        .retain(|connection| {
                            connection.from_node != node_id && connection.to_node != node_id
                        });
                    if state.app.plugin_state.graph.connections.len() != before {
                        mark_graph_changed(state);
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::success(text.disconnected));
                    }
                });
                self.refresh_workflow_canvas(cx);
                cx.notify();
                true
            }
            "delete" | "backspace" => {
                self.state.update(cx, |state, _cx| {
                    let text = PluginGraphTranslations::for_language(state.app.ui_state.language);
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let Some(node_id) = selected_node_id(&state.app.plugin_state.graph, &selected)
                    else {
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::info(text.select_node_first));
                        return;
                    };
                    if !state.app.plugin_state.graph.nodes.contains_key(&node_id) {
                        state.app.ui_state.toast_message =
                            Some(ToastMessage::info(text.special_node_read_only));
                        return;
                    }
                    state.app.plugin_state.graph.remove_node(node_id);
                    state.app.plugin_state.graph_state.graph_selection.clear();
                    if state
                        .app
                        .plugin_state
                        .graph_state
                        .keyboard_connect_source
                        .is_some_and(|(source, _)| source == node_id)
                    {
                        state.app.plugin_state.graph_state.keyboard_connect_source = None;
                    }
                    mark_graph_changed(state);
                    state.app.ui_state.toast_message =
                        Some(ToastMessage::success(text.node_removed));
                });
                self.refresh_workflow_canvas(cx);
                cx.notify();
                true
            }
            "left" | "right" | "up" | "down" => {
                self.state.update(cx, |state, _cx| {
                    let selected = state
                        .app
                        .plugin_state
                        .graph_state
                        .graph_selection
                        .selected_nodes
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let Some(node_id) = selected_node_id(&state.app.plugin_state.graph, &selected)
                    else {
                        return;
                    };
                    let distance = if event.keystroke.modifiers.shift {
                        50.0
                    } else {
                        10.0
                    };
                    let (dx, dy) = match key {
                        "left" => (-distance, 0.0),
                        "right" => (distance, 0.0),
                        "up" => (0.0, -distance),
                        "down" => (0.0, distance),
                        _ => unreachable!(),
                    };
                    if let Some(node) = state.app.plugin_state.graph.nodes.get_mut(&node_id) {
                        node.position.x += dx;
                        node.position.y += dy;
                    } else if let Some(node) =
                        state.app.plugin_state.graph.special_nodes.get_mut(&node_id)
                    {
                        node.position.x += dx;
                        node.position.y += dy;
                    }
                    mark_graph_changed(state);
                });
                self.refresh_workflow_canvas(cx);
                cx.notify();
                true
            }
            "escape"
                if self
                    .state
                    .read(cx)
                    .app
                    .plugin_state
                    .graph_state
                    .keyboard_connect_source
                    .is_some() =>
            {
                self.state.update(cx, |state, _cx| {
                    state.app.plugin_state.graph_state.keyboard_connect_source = None;
                    state.app.plugin_state.graph_state.keyboard_target_port = 0;
                });
                cx.notify();
                true
            }
            _ => false,
        }
    }

    graph_action_handler!(
        graph_select_next_node,
        crate::app::actions::GraphSelectNextNode,
        "tab"
    );
    graph_action_handler!(
        graph_select_previous_node,
        crate::app::actions::GraphSelectPreviousNode,
        "shift-tab"
    );
    graph_action_handler!(
        graph_select_next_plugin_type,
        crate::app::actions::GraphSelectNextPluginType,
        "]"
    );
    graph_action_handler!(
        graph_select_previous_plugin_type,
        crate::app::actions::GraphSelectPreviousPluginType,
        "["
    );
    graph_action_handler!(
        graph_select_next_port,
        crate::app::actions::GraphSelectNextPort,
        "="
    );
    graph_action_handler!(
        graph_select_previous_port,
        crate::app::actions::GraphSelectPreviousPort,
        "-"
    );
    graph_action_handler!(
        graph_add_selected_plugin,
        crate::app::actions::GraphAddSelectedPlugin,
        "a"
    );
    graph_action_handler!(
        graph_edit_selected_node,
        crate::app::actions::GraphEditSelectedNode,
        "enter"
    );
    graph_action_handler!(
        graph_toggle_selected_bypass,
        crate::app::actions::GraphToggleSelectedBypass,
        "b"
    );
    graph_action_handler!(
        graph_connect_selected_node,
        crate::app::actions::GraphConnectSelectedNode,
        "c"
    );
    graph_action_handler!(
        graph_disconnect_selected_node,
        crate::app::actions::GraphDisconnectSelectedNode,
        "x"
    );
    graph_action_handler!(
        graph_remove_selected_node,
        crate::app::actions::GraphRemoveSelectedNode,
        "delete"
    );
    graph_action_handler!(
        graph_move_selected_left,
        crate::app::actions::GraphMoveSelectedLeft,
        "left"
    );
    graph_action_handler!(
        graph_move_selected_right,
        crate::app::actions::GraphMoveSelectedRight,
        "right"
    );
    graph_action_handler!(
        graph_move_selected_up,
        crate::app::actions::GraphMoveSelectedUp,
        "up"
    );
    graph_action_handler!(
        graph_move_selected_down,
        crate::app::actions::GraphMoveSelectedDown,
        "down"
    );
    graph_action_handler!(
        graph_move_selected_left_large,
        crate::app::actions::GraphMoveSelectedLeftLarge,
        "shift-left"
    );
    graph_action_handler!(
        graph_move_selected_right_large,
        crate::app::actions::GraphMoveSelectedRightLarge,
        "shift-right"
    );
    graph_action_handler!(
        graph_move_selected_up_large,
        crate::app::actions::GraphMoveSelectedUpLarge,
        "shift-up"
    );
    graph_action_handler!(
        graph_move_selected_down_large,
        crate::app::actions::GraphMoveSelectedDownLarge,
        "shift-down"
    );

    pub(super) fn render_graph_keyboard_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = PluginGraphTranslations::for_language(state.app.ui_state.language);
        let selected = state
            .app
            .plugin_state
            .graph_state
            .graph_selection
            .selected_nodes
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let selected_label = selected_node_id(&state.app.plugin_state.graph, &selected)
            .and_then(|node_id| node_label(&state.app.plugin_state.graph, node_id))
            .unwrap_or_else(|| text.none_selected.to_string());
        let all = PluginType::all();
        let palette_label = all
            .get(state.app.plugin_state.graph_state.keyboard_palette_index % all.len().max(1))
            .map(|plugin| plugin.name())
            .unwrap_or(text.none_selected);
        let source_label = state
            .app
            .plugin_state
            .graph_state
            .keyboard_connect_source
            .and_then(|(node_id, port)| {
                node_label(&state.app.plugin_state.graph, node_id)
                    .map(|label| format!("{label} · {}", port + 1))
            })
            .unwrap_or_else(|| text.no_connect_source.to_string());
        let target_port = state.app.plugin_state.graph_state.keyboard_target_port + 1;

        div()
            .px(d.pad_y)
            .py(d.pad_y_half)
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(
                                Text::new(text.keyboard_editor)
                                    .size(TextSize::Xs)
                                    .weight(TextWeight::Semibold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new(format!("{}: {selected_label}", text.selected))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Text::new(format!("{}: {palette_label}", text.add_plugin))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Text::new(format!(
                                    "{}: {source_label} · {}",
                                    text.connect_source, target_port
                                ))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            ),
                    )
                    .child(Text::caption(text.keyboard_hint).color(theme.text_muted)),
            )
    }
}
