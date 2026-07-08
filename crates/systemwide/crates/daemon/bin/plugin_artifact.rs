use serde_json::Value;
use sotf_audio::PluginConfig;

#[derive(Debug, Clone)]
pub enum PluginArtifactPlan {
    RackChain { plugins: Vec<PluginConfig> },
    UnsupportedGraph { reason: String },
}

pub fn plan_plugin_artifact(artifact: Value) -> Result<PluginArtifactPlan, String> {
    match artifact {
        Value::Array(items) => parse_rack_chain(items),
        Value::Object(mut object) => {
            if has_graph_topology_keys(&object) {
                return Ok(PluginArtifactPlan::UnsupportedGraph {
                    reason: "artifact contains graph topology fields".to_string(),
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
            PluginArtifactPlan::RackChain { .. } => panic!("graph artifact was flattened"),
        }
    }
}
