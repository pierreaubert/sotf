//! Plugin Graph Screen
//!
//! Main screen for the 2D plugin graph view with nodes and connections.

use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::{GraphNodeId, NodePosition, PluginGraph, PluginType};

use super::graph::{CableElement, GraphCanvas, GraphNode};
use crate::app::types::PluginUpdateType;
use crate::theme::Theme;
use crate::ui::PlayerView;

impl PlayerView {
    /// Render the plugin graph screen
    pub(crate) fn render_plugin_graph_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract all state upfront to avoid borrow issues
        let (theme, graph, selection, connection_drag) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.plugin_graph.clone(),
                state.app.graph_selection.clone(),
                state.app.graph_connection_drag.clone(),
            )
        };

        let canvas_offset = graph
            .as_ref()
            .map(|g| g.canvas_offset)
            .unwrap_or((0.0, 0.0));
        let canvas_zoom = graph.as_ref().map(|g| g.canvas_zoom).unwrap_or(1.0);

        // Pre-render sub-components (using AnyElement to avoid lifetime issues)
        let header = self.render_graph_header(cx).into_any_element();
        let palette = self.render_graph_palette(cx).into_any_element();
        let cables = self.render_graph_cables(&graph, &theme, canvas_offset, canvas_zoom);
        let nodes = self.render_graph_nodes(cx, &graph, &selection, &theme, canvas_offset, canvas_zoom);

        let has_drag = connection_drag.is_some();
        let drag_preview = connection_drag.map(|drag| {
            let from = point(px(drag.current_position.0), px(drag.current_position.1));
            CableElement::new(from, from)
                .preview(true)
                .color(theme.accent)
                .into_any_element()
        });

        div()
            .id("plugin-graph-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Header
            .child(header)
            // Main content area
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Sidebar palette
                    .child(palette)
                    // Canvas area
                    .child(
                        div()
                            .id("graph-canvas-container")
                            .flex_1()
                            .relative()
                            .overflow_hidden()
                            .bg(theme.background)
                            // Pan with scroll wheel
                            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _window, cx| {
                                let delta_x: f32 = event.delta.pixel_delta(px(1.0)).x.into();
                                let delta_y: f32 = event.delta.pixel_delta(px(1.0)).y.into();

                                view.state.update(cx, |state, _cx| {
                                    if let Some(ref mut graph) = state.app.plugin_graph {
                                        // Check for zoom (pinch or ctrl+scroll)
                                        if event.modifiers.control || event.modifiers.alt {
                                            let zoom_delta = delta_y * 0.01;
                                            graph.canvas_zoom =
                                                (graph.canvas_zoom + zoom_delta).clamp(0.5, 2.0);
                                        } else {
                                            // Pan
                                            graph.canvas_offset.0 += delta_x;
                                            graph.canvas_offset.1 += delta_y;
                                        }
                                    }
                                });
                                cx.notify();
                            }))
                            // Canvas background with grid
                            .child(
                                GraphCanvas::new()
                                    .offset(point(px(canvas_offset.0), px(canvas_offset.1)))
                                    .zoom(canvas_zoom)
                                    .background(theme.background),
                            )
                            // Render connections (cables)
                            .children(cables)
                            // Render nodes
                            .children(nodes)
                            // Connection drag preview
                            .when(has_drag, |d| {
                                if let Some(preview) = drag_preview {
                                    d.child(preview)
                                } else {
                                    d
                                }
                            }),
                    ),
            )
    }

    /// Render the graph header with controls
    fn render_graph_header(&self, cx: &mut Context<Self>) -> Div {
        let (theme, node_count, connection_count, zoom_pct) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.plugin_graph.as_ref().map(|g| g.nodes.len()).unwrap_or(0),
                state.app.plugin_graph.as_ref().map(|g| g.connections.len()).unwrap_or(0),
                state.app.plugin_graph.as_ref().map(|g| g.canvas_zoom * 100.0).unwrap_or(100.0),
            )
        };

        div()
            .flex()
            .justify_between()
            .items_center()
            .px_4()
            .py_2()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("PLUGIN GRAPH"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("{} nodes, {} connections", node_count, connection_count)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    // Zoom controls
                    .child(
                        div()
                            .id("zoom-out")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.surface)
                            .text_sm()
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        if let Some(ref mut graph) = state.app.plugin_graph {
                                            graph.canvas_zoom = (graph.canvas_zoom - 0.1).clamp(0.5, 2.0);
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("-"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("{:.0}%", zoom_pct)),
                    )
                    .child(
                        div()
                            .id("zoom-in")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.surface)
                            .text_sm()
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        if let Some(ref mut graph) = state.app.plugin_graph {
                                            graph.canvas_zoom = (graph.canvas_zoom + 0.1).clamp(0.5, 2.0);
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("+"),
                    )
                    // Reset view
                    .child(
                        div()
                            .id("reset-view")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.surface)
                            .text_xs()
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        if let Some(ref mut graph) = state.app.plugin_graph {
                                            graph.canvas_offset = (0.0, 0.0);
                                            graph.canvas_zoom = 1.0;
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("Reset"),
                    ),
            )
    }

    /// Render the sidebar palette with draggable plugin types
    fn render_graph_palette(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.state.read(cx).app.theme.clone();

        let plugin_categories: Vec<(&str, Vec<(PluginType, String)>)> = vec![
            (
                "Effects",
                vec![
                    (PluginType::EQ, PluginType::EQ.name().to_string()),
                    (PluginType::Gain, PluginType::Gain.name().to_string()),
                    (PluginType::Compressor, PluginType::Compressor.name().to_string()),
                    (PluginType::Limiter, PluginType::Limiter.name().to_string()),
                    (PluginType::Gate, PluginType::Gate.name().to_string()),
                ],
            ),
            (
                "Spatial",
                vec![
                    (PluginType::Upmixer, PluginType::Upmixer.name().to_string()),
                    (PluginType::BinauralDecoder, PluginType::BinauralDecoder.name().to_string()),
                    (PluginType::Convolution, PluginType::Convolution.name().to_string()),
                ],
            ),
            (
                "Monitor",
                vec![
                    (PluginType::LoudnessCompensation, PluginType::LoudnessCompensation.name().to_string()),
                    (PluginType::LoudnessMonitor, PluginType::LoudnessMonitor.name().to_string()),
                    (PluginType::SpectrumAnalyzer, PluginType::SpectrumAnalyzer.name().to_string()),
                    (PluginType::ChannelMuteSolo, PluginType::ChannelMuteSolo.name().to_string()),
                ],
            ),
        ];

        div()
            .flex()
            .flex_col()
            .w(px(160.0))
            .bg(theme.background_secondary)
            .border_r_1()
            .border_color(theme.border)
            .py_2()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_muted)
                    .child("PLUGINS"),
            )
            .children(plugin_categories.into_iter().map(|(category, plugins)| {
                let theme = theme.clone();
                div()
                    .flex()
                    .flex_col()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .px_1()
                            .py_1()
                            .child(category),
                    )
                    .children(plugins.into_iter().map(|(pt, name)| {
                        let theme = theme.clone();
                        let color = plugin_color(&pt, &theme);
                        let pt_clone = pt.clone();
                        let pt_debug = format!("{:?}", pt);

                        div()
                            .id(SharedString::from(format!("palette-{}", pt_debug)))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_grab()
                            .hover(|s| s.bg(theme.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    // Add node to center of view
                                    view.state.update(cx, |state, _cx| {
                                        let graph = state.app.plugin_graph.get_or_insert_with(PluginGraph::new);
                                        let offset = graph.canvas_offset;
                                        let zoom = graph.canvas_zoom;
                                        // Place at center of viewport
                                        let x = (400.0 - offset.0) / zoom;
                                        let y = (300.0 - offset.1) / zoom;
                                        graph.add_plugin_node(&pt_clone, NodePosition::new(x, y));
                                        state.app.pending_plugin_update = Some(PluginUpdateType::Structural);
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(color),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_secondary)
                                    .child(name),
                            )
                    }))
            }))
    }

    /// Render all graph nodes
    fn render_graph_nodes(
        &self,
        cx: &mut Context<Self>,
        graph: &Option<PluginGraph>,
        selection: &sotf_audio_player::GraphSelection,
        theme: &Theme,
        canvas_offset: (f32, f32),
        canvas_zoom: f32,
    ) -> Vec<AnyElement> {
        let Some(graph) = graph else {
            return vec![];
        };

        graph
            .nodes
            .iter()
            .map(|(node_id, node)| {
                let is_selected = selection.selected_nodes.contains(node_id);
                let screen_x = node.position.x * canvas_zoom + canvas_offset.0;
                let screen_y = node.position.y * canvas_zoom + canvas_offset.1;

                let node_id = *node_id;
                let theme = theme.clone();
                let plugin_type = node.plugin.plugin_type().clone();
                let input_channels = node.input_channels;
                let output_channels = node.output_channels;
                let enabled = node.plugin.enabled;

                // Use a div wrapper for interaction handling
                div()
                    .id(SharedString::from(format!("node-{}", node_id)))
                    .absolute()
                    .left(px(screen_x))
                    .top(px(screen_y))
                    .w(px(120.0 * canvas_zoom))
                    .h(px(80.0 * canvas_zoom))
                    .cursor_grab()
                    // Selection on click
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                            let add_to_selection = event.modifiers.shift;
                            view.state.update(cx, |state, _cx| {
                                state.app.graph_selection.select_node(node_id, add_to_selection);
                            });
                            cx.notify();
                        }),
                    )
                    // Start drag
                    .on_drag(
                        NodeDragPayload { node_id },
                        |payload, _position, _window, cx| {
                            cx.new(|_| payload.clone())
                        },
                    )
                    // Render the actual node
                    .child(
                        GraphNode::new(
                            plugin_type,
                            point(px(0.0), px(0.0)),
                            input_channels,
                            output_channels,
                            &theme,
                        )
                        .selected(is_selected)
                        .enabled(enabled),
                    )
                    .into_any_element()
            })
            .collect()
    }

    /// Render all graph cables/connections
    fn render_graph_cables(
        &self,
        graph: &Option<PluginGraph>,
        theme: &Theme,
        canvas_offset: (f32, f32),
        canvas_zoom: f32,
    ) -> Vec<AnyElement> {
        let Some(graph) = graph else {
            return vec![];
        };

        graph
            .connections
            .iter()
            .filter_map(|conn| {
                let from_node = graph.nodes.get(&conn.from_node)?;
                let to_node = graph.nodes.get(&conn.to_node)?;

                // Calculate port positions
                let from_x = (from_node.position.x + 120.0) * canvas_zoom + canvas_offset.0; // Right side of node
                let from_y = calculate_port_y(from_node.position.y, conn.from_port, from_node.output_channels, canvas_zoom, canvas_offset.1);

                let to_x = to_node.position.x * canvas_zoom + canvas_offset.0; // Left side of node
                let to_y = calculate_port_y(to_node.position.y, conn.to_port, to_node.input_channels, canvas_zoom, canvas_offset.1);

                Some(
                    CableElement::new(point(px(from_x), px(from_y)), point(px(to_x), px(to_y)))
                        .color(theme.accent)
                        .into_any_element(),
                )
            })
            .collect()
    }
}

/// Payload for node dragging
#[derive(Clone)]
struct NodeDragPayload {
    node_id: GraphNodeId,
}

impl Render for NodeDragPayload {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(60.0))
            .h(px(40.0))
            .rounded_md()
            .bg(rgba(0x3b82f680))
            .opacity(0.8)
    }
}

/// Calculate port Y position
fn calculate_port_y(node_y: f32, port_index: usize, total_ports: usize, zoom: f32, offset_y: f32) -> f32 {
    if total_ports == 0 {
        return (node_y + 40.0) * zoom + offset_y; // Center of node
    }
    let port_spacing = 16.0;
    let total_height = (total_ports - 1) as f32 * port_spacing;
    let start_y = node_y + (80.0 - total_height) / 2.0;
    (start_y + port_index as f32 * port_spacing) * zoom + offset_y
}

// Plugin color scheme for different types
fn plugin_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.plugin_colors.eq,
        PluginType::Gain => theme.plugin_colors.gain,
        PluginType::Upmixer => theme.plugin_colors.upmixer,
        PluginType::Compressor => theme.plugin_colors.compressor,
        PluginType::Limiter => theme.plugin_colors.limiter,
        PluginType::Gate => theme.plugin_colors.gate,
        PluginType::LoudnessCompensation => theme.plugin_colors.loudness,
        PluginType::BinauralDecoder => theme.plugin_colors.binaural,
        PluginType::Convolution => theme.plugin_colors.convolution,
        PluginType::LoudnessMonitor => theme.plugin_colors.monitor,
        PluginType::SpectrumAnalyzer => theme.plugin_colors.spectrum,
        PluginType::ChannelMuteSolo => theme.plugin_colors.mute_solo,
    }
}
