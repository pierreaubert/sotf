use super::consts::MAX_WORKFLOW_PORTS;
use super::consts::NODE_TYPE_OUTPUT_DEVICE;
use super::consts::NODE_TYPE_PLAYER;
use super::consts::NODE_TYPE_PLUGIN;
use crate::theme::Theme;
use gpui_ui_kit::workflow::{Position, WorkflowGraph, WorkflowNodeData, WorkflowTheme};

/// Create the default graph: Player → EQ → Output
pub(super) fn create_default_graph(output_name: &str, output_channels: usize) -> WorkflowGraph {
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
    let output_channels_clamped = output_channels.min(MAX_WORKFLOW_PORTS);
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

/// Create a WorkflowTheme from the app Theme
pub(super) fn create_workflow_theme(theme: &Theme) -> WorkflowTheme {
    WorkflowTheme {
        canvas_background: theme.background,
        grid_color: Theme::with_opacity(theme.border, 0.3),
        grid_spacing: 20.0,
        node_background: theme.surface,
        node_border: theme.border,
        node_border_selected: theme.accent,
        node_header: theme.background_secondary,
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
        connection_preview: Theme::with_opacity(theme.accent, 0.6),
        selection_fill: Theme::with_opacity(theme.accent, 0.1),
        selection_border: theme.accent,
    }
}
