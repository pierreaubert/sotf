//! Plugin configuration types for serialization/deserialization.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Plugin configuration for serialization/deserialization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin type identifier
    pub plugin_type: String,
    /// Plugin parameters
    pub parameters: serde_json::Value,
}

impl PluginConfig {
    /// Create a new plugin config
    pub fn new(plugin_type: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self {
            plugin_type: plugin_type.into(),
            parameters,
        }
    }

    /// Create a plugin config and validate its invariants.
    pub fn try_new(
        plugin_type: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Result<Self, String> {
        let config = Self::new(plugin_type, parameters);
        config.validate()?;
        Ok(config)
    }

    /// Validate plugin config invariants that serde cannot express.
    pub fn validate(&self) -> Result<(), String> {
        if self.plugin_type.trim().is_empty() {
            return Err("plugin_type must not be empty".to_string());
        }

        Ok(())
    }
}

/// Graph-based plugin configuration for DAG processing.
///
/// Unlike `Vec<PluginConfig>` (linear chain), this supports parallel paths
/// needed for multi-driver crossover setups.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphConfig {
    pub nodes: Vec<PluginGraphNodeConfig>,
    pub edges: Vec<PluginGraphEdgeConfig>,
}

impl PluginGraphConfig {
    /// Create a plugin graph config and validate that it is a DAG.
    pub fn try_new(
        nodes: Vec<PluginGraphNodeConfig>,
        edges: Vec<PluginGraphEdgeConfig>,
    ) -> Result<Self, String> {
        let config = Self { nodes, edges };
        config.validate()?;
        Ok(config)
    }

    /// Validate graph invariants: unique nodes, valid endpoints, and acyclicity.
    pub fn validate(&self) -> Result<(), String> {
        let mut node_ids = HashSet::with_capacity(self.nodes.len());
        for node in &self.nodes {
            node.validate()?;
            if !node_ids.insert(node.id) {
                return Err(format!("duplicate plugin graph node id {}", node.id));
            }
        }

        let mut incoming_counts: HashMap<usize, usize> =
            self.nodes.iter().map(|node| (node.id, 0)).collect();
        let mut outgoing_edges: HashMap<usize, Vec<usize>> = HashMap::new();

        for edge in &self.edges {
            if !node_ids.contains(&edge.from_node) {
                return Err(format!(
                    "plugin graph edge references missing from_node {}",
                    edge.from_node
                ));
            }
            if !node_ids.contains(&edge.to_node) {
                return Err(format!(
                    "plugin graph edge references missing to_node {}",
                    edge.to_node
                ));
            }

            *incoming_counts
                .get_mut(&edge.to_node)
                .expect("edge endpoint existence checked above") += 1;
            outgoing_edges
                .entry(edge.from_node)
                .or_default()
                .push(edge.to_node);
        }

        let mut ready: VecDeque<usize> = incoming_counts
            .iter()
            .filter_map(|(&node_id, &count)| (count == 0).then_some(node_id))
            .collect();
        let mut visited = 0;

        while let Some(node_id) = ready.pop_front() {
            visited += 1;
            if let Some(targets) = outgoing_edges.get(&node_id) {
                for &target in targets {
                    let count = incoming_counts
                        .get_mut(&target)
                        .expect("edge endpoint existence checked above");
                    *count -= 1;
                    if *count == 0 {
                        ready.push_back(target);
                    }
                }
            }
        }

        if visited != self.nodes.len() {
            return Err("plugin graph must be acyclic".to_string());
        }

        Ok(())
    }
}

/// A node in the plugin graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphNodeConfig {
    /// Unique node ID (used to reference in edges)
    pub id: usize,
    pub plugin_type: String,
    pub parameters: serde_json::Value,
    /// Number of input channels this node expects
    pub input_channels: usize,
}

impl PluginGraphNodeConfig {
    /// Create a plugin graph node config and validate its local invariants.
    pub fn try_new(
        id: usize,
        plugin_type: impl Into<String>,
        parameters: serde_json::Value,
        input_channels: usize,
    ) -> Result<Self, String> {
        let config = Self {
            id,
            plugin_type: plugin_type.into(),
            parameters,
            input_channels,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate node-local invariants.
    pub fn validate(&self) -> Result<(), String> {
        if self.plugin_type.trim().is_empty() {
            return Err(format!(
                "plugin graph node {} plugin_type is empty",
                self.id
            ));
        }
        if self.input_channels == 0 {
            return Err(format!(
                "plugin graph node {} input_channels must be greater than 0",
                self.id
            ));
        }

        Ok(())
    }
}

/// An edge connecting two nodes in the plugin graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphEdgeConfig {
    pub from_node: usize,
    pub to_node: usize,
}

impl PluginGraphEdgeConfig {
    /// Create a plugin graph edge. Endpoint existence is validated by `PluginGraphConfig`.
    pub fn new(from_node: usize, to_node: usize) -> Self {
        Self { from_node, to_node }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: usize) -> PluginGraphNodeConfig {
        PluginGraphNodeConfig {
            id,
            plugin_type: "gain".to_string(),
            parameters: json!({}),
            input_channels: 2,
        }
    }

    #[test]
    fn plugin_config_rejects_empty_type() {
        let error = PluginConfig::try_new(" ", json!({})).unwrap_err();
        assert!(error.contains("plugin_type"));
    }

    #[test]
    fn plugin_graph_accepts_valid_dag() {
        let graph = PluginGraphConfig::try_new(
            vec![node(0), node(1), node(2)],
            vec![
                PluginGraphEdgeConfig::new(0, 1),
                PluginGraphEdgeConfig::new(0, 2),
            ],
        )
        .unwrap();

        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn plugin_graph_rejects_duplicate_node_ids() {
        let error = PluginGraphConfig::try_new(vec![node(0), node(0)], vec![]).unwrap_err();
        assert!(error.contains("duplicate"));
    }

    #[test]
    fn plugin_graph_rejects_missing_edge_endpoint() {
        let error =
            PluginGraphConfig::try_new(vec![node(0)], vec![PluginGraphEdgeConfig::new(0, 1)])
                .unwrap_err();
        assert!(error.contains("to_node"));
    }

    #[test]
    fn plugin_graph_rejects_cycles() {
        let error = PluginGraphConfig::try_new(
            vec![node(0), node(1), node(2)],
            vec![
                PluginGraphEdgeConfig::new(0, 1),
                PluginGraphEdgeConfig::new(1, 2),
                PluginGraphEdgeConfig::new(2, 0),
            ],
        )
        .unwrap_err();
        assert!(error.contains("acyclic"));
    }

    #[test]
    fn plugin_graph_rejects_zero_channel_nodes() {
        let mut invalid = node(0);
        invalid.input_channels = 0;

        let error = PluginGraphConfig::try_new(vec![invalid], vec![]).unwrap_err();
        assert!(error.contains("input_channels"));
    }

    #[test]
    fn plugin_config_accepts_valid() {
        let config = PluginConfig::try_new("eq", json!({"freq": 1000.0})).unwrap();
        assert_eq!(config.plugin_type, "eq");
    }

    #[test]
    fn plugin_config_validate_accepts_non_empty_type() {
        let config = PluginConfig::new("gain", json!({}));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn plugin_graph_accepts_empty_graph() {
        let graph = PluginGraphConfig::try_new(vec![], vec![]).unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn plugin_graph_accepts_disconnected_nodes() {
        let graph = PluginGraphConfig::try_new(vec![node(0), node(1)], vec![]).unwrap();
        assert_eq!(graph.nodes.len(), 2);
    }

    #[test]
    fn plugin_graph_rejects_self_loop() {
        let error = PluginGraphConfig::try_new(
            vec![node(0)],
            vec![PluginGraphEdgeConfig::new(0, 0)],
        )
        .unwrap_err();
        assert!(error.contains("acyclic"));
    }

    #[test]
    fn plugin_graph_rejects_missing_from_node() {
        let error = PluginGraphConfig::try_new(
            vec![node(1)],
            vec![PluginGraphEdgeConfig::new(0, 1)],
        )
        .unwrap_err();
        assert!(error.contains("from_node"));
    }

    #[test]
    fn plugin_graph_accepts_multiple_edges() {
        let graph = PluginGraphConfig::try_new(
            vec![node(0), node(1)],
            vec![
                PluginGraphEdgeConfig::new(0, 1),
                PluginGraphEdgeConfig::new(0, 1),
            ],
        )
        .unwrap();
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn plugin_graph_rejects_large_cycle() {
        let error = PluginGraphConfig::try_new(
            vec![node(0), node(1), node(2), node(3)],
            vec![
                PluginGraphEdgeConfig::new(0, 1),
                PluginGraphEdgeConfig::new(1, 2),
                PluginGraphEdgeConfig::new(2, 3),
                PluginGraphEdgeConfig::new(3, 0),
            ],
        )
        .unwrap_err();
        assert!(error.contains("acyclic"));
    }

    #[test]
    fn plugin_graph_node_try_new_valid_and_invalid() {
        let valid = PluginGraphNodeConfig::try_new(7, "delay", json!({"ms": 100}), 6).unwrap();
        assert_eq!(valid.id, 7);
        assert_eq!(valid.input_channels, 6);

        let err = PluginGraphNodeConfig::try_new(8, "   ", json!({}), 2).unwrap_err();
        assert!(err.contains("plugin_type"));

        let err = PluginGraphNodeConfig::try_new(9, "mixer", json!({}), 0).unwrap_err();
        assert!(err.contains("input_channels"));
    }
}
