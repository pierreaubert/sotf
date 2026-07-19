//! Business logic for managing A/B Compare plugin sub-racks.
//!
//! Pure functions that convert between `Vec<PluginInRack>` and JSON `PathConfig`,
//! and perform add/remove/move operations on the plugin list.

use crate::plugin_graph::{GraphNodeId, PluginGraph, SpecialNodeType};
use std::collections::{HashMap, HashSet};

pub use sotf_plugins::plugin_ab_compare::{
    GraphEdgeConfig, GraphNodeConfig, PathConfig, PluginInRack,
};

/// Plugin types available in A/B sub-racks, derived from the canonical plugin
/// catalog. Nested A/B, analyzers, infrastructure, external-discovery entries,
/// and platform I/O are excluded by catalog metadata.
pub fn allowed_plugin_types() -> impl Iterator<Item = (&'static str, &'static str)> {
    sotf_plugins::ab_compare_catalog_entries()
        .map(|entry| (entry.canonical_type, entry.metadata.exposed_name))
}

/// Parse a path config JSON string into a flat list of plugins.
/// Returns empty vec for `None`, single-element vec for `Plugin`, full vec for `Rack`.
/// Graph configs are not editable as a rack and return empty vec.
pub fn parse_path_config(json: &str) -> Vec<PluginInRack> {
    let config: PathConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to parse path config: {e}");
            return Vec::new();
        }
    };

    match config {
        PathConfig::None => Vec::new(),
        PathConfig::Plugin {
            plugin_type,
            parameters,
        } => vec![PluginInRack {
            plugin_type,
            parameters,
        }],
        PathConfig::Rack { plugins } => plugins,
        PathConfig::Graph { .. } => {
            log::warn!("Graph path configs cannot be edited as a rack");
            Vec::new()
        }
    }
}

/// Encode a list of plugins back into a PathConfig JSON string.
pub fn encode_path_config(plugins: &[PluginInRack]) -> String {
    let config = match plugins.len() {
        0 => PathConfig::None,
        _ => PathConfig::Rack {
            plugins: plugins.to_vec(),
        },
    };
    serde_json::to_string(&config).unwrap_or_else(|_| r#"{"type":"None"}"#.to_string())
}

/// Convert an editable player graph into a lossless A/B path configuration.
///
/// A completely identity-wired linear graph is represented as a compact rack.
/// Routed or nonlinear plugin topology is represented as a graph with one edge
/// per channel connection. Input and output special nodes are implicit in the
/// A/B host and therefore must use complete identity boundary wiring.
pub fn path_config_from_plugin_graph(
    graph: &PluginGraph,
    sample_rate: f64,
) -> Result<PathConfig, String> {
    if graph.nodes.is_empty() {
        return Ok(PathConfig::None);
    }

    validate_implicit_io_boundaries(graph)?;

    if let Some(order) = graph.linear_order()
        && order
            .windows(2)
            .all(|pair| has_complete_identity_wiring(graph, pair[0], pair[1]))
    {
        let plugins = order
            .into_iter()
            .filter_map(|id| graph.nodes.get(&id))
            .filter_map(|node| node.plugin.to_plugin_config(sample_rate))
            .map(|config| PluginInRack {
                plugin_type: config.plugin_type,
                parameters: config.parameters,
            })
            .collect();
        return Ok(PathConfig::Rack { plugins });
    }

    let ordered_ids = graph.topological_sort()?;
    let mut ids = HashMap::new();
    let mut nodes = Vec::new();

    for id in ordered_ids {
        let Some(node) = graph.nodes.get(&id) else {
            continue;
        };
        let Some(config) = node.plugin.to_plugin_config(sample_rate) else {
            return Err(format!(
                "nonlinear A/B graph contains disabled or suspended plugin '{}'",
                node.plugin.display_name()
            ));
        };
        let runtime_id = id.to_string();
        ids.insert(id, runtime_id.clone());
        nodes.push(GraphNodeConfig {
            id: runtime_id,
            plugin_type: config.plugin_type,
            parameters: config.parameters,
        });
    }

    let mut edges = Vec::new();
    for connection in &graph.connections {
        match (ids.get(&connection.from_node), ids.get(&connection.to_node)) {
            (Some(from), Some(to)) => edges.push(GraphEdgeConfig {
                from: from.clone(),
                to: to.clone(),
                channel_map: Some(vec![connection.from_port]),
                destination_offset: connection.to_port,
            }),
            (Some(_), None) | (None, Some(_)) => {
                // Validated Input/Output boundaries are implicit in DawHost.
            }
            (None, None) => {
                return Err("A/B graph cannot preserve a special-node-only route".into());
            }
        }
    }

    Ok(PathConfig::Graph { nodes, edges })
}

/// Serialize [`path_config_from_plugin_graph`] for plugin parameters or saved sessions.
pub fn encode_plugin_graph_path_config(
    graph: &PluginGraph,
    sample_rate: f64,
) -> Result<String, String> {
    serde_json::to_string(&path_config_from_plugin_graph(graph, sample_rate)?)
        .map_err(|error| format!("failed to serialize A/B path graph: {error}"))
}

/// Collapse a graph-shaped path to the simple rack representation only when
/// its topology is one complete chain and every channel route is identity.
///
/// Routed linear graphs (crossed, sparse, or offset channels) deliberately
/// remain graphs even though their node topology has no branch.
pub fn simplify_linear_path_config(config: PathConfig) -> PathConfig {
    let PathConfig::Graph { nodes, edges } = &config else {
        return config;
    };
    if nodes.is_empty() {
        return PathConfig::None;
    }

    let node_ids: HashSet<&str> = nodes.iter().map(|node| node.id.as_str()).collect();
    let mut successors: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut predecessors: HashMap<&str, HashSet<&str>> = HashMap::new();
    for edge in edges {
        if !node_ids.contains(edge.from.as_str()) || !node_ids.contains(edge.to.as_str()) {
            return config;
        }
        successors
            .entry(edge.from.as_str())
            .or_default()
            .insert(edge.to.as_str());
        predecessors
            .entry(edge.to.as_str())
            .or_default()
            .insert(edge.from.as_str());
    }
    if node_ids.iter().any(|id| {
        successors.get(id).is_some_and(|next| next.len() > 1)
            || predecessors
                .get(id)
                .is_some_and(|previous| previous.len() > 1)
    }) {
        return config;
    }
    let roots: Vec<_> = node_ids
        .iter()
        .copied()
        .filter(|id| predecessors.get(id).is_none_or(HashSet::is_empty))
        .collect();
    if roots.len() != 1 {
        return config;
    }

    let mut order = Vec::with_capacity(nodes.len());
    let mut current = roots[0];
    loop {
        order.push(current);
        let Some(next) = successors.get(current).and_then(|next| next.iter().next()) else {
            break;
        };
        current = next;
        if order.contains(&current) {
            return config;
        }
    }
    if order.len() != nodes.len() {
        return config;
    }
    if order
        .windows(2)
        .any(|pair| !graph_pair_has_identity_routes(edges, pair[0], pair[1]))
    {
        return config;
    }

    let nodes_by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    PathConfig::Rack {
        plugins: order
            .into_iter()
            .map(|id| {
                let node = nodes_by_id[id];
                PluginInRack {
                    plugin_type: node.plugin_type.clone(),
                    parameters: node.parameters.clone(),
                }
            })
            .collect(),
    }
}

fn graph_pair_has_identity_routes(edges: &[GraphEdgeConfig], from: &str, to: &str) -> bool {
    let pair_edges: Vec<_> = edges
        .iter()
        .filter(|edge| edge.from == from && edge.to == to)
        .collect();
    if pair_edges.len() == 1
        && pair_edges[0].channel_map.is_none()
        && pair_edges[0].destination_offset == 0
    {
        return true;
    }

    let mut routes = Vec::new();
    for edge in pair_edges {
        let Some(source_channels) = &edge.channel_map else {
            return false;
        };
        routes.extend(
            source_channels
                .iter()
                .enumerate()
                .map(|(index, source)| (*source, edge.destination_offset + index)),
        );
    }
    routes.sort_unstable();
    routes.dedup();
    !routes.is_empty()
        && routes
            .iter()
            .enumerate()
            .all(|(channel, route)| *route == (channel, channel))
}

fn has_complete_identity_wiring(graph: &PluginGraph, from: GraphNodeId, to: GraphNodeId) -> bool {
    let channels = graph
        .node_output_channels(from)
        .min(graph.node_input_channels(to));
    let routes: HashSet<(usize, usize)> = graph
        .connections
        .iter()
        .filter(|connection| connection.from_node == from && connection.to_node == to)
        .map(|connection| (connection.from_port, connection.to_port))
        .collect();

    routes.len() == channels && (0..channels).all(|channel| routes.contains(&(channel, channel)))
}

fn validate_implicit_io_boundaries(graph: &PluginGraph) -> Result<(), String> {
    for special in graph.special_nodes.values() {
        if matches!(
            special.node_type,
            SpecialNodeType::Split | SpecialNodeType::Merge
        ) {
            return Err(format!(
                "A/B graph cannot yet preserve the '{}' special node; connect plugin ports directly",
                special.display_name()
            ));
        }
    }

    for connection in &graph.connections {
        let from_special = graph.special_nodes.get(&connection.from_node);
        let to_special = graph.special_nodes.get(&connection.to_node);
        if from_special.is_none() && to_special.is_none() {
            continue;
        }
        if from_special.is_some() && to_special.is_some() {
            return Err("A/B graph cannot preserve a special-node-only route".into());
        }
        if let Some(special) = from_special
            && special.node_type != SpecialNodeType::Input
        {
            return Err("only Input may route from a special node in an A/B graph".into());
        }
        if let Some(special) = to_special
            && special.node_type != SpecialNodeType::Output
        {
            return Err("only Output may route to a special node in an A/B graph".into());
        }
    }

    for (&id, node) in &graph.nodes {
        let plugin_predecessors = graph.connections.iter().any(|connection| {
            connection.to_node == id && graph.nodes.contains_key(&connection.from_node)
        });
        let input_boundaries: Vec<_> = graph
            .connections
            .iter()
            .filter(|connection| {
                connection.to_node == id
                    && graph
                        .special_nodes
                        .get(&connection.from_node)
                        .is_some_and(|special| special.node_type == SpecialNodeType::Input)
            })
            .collect();
        if !input_boundaries.is_empty() {
            if plugin_predecessors
                || !boundary_is_complete_identity(&input_boundaries, node.input_channels)
            {
                return Err(format!(
                    "input boundary for '{}' must be complete identity wiring to a graph root",
                    node.plugin.display_name()
                ));
            }
        }

        let plugin_successors = graph.connections.iter().any(|connection| {
            connection.from_node == id && graph.nodes.contains_key(&connection.to_node)
        });
        let output_boundaries: Vec<_> = graph
            .connections
            .iter()
            .filter(|connection| {
                connection.from_node == id
                    && graph
                        .special_nodes
                        .get(&connection.to_node)
                        .is_some_and(|special| special.node_type == SpecialNodeType::Output)
            })
            .collect();
        if !output_boundaries.is_empty() {
            if plugin_successors
                || !boundary_is_complete_identity(&output_boundaries, node.output_channels)
            {
                return Err(format!(
                    "output boundary for '{}' must be complete identity wiring from a graph sink",
                    node.plugin.display_name()
                ));
            }
        }
    }
    Ok(())
}

fn boundary_is_complete_identity(
    connections: &[&crate::plugin_graph::GraphConnection],
    channels: usize,
) -> bool {
    let routes: HashSet<_> = connections
        .iter()
        .map(|connection| (connection.from_port, connection.to_port))
        .collect();
    routes.len() == channels && (0..channels).all(|channel| routes.contains(&(channel, channel)))
}

/// Add a new plugin of the given type to the end of the rack.
pub fn add_path_plugin(plugins: &mut Vec<PluginInRack>, plugin_type: &str) {
    plugins.push(PluginInRack {
        plugin_type: plugin_type.to_string(),
        parameters: serde_json::json!({}),
    });
}

/// Remove a plugin at the given index.
pub fn remove_path_plugin(plugins: &mut Vec<PluginInRack>, index: usize) {
    if index < plugins.len() {
        plugins.remove(index);
    }
}

/// Move a plugin from one index to another.
pub fn move_path_plugin(plugins: &mut Vec<PluginInRack>, from: usize, to: usize) {
    if from >= plugins.len() || to >= plugins.len() || from == to {
        return;
    }
    let item = plugins.remove(from);
    plugins.insert(to, item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_graph::{NodePosition, SpecialNodeType};
    use sotf_audio::plugins::PluginType;

    fn connect_identity(graph: &mut PluginGraph, from: GraphNodeId, to: GraphNodeId) {
        let channels = graph
            .node_output_channels(from)
            .min(graph.node_input_channels(to));
        for channel in 0..channels {
            graph.add_connection(from, channel, to, channel).unwrap();
        }
    }

    #[test]
    fn test_parse_none() {
        let plugins = parse_path_config(r#"{"type":"None"}"#);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_parse_single_plugin() {
        let plugins = parse_path_config(
            r#"{"type":"Plugin","plugin_type":"gain","parameters":{"gain_db":-3.0}}"#,
        );
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].plugin_type, "gain");
    }

    #[test]
    fn test_parse_rack() {
        let json = r#"{"type":"Rack","plugins":[{"plugin_type":"gain","parameters":{"gain_db":-3.0}},{"plugin_type":"eq","parameters":{}}]}"#;
        let plugins = parse_path_config(json);
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].plugin_type, "gain");
        assert_eq!(plugins[1].plugin_type, "eq");
    }

    #[test]
    fn test_round_trip() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        let json = encode_path_config(&plugins);
        let decoded = parse_path_config(&json);

        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].plugin_type, "eq");
        assert_eq!(decoded[1].plugin_type, "gain");
        assert_eq!(decoded[2].plugin_type, "compressor");
    }

    #[test]
    fn test_encode_empty() {
        let json = encode_path_config(&[]);
        let config: PathConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(config, PathConfig::None));
    }

    #[test]
    fn linear_identity_graph_converts_to_rack() {
        let mut graph = PluginGraph::new();
        let input = graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
        let gain = graph
            .add_plugin_node(&PluginType::Gain, NodePosition::new(1.0, 0.0))
            .unwrap();
        let eq = graph
            .add_plugin_node(&PluginType::EQ, NodePosition::new(2.0, 0.0))
            .unwrap();
        let output =
            graph.add_special_node(SpecialNodeType::Output, NodePosition::new(3.0, 0.0), 2);
        connect_identity(&mut graph, input, gain);
        connect_identity(&mut graph, gain, eq);
        connect_identity(&mut graph, eq, output);

        let config = path_config_from_plugin_graph(&graph, 48_000.0).unwrap();
        let PathConfig::Rack { plugins } = config else {
            panic!("identity-wired linear graph should use rack representation");
        };
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].plugin_type, "gain");
        assert_eq!(plugins[1].plugin_type, "eq");
    }

    #[test]
    fn crossed_linear_graph_preserves_exact_ports() {
        let mut graph = PluginGraph::new();
        let input = graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
        let gain = graph
            .add_plugin_node(&PluginType::Gain, NodePosition::new(1.0, 0.0))
            .unwrap();
        let eq = graph
            .add_plugin_node(&PluginType::EQ, NodePosition::new(2.0, 0.0))
            .unwrap();
        let output =
            graph.add_special_node(SpecialNodeType::Output, NodePosition::new(3.0, 0.0), 2);
        connect_identity(&mut graph, input, gain);
        graph.add_connection(gain, 0, eq, 1).unwrap();
        graph.add_connection(gain, 1, eq, 0).unwrap();
        connect_identity(&mut graph, eq, output);

        let config = path_config_from_plugin_graph(&graph, 48_000.0).unwrap();
        let PathConfig::Graph { nodes, edges } = config else {
            panic!("routed linear graph must not be flattened into a rack");
        };
        assert_eq!(nodes.len(), 2);
        assert_eq!(edges.len(), 2);
        assert!(edges.iter().any(|edge| {
            edge.channel_map.as_deref() == Some(&[0]) && edge.destination_offset == 1
        }));
        assert!(edges.iter().any(|edge| {
            edge.channel_map.as_deref() == Some(&[1]) && edge.destination_offset == 0
        }));
    }

    #[test]
    fn nonlinear_graph_preserves_each_plugin_port_connection() {
        let mut graph = PluginGraph::new();
        let input = graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
        let path_a = graph
            .add_plugin_node(&PluginType::Gain, NodePosition::new(1.0, -1.0))
            .unwrap();
        let path_b = graph
            .add_plugin_node(&PluginType::Gain, NodePosition::new(1.0, 1.0))
            .unwrap();
        let merge = graph
            .add_plugin_node(&PluginType::EQ, NodePosition::new(2.0, 0.0))
            .unwrap();
        let output =
            graph.add_special_node(SpecialNodeType::Output, NodePosition::new(3.0, 0.0), 2);
        connect_identity(&mut graph, input, path_a);
        connect_identity(&mut graph, input, path_b);
        connect_identity(&mut graph, path_a, merge);
        connect_identity(&mut graph, path_b, merge);
        connect_identity(&mut graph, merge, output);

        let config = path_config_from_plugin_graph(&graph, 48_000.0).unwrap();
        let PathConfig::Graph { nodes, edges } = config else {
            panic!("branched graph must retain graph representation");
        };
        assert_eq!(nodes.len(), 3);
        assert_eq!(edges.len(), 4);
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.destination_offset == 0)
                .count(),
            2
        );
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.destination_offset == 1)
                .count(),
            2
        );
    }

    #[test]
    fn identity_graph_simplifies_to_rack_but_crossed_routes_do_not() {
        let nodes = vec![
            GraphNodeConfig {
                id: "first".into(),
                plugin_type: "gain".into(),
                parameters: serde_json::json!({"gain_db": -1.0}),
            },
            GraphNodeConfig {
                id: "second".into(),
                plugin_type: "eq".into(),
                parameters: serde_json::json!({"filters": []}),
            },
        ];
        let identity = PathConfig::Graph {
            nodes: nodes.clone(),
            edges: vec![
                GraphEdgeConfig {
                    from: "first".into(),
                    to: "second".into(),
                    channel_map: Some(vec![0]),
                    destination_offset: 0,
                },
                GraphEdgeConfig {
                    from: "first".into(),
                    to: "second".into(),
                    channel_map: Some(vec![1]),
                    destination_offset: 1,
                },
            ],
        };
        let PathConfig::Rack { plugins } = simplify_linear_path_config(identity) else {
            panic!("identity-routed graph should simplify to a rack");
        };
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].parameters["gain_db"], -1.0);

        let crossed = PathConfig::Graph {
            nodes,
            edges: vec![
                GraphEdgeConfig {
                    from: "first".into(),
                    to: "second".into(),
                    channel_map: Some(vec![0]),
                    destination_offset: 1,
                },
                GraphEdgeConfig {
                    from: "first".into(),
                    to: "second".into(),
                    channel_map: Some(vec![1]),
                    destination_offset: 0,
                },
            ],
        };
        assert!(matches!(
            simplify_linear_path_config(crossed),
            PathConfig::Graph { .. }
        ));
    }

    #[test]
    fn test_remove() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        remove_path_plugin(&mut plugins, 1);
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].plugin_type, "eq");
        assert_eq!(plugins[1].plugin_type, "compressor");
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        remove_path_plugin(&mut plugins, 5); // should not panic
        assert_eq!(plugins.len(), 1);
    }

    #[test]
    fn test_move_forward() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        move_path_plugin(&mut plugins, 0, 2);
        assert_eq!(plugins[0].plugin_type, "gain");
        assert_eq!(plugins[1].plugin_type, "compressor");
        assert_eq!(plugins[2].plugin_type, "eq");
    }

    #[test]
    fn test_move_backward() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        move_path_plugin(&mut plugins, 2, 0);
        assert_eq!(plugins[0].plugin_type, "compressor");
        assert_eq!(plugins[1].plugin_type, "eq");
        assert_eq!(plugins[2].plugin_type, "gain");
    }

    #[test]
    fn test_parse_invalid_json() {
        let plugins = parse_path_config("not json");
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_parse_graph_returns_empty() {
        let json = r#"{"type":"Graph","nodes":[],"edges":[]}"#;
        let plugins = parse_path_config(json);
        assert!(plugins.is_empty());
    }
}
