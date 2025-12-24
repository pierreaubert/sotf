//! Plugin Graph Screen
//!
//! Full-screen view with signal chain rack strip at top and workflow canvas below.
//! Uses the WorkflowCanvas from gpui-ui-kit for pan/zoom, connections, and hit testing.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{
    Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData, WorkflowTheme,
};
use sotf_audio_player::{PluginGraph, PluginType};

use crate::app::types::PluginUpdateType;
use crate::theme::Theme;
use crate::ui::PlayerView;

/// Drag information for plugin reordering
#[derive(Clone)]
pub struct PluginDragInfo {
    pub source_index: usize,
    pub name: String,
    pub color: Rgba,
}

impl Render for PluginDragInfo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(self.color)
            .rounded_md()
            .text_sm()
            .text_color(rgb(0xffffff))
            .child(self.name.clone())
    }
}

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

    /// Render the plugin graph screen with signal chain strip and workflow canvas
    pub(crate) fn render_plugin_graph_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure the canvas entity exists
        self.ensure_workflow_canvas(cx);

        let (theme, workflow_canvas) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.workflow_canvas.clone())
        };

        div()
            .id("plugin-graph-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Signal chain rack strip at top
            .child(self.render_graph_rack_strip(cx))
            // Workflow canvas fills remaining space
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .relative()
                    .when_some(workflow_canvas, |el, canvas| el.child(canvas)),
            )
    }

    /// Render the signal chain rack strip (like ui_rack but without +Add button)
    fn render_graph_rack_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (plugins_data, selected_idx, theme) = {
            let state = self.state.read(cx);
            let plugins: Vec<_> = state
                .app
                .plugin_chain
                .plugins()
                .iter()
                .map(|p| {
                    (
                        p.plugin_type().clone(),
                        p.enabled,
                        p.plugin_type().name().to_string(),
                    )
                })
                .collect();
            (
                plugins,
                state.app.selected_plugin_index,
                state.app.theme.clone(),
            )
        };

        // Pre-compute static data for plugin modules
        let modules_info: Vec<_> = plugins_data
            .iter()
            .enumerate()
            .map(|(idx, (pt, enabled, name))| {
                (
                    idx,
                    plugin_color(pt, &theme),
                    plugin_icon(pt),
                    name.clone(),
                    *enabled,
                    selected_idx == idx,
                    pt.clone(),
                )
            })
            .collect();

        let is_empty = plugins_data.is_empty();
        let plugin_count = plugins_data.len();

        div()
            .flex()
            .flex_col()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            // Header - just title and count, no +Add button
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("SIGNAL CHAIN"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("{} plugins", plugin_count)),
                    ),
            )
            // Plugin modules strip
            .child(
                div()
                    .id("plugin-rack-graph")
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .overflow_x_scroll()
                    .min_h(px(140.0))
                    .children(modules_info.into_iter().map(
                        |(idx, color, icon, name, enabled, is_selected, plugin_type)| {
                            let theme_c = theme.clone();
                            let drag_info = PluginDragInfo {
                                source_index: idx,
                                name: name.clone(),
                                color,
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                // Connection line before
                                .child(div().w(px(20.0)).h(px(2.0)).bg(if enabled {
                                    theme_c.accent
                                } else {
                                    theme_c.text_muted
                                }))
                                // Plugin module box
                                .child(
                                    div()
                                        .id(("plugin-module-graph", idx))
                                        .group("plugin-module-graph")
                                        .w(px(80.0))
                                        .h(px(90.0))
                                        .flex()
                                        .flex_col()
                                        .rounded_lg()
                                        .border_2()
                                        .border_color(if is_selected {
                                            color
                                        } else {
                                            theme_c.border
                                        })
                                        .bg(theme_c.surface)
                                        .when(!enabled, |d| d.opacity(0.6))
                                        .shadow_md()
                                        .cursor_grab()
                                        .hover(|s| s.border_color(color))
                                        .drag_over::<PluginDragInfo>(|style, _, _, _| {
                                            style
                                                .bg(rgba(0x3b82f640))
                                                .border_color(rgb(0x3b82f6))
                                        })
                                        .on_drop(cx.listener(
                                            move |view, info: &PluginDragInfo, _window, cx| {
                                                let source = info.source_index;
                                                let target = idx;
                                                if source != target {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.plugin_chain.move_plugin(source, target);
                                                        state.app.selected_plugin_index = target;
                                                        state.app.pending_plugin_update =
                                                            Some(PluginUpdateType::Structural);
                                                        state.app.update_level_meter_groups();
                                                    });
                                                    cx.notify();
                                                }
                                            },
                                        ))
                                        .on_drag(drag_info, |info, _position, _window, cx| {
                                            cx.new(|_| info.clone())
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |view, _: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.selected_plugin_index = idx
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        // Top bar with color
                                        .child(div().h(px(4.0)).w_full().bg(color).rounded_t_md())
                                        // Remove button (X)
                                        .child(
                                            div()
                                                .absolute()
                                                .top(px(8.0))
                                                .right(px(4.0))
                                                .w(px(12.0))
                                                .h(px(12.0))
                                                .rounded_full()
                                                .bg(theme_c.error)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_xs()
                                                .text_color(rgb(0xffffff))
                                                .cursor_pointer()
                                                .opacity(0.0)
                                                .group_hover("plugin-module-graph", |s| s.opacity(1.0))
                                                .hover(|s| s.bg(theme_c.error))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.plugin_chain.remove_plugin(idx);
                                                                if state.app.selected_plugin_index
                                                                    >= state.app.plugin_chain.len()
                                                                    && state.app.plugin_chain.len() > 0
                                                                {
                                                                    state.app.selected_plugin_index =
                                                                        state.app.plugin_chain.len() - 1;
                                                                }
                                                                state.app.pending_plugin_update =
                                                                    Some(PluginUpdateType::Structural);
                                                                state.app.update_level_meter_groups();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .child("×"),
                                        )
                                        // Power indicator
                                        .child(
                                            div()
                                                .absolute()
                                                .top(px(8.0))
                                                .left(px(4.0))
                                                .w(px(12.0))
                                                .h(px(12.0))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |view, _e: &MouseUpEvent, _, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.plugin_chain.toggle_plugin(idx);
                                                                state.app.pending_plugin_update =
                                                                    Some(PluginUpdateType::Structural);
                                                                state.app.update_level_meter_groups();
                                                            });
                                                            cx.notify();
                                                        },
                                                    ),
                                                )
                                                .bg(if enabled {
                                                    theme_c.success
                                                } else {
                                                    theme_c.error
                                                })
                                                .text_size(px(8.0))
                                                .text_color(rgb(0xffffff))
                                                .child(if enabled { "●" } else { "○" }),
                                        )
                                        // Icon
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_xl()
                                                .text_color(color)
                                                .child(icon),
                                        )
                                        // Name
                                        .child(
                                            div()
                                                .px_2()
                                                .pb_2()
                                                .text_xs()
                                                .text_color(theme_c.text_primary)
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_align(TextAlign::Center)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(short_name(&plugin_type)),
                                        ),
                                )
                                // Connection line after
                                .child(div().w(px(20.0)).h(px(2.0)).bg(if enabled {
                                    theme_c.accent
                                } else {
                                    theme_c.text_muted
                                }))
                        },
                    ))
                    // Empty state
                    .when(is_empty, |d| {
                        d.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.text_muted)
                                .child("Drag plugins from the canvas to add them"),
                        )
                    }),
            )
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

// ============================================================================
// Helper Functions
// ============================================================================

/// Get plugin color based on type
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

/// Get plugin icon based on type
fn plugin_icon(plugin_type: &PluginType) -> &'static str {
    match plugin_type {
        PluginType::EQ => "〰",
        PluginType::Gain => "🔊",
        PluginType::Upmixer => "🔀",
        PluginType::Compressor => "📉",
        PluginType::Limiter => "🛑",
        PluginType::Gate => "🚪",
        PluginType::LoudnessCompensation => "👂",
        PluginType::BinauralDecoder => "🎧",
        PluginType::Convolution => "🌊",
        PluginType::LoudnessMonitor => "📊",
        PluginType::SpectrumAnalyzer => "📈",
        PluginType::ChannelMuteSolo => "🎚",
    }
}

/// Get short name for plugin type
fn short_name(plugin_type: &PluginType) -> &'static str {
    match plugin_type {
        PluginType::EQ => "EQ",
        PluginType::Gain => "Gain",
        PluginType::Upmixer => "Upmix",
        PluginType::Compressor => "Comp",
        PluginType::Limiter => "Limit",
        PluginType::Gate => "Gate",
        PluginType::LoudnessCompensation => "Loud",
        PluginType::BinauralDecoder => "Bin",
        PluginType::Convolution => "Conv",
        PluginType::LoudnessMonitor => "Mon",
        PluginType::SpectrumAnalyzer => "Spectr",
        PluginType::ChannelMuteSolo => "Mix",
    }
}
