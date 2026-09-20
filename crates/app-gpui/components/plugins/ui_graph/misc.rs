use gpui::*;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};

/// Resolve a stable plugin UUID in the routing draft, including non-linear graphs.
pub(super) fn resolve_plugin_node(
    canvas: &Entity<WorkflowCanvas>,
    state: &Entity<crate::app::AppState>,
    node_id: NodeId,
    cx: &mut App,
) -> Option<sotf_audio_player::GraphNodeId> {
    let plugin_uuid = canvas
        .read(cx)
        .graph()
        .nodes
        .get(&node_id)
        .and_then(|n| n.user_data.get("plugin_node_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| sotf_audio_player::GraphNodeId::parse_str(s).ok())?;
    state
        .read(cx)
        .app
        .plugin_state
        .routing_controller()
        .graph
        .nodes
        .contains_key(&plugin_uuid)
        .then_some(plugin_uuid)
}
