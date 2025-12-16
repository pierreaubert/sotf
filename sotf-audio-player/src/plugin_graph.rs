//! Plugin Graph Data Model
//!
//! Provides a graph-based representation of plugin connections where plugins
//! can be positioned on a 2D canvas and connected at the channel level.
//!
//! This is an alternative to the linear PluginChain, offering more flexibility
//! for complex routing scenarios like splits, merges, and parallel processing.

use crate::plugins::{Plugin, PluginType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// Unique identifier for graph nodes
pub type GraphNodeId = Uuid;

/// Position on the 2D canvas
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

impl NodePosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A connection between two ports (channel-level routing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConnection {
    pub id: Uuid,
    pub from_node: GraphNodeId,
    pub from_port: usize, // Output channel index
    pub to_node: GraphNodeId,
    pub to_port: usize, // Input channel index
}

impl GraphConnection {
    pub fn new(from_node: GraphNodeId, from_port: usize, to_node: GraphNodeId, to_port: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_node,
            from_port,
            to_node,
            to_port,
        }
    }
}

/// A plugin node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginGraphNode {
    pub id: GraphNodeId,
    pub plugin: Plugin,
    pub position: NodePosition,
    /// For Rack-as-node: when true, the contained chain is collapsed
    pub collapsed: bool,
    /// Cached channel counts (updated when plugin changes)
    pub input_channels: usize,
    pub output_channels: usize,
}

impl PluginGraphNode {
    pub fn new(plugin: Plugin, position: NodePosition) -> Self {
        // Default to stereo, actual values depend on plugin type
        let (input_channels, output_channels) = Self::channel_counts_for(&plugin);
        Self {
            id: Uuid::new_v4(),
            plugin,
            position,
            collapsed: false,
            input_channels,
            output_channels,
        }
    }

    /// Get default channel counts based on plugin type
    fn channel_counts_for(plugin: &Plugin) -> (usize, usize) {
        match plugin.plugin_type() {
            PluginType::Upmixer => (2, 6),        // Stereo to 5.1
            PluginType::BinauralDecoder => (6, 2), // Surround to stereo
            _ => (2, 2),                          // Most plugins are stereo in/out
        }
    }

    /// Update channel counts (call after plugin settings change)
    pub fn update_channel_counts(&mut self) {
        let (input, output) = Self::channel_counts_for(&self.plugin);
        self.input_channels = input;
        self.output_channels = output;
    }
}

/// Special nodes for explicit I/O routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpecialNodeType {
    /// Graph audio input (from audio source)
    Input,
    /// Graph audio output (to audio device)
    Output,
    /// Split: 1 input channel copied to N outputs
    Split,
    /// Merge: N input channels mixed to 1 output
    Merge,
}

/// A special I/O node (Input, Output, Split, Merge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialNode {
    pub id: GraphNodeId,
    pub node_type: SpecialNodeType,
    pub position: NodePosition,
    pub channels: usize,
}

impl SpecialNode {
    pub fn new(node_type: SpecialNodeType, position: NodePosition, channels: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type,
            position,
            channels,
        }
    }

    /// Input port count for this node type
    pub fn input_channels(&self) -> usize {
        match self.node_type {
            SpecialNodeType::Input => 0, // No inputs (source)
            SpecialNodeType::Output => self.channels,
            SpecialNodeType::Split => 1,
            SpecialNodeType::Merge => self.channels,
        }
    }

    /// Output port count for this node type
    pub fn output_channels(&self) -> usize {
        match self.node_type {
            SpecialNodeType::Input => self.channels,
            SpecialNodeType::Output => 0, // No outputs (sink)
            SpecialNodeType::Split => self.channels,
            SpecialNodeType::Merge => 1,
        }
    }
}

/// The complete plugin graph state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginGraph {
    /// Plugin nodes
    pub nodes: HashMap<GraphNodeId, PluginGraphNode>,
    /// Special I/O nodes
    pub special_nodes: HashMap<GraphNodeId, SpecialNode>,
    /// Connections between nodes
    pub connections: Vec<GraphConnection>,
    /// Canvas pan offset
    pub canvas_offset: (f32, f32),
    /// Canvas zoom level (0.5 to 2.0)
    pub canvas_zoom: f32,
    /// Next plugin ID for creating new plugins
    next_plugin_id: usize,
}

impl Default for PluginGraph {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            special_nodes: HashMap::new(),
            connections: Vec::new(),
            canvas_offset: (0.0, 0.0),
            canvas_zoom: 1.0,
            next_plugin_id: 1,
        }
    }
}

impl PluginGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a plugin node at the given position
    pub fn add_plugin_node(&mut self, plugin_type: &PluginType, position: NodePosition) -> GraphNodeId {
        let plugin = Plugin::new(self.next_plugin_id, plugin_type);
        self.next_plugin_id += 1;
        let node = PluginGraphNode::new(plugin, position);
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    /// Add a special node at the given position
    pub fn add_special_node(
        &mut self,
        node_type: SpecialNodeType,
        position: NodePosition,
        channels: usize,
    ) -> GraphNodeId {
        let node = SpecialNode::new(node_type, position, channels);
        let id = node.id;
        self.special_nodes.insert(id, node);
        id
    }

    /// Remove a node and all its connections
    pub fn remove_node(&mut self, node_id: GraphNodeId) {
        self.nodes.remove(&node_id);
        self.special_nodes.remove(&node_id);
        self.connections
            .retain(|c| c.from_node != node_id && c.to_node != node_id);
    }

    /// Add a connection between two ports
    pub fn add_connection(
        &mut self,
        from_node: GraphNodeId,
        from_port: usize,
        to_node: GraphNodeId,
        to_port: usize,
    ) -> Result<Uuid, String> {
        // Validate nodes exist
        if !self.node_exists(from_node) {
            return Err(format!("Source node {:?} not found", from_node));
        }
        if !self.node_exists(to_node) {
            return Err(format!("Target node {:?} not found", to_node));
        }

        // Check for duplicate connection
        if self
            .connections
            .iter()
            .any(|c| c.from_node == from_node && c.from_port == from_port && c.to_node == to_node && c.to_port == to_port)
        {
            return Err("Connection already exists".to_string());
        }

        // Check for cycles
        if self.would_create_cycle(from_node, to_node) {
            return Err("Connection would create a cycle".to_string());
        }

        let conn = GraphConnection::new(from_node, from_port, to_node, to_port);
        let id = conn.id;
        self.connections.push(conn);
        Ok(id)
    }

    /// Remove a connection by ID
    pub fn remove_connection(&mut self, connection_id: Uuid) {
        self.connections.retain(|c| c.id != connection_id);
    }

    /// Check if a node exists (either plugin or special)
    pub fn node_exists(&self, node_id: GraphNodeId) -> bool {
        self.nodes.contains_key(&node_id) || self.special_nodes.contains_key(&node_id)
    }

    /// Check if adding an edge from -> to would create a cycle
    fn would_create_cycle(&self, from_node: GraphNodeId, to_node: GraphNodeId) -> bool {
        // If from == to, it's a self-loop (cycle)
        if from_node == to_node {
            return true;
        }

        // BFS from to_node to see if we can reach from_node
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(to_node);

        while let Some(current) = queue.pop_front() {
            if current == from_node {
                return true;
            }
            if visited.insert(current) {
                // Find all nodes that current connects to
                for conn in &self.connections {
                    if conn.from_node == current {
                        queue.push_back(conn.to_node);
                    }
                }
            }
        }
        false
    }

    /// Get topologically sorted node IDs (for processing order)
    pub fn topological_sort(&self) -> Result<Vec<GraphNodeId>, String> {
        let all_nodes: Vec<GraphNodeId> = self
            .nodes
            .keys()
            .chain(self.special_nodes.keys())
            .copied()
            .collect();

        let mut in_degree: HashMap<GraphNodeId, usize> = all_nodes.iter().map(|&id| (id, 0)).collect();

        // Calculate in-degrees
        for conn in &self.connections {
            if let Some(deg) = in_degree.get_mut(&conn.to_node) {
                *deg += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<GraphNodeId> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut sorted = Vec::new();

        while let Some(node) = queue.pop_front() {
            sorted.push(node);

            for conn in &self.connections {
                if conn.from_node == node {
                    if let Some(deg) = in_degree.get_mut(&conn.to_node) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(conn.to_node);
                        }
                    }
                }
            }
        }

        if sorted.len() != all_nodes.len() {
            return Err("Graph contains a cycle".to_string());
        }

        Ok(sorted)
    }

    /// Get connections originating from a node
    pub fn connections_from(&self, node_id: GraphNodeId) -> Vec<&GraphConnection> {
        self.connections.iter().filter(|c| c.from_node == node_id).collect()
    }

    /// Get connections going to a node
    pub fn connections_to(&self, node_id: GraphNodeId) -> Vec<&GraphConnection> {
        self.connections.iter().filter(|c| c.to_node == node_id).collect()
    }

    /// Get input channel count for a node
    pub fn node_input_channels(&self, node_id: GraphNodeId) -> usize {
        if let Some(node) = self.nodes.get(&node_id) {
            node.input_channels
        } else if let Some(special) = self.special_nodes.get(&node_id) {
            special.input_channels()
        } else {
            0
        }
    }

    /// Get output channel count for a node
    pub fn node_output_channels(&self, node_id: GraphNodeId) -> usize {
        if let Some(node) = self.nodes.get(&node_id) {
            node.output_channels
        } else if let Some(special) = self.special_nodes.get(&node_id) {
            special.output_channels()
        } else {
            0
        }
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.special_nodes.is_empty()
    }

    /// Get all enabled plugin nodes in topological order
    pub fn enabled_plugins_sorted(&self) -> Result<Vec<&PluginGraphNode>, String> {
        let sorted_ids = self.topological_sort()?;
        Ok(sorted_ids
            .into_iter()
            .filter_map(|id| self.nodes.get(&id))
            .filter(|node| node.plugin.enabled)
            .collect())
    }
}

/// UI-only selection state (not persisted)
#[derive(Debug, Clone, Default)]
pub struct GraphSelection {
    pub selected_nodes: HashSet<GraphNodeId>,
    pub selected_connections: HashSet<Uuid>,
}

impl GraphSelection {
    pub fn clear(&mut self) {
        self.selected_nodes.clear();
        self.selected_connections.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.selected_nodes.is_empty() && self.selected_connections.is_empty()
    }

    pub fn select_node(&mut self, node_id: GraphNodeId, add_to_selection: bool) {
        if !add_to_selection {
            self.clear();
        }
        self.selected_nodes.insert(node_id);
    }

    pub fn select_connection(&mut self, conn_id: Uuid, add_to_selection: bool) {
        if !add_to_selection {
            self.clear();
        }
        self.selected_connections.insert(conn_id);
    }

    pub fn toggle_node(&mut self, node_id: GraphNodeId) {
        if self.selected_nodes.contains(&node_id) {
            self.selected_nodes.remove(&node_id);
        } else {
            self.selected_nodes.insert(node_id);
        }
    }
}

/// State for dragging a new connection
#[derive(Debug, Clone)]
pub struct ConnectionDrag {
    pub from_node: GraphNodeId,
    pub from_port: usize,
    pub is_output: bool, // True if dragging from an output port
    pub current_position: (f32, f32),
}

/// State for dragging a node
#[derive(Debug, Clone)]
pub struct NodeDrag {
    pub node_id: GraphNodeId,
    pub offset: (f32, f32), // Offset from node position to mouse position
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_remove_nodes() {
        let mut graph = PluginGraph::new();

        let node_id = graph.add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0));
        assert!(graph.nodes.contains_key(&node_id));
        assert_eq!(graph.nodes.len(), 1);

        graph.remove_node(node_id);
        assert!(!graph.nodes.contains_key(&node_id));
        assert!(graph.nodes.is_empty());
    }

    #[test]
    fn test_connections() {
        let mut graph = PluginGraph::new();

        let node1 = graph.add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0));
        let node2 = graph.add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0));

        // Add connection
        let conn_id = graph.add_connection(node1, 0, node2, 0).unwrap();
        assert_eq!(graph.connections.len(), 1);

        // Remove connection
        graph.remove_connection(conn_id);
        assert!(graph.connections.is_empty());
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = PluginGraph::new();

        let node1 = graph.add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0));
        let node2 = graph.add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0));

        // Valid connection
        graph.add_connection(node1, 0, node2, 0).unwrap();

        // This would create a cycle
        let result = graph.add_connection(node2, 0, node1, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = PluginGraph::new();

        let node1 = graph.add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0));
        let node2 = graph.add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0));
        let node3 = graph.add_plugin_node(&PluginType::Compressor, NodePosition::new(500.0, 100.0));

        graph.add_connection(node1, 0, node2, 0).unwrap();
        graph.add_connection(node2, 0, node3, 0).unwrap();

        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted.len(), 3);

        // node1 should come before node2, node2 before node3
        let pos1 = sorted.iter().position(|&id| id == node1).unwrap();
        let pos2 = sorted.iter().position(|&id| id == node2).unwrap();
        let pos3 = sorted.iter().position(|&id| id == node3).unwrap();

        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
    }
}
