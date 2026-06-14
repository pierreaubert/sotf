use gpui::*;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};

/// Resolve the workflow node id to a `(plugin_uuid, linear_index)` pair so
/// the same dispatch can address the plugin via either its stable UUID
/// (used by `editing_graph_node_uuid`) or its linear index (used by
/// `PluginEditingManager::toggle_plugin` etc.). Returns None if the
/// workflow node isn't a plugin node — the menu was registered only for
/// plugin nodes, but this defensively handles user_data drift.
pub(super) fn resolve_plugin_node(
    canvas: &Entity<WorkflowCanvas>,
    state: &Entity<crate::app::AppState>,
    node_id: NodeId,
    cx: &mut App,
) -> Option<(sotf_audio_player::GraphNodeId, usize)> {
    let plugin_uuid = canvas
        .read(cx)
        .graph()
        .nodes
        .get(&node_id)
        .and_then(|n| n.user_data.get("plugin_node_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| sotf_audio_player::GraphNodeId::parse_str(s).ok())?;

    let plugin_index = state
        .read(cx)
        .app
        .plugin_state
        .graph
        .plugins_linear()?
        .iter()
        .position(|n| n.id == plugin_uuid)?;

    Some((plugin_uuid, plugin_index))
}
