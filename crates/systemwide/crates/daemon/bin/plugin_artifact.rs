use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::engine::PluginGraphConfig;

#[derive(Debug, Clone)]
pub enum PluginArtifactPlan {
    RackChain { plugins: Vec<PluginConfig> },
    Graph { graph: PluginGraphConfig },
    UnsupportedGraph { reason: String },
}

pub fn plan_plugin_artifact(artifact: Value) -> Result<PluginArtifactPlan, String> {
    match artifact {
        Value::Array(items) => parse_rack_chain(items),
        Value::Object(mut object) => {
            if let Some(graph) = object.remove("graph") {
                return parse_graph_value(graph);
            }
            if object.contains_key("nodes") || object.contains_key("edges") {
                return parse_graph_value(Value::Object(object));
            }
            if has_graph_topology_keys(&object) {
                return Ok(PluginArtifactPlan::UnsupportedGraph {
                    reason: "artifact uses a graph representation without engine nodes/edges"
                        .to_string(),
                });
            }

            if object.contains_key("channels") {
                return Ok(PluginArtifactPlan::UnsupportedGraph {
                    reason: "artifact contains per-channel plugin topology".to_string(),
                });
            }

            if let Some(plugins) = object.remove("plugins") {
                return parse_plugin_array_value(plugins, "plugins");
            }
            if let Some(plugins) = object.remove("global_plugins") {
                return parse_plugin_array_value(plugins, "global_plugins");
            }

            Err("plugin artifact must be an array or contain a plugins/global_plugins array".into())
        }
        _ => Err("plugin artifact must be an array or object".into()),
    }
}

fn parse_graph_value(value: Value) -> Result<PluginArtifactPlan, String> {
    let graph: PluginGraphConfig =
        serde_json::from_value(value).map_err(|error| format!("invalid plugin graph: {error}"))?;
    graph
        .validate()
        .map_err(|error| format!("invalid plugin graph: {error}"))?;
    if graph.nodes.is_empty() {
        return Err("plugin graph must contain at least one node".to_string());
    }
    if let Some(node) = graph
        .nodes
        .iter()
        .find(|node| is_system_plugin_type(&node.plugin_type))
    {
        return Err(format!(
            "plugin graph node {} uses daemon-owned system plugin type '{}'",
            node.id, node.plugin_type
        ));
    }
    Ok(PluginArtifactPlan::Graph { graph })
}

fn has_graph_topology_keys(object: &serde_json::Map<String, Value>) -> bool {
    ["graph", "nodes", "edges", "routes", "routing", "buses"]
        .iter()
        .any(|key| object.contains_key(*key))
}

fn parse_plugin_array_value(value: Value, field: &str) -> Result<PluginArtifactPlan, String> {
    match value {
        Value::Array(items) => parse_rack_chain(items),
        _ => Err(format!("{} must be an array", field)),
    }
}

fn parse_rack_chain(items: Vec<Value>) -> Result<PluginArtifactPlan, String> {
    let mut plugins = Vec::new();
    for item in items {
        if let Some(plugin) = parse_plugin_entry(item)? {
            plugins.push(plugin);
        }
    }

    if plugins.is_empty() {
        return Err("rack-compatible artifact did not contain any user plugins".into());
    }

    Ok(PluginArtifactPlan::RackChain { plugins })
}

fn parse_plugin_entry(item: Value) -> Result<Option<PluginConfig>, String> {
    let Value::Object(mut object) = item else {
        return Err("plugin entries must be objects".into());
    };

    let plugin_type = object
        .remove("plugin_type")
        .or_else(|| object.remove("type"))
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .ok_or_else(|| "plugin entry is missing plugin_type/type".to_string())?;

    if is_system_plugin_type(&plugin_type) {
        return Ok(None);
    }

    let parameters = object.remove("parameters").unwrap_or(Value::Object(object));
    Ok(Some(PluginConfig {
        plugin_type,
        parameters,
    }))
}

fn is_system_plugin_type(plugin_type: &str) -> bool {
    matches!(
        plugin_type,
        "hal_input" | "hal_output" | "loudness_monitor" | "spectrum_analyzer"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_artifact_plans_as_rack_chain() {
        let artifact = serde_json::json!([
            { "plugin_type": "eq", "parameters": { "gain_db": 1.0 } },
            { "type": "gain", "parameters": { "gain_db": -3.0 } }
        ]);

        let plan = plan_plugin_artifact(artifact).expect("valid rack artifact");
        match plan {
            PluginArtifactPlan::RackChain { plugins } => {
                assert_eq!(plugins.len(), 2);
                assert_eq!(plugins[0].plugin_type, "eq");
                assert_eq!(plugins[1].plugin_type, "gain");
            }
            PluginArtifactPlan::Graph { .. } => panic!("unexpected graph artifact"),
            PluginArtifactPlan::UnsupportedGraph { reason } => {
                panic!("unexpected graph artifact: {}", reason)
            }
        }
    }

    #[test]
    fn graph_topology_is_not_flattened_into_rack_chain() {
        let artifact = serde_json::json!({
            "global_plugins": [{ "plugin_type": "eq", "parameters": {} }],
            "channels": {
                "L": { "plugins": [{ "plugin_type": "gain", "parameters": {} }] }
            }
        });

        let plan = plan_plugin_artifact(artifact).expect("recognized graph artifact");
        match plan {
            PluginArtifactPlan::UnsupportedGraph { reason } => {
                assert!(reason.contains("per-channel"));
            }
            PluginArtifactPlan::Graph { .. } => {
                panic!("per-channel artifact is not an engine graph")
            }
            PluginArtifactPlan::RackChain { .. } => panic!("graph artifact was flattened"),
        }
    }

    #[test]
    fn engine_graph_artifact_preserves_nodes_edges_and_parameters() {
        let artifact = serde_json::json!({
            "graph": {
                "nodes": [
                    {
                        "id": 10,
                        "plugin_type": "gain",
                        "parameters": {"gain_db": -3.0},
                        "input_channels": 2,
                        "bypassed": true
                    },
                    {
                        "id": 20,
                        "plugin_type": "eq",
                        "parameters": {"filters": []},
                        "input_channels": 2
                    }
                ],
                "edges": [{"from_node": 10, "to_node": 20}]
            }
        });

        let plan = plan_plugin_artifact(artifact).unwrap();
        let PluginArtifactPlan::Graph { graph } = plan else {
            panic!("expected graph plan");
        };
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.nodes[0].id, 10);
        assert_eq!(graph.nodes[0].parameters["gain_db"], -3.0);
        assert!(graph.nodes[0].bypassed);
        assert_eq!(graph.edges[0].from_node, 10);
        assert_eq!(graph.edges[0].to_node, 20);
    }

    #[test]
    fn engine_graph_artifact_rejects_cycles_and_daemon_owned_nodes() {
        let cycle = serde_json::json!({
            "nodes": [
                {"id": 1, "plugin_type": "gain", "parameters": {}, "input_channels": 2},
                {"id": 2, "plugin_type": "gain", "parameters": {}, "input_channels": 2}
            ],
            "edges": [
                {"from_node": 1, "to_node": 2},
                {"from_node": 2, "to_node": 1}
            ]
        });
        assert!(plan_plugin_artifact(cycle).unwrap_err().contains("acyclic"));

        let system_node = serde_json::json!({
            "nodes": [{
                "id": 1,
                "plugin_type": "loudness_monitor",
                "parameters": {},
                "input_channels": 2
            }],
            "edges": []
        });
        assert!(
            plan_plugin_artifact(system_node)
                .unwrap_err()
                .contains("daemon-owned")
        );
    }
}
