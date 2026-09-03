use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::engine::PluginGraphConfig;
use sotf_audio_player::room_eq_types::{DspChainOutput, build_room_eq_plugin_graph_config};

const MAX_PLUGIN_ARTIFACT_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub enum PluginArtifactPlan {
    RackChain { plugins: Vec<PluginConfig> },
    Graph { graph: PluginGraphConfig },
    UnsupportedGraph { reason: String },
}

#[derive(Debug, Clone)]
pub struct PluginArtifactFilePlan {
    pub plan: PluginArtifactPlan,
    pub required_channels: Option<usize>,
}

/// Load a configuration directly in the daemon so large RoomEQ artifacts do
/// not get materialised as Foundation dictionaries, re-encoded, and copied
/// through the line-oriented control socket.
pub fn plan_plugin_artifact_file(
    path: &std::path::Path,
    sample_rate: f64,
) -> Result<PluginArtifactFilePlan, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot open configuration '{}': {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect configuration '{}': {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "configuration '{}' is not a regular file",
            path.display()
        ));
    }
    if metadata.len() > MAX_PLUGIN_ARTIFACT_FILE_BYTES {
        return Err(format!(
            "configuration is too large ({} MiB; maximum is {} MiB)",
            metadata.len() / (1024 * 1024),
            MAX_PLUGIN_ARTIFACT_FILE_BYTES / (1024 * 1024)
        ));
    }

    let room_eq_file = file
        .try_clone()
        .map_err(|error| format!("cannot read configuration '{}': {error}", path.display()))?;
    let room_eq_result: Result<DspChainOutput, _> =
        serde_json::from_reader(std::io::BufReader::new(std::io::Read::take(
            room_eq_file,
            MAX_PLUGIN_ARTIFACT_FILE_BYTES + 1,
        )));
    if let Ok(output) = room_eq_result
        && !output.channels.is_empty()
    {
        let graph = build_room_eq_plugin_graph_config(&output, sample_rate)
            .map_err(|error| format!("invalid RoomEQ configuration: {error}"))?;
        let required_channels = graph
            .nodes
            .iter()
            .map(|node| node.input_channels)
            .max()
            .unwrap_or(output.channels.len());
        return Ok(PluginArtifactFilePlan {
            plan: PluginArtifactPlan::Graph { graph },
            required_channels: Some(required_channels),
        });
    }

    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind configuration '{}': {error}", path.display()))?;
    let artifact: Value = serde_json::from_reader(std::io::BufReader::new(std::io::Read::take(
        file,
        MAX_PLUGIN_ARTIFACT_FILE_BYTES + 1,
    )))
    .map_err(|error| format!("invalid JSON configuration: {error}"))?;
    Ok(PluginArtifactFilePlan {
        plan: plan_plugin_artifact(artifact)?,
        required_channels: None,
    })
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
    fn room_eq_file_builds_channel_accurate_graph() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("room-eq.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "version": "0.5",
                "channels": {
                    "L": {"channel": "L", "plugins": [{"plugin_type": "gain", "parameters": {"gain_db": -1.0}}]},
                    "R": {"channel": "R", "plugins": [{"plugin_type": "gain", "parameters": {"gain_db": -2.0}}]}
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let file_plan = plan_plugin_artifact_file(&path, 48_000.0).unwrap();
        assert_eq!(file_plan.required_channels, Some(2));
        assert!(matches!(file_plan.plan, PluginArtifactPlan::Graph { .. }));
    }

    #[test]
    fn room_eq_v2_asymmetric_iir_file_builds_the_expected_graph() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dsp-iir.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "version": "2.1.0",
                "channels": {
                    "L": {
                        "channel": "L",
                        "plugins": [{
                            "plugin_type": "eq",
                            "parameters": {
                                "label": "room_eq_correction",
                                "filters": [{
                                    "filter_type": "peak",
                                    "freq": 55.8,
                                    "q": 2.07,
                                    "db_gain": -9.0
                                }]
                            }
                        }]
                    },
                    "R": {
                        "channel": "R",
                        "plugins": [
                            {
                                "plugin_type": "delay",
                                "parameters": {"delay_ms": 0.19}
                            },
                            {
                                "plugin_type": "eq",
                                "parameters": {
                                    "label": "room_eq_correction",
                                    "filters": [{
                                        "filter_type": "peak",
                                        "freq": 86.8,
                                        "q": 3.0,
                                        "db_gain": 3.0
                                    }]
                                }
                            }
                        ]
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let file_plan = plan_plugin_artifact_file(&path, 48_000.0).unwrap();
        assert_eq!(file_plan.required_channels, Some(2));
        let PluginArtifactPlan::Graph { graph } = file_plan.plan else {
            panic!("RoomEQ v2.1 artifact did not produce a graph");
        };
        assert_eq!(graph.nodes.len(), 5);
        assert_eq!(graph.edges.len(), 4);
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.plugin_type == "delay")
                .count(),
            1
        );
    }

    #[test]
    #[ignore = "requires SOTF_GENERATED_ROOM_EQ_DIR"]
    fn all_generated_room_eq_files_build_graphs() {
        fn visit(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(&path, files);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    files.push(path);
                }
            }
        }

        let root = std::env::var_os("SOTF_GENERATED_ROOM_EQ_DIR")
            .expect("set SOTF_GENERATED_ROOM_EQ_DIR to the measured artifact directory");
        let mut files = Vec::new();
        visit(std::path::Path::new(&root), &mut files);
        assert!(!files.is_empty(), "no JSON artifacts found");
        for path in files {
            let plan = plan_plugin_artifact_file(&path, 48_000.0)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                plan.required_channels.is_some(),
                "{} did not produce a RoomEQ graph",
                path.display()
            );
        }
    }

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
