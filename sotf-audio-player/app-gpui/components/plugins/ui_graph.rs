//! Plugin Graph Screen
//!
//! Full-screen workflow canvas for node-based plugin editing.
//! Uses the WorkflowCanvas from gpui-ui-kit for pan/zoom, connections, and hit testing.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{
    Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData, WorkflowTheme,
};
use gpui_ui_kit::MenuItem;
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{PluginGraph, SpecialNodeType};

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

            // Set theme and menu items
            let state = self.state.read(cx);
            let theme = state.app.theme.clone();
            let input_devices = state.app.input_devices.clone();
            let output_devices = state.app.output_devices.clone();
            let workflow_theme = create_workflow_theme(&theme);
            let menu_items = build_menu_items(&input_devices, &output_devices);

            canvas.update(cx, |canvas, _cx| {
                canvas.set_theme(workflow_theme);
                canvas.set_menu_items(menu_items);
            });

            // Store the canvas entity
            self.state.update(cx, |state, _cx| {
                state.app.workflow_canvas = Some(canvas);
            });
        }
    }

    /// Render the plugin graph screen with workflow canvas
    pub(crate) fn render_plugin_graph_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure the canvas entity exists
        self.ensure_workflow_canvas(cx);

        let (theme, workflow_canvas) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.workflow_canvas.clone())
        };

        div()
            .id("plugin-graph-screen")
            .size_full()
            .bg(theme.background)
            .when_some(workflow_canvas, |el, canvas| el.child(canvas))
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

    // Convert special nodes (Input/Output devices) to workflow nodes
    for (_special_node_id, special_node) in &graph.special_nodes {
        let (input_ports, output_ports) = match special_node.node_type {
            SpecialNodeType::Input => (0, special_node.channels.min(8)),
            SpecialNodeType::Output => (special_node.channels.min(8), 0),
            SpecialNodeType::Split => (1, special_node.channels.min(8)),
            SpecialNodeType::Merge => (special_node.channels.min(8), 1),
        };

        let workflow_node = WorkflowNodeData::new(
            &special_node.display_name(),
            Position::new(special_node.position.x, special_node.position.y),
        )
        .with_ports(input_ports, output_ports)
        .with_size(180.0, 70.0)
        .with_user_data(serde_json::json!({
            "node_type": format!("{:?}", special_node.node_type),
            "channels": special_node.channels,
            "is_special": true,
        }));

        workflow_graph.add_node(workflow_node);
    }

    // Convert plugin nodes to workflow nodes
    for (_graph_node_id, node) in &graph.nodes {
        let plugin_type = node.plugin.plugin_type();

        let workflow_node = WorkflowNodeData::new(
            plugin_type.name(),
            Position::new(node.position.x, node.position.y),
        )
        .with_ports(node.input_channels.min(8), node.output_channels.min(8))
        .with_size(180.0, 90.0)
        .with_user_data(serde_json::json!({
            "plugin_type": format!("{:?}", plugin_type),
            "enabled": node.plugin.enabled,
            "is_special": false,
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

/// Build menu items for the workflow canvas context menu
fn build_menu_items(input_devices: &[AudioDevice], output_devices: &[AudioDevice]) -> Vec<MenuItem> {
    let mut items = Vec::new();

    // Input devices section
    items.push(MenuItem::new("input-header", "Input Devices").disabled(true));
    for (idx, device) in input_devices.iter().enumerate() {
        let channels = device
            .default_config
            .as_ref()
            .map(|c| c.channels)
            .unwrap_or(2);
        let name = format!("{} ({} ch)", device.name, channels);
        items.push(MenuItem::new(format!("input-{}", idx), name));
    }
    if input_devices.is_empty() {
        items.push(MenuItem::new("no-inputs", "(no input devices)").disabled(true));
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
    items.push(MenuItem::new("plugin-binaural", "Binaural Decoder"));
    items.push(MenuItem::new("plugin-convolution", "Convolution"));
    items.push(MenuItem::new("plugin-loudness-comp", "Loudness Compensation"));
    items.push(MenuItem::new("plugin-loudness-mon", "Loudness Monitor"));
    items.push(MenuItem::new("plugin-spectrum", "Spectrum Analyzer"));
    items.push(MenuItem::new("plugin-mute-solo", "Channel Mute/Solo"));

    items
}
