use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::engine::PluginGraphConfig;

pub(super) fn pipeline_spec_to_json(spec: &PipelineSpec) -> Value {
    serde_json::json!({
        "output_device": spec.output_device,
        "input_channels": spec.input_channels,
        "output_channels": spec.output_channels,
        "user_plugin_count": spec.user_plugins.len(),
        "topology": if spec.user_graph.is_some() { "graph" } else { "rack" },
        "user_graph_node_count": spec.user_graph.as_ref().map_or(0, |graph| graph.nodes.len()),
        "user_graph_edge_count": spec.user_graph.as_ref().map_or(0, |graph| graph.edges.len()),
        "user_plugin_types": spec
            .user_plugins
            .iter()
            .map(|p| p.plugin_type.as_str())
            .collect::<Vec<_>>(),
    })
}

#[derive(Clone, Debug)]
pub(super) struct PipelineSpec {
    pub(super) output_device: Option<String>,
    pub(super) user_plugins: Vec<PluginConfig>,
    pub(super) user_graph: Option<PluginGraphConfig>,
    pub(super) input_channels: usize,
    pub(super) output_channels: usize,
}

/// Whether two pipeline specs describe the same audio graph: same device,
/// same channel geometry, and same user plugins/graph (compared by value, so
/// key order and float formatting cannot cause a spurious restart).
pub(super) fn pipeline_specs_match(first: &PipelineSpec, second: &PipelineSpec) -> bool {
    if first.output_device != second.output_device
        || first.input_channels != second.input_channels
        || first.output_channels != second.output_channels
    {
        return false;
    }
    let plugins_match = match (
        serde_json::to_value(&first.user_plugins),
        serde_json::to_value(&second.user_plugins),
    ) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    };
    if !plugins_match {
        return false;
    }
    match (
        serde_json::to_value(&first.user_graph),
        serde_json::to_value(&second.user_graph),
    ) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
}

impl Default for PipelineSpec {
    fn default() -> Self {
        Self {
            output_device: None,
            user_plugins: Vec::new(),
            user_graph: None,
            input_channels: 2,
            output_channels: 2,
        }
    }
}
