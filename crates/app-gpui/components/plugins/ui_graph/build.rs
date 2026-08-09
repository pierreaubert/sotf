use super::consts::convert_plugin_graph;
use super::create::create_default_graph;
use gpui_ui_kit::workflow::WorkflowGraph;
use sotf_audio_player::PluginGraph;

/// Build a WorkflowGraph from the PluginGraph, or create a default graph
pub(super) fn build_workflow_graph(
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
