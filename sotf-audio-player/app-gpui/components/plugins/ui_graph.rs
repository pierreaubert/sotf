//! Plugin Graph Screen
//!
//! Main screen for the 2D plugin graph view with nodes and connections.
//! Uses the WorkflowCanvas from gpui-ui-kit for pan/zoom, connections, and hit testing.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{
    Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData, WorkflowTheme,
};
use sotf_audio_player::{NodePosition, PluginGraph, PluginType};

use crate::app::types::PluginUpdateType;
use crate::theme::Theme;
use crate::ui::PlayerView;

impl PlayerView {
    /// Ensure the WorkflowCanvas entity exists, creating it if needed
    pub(crate) fn ensure_workflow_canvas(&self, cx: &mut Context<Self>) {
        let has_canvas = self.state.read(cx).app.workflow_canvas.is_some();

        if !has_canvas {
            // Build workflow graph from plugin graph
            let plugin_graph = self.state.read(cx).app.plugin_graph.clone();
            let workflow_graph = build_workflow_graph(&plugin_graph);

            // Create the WorkflowCanvas entity
            let canvas = cx.new(|cx| WorkflowCanvas::with_graph(workflow_graph, cx));

            // Set theme
            let theme = self.state.read(cx).app.theme.clone();
            let workflow_theme = create_workflow_theme(&theme);
            canvas.update(cx, |canvas, _cx| {
                canvas.set_theme(workflow_theme);
            });

            // Store the canvas entity
            self.state.update(cx, |state, _cx| {
                state.app.workflow_canvas = Some(canvas);
            });
        }
    }

    /// Render the plugin graph screen using WorkflowCanvas from gpui-ui-kit
    pub(crate) fn render_plugin_graph_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure the canvas entity exists
        self.ensure_workflow_canvas(cx);

        // Extract state
        let (theme, workflow_canvas) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.workflow_canvas.clone())
        };

        // Pre-render sub-components
        let header = self.render_graph_header(cx).into_any_element();
        let palette = self.render_graph_palette(cx).into_any_element();

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
                    // Canvas area - render the WorkflowCanvas entity
                    .child(
                        div()
                            .flex_1()
                            .size_full()
                            .relative()
                            .when_some(workflow_canvas, |el, canvas| el.child(canvas)),
                    ),
            )
    }

    /// Render the graph header with controls
    fn render_graph_header(&self, cx: &mut Context<Self>) -> Div {
        let (theme, node_count, connection_count, zoom_pct) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state
                    .app
                    .plugin_graph
                    .as_ref()
                    .map(|g| g.nodes.len())
                    .unwrap_or(0),
                state
                    .app
                    .plugin_graph
                    .as_ref()
                    .map(|g| g.connections.len())
                    .unwrap_or(0),
                state
                    .app
                    .plugin_graph
                    .as_ref()
                    .map(|g| g.canvas_zoom * 100.0)
                    .unwrap_or(100.0),
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
                    .child(div().text_xs().text_color(theme.text_muted).child(format!(
                        "{} nodes, {} connections",
                        node_count, connection_count
                    ))),
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
                                            graph.canvas_zoom =
                                                (graph.canvas_zoom - 0.1).clamp(0.5, 2.0);
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
                                            graph.canvas_zoom =
                                                (graph.canvas_zoom + 0.1).clamp(0.5, 2.0);
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
                    (
                        PluginType::Compressor,
                        PluginType::Compressor.name().to_string(),
                    ),
                    (PluginType::Limiter, PluginType::Limiter.name().to_string()),
                    (PluginType::Gate, PluginType::Gate.name().to_string()),
                ],
            ),
            (
                "Spatial",
                vec![
                    (PluginType::Upmixer, PluginType::Upmixer.name().to_string()),
                    (
                        PluginType::BinauralDecoder,
                        PluginType::BinauralDecoder.name().to_string(),
                    ),
                    (
                        PluginType::Convolution,
                        PluginType::Convolution.name().to_string(),
                    ),
                ],
            ),
            (
                "Monitor",
                vec![
                    (
                        PluginType::LoudnessCompensation,
                        PluginType::LoudnessCompensation.name().to_string(),
                    ),
                    (
                        PluginType::LoudnessMonitor,
                        PluginType::LoudnessMonitor.name().to_string(),
                    ),
                    (
                        PluginType::SpectrumAnalyzer,
                        PluginType::SpectrumAnalyzer.name().to_string(),
                    ),
                    (
                        PluginType::ChannelMuteSolo,
                        PluginType::ChannelMuteSolo.name().to_string(),
                    ),
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
                                        let graph = state
                                            .app
                                            .plugin_graph
                                            .get_or_insert_with(PluginGraph::new);
                                        let offset = graph.canvas_offset;
                                        let zoom = graph.canvas_zoom;
                                        // Place at center of viewport
                                        let x = (400.0 - offset.0) / zoom;
                                        let y = (300.0 - offset.1) / zoom;
                                        graph.add_plugin_node(&pt_clone, NodePosition::new(x, y));
                                        state.app.pending_plugin_update =
                                            Some(PluginUpdateType::Structural);
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(color))
                            .child(div().text_xs().text_color(theme.text_secondary).child(name))
                    }))
            }))
    }
}

// ============================================================================
// Workflow Canvas Integration
// ============================================================================

/// Build a WorkflowGraph from the PluginGraph
fn build_workflow_graph(plugin_graph: &Option<PluginGraph>) -> WorkflowGraph {
    let Some(graph) = plugin_graph else {
        return WorkflowGraph::new();
    };

    let mut workflow_graph = WorkflowGraph::new();

    // Convert plugin nodes to workflow nodes
    for (_graph_node_id, node) in &graph.nodes {
        let plugin_type = node.plugin.plugin_type();

        let workflow_node = WorkflowNodeData::new(
            plugin_type.name(),
            Position::new(node.position.x, node.position.y),
        )
        .with_ports(node.input_channels.min(1), node.output_channels.min(1))
        .with_size(180.0, 90.0)
        .with_user_data(serde_json::json!({
            "plugin_type": format!("{:?}", plugin_type),
            "enabled": node.plugin.enabled,
        }));

        workflow_graph.add_node(workflow_node);
    }

    // TODO: Convert connections when ID mapping is implemented

    workflow_graph
}

/// Create a WorkflowTheme from the app Theme
fn create_workflow_theme(theme: &Theme) -> WorkflowTheme {
    WorkflowTheme {
        canvas_background: theme.background,
        grid_color: Rgba {
            r: theme.border.r,
            g: theme.border.g,
            b: theme.border.b,
            a: 0.3,
        },
        grid_spacing: 20.0,
        node_background: theme.surface,
        node_border: theme.border,
        node_border_selected: theme.accent,
        node_header: Rgba {
            r: theme.surface.r * 0.8,
            g: theme.surface.g * 0.8,
            b: theme.surface.b * 0.8,
            a: theme.surface.a,
        },
        node_text: theme.text_primary,
        node_border_radius: 8.0,
        node_header_height: 28.0,
        node_content_padding: 8.0,
        port_input: theme.info,
        port_output: theme.success,
        port_hover: theme.accent_hover,
        port_valid: theme.success,
        port_invalid: theme.error,
        port_radius: 6.0,
        connection_color: theme.text_secondary,
        connection_selected: theme.accent,
        connection_width: 2.0,
        connection_preview: Rgba {
            r: theme.accent.r,
            g: theme.accent.g,
            b: theme.accent.b,
            a: 0.6,
        },
        selection_fill: Rgba {
            r: theme.accent.r,
            g: theme.accent.g,
            b: theme.accent.b,
            a: 0.1,
        },
        selection_border: theme.accent,
    }
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
