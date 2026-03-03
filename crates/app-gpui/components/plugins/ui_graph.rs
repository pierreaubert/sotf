//! Plugin Graph Screen
//!
//! Full-screen workflow canvas for node-based plugin editing.
//! Uses the WorkflowCanvas from gpui-ui-kit for pan/zoom, connections, and hit testing.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::MenuItem;
use gpui_ui_kit::workflow::{
    NodeId, Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData, WorkflowTheme,
};
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{PluginGraph, PluginSettings, PluginType, SpecialNodeType};

use crate::app::types::Screen;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::{
    render_compressor_plugin, render_downmix_plugin, render_eq_plugin, render_gain_plugin,
    render_gate_plugin, render_limiter_plugin, render_mono_to_stereo_plugin, render_upmixer_plugin,
    ui_compressor, ui_downmix, ui_eq, ui_gain, ui_gate, ui_limiter, ui_mono_to_stereo, ui_upmixer,
};
use crate::theme::Theme;
use crate::ui::PlayerView;

// ============================================================================
// Node Type Constants
// ============================================================================

/// Node types for user_data
const NODE_TYPE_PLAYER: &str = "player";
const NODE_TYPE_INPUT_DEVICE: &str = "input_device";
const NODE_TYPE_OUTPUT_DEVICE: &str = "output_device";
const NODE_TYPE_PLUGIN: &str = "plugin";

// ============================================================================
// Drag and Drop Types
// ============================================================================

/// Drag data for palette items
#[derive(Clone)]
pub struct PaletteDragData {
    pub item_type: PaletteItemType,
    pub label: String,
    pub color: Rgba,
    pub text_on_accent: Rgba,
}

/// Type of item being dragged from palette
#[derive(Clone)]
pub enum PaletteItemType {
    Player,
    Plugin(PluginType),
}

impl Render for PaletteDragData {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(self.color)
            .rounded_md()
            .text_sm()
            .text_color(self.text_on_accent)
            .shadow_lg()
            .child(self.label.clone())
    }
}

impl PlayerView {
    /// Ensure the WorkflowCanvas entity exists, creating it if needed
    pub(crate) fn ensure_workflow_canvas(&self, cx: &mut Context<Self>) {
        let has_canvas = self
            .state
            .read(cx)
            .app
            .plugin_state
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
                    state.app.plugin_state.plugin_graph.clone(),
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
            let state_for_callback = self.state.clone();

            canvas.update(cx, |canvas, _cx| {
                canvas.set_theme(workflow_theme);
                canvas.set_menu_items(menu_items);

                // Set double-click callback to open node editor modal
                canvas.set_on_node_double_click(move |node_id, _window, cx| {
                    state_for_callback.update(cx, |state, _cx| {
                        state.app.plugin_state.editing_plugin_node = Some(node_id);
                        state.app.ui_state.input_mode = crate::app::InputMode::EditingPluginNode;
                    });
                });
            });

            // Store the canvas entity
            self.state.update(cx, |state, _cx| {
                state.app.plugin_state.workflow_canvas = Some(canvas);
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
                .workflow_canvas
                .as_ref()
                .map(|canvas| {
                    let stats = canvas.read(cx).stats();
                    (stats.0, stats.1)
                })
                .unwrap_or((0, 0));
            let pc = state.app.plugin_state.chain.len();
            (
                state.app.ui_state.theme.clone(),
                state.app.plugin_state.workflow_canvas.clone(),
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
                        let drag_highlight = Theme::opacity_8pct(theme.drag_over_border);
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

    /// Handle dropping a palette item onto the canvas
    fn handle_palette_drop(&mut self, data: &PaletteDragData, cx: &mut Context<Self>) {
        let canvas = self.state.read(cx).app.plugin_state.workflow_canvas.clone();
        if let Some(canvas) = canvas {
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
                    WorkflowNodeData::new(plugin_type.name(), Position::new(300.0, 200.0))
                        .with_ports(inputs, outputs)
                        .with_size(160.0, 90.0)
                        .with_user_data(serde_json::json!({
                            "node_type": NODE_TYPE_PLUGIN,
                            "plugin_type": format!("{:?}", plugin_type),
                            "enabled": true,
                        }))
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
    fn render_graph_header(
        &self,
        node_count: usize,
        connection_count: usize,
        plugin_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .justify_between()
            .items_center()
            .px_4()
            .py_2()
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
                    .rounded_md()
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
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("SIGNAL"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("#{} plugins", plugin_count)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("#{} links", connection_count)),
                    ),
            )
            // Reset view button on the right
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("{} nodes", node_count)),
                    )
                    // Reset view button
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
                                    if let Some(canvas) =
                                        view.state.read(cx).app.plugin_state.workflow_canvas.clone()
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
    fn render_graph_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        // Input sources
        let input_items = vec![("Player", PaletteItemType::Player, theme.success)];

        // Plugin categories
        let plugin_categories = vec![
            (
                "Effects",
                vec![
                    (PluginType::EQ, "EQ"),
                    (PluginType::Gain, "Gain"),
                    (PluginType::Compressor, "Comp"),
                    (PluginType::Limiter, "Limit"),
                    (PluginType::Gate, "Gate"),
                ],
            ),
            (
                "Spatial",
                vec![
                    (PluginType::Upmixer, "Upmix"),
                    (PluginType::Downmix, "Downmix"),
                    (PluginType::MonoToStereo, "Mono->2.0"),
                    (PluginType::BinauralDecoder, "Binaural"),
                    (PluginType::Convolution, "Convo"),
                ],
            ),
            (
                "Monitor",
                vec![
                    (PluginType::LoudnessCompensation, "Loud Comp"),
                    (PluginType::LoudnessMonitor, "LUFS"),
                    (PluginType::SpectrumAnalyzer, "Spectrum"),
                    (PluginType::ChannelMuteSolo, "M/S"),
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
            .py_2()
            .overflow_y_scroll()
            // Input sources section
            .child(
                div()
                    .px_2()
                    .pb_2()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("INPUT"),
            )
            .child(div().flex().flex_col().gap_1().px_2().mb_3().children(
                input_items.into_iter().map(|(label, item_type, color)| {
                    let drag_data = PaletteDragData {
                        item_type,
                        label: label.to_string(),
                        color,
                        text_on_accent: theme.text_on_accent,
                    };
                    div()
                        .id(SharedString::from(format!("palette-{}", label)))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme.background)
                        .border_l_2()
                        .border_color(color)
                        .text_xs()
                        .text_color(theme.text_secondary)
                        .cursor_grab()
                        .hover(|s| s.bg(theme.background_secondary))
                        .on_drag(drag_data, |info, _pos, _window, cx| {
                            cx.new(|_| info.clone())
                        })
                        .child(label)
                }),
            ))
            // Plugins section header
            .child(
                div()
                    .px_2()
                    .pb_2()
                    .text_xs()
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
                    .gap_1()
                    .px_2()
                    .mb_2()
                    .child(div().text_xs().text_color(theme.text_muted).child(category))
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
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.background)
                            .border_l_2()
                            .border_color(color)
                            .text_xs()
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

/// Get a color for a plugin type
fn plugin_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.info,
        PluginType::Gain => theme.success,
        PluginType::Compressor
        | PluginType::Limiter
        | PluginType::Gate
        | PluginType::Expander
        | PluginType::MultibandCompressor
        | PluginType::MultibandExpander => theme.warning,
        PluginType::Upmixer
        | PluginType::BinauralDecoder
        | PluginType::Convolution
        | PluginType::XTC => theme.accent,
        PluginType::LoudnessCompensation
        | PluginType::FletcherMunson
        | PluginType::LoudnessMonitor
        | PluginType::SpectrumAnalyzer
        | PluginType::ChannelMuteSolo => theme.text_muted,
        PluginType::Matrix => theme.accent,
        PluginType::Denoiser => theme.info,
        PluginType::Pnd => theme.info,
        PluginType::ABCompare => theme.warning, // A/B Compare - use warning color
        PluginType::BandSplit | PluginType::BandMerge => theme.accent, // Band processing - use accent
        PluginType::Downmix => theme.accent,
        PluginType::MonoToStereo => theme.accent,
        PluginType::Crossfeed => theme.accent,
    }
}

/// Get input and output channel counts for a plugin type
fn plugin_channel_counts(plugin_type: &PluginType) -> (usize, usize) {
    match plugin_type {
        // Most plugins are 2-in, 2-out (stereo passthrough)
        PluginType::EQ
        | PluginType::Gain
        | PluginType::Compressor
        | PluginType::Limiter
        | PluginType::Gate
        | PluginType::Expander
        | PluginType::MultibandCompressor
        | PluginType::MultibandExpander
        | PluginType::LoudnessCompensation
        | PluginType::FletcherMunson
        | PluginType::LoudnessMonitor
        | PluginType::SpectrumAnalyzer
        | PluginType::ChannelMuteSolo
        | PluginType::XTC
        | PluginType::Denoiser
        | PluginType::Pnd => (2, 2),
        // Upmixer: stereo in, multi-channel out (5.0 = 5 channels, show up to 5)
        PluginType::Upmixer => (2, 5),
        // Binaural decoder: multi-channel in, stereo out
        PluginType::BinauralDecoder => (5, 2),
        // Convolution: stereo in/out
        PluginType::Convolution => (2, 2),
        // Matrix: variable in/out (default to 2x2)
        PluginType::Matrix => (2, 2),
        // A/B Compare: stereo in/out
        PluginType::ABCompare => (2, 2),
        // Band Split: 2 in, 4 out (2 bands x 2 channels)
        PluginType::BandSplit => (2, 4),
        // Band Merge: 4 in, 2 out (2 bands x 2 channels merged back)
        PluginType::BandMerge => (4, 2),
        // Downmix: multi-channel in, stereo out
        PluginType::Downmix => (6, 2),
        // Mono to Stereo: mono in, stereo out
        PluginType::MonoToStereo => (1, 2),
        // Crossfeed: stereo in/out
        PluginType::Crossfeed => (2, 2),
    }
}

// ============================================================================
// Workflow Canvas Integration
// ============================================================================

/// Build a WorkflowGraph from the PluginGraph, or create a default graph
fn build_workflow_graph(
    plugin_graph: &Option<PluginGraph>,
    default_output_name: &str,
    default_output_channels: usize,
) -> WorkflowGraph {
    // If we have an existing plugin graph, convert it
    if let Some(graph) = plugin_graph {
        return convert_plugin_graph(graph);
    }

    // Otherwise, create a default graph: Player → EQ → Output
    create_default_graph(default_output_name, default_output_channels)
}

/// Create the default graph: Player → EQ → Output
fn create_default_graph(output_name: &str, output_channels: usize) -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();

    // Player node (input source) - only output ports (green)
    let player_node = WorkflowNodeData::new("Player", Position::new(50.0, 150.0))
        .with_ports(0, 2) // No inputs, 2 outputs (stereo)
        .with_size(140.0, 80.0)
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_PLAYER,
            "channels": 2,
        }));
    let player_id = player_node.id;

    // EQ node (plugin) - both input and output ports
    let eq_node = WorkflowNodeData::new("EQ", Position::new(250.0, 150.0))
        .with_ports(2, 2) // 2 inputs, 2 outputs
        .with_size(140.0, 90.0)
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_PLUGIN,
            "plugin_type": "EQ",
            "enabled": true,
        }));
    let eq_id = eq_node.id;

    // Output node - only input ports (blue)
    let output_channels_clamped = output_channels.min(8);
    let output_node = WorkflowNodeData::new(output_name, Position::new(450.0, 150.0))
        .with_ports(output_channels_clamped, 0) // N inputs, no outputs
        .with_size(160.0, 80.0 + (output_channels_clamped as f32 * 8.0))
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_OUTPUT_DEVICE,
            "channels": output_channels,
        }));
    let output_id = output_node.id;

    // Add nodes
    graph.add_node(player_node);
    graph.add_node(eq_node);
    graph.add_node(output_node);

    // Add connections: Player → EQ → Output
    // Connect stereo (2 channels) with "fat" links (all channels bundled)
    let _ = graph.add_connection(player_id, 0, eq_id, 0); // L channel
    let _ = graph.add_connection(player_id, 1, eq_id, 1); // R channel
    let _ = graph.add_connection(eq_id, 0, output_id, 0); // L channel
    let _ = graph.add_connection(eq_id, 1, output_id, 1); // R channel

    graph
}

/// Convert an existing PluginGraph to a WorkflowGraph
fn convert_plugin_graph(graph: &PluginGraph) -> WorkflowGraph {
    let mut workflow_graph = WorkflowGraph::new();

    // Map from plugin graph node IDs to workflow node IDs
    let mut id_map: std::collections::HashMap<NodeId, NodeId> = std::collections::HashMap::new();

    // Convert special nodes (Input/Output devices) to workflow nodes
    for (special_node_id, special_node) in &graph.special_nodes {
        let (input_ports, output_ports, node_type) = match special_node.node_type {
            SpecialNodeType::Input => (0, special_node.channels.min(8), NODE_TYPE_INPUT_DEVICE),
            SpecialNodeType::Output => (special_node.channels.min(8), 0, NODE_TYPE_OUTPUT_DEVICE),
            SpecialNodeType::Split => (1, special_node.channels.min(8), "split"),
            SpecialNodeType::Merge => (special_node.channels.min(8), 1, "merge"),
        };

        let height = 80.0 + (input_ports.max(output_ports) as f32 * 8.0);
        let workflow_node = WorkflowNodeData::new(
            special_node.display_name(),
            Position::new(special_node.position.x, special_node.position.y),
        )
        .with_ports(input_ports, output_ports)
        .with_size(160.0, height)
        .with_user_data(serde_json::json!({
            "node_type": node_type,
            "channels": special_node.channels,
        }));

        let workflow_id = workflow_node.id;
        id_map.insert(*special_node_id, workflow_id);
        workflow_graph.add_node(workflow_node);
    }

    // Convert plugin nodes to workflow nodes
    for (graph_node_id, node) in &graph.nodes {
        let plugin_type = node.plugin.plugin_type();
        let (input_ports, output_ports) = (node.input_channels.min(8), node.output_channels.min(8));

        let height = 90.0 + ((input_ports.max(output_ports)).saturating_sub(2) as f32 * 8.0);
        let workflow_node = WorkflowNodeData::new(
            plugin_type.name(),
            Position::new(node.position.x, node.position.y),
        )
        .with_ports(input_ports, output_ports)
        .with_size(160.0, height)
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_PLUGIN,
            "plugin_type": format!("{:?}", plugin_type),
            "plugin_node_id": graph_node_id.to_string(),
            "enabled": node.plugin.enabled,
        }));

        let workflow_id = workflow_node.id;
        id_map.insert(*graph_node_id, workflow_id);
        workflow_graph.add_node(workflow_node);
    }

    // Convert connections
    for conn in &graph.connections {
        if let (Some(&from_id), Some(&to_id)) =
            (id_map.get(&conn.from_node), id_map.get(&conn.to_node))
        {
            let _ = workflow_graph.add_connection(from_id, conn.from_port, to_id, conn.to_port);
        }
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
        port_input: theme.info,     // Blue for input ports
        port_output: theme.success, // Green for output ports
        port_hover: theme.accent_hover,
        port_valid: theme.success,
        port_invalid: theme.error,
        port_radius: 6.0,
        connection_color: theme.text_secondary,
        connection_selected: theme.accent,
        connection_width: 4.0,      // Fat links (all channels)
        connection_width_thin: 1.5, // Thin links (single channel)
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

/// Build menu items for the workflow canvas context menu
fn build_menu_items(
    input_devices: &[AudioDevice],
    output_devices: &[AudioDevice],
) -> Vec<MenuItem> {
    let mut items = Vec::new();

    // Input sources section
    items.push(MenuItem::new("input-header", "Input Sources").disabled(true));
    items.push(MenuItem::new("input-player", "Player (Audio Files)"));

    // Add hardware input devices
    for (idx, device) in input_devices.iter().enumerate() {
        let channels = device
            .default_config
            .as_ref()
            .map(|c| c.channels)
            .unwrap_or(2);
        let name = format!("{} ({} ch)", device.name, channels);
        items.push(MenuItem::new(format!("input-device-{}", idx), name));
    }

    items.push(MenuItem::separator());

    // Output devices section
    items.push(MenuItem::new("output-header", "Output Devices").disabled(true));
    for (idx, device) in output_devices.iter().enumerate() {
        let channels = device
            .default_config
            .as_ref()
            .map(|c| c.channels)
            .unwrap_or(2);
        let name = format!("{} ({} ch)", device.name, channels);
        items.push(MenuItem::new(format!("output-{}", idx), name));
    }
    if output_devices.is_empty() {
        items.push(MenuItem::new("no-outputs", "(no output devices)").disabled(true));
    }

    items.push(MenuItem::separator());

    // Plugins section
    items.push(MenuItem::new("plugins-header", "Plugins").disabled(true));
    items.push(MenuItem::new("plugin-eq", "Parametric EQ"));
    items.push(MenuItem::new("plugin-gain", "Gain"));
    items.push(MenuItem::new("plugin-compressor", "Compressor"));
    items.push(MenuItem::new("plugin-limiter", "Limiter"));
    items.push(MenuItem::new("plugin-gate", "Gate"));
    items.push(MenuItem::new("plugin-upmixer", "Upmixer"));
    items.push(MenuItem::new("plugin-downmix", "Downmix"));
    items.push(MenuItem::new("plugin-mono-to-stereo", "Mono to Stereo"));
    items.push(MenuItem::new("plugin-binaural", "Binaural Decoder"));
    items.push(MenuItem::new("plugin-convolution", "Convolution"));
    items.push(MenuItem::new(
        "plugin-loudness-comp",
        "Loudness Compensation",
    ));
    items.push(MenuItem::new("plugin-loudness-mon", "Loudness Monitor"));
    items.push(MenuItem::new("plugin-spectrum", "Spectrum Analyzer"));
    items.push(MenuItem::new("plugin-mute-solo", "Channel Mute/Solo"));

    items
}

// ============================================================================
// Plugin Node Modal
// ============================================================================

impl PlayerView {
    /// Render the plugin node editor modal
    pub(crate) fn render_plugin_node_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        // Get the node being edited
        let node_id = state.app.plugin_state.editing_plugin_node;
        let node_info = node_id.and_then(|id| {
            state
                .app
                .plugin_state
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

        // Look up the actual plugin settings from the plugin graph
        let plugin_settings = plugin_node_id.as_ref().and_then(|id_str| {
            sotf_audio_player::GraphNodeId::parse_str(id_str)
                .ok()
                .and_then(|uuid| {
                    state
                        .app
                        .plugin_state
                        .plugin_graph
                        .as_ref()
                        .and_then(|graph| graph.nodes.get(&uuid))
                        .map(|node| node.plugin.settings.clone())
                })
        });

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

        // Create the modal
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.overlay_bg)
            .on_mouse_down(MouseButton::Left, {
                let state = state_for_close.clone();
                move |_, _, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                        state.app.plugin_state.editing_plugin_node = None;
                    });
                }
            })
            .child(
                div()
                    .id("plugin-node-modal")
                    .w(rems(43.75))
                    .max_h(rems(37.5))
                    .bg(theme.surface)
                    .rounded_lg()
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
                            .px_4()
                            .py_3()
                            .bg(theme.background_secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    // Load button
                                    .child(
                                        div()
                                            .id("modal-load")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.surface)
                                            .text_sm()
                                            .text_color(theme.text_secondary)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.surface_hover))
                                            .child("Load"),
                                    )
                                    // Save button
                                    .child(
                                        div()
                                            .id("modal-save")
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.surface)
                                            .text_sm()
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
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(theme.error)
                                            .text_sm()
                                            .text_color(theme.text_on_accent)
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.8))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                cx.stop_propagation();
                                                state.update(cx, |state, _cx| {
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::Normal;
                                                    state.app.plugin_state.editing_plugin_node =
                                                        None;
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
                            .p_4()
                            .child(self.render_plugin_node_content(
                                &node_type,
                                plugin_type.as_deref(),
                                plugin_settings.as_ref(),
                                &theme,
                                cx,
                            )),
                    ),
            )
    }

    /// Render the content for a plugin node based on its type
    fn render_plugin_node_content(
        &self,
        node_type: &str,
        plugin_type: Option<&str>,
        plugin_settings: Option<&PluginSettings>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node_type {
            NODE_TYPE_PLUGIN => {
                // If we have actual plugin settings, render the real plugin UI
                if let Some(settings) = plugin_settings {
                    return self.render_plugin_settings_ui(settings, theme, cx);
                }

                // Fallback: show placeholder based on plugin type
                match plugin_type {
                    Some("EQ") => div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Parametric EQ"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("No plugin data available"),
                        )
                        .into_any_element(),
                    Some("Gain") => div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Gain"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("No plugin data available"),
                        )
                        .into_any_element(),
                    _ => div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child(format!("Plugin type: {}", plugin_type.unwrap_or("Unknown")))
                        .into_any_element(),
                }
            }
            NODE_TYPE_PLAYER => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Audio Player"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("This node represents the audio file playback source."),
                )
                .into_any_element(),
            NODE_TYPE_OUTPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Output Device"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("This node represents the audio output destination."),
                )
                .into_any_element(),
            NODE_TYPE_INPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Input Device"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("This node represents an audio input source."),
                )
                .into_any_element(),
            _ => div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("Unknown node type")
                .into_any_element(),
        }
    }

    /// Render the actual plugin UI based on settings
    fn render_plugin_settings_ui(
        &self,
        settings: &PluginSettings,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = self.state.clone();
        let plugin_idx = 0; // Modal doesn't need actual index for display

        match settings {
            PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } => render_eq_plugin(
                entity,
                plugin_idx,
                ui_eq::EqRenderState {
                    channels: *channels,
                    filters,
                    channel_filters,
                    per_channel_mode: *per_channel_mode,
                    is_editing: false,
                    selected_param: 0,
                    selected_band_idx: 0,
                },
                theme,
                cx,
            )
            .into_any_element(),

            PluginSettings::Gain { gain_db, .. } => render_gain_plugin(
                entity,
                plugin_idx,
                ui_gain::GainRenderState {
                    gain_db: *gain_db,
                    is_editing: false,
                    selected_param: 0,
                },
                theme,
            )
            .into_any_element(),

            PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => render_compressor_plugin(
                entity,
                plugin_idx,
                ui_compressor::CompressorRenderState {
                    threshold_db: *threshold_db,
                    ratio: *ratio,
                    attack_ms: *attack_ms,
                    release_ms: *release_ms,
                    knee_db: *knee_db,
                    makeup_gain_db: *makeup_gain_db,
                    mix: *mix,
                    auto_makeup: *auto_makeup,
                    link_channels: *link_channels,
                    sidechain_hpf_hz: *sidechain_hpf_hz,
                    is_editing: false,
                    selected_param: 0,
                    data: None,
                },
                theme,
            )
            .into_any_element(),

            PluginSettings::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => render_limiter_plugin(
                entity,
                plugin_idx,
                ui_limiter::LimiterRenderState {
                    threshold_db: *threshold_db,
                    release_ms: *release_ms,
                    lookahead_ms: *lookahead_ms,
                    soft: *soft,
                    mix: *mix,
                    is_editing: false,
                    selected_param: 0,
                },
                theme,
            )
            .into_any_element(),

            PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => render_gate_plugin(
                entity,
                plugin_idx,
                ui_gate::GateRenderState {
                    threshold_db: *threshold_db,
                    ratio: *ratio,
                    attack_ms: *attack_ms,
                    hold_ms: *hold_ms,
                    release_ms: *release_ms,
                    mix: *mix,
                    link_channels: *link_channels,
                    sidechain_hpf_hz: *sidechain_hpf_hz,
                    is_editing: false,
                    selected_param: 0,
                    data: None,
                },
                theme,
            )
            .into_any_element(),

            PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
                enable_ml_detection,
                ..
            } => render_upmixer_plugin(
                entity,
                plugin_idx,
                ui_upmixer::UpmixerRenderState {
                    speaker_config,
                    gain_front_direct: *gain_front_direct,
                    gain_front_ambient: *gain_front_ambient,
                    gain_rear_ambient: *gain_rear_ambient,
                    height_gain: *height_gain,
                    stereo_width: *stereo_width,
                    center_spread: *center_spread,
                    surround_direct_bleed: *surround_direct_bleed,
                    rear_late_reflection: *rear_late_reflection,
                    lfe_cutoff_hz: *lfe_cutoff_hz,
                    lfe_gain: *lfe_gain,
                    bandpass_hz: *bandpass_hz,
                    enable_subharmonic_synth: *enable_subharmonic_synth,
                    subharmonic_gain: *subharmonic_gain,
                    subharmonic_freq_hz: *subharmonic_freq_hz,
                    subharmonic_attack_ms: *subharmonic_attack_ms,
                    subharmonic_release_ms: *subharmonic_release_ms,
                    decorrelation_mode: *decorrelation_mode,
                    decorrelation_lfo_rate_hz: *decorrelation_lfo_rate_hz,
                    velvet_noise_duration_ms: *velvet_noise_duration_ms,
                    velvet_noise_density: *velvet_noise_density,
                    enable_hr_direct: *enable_hr_direct,
                    hr_sharpen: *hr_sharpen,
                    height_hf_cap_hz: *height_hf_cap_hz,
                    height_transient_reduction: *height_transient_reduction,
                    height_direct_leak: *height_direct_leak,
                    ambient_boost: *ambient_boost,
                    safety_cap_db: *safety_cap_db,
                    rear_ambient_boost: *rear_ambient_boost,
                    dialogue_weight: *dialogue_weight,
                    voice_freq_min_hz: *voice_freq_min_hz,
                    voice_freq_max_hz: *voice_freq_max_hz,
                    dialogue_centroid_weight: *dialogue_centroid_weight,
                    dialogue_variance_weight: *dialogue_variance_weight,
                    dialogue_coherence_weight: *dialogue_coherence_weight,
                    bypass_decorrelation: *bypass_decorrelation,
                    bypass_transient_detection: *bypass_transient_detection,
                    bypass_all_processing: *bypass_all_processing,
                    enable_ml_detection: *enable_ml_detection,
                    is_editing: false,
                    selected_param: 0,
                    config_open: false,
                },
                theme,
            )
            .into_any_element(),

            PluginSettings::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => render_downmix_plugin(
                entity,
                plugin_idx,
                ui_downmix::DownmixRenderState {
                    center_gain_db: *center_gain_db,
                    surround_gain_db: *surround_gain_db,
                    height_gain_db: *height_gain_db,
                    lfe_gain_db: *lfe_gain_db,
                    phase_coherence: *phase_coherence,
                    phase_blend_low_hz: *phase_blend_low_hz,
                    phase_blend_high_hz: *phase_blend_high_hz,
                    is_editing: false,
                    selected_param: 0,
                },
                theme,
            )
            .into_any_element(),

            PluginSettings::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => render_mono_to_stereo_plugin(
                entity,
                plugin_idx,
                ui_mono_to_stereo::MonoToStereoRenderState {
                    stereo_width: *stereo_width,
                    haas_delay_ms: *haas_delay_ms,
                    enable_comp_eq: *enable_comp_eq,
                    comp_eq_depth_db: *comp_eq_depth_db,
                    decor_low_hz: *decor_low_hz,
                    decor_high_hz: *decor_high_hz,
                    is_editing: false,
                    selected_param: 0,
                },
                theme,
            )
            .into_any_element(),

            // For other plugins, show a simple description
            _ => {
                let name = settings.plugin_type().name().to_string();
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child("Plugin controls not yet implemented for this type"),
                    )
                    .into_any_element()
            }
        }
    }
}
