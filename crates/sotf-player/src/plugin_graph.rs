//! Plugin Graph Data Model
//!
//! Provides a graph-based representation of plugin connections where plugins
//! can be positioned on a 2D canvas and connected at the channel level.
//!
//! This is an alternative to the linear PluginChain, offering more flexibility
//! for complex routing scenarios like splits, merges, and parallel processing.

use serde::{Deserialize, Serialize};
use sotf_audio::PluginConfig;
use sotf_audio::plugins::{
    ChannelConflict, Plugin, PluginSettings, PluginType, resize_matrix, upmixer_output_channels,
};
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
    pub fn new(
        from_node: GraphNodeId,
        from_port: usize,
        to_node: GraphNodeId,
        to_port: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_node,
            from_port,
            to_node,
            to_port,
        }
    }
}

/// Role of a plugin node within the default rack scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodeRole {
    /// Permanent input loudness monitor (first in chain)
    InputMonitor,
    /// Permanent replay-gain compensation (starts disabled)
    ReplayGain,
    /// Permanent channel routing matrix
    Matrix,
    /// Permanent output loudness monitor (last in chain)
    OutputMonitor,
    /// User-added processing plugin
    #[default]
    User,
}

/// A plugin node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginGraphNode {
    pub id: GraphNodeId,
    pub plugin: Plugin,
    pub position: NodePosition,
    /// Role within the default rack (permanent vs user plugin)
    #[serde(default)]
    pub role: NodeRole,
    /// For Rack-as-node: when true, the contained chain is collapsed
    pub collapsed: bool,
    /// Cached channel counts (updated when plugin changes)
    pub input_channels: usize,
    pub output_channels: usize,
}

impl PluginGraphNode {
    pub fn new(plugin: Plugin, position: NodePosition) -> Self {
        Self::with_role(plugin, position, NodeRole::User)
    }

    pub fn with_role(plugin: Plugin, position: NodePosition, role: NodeRole) -> Self {
        let (input_channels, output_channels) = Self::channel_counts_for(&plugin);
        Self {
            id: Uuid::new_v4(),
            plugin,
            position,
            role,
            collapsed: false,
            input_channels,
            output_channels,
        }
    }

    /// Get default channel counts based on plugin type
    fn channel_counts_for(plugin: &Plugin) -> (usize, usize) {
        match plugin.plugin_type() {
            PluginType::Upmixer | PluginType::AAE => (2, 6), // Stereo to 5.1
            PluginType::BinauralDecoder => (6, 2), // Surround to stereo
            _ => (2, 2),                           // Most plugins are stereo in/out
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
    /// Optional label (e.g., device name for Input/Output nodes)
    #[serde(default)]
    pub label: Option<String>,
}

impl SpecialNode {
    pub fn new(node_type: SpecialNodeType, position: NodePosition, channels: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type,
            position,
            channels,
            label: None,
        }
    }

    /// Create a special node with a label (e.g., device name)
    pub fn with_label(
        node_type: SpecialNodeType,
        position: NodePosition,
        channels: usize,
        label: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type,
            position,
            channels,
            label: Some(label),
        }
    }

    /// Get display name for this node
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            label.clone()
        } else {
            match self.node_type {
                SpecialNodeType::Input => "Audio Input".to_string(),
                SpecialNodeType::Output => "Audio Output".to_string(),
                SpecialNodeType::Split => "Split".to_string(),
                SpecialNodeType::Merge => "Merge".to_string(),
            }
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
    pub fn add_plugin_node(
        &mut self,
        plugin_type: &PluginType,
        position: NodePosition,
    ) -> GraphNodeId {
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

    /// Add a special node with a label (e.g., device name) at the given position
    pub fn add_special_node_with_label(
        &mut self,
        node_type: SpecialNodeType,
        position: NodePosition,
        channels: usize,
        label: String,
    ) -> GraphNodeId {
        let node = SpecialNode::with_label(node_type, position, channels, label);
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
        if self.connections.iter().any(|c| {
            c.from_node == from_node
                && c.from_port == from_port
                && c.to_node == to_node
                && c.to_port == to_port
        }) {
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

        let mut in_degree: HashMap<GraphNodeId, usize> =
            all_nodes.iter().map(|&id| (id, 0)).collect();

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
        self.connections
            .iter()
            .filter(|c| c.from_node == node_id)
            .collect()
    }

    /// Get connections going to a node
    pub fn connections_to(&self, node_id: GraphNodeId) -> Vec<&GraphConnection> {
        self.connections
            .iter()
            .filter(|c| c.to_node == node_id)
            .collect()
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

    // =========================================================================
    // I/O Node Helpers
    // =========================================================================

    /// Get the Input special node (if it exists)
    pub fn input_node(&self) -> Option<&SpecialNode> {
        self.special_nodes
            .values()
            .find(|n| matches!(n.node_type, SpecialNodeType::Input))
    }

    /// Get the Output special node (if it exists)
    pub fn output_node(&self) -> Option<&SpecialNode> {
        self.special_nodes
            .values()
            .find(|n| matches!(n.node_type, SpecialNodeType::Output))
    }

    /// Get the Input node ID (if it exists)
    pub fn input_node_id(&self) -> Option<GraphNodeId> {
        self.input_node().map(|n| n.id)
    }

    /// Get the Output node ID (if it exists)
    pub fn output_node_id(&self) -> Option<GraphNodeId> {
        self.output_node().map(|n| n.id)
    }

    /// Get mutable reference to Input special node
    pub fn input_node_mut(&mut self) -> Option<&mut SpecialNode> {
        self.special_nodes
            .values_mut()
            .find(|n| matches!(n.node_type, SpecialNodeType::Input))
    }

    /// Get mutable reference to Output special node
    pub fn output_node_mut(&mut self) -> Option<&mut SpecialNode> {
        self.special_nodes
            .values_mut()
            .find(|n| matches!(n.node_type, SpecialNodeType::Output))
    }

    // =========================================================================
    // LoudnessMonitor Detection
    // =========================================================================

    /// Check if a LoudnessMonitor plugin is connected (directly or transitively) from the Input node
    /// Uses BFS forward from Input to find any LoudnessMonitor in the signal path
    pub fn has_loudness_monitor_at_input(&self) -> bool {
        let Some(input_id) = self.input_node_id() else {
            return false;
        };

        // BFS forward from Input node
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(input_id);

        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                // Check if this node is a LoudnessMonitor
                if let Some(node) = self.nodes.get(&current) {
                    if matches!(node.plugin.plugin_type(), PluginType::LoudnessMonitor) {
                        return true;
                    }
                }

                // Add all nodes that this node connects to
                for conn in &self.connections {
                    if conn.from_node == current {
                        queue.push_back(conn.to_node);
                    }
                }
            }
        }
        false
    }

    /// Check if a LoudnessMonitor plugin is connected (directly or transitively) to the Output node
    /// Uses BFS backward from Output to find any LoudnessMonitor in the signal path
    pub fn has_loudness_monitor_at_output(&self) -> bool {
        let Some(output_id) = self.output_node_id() else {
            return false;
        };

        // BFS backward from Output node
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(output_id);

        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                // Check if this node is a LoudnessMonitor
                if let Some(node) = self.nodes.get(&current) {
                    if matches!(node.plugin.plugin_type(), PluginType::LoudnessMonitor) {
                        return true;
                    }
                }

                // Add all nodes that connect TO this node (backward search)
                for conn in &self.connections {
                    if conn.to_node == current {
                        queue.push_back(conn.from_node);
                    }
                }
            }
        }
        false
    }

    // =========================================================================
    // Channel Flow Computation
    // =========================================================================

    /// Get the channel count at the Input node
    pub fn input_channel_count(&self) -> usize {
        self.input_node().map(|n| n.channels).unwrap_or(2)
    }

    /// Compute the channel count at the Output node by walking the signal path
    /// Returns (input_channels, output_channels)
    pub fn compute_channel_flow(&self) -> (usize, usize) {
        let input_channels = self.input_channel_count();
        let mut current_channels = input_channels;

        // Walk the graph in topological order to compute output channels
        if let Ok(sorted) = self.topological_sort() {
            for node_id in sorted {
                if let Some(plugin_node) = self.nodes.get(&node_id) {
                    // Only enabled plugins affect channel count
                    if plugin_node.plugin.enabled {
                        match plugin_node.plugin.plugin_type() {
                            PluginType::Upmixer | PluginType::AAE => {
                                // Upmixer/AAE increases channels (stereo to surround)
                                current_channels = plugin_node.output_channels;
                            }
                            PluginType::BinauralDecoder => {
                                // Binaural decoder reduces to stereo
                                current_channels = 2;
                            }
                            _ => {
                                // Most plugins pass through unchanged
                            }
                        }
                    }
                }
            }
        }

        (input_channels, current_channels)
    }

    // =========================================================================
    // Default Rack
    // =========================================================================

    /// Create the default rack: Input → InputMonitor → ReplayGain → [user zone] → Matrix → OutputMonitor → Output
    pub fn with_default_rack() -> Self {
        let mut g = Self::new();
        let spacing = 180.0;
        let y = 200.0;

        let input_id = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, y), 2);
        let im = g.add_plugin_node_with_role(
            &PluginType::LoudnessMonitor,
            NodePosition::new(spacing, y),
            NodeRole::InputMonitor,
            true,
        );
        let rg = g.add_plugin_node_with_role(
            &PluginType::Gain,
            NodePosition::new(spacing * 2.0, y),
            NodeRole::ReplayGain,
            true,
        );
        // ReplayGain starts disabled
        if let Some(node) = g.nodes.get_mut(&rg) {
            node.plugin.enabled = false;
        }
        let mx = g.add_plugin_node_with_role(
            &PluginType::Matrix,
            NodePosition::new(spacing * 3.0, y),
            NodeRole::Matrix,
            true,
        );
        let om = g.add_plugin_node_with_role(
            &PluginType::LoudnessMonitor,
            NodePosition::new(spacing * 4.0, y),
            NodeRole::OutputMonitor,
            true,
        );
        let output_id = g.add_special_node(
            SpecialNodeType::Output,
            NodePosition::new(spacing * 5.0, y),
            2,
        );

        // Wire linearly (stereo: connect port 0 and port 1)
        let chain = [input_id, im, rg, mx, om, output_id];
        for pair in chain.windows(2) {
            let from = pair[0];
            let to = pair[1];
            let out_ch = g.node_output_channels(from);
            let in_ch = g.node_input_channels(to);
            let ch = out_ch.min(in_ch).max(1);
            for port in 0..ch {
                let _ = g.add_connection(from, port, to, port);
            }
        }

        g
    }

    /// Add a plugin node with a specific role and permanent flag
    fn add_plugin_node_with_role(
        &mut self,
        plugin_type: &PluginType,
        position: NodePosition,
        role: NodeRole,
        permanent: bool,
    ) -> GraphNodeId {
        let plugin = if permanent {
            Plugin::new_permanent(self.next_plugin_id, plugin_type)
        } else {
            Plugin::new(self.next_plugin_id, plugin_type)
        };
        self.next_plugin_id += 1;
        let node = PluginGraphNode::with_role(plugin, position, role);
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    // =========================================================================
    // Linearity Detection
    // =========================================================================

    /// Returns true when the graph forms a single linear path from Input to Output.
    pub fn is_linear(&self) -> bool {
        self.try_linear_order().is_some()
    }

    /// Returns the ordered node IDs from Input to Output when the graph is linear.
    /// Returns None if the graph is not linear.
    pub fn linear_order(&self) -> Option<Vec<GraphNodeId>> {
        self.try_linear_order()
    }

    /// Single implementation: walk from Input to Output collecting the path.
    /// Returns None if the topology is not a single linear chain.
    fn try_linear_order(&self) -> Option<Vec<GraphNodeId>> {
        let input = self.input_node_id()?;
        let target = self.output_node_id()?;

        // Deduplicate per-channel connections into logical edges
        let logical_edges: HashSet<(GraphNodeId, GraphNodeId)> = self
            .connections
            .iter()
            .map(|c| (c.from_node, c.to_node))
            .collect();

        // Every node must have ≤1 outgoing and ≤1 incoming logical edge
        let total_nodes = self.nodes.len() + self.special_nodes.len();
        let all_ids = self.nodes.keys().chain(self.special_nodes.keys());

        for &id in all_ids {
            let out_count = logical_edges.iter().filter(|(f, _)| *f == id).count();
            let in_count = logical_edges.iter().filter(|(_, t)| *t == id).count();
            if out_count > 1 || in_count > 1 {
                return None;
            }
        }

        // Walk from Input to Output, collecting the path
        let mut order = Vec::with_capacity(total_nodes);
        let mut current = input;

        loop {
            order.push(current);
            if current == target {
                break;
            }
            current = logical_edges
                .iter()
                .find(|(f, _)| *f == current)
                .map(|(_, t)| *t)?;
        }

        // Every node must be on the path
        if order.len() != total_nodes {
            return None;
        }

        Some(order)
    }

    /// Returns plugin nodes in linear order (excluding special nodes).
    /// Returns None if the graph is not linear.
    pub fn plugins_linear(&self) -> Option<Vec<&PluginGraphNode>> {
        self.linear_order().map(|ids| {
            ids.into_iter()
                .filter_map(|id| self.nodes.get(&id))
                .collect()
        })
    }

    /// Find the first plugin node with the given role.
    pub fn node_for_role(&self, role: NodeRole) -> Option<&PluginGraphNode> {
        self.nodes.values().find(|n| n.role == role)
    }

    /// Find the node ID for the first plugin with the given role.
    pub fn node_id_for_role(&self, role: NodeRole) -> Option<GraphNodeId> {
        self.node_for_role(role).map(|n| n.id)
    }

    /// Find the mutable plugin node with the given role.
    pub fn node_for_role_mut(&mut self, role: NodeRole) -> Option<&mut PluginGraphNode> {
        self.nodes.values_mut().find(|n| n.role == role)
    }

    // =========================================================================
    // Linear Mutation Methods
    // =========================================================================

    /// Insert a user plugin before the Matrix node in a linear graph.
    /// Returns the new node's ID, or an error if the graph is not linear.
    pub fn add_user_plugin(&mut self, plugin_type: &PluginType) -> Result<GraphNodeId, String> {
        let order = self.linear_order().ok_or("Graph is not linear")?;

        // Find the Matrix node position in the chain
        let matrix_id = self
            .node_id_for_role(NodeRole::Matrix)
            .ok_or("No Matrix node in graph")?;
        let matrix_pos = order
            .iter()
            .position(|&id| id == matrix_id)
            .ok_or("Matrix not in linear order")?;

        if matrix_pos == 0 {
            return Err("Matrix cannot be the first node in the chain".to_string());
        }

        // The predecessor of Matrix is where we splice in
        let pred_id = order[matrix_pos - 1];

        // Position the new node between predecessor and Matrix
        let pred_pos = self.node_position(pred_id).unwrap_or_default();
        let matrix_node_pos = self.node_position(matrix_id).unwrap_or_default();
        let new_pos = NodePosition::new((pred_pos.x + matrix_node_pos.x) / 2.0, pred_pos.y);

        // Create the new plugin node
        let new_id = self.add_plugin_node_with_role(plugin_type, new_pos, NodeRole::User, false);

        // Re-wire: remove pred→matrix connections, add pred→new and new→matrix
        self.rewire_insert(pred_id, matrix_id, new_id);

        Ok(new_id)
    }

    /// Remove a user plugin and re-wire its predecessor to its successor.
    pub fn remove_user_plugin(&mut self, node_id: GraphNodeId) -> Result<(), String> {
        let node = self.nodes.get(&node_id).ok_or("Node not found")?;
        if node.role != NodeRole::User {
            return Err("Cannot remove permanent plugin".to_string());
        }

        let order = self.linear_order().ok_or("Graph is not linear")?;
        let pos = order
            .iter()
            .position(|&id| id == node_id)
            .ok_or("Node not in linear order")?;

        if pos == 0 || pos + 1 >= order.len() {
            return Err("User plugin is at graph boundary — cannot rewire".to_string());
        }
        let pred_id = order[pos - 1];
        let succ_id = order[pos + 1];

        // Remove connections to/from this node, then wire pred→succ
        self.connections
            .retain(|c| c.from_node != node_id && c.to_node != node_id);
        self.nodes.remove(&node_id);

        let ch = self
            .node_output_channels(pred_id)
            .min(self.node_input_channels(succ_id))
            .max(1);
        for port in 0..ch {
            let _ = self.add_connection(pred_id, port, succ_id, port);
        }

        Ok(())
    }

    /// Swap a user plugin with its predecessor in the linear chain.
    pub fn move_user_plugin_up(&mut self, node_id: GraphNodeId) -> Result<(), String> {
        self.move_user_plugin(node_id, -1)
    }

    /// Swap a user plugin with its successor in the linear chain.
    pub fn move_user_plugin_down(&mut self, node_id: GraphNodeId) -> Result<(), String> {
        self.move_user_plugin(node_id, 1)
    }

    fn move_user_plugin(&mut self, node_id: GraphNodeId, direction: i32) -> Result<(), String> {
        let node = self.nodes.get(&node_id).ok_or("Node not found")?;
        if node.role != NodeRole::User {
            return Err("Cannot move permanent plugin".to_string());
        }

        let order = self.linear_order().ok_or("Graph is not linear")?;
        let pos = order
            .iter()
            .position(|&id| id == node_id)
            .ok_or("Node not in linear order")? as i32;

        let swap_pos = pos + direction;
        if swap_pos < 0 || swap_pos as usize >= order.len() {
            return Err("Cannot move beyond graph boundaries".to_string());
        }
        let swap_id = order[swap_pos as usize];

        // The swap target must also be a user plugin
        if let Some(swap_node) = self.nodes.get(&swap_id) {
            if swap_node.role != NodeRole::User {
                return Err("Cannot swap with permanent plugin".to_string());
            }
        } else {
            return Err("Cannot swap with special node".to_string());
        }

        // Rewire: we need the predecessor and successor of the pair
        let (first_pos, second_pos) = if direction < 0 {
            (swap_pos as usize, pos as usize)
        } else {
            (pos as usize, swap_pos as usize)
        };

        if first_pos == 0 || second_pos + 1 >= order.len() {
            return Err("Cannot swap at graph boundaries".to_string());
        }
        let before_id = order[first_pos - 1];
        let first_id = order[first_pos];
        let second_id = order[second_pos];
        let after_id = order[second_pos + 1];

        // Remove exactly the three edges in the chain segment
        self.connections.retain(|c| {
            !((c.from_node == before_id && c.to_node == first_id)
                || (c.from_node == first_id && c.to_node == second_id)
                || (c.from_node == second_id && c.to_node == after_id))
        });

        // Re-wire: before → second → first → after
        for (from, to) in [
            (before_id, second_id),
            (second_id, first_id),
            (first_id, after_id),
        ] {
            let ch = self
                .node_output_channels(from)
                .min(self.node_input_channels(to))
                .max(1);
            for port in 0..ch {
                let _ = self.add_connection(from, port, to, port);
            }
        }

        Ok(())
    }

    /// Toggle a plugin's enabled state.
    pub fn toggle_plugin(&mut self, node_id: GraphNodeId) -> Result<(), String> {
        let node = self.nodes.get_mut(&node_id).ok_or("Node not found")?;
        node.plugin.enabled = !node.plugin.enabled;
        Ok(())
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    /// Get the canvas position of any node (plugin or special).
    fn node_position(&self, id: GraphNodeId) -> Option<NodePosition> {
        self.nodes
            .get(&id)
            .map(|n| n.position)
            .or_else(|| self.special_nodes.get(&id).map(|n| n.position))
    }

    // =========================================================================
    // Index-based Linear Access (PluginChain compatibility)
    // =========================================================================

    /// Number of plugin nodes (excluding special nodes).
    /// Equivalent to `PluginChain::len()`.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Alias for `len()`.
    pub fn plugin_count(&self) -> usize {
        self.len()
    }

    /// Get all plugins in linear order as a Vec of references.
    /// Returns an empty Vec if the graph is not linear.
    /// Use this as a replacement for `PluginChain::plugins()`.
    pub fn plugins(&self) -> Vec<&Plugin> {
        self.plugins_linear()
            .map(|nodes| nodes.into_iter().map(|n| &n.plugin).collect())
            .unwrap_or_default()
    }

    /// Add a user plugin before the Matrix and return its linear index.
    /// Convenience for callers that use index-based access (PluginChain compat).
    pub fn add_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let _ = self.add_user_plugin(plugin_type);
        // Return the index of the newly inserted plugin (just before Matrix)
        self.user_plugin_insert_index().saturating_sub(1)
    }

    /// Get plugin by linear index (excluding special nodes).
    pub fn get_plugin(&self, index: usize) -> Option<&Plugin> {
        self.plugins_linear()
            .and_then(|p| p.get(index).map(|n| &n.plugin))
    }

    /// Get mutable plugin by linear index (excluding special nodes).
    pub fn get_plugin_mut(&mut self, index: usize) -> Option<&mut Plugin> {
        let plugin_ids: Vec<GraphNodeId> = self
            .linear_order()?
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();
        let node_id = plugin_ids.get(index).copied()?;
        self.nodes.get_mut(&node_id).map(|n| &mut n.plugin)
    }

    /// Set plugin settings by linear index. Returns the old settings.
    pub fn set_plugin_settings_by_index(
        &mut self,
        index: usize,
        settings: PluginSettings,
    ) -> Option<PluginSettings> {
        let node_id = {
            let plugins = self.plugins_linear()?;
            plugins.get(index).map(|n| n.id)?
        };
        let node = self.nodes.get_mut(&node_id)?;
        let old = std::mem::replace(&mut node.plugin.settings, settings);
        Some(old)
    }

    /// Check if an index-based plugin can be moved up in the linear chain.
    pub fn can_move_up_by_index(&self, index: usize) -> bool {
        let Some(order) = self.linear_order() else {
            return false;
        };
        let plugin_ids: Vec<GraphNodeId> = order
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();
        let Some(&node_id) = plugin_ids.get(index) else {
            return false;
        };
        let Some(node) = self.nodes.get(&node_id) else {
            return false;
        };
        if node.role != NodeRole::User {
            return false;
        }
        // Check if the predecessor in plugin order is also a User
        index > 0
            && plugin_ids
                .get(index - 1)
                .and_then(|id| self.nodes.get(id))
                .is_some_and(|n| n.role == NodeRole::User)
    }

    /// Check if an index-based plugin can be moved down in the linear chain.
    pub fn can_move_down_by_index(&self, index: usize) -> bool {
        let Some(order) = self.linear_order() else {
            return false;
        };
        let plugin_ids: Vec<GraphNodeId> = order
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();
        let Some(&node_id) = plugin_ids.get(index) else {
            return false;
        };
        let Some(node) = self.nodes.get(&node_id) else {
            return false;
        };
        if node.role != NodeRole::User {
            return false;
        }
        index + 1 < plugin_ids.len()
            && plugin_ids
                .get(index + 1)
                .and_then(|id| self.nodes.get(id))
                .is_some_and(|n| n.role == NodeRole::User)
    }

    /// Toggle a plugin's enabled state by linear index.
    pub fn toggle_plugin_by_index(&mut self, index: usize) -> Result<(), String> {
        let node_id = self
            .plugins_linear()
            .and_then(|p| p.get(index).map(|n| n.id))
            .ok_or("Invalid index")?;
        self.toggle_plugin(node_id)
    }

    /// Remove a user plugin by linear index.
    pub fn remove_plugin_by_index(&mut self, index: usize) -> Result<(), String> {
        let node_id = self
            .plugins_linear()
            .and_then(|p| p.get(index).map(|n| n.id))
            .ok_or("Invalid index")?;
        self.remove_user_plugin(node_id)
    }

    /// Move a plugin up by linear index.
    pub fn move_plugin_up_by_index(&mut self, index: usize) -> Result<(), String> {
        let node_id = self
            .plugins_linear()
            .and_then(|p| p.get(index).map(|n| n.id))
            .ok_or("Invalid index")?;
        self.move_user_plugin_up(node_id)
    }

    /// Move a plugin down by linear index.
    pub fn move_plugin_down_by_index(&mut self, index: usize) -> Result<(), String> {
        let node_id = self
            .plugins_linear()
            .and_then(|p| p.get(index).map(|n| n.id))
            .ok_or("Invalid index")?;
        self.move_user_plugin_down(node_id)
    }

    /// Insert a plugin at a specific linear index. The node is wired into the chain.
    /// Equivalent to `PluginChain::insert_plugin()`.
    pub fn insert_plugin(
        &mut self,
        index: usize,
        plugin_type: &PluginType,
    ) -> Result<GraphNodeId, String> {
        let order = self.linear_order().ok_or("Graph is not linear")?;
        let plugin_ids: Vec<GraphNodeId> = order
            .iter()
            .filter(|id| self.nodes.contains_key(id))
            .copied()
            .collect();

        // Clamp index to valid range
        let index = index.min(plugin_ids.len());

        // Find the predecessor and successor in the full order (including special nodes)
        let pred_id = if index == 0 {
            // Before the first plugin → predecessor is the Input special node
            order[0]
        } else {
            plugin_ids[index - 1]
        };
        let succ_id = if index >= plugin_ids.len() {
            // After the last plugin → successor is the Output special node
            *order.last().unwrap()
        } else {
            plugin_ids[index]
        };

        let pred_pos = self.node_position(pred_id).unwrap_or_default();
        let succ_pos = self.node_position(succ_id).unwrap_or_default();
        let new_pos = NodePosition::new((pred_pos.x + succ_pos.x) / 2.0, pred_pos.y);

        let new_id = self.add_plugin_node_with_role(plugin_type, new_pos, NodeRole::User, false);
        self.rewire_insert(pred_id, succ_id, new_id);
        Ok(new_id)
    }

    /// Move a plugin from one linear index to another (drag-and-drop).
    /// Both positions must be user plugins. Tracks the node by ID so intermediate
    /// swaps don't cause the wrong plugin to be moved.
    pub fn move_plugin(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        // Grab the node ID and validate both endpoints are User plugins
        let node_id = match self.plugins_linear() {
            Some(p) => match (p.get(from), p.get(to)) {
                (Some(f), Some(t)) if f.role == NodeRole::User && t.role == NodeRole::User => f.id,
                _ => return,
            },
            None => return,
        };

        let steps = from.abs_diff(to);
        for _ in 0..steps {
            let result = if from < to {
                self.move_user_plugin_down(node_id)
            } else {
                self.move_user_plugin_up(node_id)
            };
            if result.is_err() {
                break;
            }
        }
    }

    /// Whether the graph has an enabled spectrum analyzer.
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.nodes.values().any(|n| {
            n.plugin.enabled && matches!(n.plugin.plugin_type(), PluginType::SpectrumAnalyzer)
        })
    }

    /// Find channel conflicts for a given input channel count.
    /// Matches the logic of `PluginChain::find_channel_conflicts`.
    pub fn find_channel_conflicts(&self, input_channels: usize) -> Vec<ChannelConflict> {
        let Some(plugins) = self.plugins_linear() else {
            return vec![];
        };
        let mut current_channels = input_channels;
        let mut conflicts = Vec::new();

        for (i, node) in plugins.iter().enumerate() {
            if !node.plugin.enabled || node.plugin.suspended {
                continue;
            }

            // Use the settings-based required_input_channels (None = accepts any)
            if let Some(required) = node.plugin.settings.required_input_channels() {
                if required != current_channels {
                    conflicts.push(ChannelConflict {
                        index: i,
                        plugin_type: node.plugin.plugin_type(),
                        required_channels: required,
                        actual_channels: current_channels,
                    });
                    continue; // Skip output tracking for conflicting plugin
                }
            }

            // Track output channel changes
            match &node.plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. }
                | PluginSettings::AAE { speaker_config, .. } => {
                    current_channels = upmixer_output_channels(speaker_config);
                }
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    current_channels = upmixer_output_channels(target_layout);
                }
                PluginSettings::BinauralDecoder { .. }
                | PluginSettings::Downmix { .. }
                | PluginSettings::MonoToStereo { .. } => {
                    current_channels = 2;
                }
                PluginSettings::Matrix {
                    output_channels, ..
                } => {
                    current_channels = *output_channels;
                }
                PluginSettings::BandSplit { .. } => {
                    current_channels *= 2;
                }
                PluginSettings::BandMerge { bands, .. } => {
                    current_channels /= if *bands > 0 { *bands } else { 2 };
                }
                _ => {}
            }
        }
        conflicts
    }

    /// Suspend plugins at the given linear indices.
    pub fn suspend_plugins(&mut self, indices: &[usize]) {
        let Some(ids) = self.linear_order() else {
            return;
        };
        let plugin_ids: Vec<GraphNodeId> = ids
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();
        for &idx in indices {
            if let Some(&node_id) = plugin_ids.get(idx) {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.plugin.suspended = true;
                }
            }
        }
    }

    /// Find the linear index of the first plugin of a given type.
    pub fn find_plugin_index(&self, plugin_type: &PluginType) -> Option<usize> {
        self.plugins_linear()?
            .iter()
            .position(|n| n.plugin.plugin_type() == *plugin_type)
    }

    /// Clear all suspensions.
    pub fn clear_suspensions(&mut self) {
        for node in self.nodes.values_mut() {
            node.plugin.suspended = false;
        }
    }

    /// Whether any plugin is suspended.
    pub fn has_suspensions(&self) -> bool {
        self.nodes.values().any(|n| n.plugin.suspended)
    }

    // =========================================================================
    // Channel & Speaker Config
    // =========================================================================

    /// Output channel count assuming stereo input.
    pub fn output_channels(&self) -> usize {
        self.output_channels_for_input(2)
    }

    /// Walk the chain to determine the output channel count for a given input.
    pub fn output_channels_for_input(&self, input_channels: usize) -> usize {
        let Some(plugins) = self.plugins_linear() else {
            return input_channels;
        };
        for node in plugins.iter().rev() {
            if !node.plugin.enabled || node.plugin.suspended {
                continue;
            }
            match &node.plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. }
                | PluginSettings::AAE { speaker_config, .. } => {
                    return upmixer_output_channels(speaker_config);
                }
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    return upmixer_output_channels(target_layout);
                }
                PluginSettings::BinauralDecoder { .. }
                | PluginSettings::Downmix { .. }
                | PluginSettings::MonoToStereo { .. } => return 2,
                PluginSettings::Matrix {
                    output_channels, ..
                } => return *output_channels,
                _ => continue,
            }
        }
        input_channels
    }

    /// Get the speaker config from the last enabled upmixer/binaural plugin.
    pub fn output_speaker_config(&self) -> Option<String> {
        let plugins = self.plugins_linear()?;
        for node in plugins.iter().rev() {
            if !node.plugin.enabled {
                continue;
            }
            match &node.plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. }
                | PluginSettings::AAE { speaker_config, .. } => {
                    return Some(speaker_config.clone());
                }
                PluginSettings::BinauralDecoder { .. } => return Some("2.0".to_string()),
                _ => continue,
            }
        }
        None
    }

    /// Speaker config active at a given linear index.
    pub fn speaker_config_at_index(&self, target_index: usize) -> Option<String> {
        let plugins = self.plugins_linear()?;
        let mut config: Option<String> = None;
        for (i, node) in plugins.iter().enumerate() {
            if i >= target_index {
                break;
            }
            if !node.plugin.enabled {
                continue;
            }
            match &node.plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. }
                | PluginSettings::AAE { speaker_config, .. }
                | PluginSettings::AmbisonicsDecoder {
                    target_layout: speaker_config,
                    ..
                } => config = Some(speaker_config.clone()),
                PluginSettings::BinauralDecoder { .. }
                | PluginSettings::Downmix { .. }
                | PluginSettings::MonoToStereo { .. } => config = Some("2.0".to_string()),
                _ => {}
            }
        }
        config
    }

    // =========================================================================
    // Role-based Queries
    // =========================================================================

    /// Whether the plugin at `index` in linear order is the input monitor.
    pub fn is_input_monitor(&self, index: usize) -> bool {
        self.plugins_linear().is_some_and(|p| {
            p.get(index)
                .is_some_and(|n| n.role == NodeRole::InputMonitor)
        })
    }

    /// Whether the plugin at `index` in linear order is the output monitor.
    pub fn is_output_monitor(&self, index: usize) -> bool {
        self.plugins_linear().is_some_and(|p| {
            p.get(index)
                .is_some_and(|n| n.role == NodeRole::OutputMonitor)
        })
    }

    /// Index of the insert position for user plugins (before the Matrix).
    pub fn user_plugin_insert_index(&self) -> usize {
        let Some(plugins) = self.plugins_linear() else {
            return 0;
        };
        plugins
            .iter()
            .position(|n| n.role == NodeRole::Matrix)
            .unwrap_or(plugins.len())
    }

    // =========================================================================
    // Replay Gain
    // =========================================================================

    /// Set the replay gain on the permanent Gain (ReplayGain) plugin.
    pub fn set_replay_gain(&mut self, gain_db: Option<f64>) {
        let Some(node) = self.node_for_role_mut(NodeRole::ReplayGain) else {
            return;
        };
        match gain_db {
            Some(db) => {
                node.plugin.enabled = true;
                node.plugin.settings = PluginSettings::Gain {
                    channels: match &node.plugin.settings {
                        PluginSettings::Gain { channels, .. } => *channels,
                        _ => 2,
                    },
                    gain_db: db,
                    smoothing_ms: match &node.plugin.settings {
                        PluginSettings::Gain { smoothing_ms, .. } => *smoothing_ms,
                        _ => {
                            use sotf_plugins::param_specs::{find_by_key, gain};
                            find_by_key(gain::PARAMS, "smoothing_ms").default_f64()
                        }
                    },
                };
            }
            None => {
                node.plugin.enabled = false;
            }
        }
    }

    /// Read the current replay gain value.
    pub fn replay_gain_db(&self) -> Option<f64> {
        let node = self.node_for_role(NodeRole::ReplayGain)?;
        if node.plugin.enabled {
            match &node.plugin.settings {
                PluginSettings::Gain { gain_db, .. } => Some(*gain_db),
                _ => None,
            }
        } else {
            None
        }
    }

    // =========================================================================
    // Channel-Dependent Plugin Updates
    // =========================================================================

    /// Adapt the matrix plugin to match the file's channel count.
    pub fn adapt_matrix_to_input(&mut self, file_channels: usize) {
        let Some(order) = self.linear_order() else {
            return;
        };
        let plugin_ids: Vec<GraphNodeId> = order
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();

        let mut running_channels = file_channels;
        for &node_id in &plugin_ids {
            let node = &self.nodes[&node_id];
            if !node.plugin.enabled || node.plugin.suspended {
                continue;
            }
            match &node.plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. }
                | PluginSettings::AAE { speaker_config, .. } => {
                    running_channels = upmixer_output_channels(speaker_config);
                    continue;
                }
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    running_channels = upmixer_output_channels(target_layout);
                    continue;
                }
                PluginSettings::BinauralDecoder { .. }
                | PluginSettings::Downmix { .. }
                | PluginSettings::MonoToStereo { .. } => {
                    running_channels = 2;
                    continue;
                }
                _ => {}
            }
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } = &node.plugin.settings
            {
                if *input_channels != running_channels {
                    let old_in = *input_channels;
                    let old_out = *output_channels;
                    // Need mutable access
                    let node = self.nodes.get_mut(&node_id)
                        .expect("plugin_ids/nodes desync: node missing from map");
                    if let PluginSettings::Matrix {
                        input_channels,
                        output_channels,
                        matrix,
                        channel_states,
                    } = &mut node.plugin.settings
                    {
                        resize_matrix(matrix, old_in, old_out, running_channels, running_channels);
                        *input_channels = running_channels;
                        *output_channels = running_channels;
                        channel_states
                            .resize(running_channels, sotf_plugins::ChannelState::default());
                    }
                    break;
                }
            }
        }
    }

    /// Update channel-dependent plugins (EQ, Gain, BinauralDecoder, Matrix, etc.)
    /// after any topology change.
    pub fn update_channel_dependent_plugins(&mut self) {
        let Some(order) = self.linear_order() else {
            return;
        };
        let plugin_ids: Vec<GraphNodeId> = order
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();

        let mut current_channels: usize = 2;

        for &node_id in &plugin_ids {
            let node = &self.nodes[&node_id];
            let mut updated_settings = None;

            match &node.plugin.settings {
                PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters,
                    tdf2,
                    topology,
                } => {
                    if *channels != current_channels {
                        let ch_filters_match = channel_filters
                            .as_ref()
                            .is_none_or(|cf| cf.len() == current_channels);
                        let (new_channel_filters, new_per_channel_mode) =
                            if *per_channel_mode && !ch_filters_match {
                                (None, false)
                            } else {
                                (channel_filters.clone(), *per_channel_mode)
                            };
                        updated_settings = Some(PluginSettings::EQ {
                            channels: current_channels,
                            filters: filters.clone(),
                            channel_filters: new_channel_filters,
                            per_channel_mode: new_per_channel_mode,
                            max_filters: *max_filters,
                            tdf2: *tdf2,
                            topology: *topology,
                        });
                    }
                }
                PluginSettings::Gain {
                    channels,
                    gain_db,
                    smoothing_ms,
                } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::Gain {
                            channels: current_channels,
                            gain_db: *gain_db,
                            smoothing_ms: *smoothing_ms,
                        });
                    }
                }
                PluginSettings::BinauralDecoder {
                    sofa_file,
                    input_channels,
                    enable_optimization,
                    externalization,
                    near_field_strength,
                    crossfade_mode,
                    late_reverb_enabled,
                    late_reverb_mix,
                    late_reverb_rt60,
                    late_reverb_damping,
                    headphone_eq_enabled,
                } => {
                    if *input_channels != current_channels {
                        updated_settings = Some(PluginSettings::BinauralDecoder {
                            sofa_file: sofa_file.clone(),
                            input_channels: current_channels,
                            enable_optimization: *enable_optimization,
                            externalization: *externalization,
                            near_field_strength: *near_field_strength,
                            crossfade_mode: *crossfade_mode,
                            late_reverb_enabled: *late_reverb_enabled,
                            late_reverb_mix: *late_reverb_mix,
                            late_reverb_rt60: *late_reverb_rt60,
                            late_reverb_damping: *late_reverb_damping,
                            headphone_eq_enabled: *headphone_eq_enabled,
                        });
                    }
                }
                PluginSettings::Matrix {
                    input_channels,
                    output_channels,
                    matrix,
                    channel_states,
                } => {
                    if *input_channels != current_channels {
                        let mut new_matrix = matrix.clone();
                        resize_matrix(
                            &mut new_matrix,
                            *input_channels,
                            *output_channels,
                            current_channels,
                            current_channels,
                        );
                        let mut new_states = channel_states.clone();
                        new_states.resize(current_channels, sotf_plugins::ChannelState::default());
                        updated_settings = Some(PluginSettings::Matrix {
                            input_channels: current_channels,
                            output_channels: current_channels,
                            matrix: new_matrix,
                            channel_states: new_states,
                        });
                    }
                }
                PluginSettings::Downmix {
                    input_channels,
                    center_gain_db,
                    surround_gain_db,
                    height_gain_db,
                    lfe_gain_db,
                    phase_coherence,
                    phase_blend_low_hz,
                    phase_blend_high_hz,
                    itu_mode,
                } => {
                    if *input_channels != current_channels {
                        updated_settings = Some(PluginSettings::Downmix {
                            input_channels: current_channels,
                            center_gain_db: *center_gain_db,
                            surround_gain_db: *surround_gain_db,
                            height_gain_db: *height_gain_db,
                            lfe_gain_db: *lfe_gain_db,
                            phase_coherence: *phase_coherence,
                            phase_blend_low_hz: *phase_blend_low_hz,
                            phase_blend_high_hz: *phase_blend_high_hz,
                            itu_mode: *itu_mode,
                        });
                    }
                }
                PluginSettings::BandSplit {
                    channels,
                    frequency,
                    crossover_type,
                } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::BandSplit {
                            channels: current_channels,
                            frequency: *frequency,
                            crossover_type: crossover_type.clone(),
                        });
                    }
                }
                PluginSettings::BandMerge { channels, bands } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::BandMerge {
                            channels: current_channels,
                            bands: *bands,
                        });
                    }
                }
                _ => {}
            }

            if let Some(new_settings) = updated_settings {
                self.nodes.get_mut(&node_id)
                    .expect("plugin_ids/nodes desync: node missing from map")
                    .plugin.settings = new_settings;
            }

            // Track output channels for the next plugin
            let node = &self.nodes[&node_id];
            if node.plugin.enabled && !node.plugin.suspended {
                match &node.plugin.settings {
                    PluginSettings::Upmixer { speaker_config, .. }
                    | PluginSettings::AAE { speaker_config, .. } => {
                        current_channels = upmixer_output_channels(speaker_config);
                    }
                    PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                        current_channels = upmixer_output_channels(target_layout);
                    }
                    PluginSettings::BinauralDecoder { .. } => current_channels = 2,
                    PluginSettings::Matrix {
                        output_channels, ..
                    } => current_channels = *output_channels,
                    PluginSettings::Downmix { .. } | PluginSettings::MonoToStereo { .. } => {
                        current_channels = 2;
                    }
                    PluginSettings::BandSplit { .. } => current_channels *= 2,
                    PluginSettings::BandMerge { bands, .. } => {
                        current_channels /= if *bands > 0 { *bands } else { 2 };
                    }
                    _ => {}
                }
            }
        }
    }

    // =========================================================================
    // Preset Save / Load
    // =========================================================================

    /// Save the user plugins to a JSON preset file.
    /// Uses the same v2 format as PluginChain for backward compatibility.
    pub fn save_to_file(
        &self,
        presets_dir: &std::path::Path,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::path::Path::new(filename);
        let extension = path.extension().and_then(|ext| ext.to_str());

        if let Some(ext) = extension {
            if ext != "json" {
                return Err(format!(
                    "Only .json files are supported. Please use .json extension instead of .{}",
                    ext
                )
                .into());
            }
        }

        let filename = if extension.is_none() {
            format!("{}.json", filename)
        } else {
            filename.to_string()
        };

        // Sanitize: use only the file name component to prevent path traversal
        let safe_name = std::path::Path::new(&filename)
            .file_name()
            .ok_or("Invalid preset filename")?;
        let full_path = presets_dir.join(safe_name);

        // Extract plugins in linear order for serialization
        let plugins: Vec<Plugin> = self
            .plugins_linear()
            .map(|nodes| nodes.into_iter().map(|n| n.plugin.clone()).collect())
            .unwrap_or_default();

        #[derive(serde::Serialize)]
        struct PluginPreset {
            version: u32,
            plugins: Vec<Plugin>,
        }

        let preset = PluginPreset {
            version: 2,
            plugins,
        };

        let json = serde_json::to_string_pretty(&preset)?;
        std::fs::write(&full_path, json)?;

        log::info!("Saved plugin graph to {}", full_path.display());
        Ok(())
    }

    /// Load a plugin preset from a JSON file into this graph.
    ///
    /// Rebuilds the graph as a fresh default rack with the loaded user plugins
    /// inserted before the Matrix. Permanent plugins from the file are skipped
    /// (the default rack provides them). Individual plugin deserialization
    /// failures are skipped and returned as warnings.
    pub fn load_from_file(
        &mut self,
        presets_dir: &std::path::Path,
        filename: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let path = std::path::Path::new(filename);
        let final_filename = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };

        // Sanitize: use only the file name component to prevent path traversal
        let safe_name = std::path::Path::new(&final_filename)
            .file_name()
            .ok_or("Invalid preset filename")?;
        let full_path = presets_dir.join(safe_name);
        let json = std::fs::read_to_string(&full_path)?;

        // Parse with lenient per-plugin deserialization
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct PluginPresetRaw {
            #[serde(default = "default_version")]
            version: u32,
            plugins: Vec<serde_json::Value>,
        }
        fn default_version() -> u32 {
            2
        }

        let raw: PluginPresetRaw = match serde_json::from_str(&json) {
            Ok(p) => p,
            Err(_) => {
                let plugins: Vec<serde_json::Value> = serde_json::from_str(&json)?;
                PluginPresetRaw {
                    version: 0,
                    plugins,
                }
            }
        };

        let mut warnings = Vec::new();
        let mut loaded_plugins = Vec::new();

        for (i, val) in raw.plugins.iter().enumerate() {
            match serde_json::from_value::<Plugin>(val.clone()) {
                Ok(p) => loaded_plugins.push(p),
                Err(e) => {
                    let ptype = val
                        .get("settings")
                        .and_then(|s| {
                            s.as_str()
                                .map(String::from)
                                .or_else(|| s.as_object().and_then(|o| o.keys().next().cloned()))
                        })
                        .unwrap_or_else(|| "unknown".into());
                    let msg = format!("Plugin {} ('{}') skipped: {}", i, ptype, e);
                    log::warn!("{}", msg);
                    warnings.push(msg);
                }
            }
        }

        // Strip edge LoudnessMonitors (presets may include the permanent monitors)
        while loaded_plugins
            .first()
            .is_some_and(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor) && !p.permanent)
        {
            loaded_plugins.remove(0);
        }
        while loaded_plugins
            .last()
            .is_some_and(|p| matches!(p.plugin_type(), PluginType::LoudnessMonitor) && !p.permanent)
        {
            loaded_plugins.pop();
        }

        // Rebuild: start with a fresh default rack, then insert user plugins
        *self = Self::with_default_rack();

        // Extract non-permanent plugins and insert them
        let user_plugins: Vec<Plugin> = loaded_plugins
            .into_iter()
            .filter(|p| !p.permanent)
            .collect();

        // Update next_plugin_id to avoid collisions
        let max_id = user_plugins.iter().map(|p| p.id).max().unwrap_or(0);
        if max_id >= self.next_plugin_id {
            self.next_plugin_id = max_id + 1;
        }

        for plugin in user_plugins {
            // Compute linear order and predecessor BEFORE inserting the unconnected node,
            // otherwise linear_order() fails due to the disconnected node.
            let matrix_id = self
                .node_id_for_role(NodeRole::Matrix)
                .ok_or_else(|| "No Matrix node in graph".to_string())?;
            let order = self
                .linear_order()
                .ok_or_else(|| "Graph is not linear during plugin load".to_string())?;
            let matrix_pos = order
                .iter()
                .position(|&id| id == matrix_id)
                .ok_or_else(|| "Matrix not in linear order".to_string())?;
            if matrix_pos == 0 {
                return Err("Matrix cannot be the first node in the chain".into());
            }
            let pred_id = order[matrix_pos - 1];

            let pos_x = self
                .node_position(matrix_id)
                .map(|p| p.x - 10.0)
                .unwrap_or(400.0);
            let node =
                PluginGraphNode::with_role(plugin, NodePosition::new(pos_x, 200.0), NodeRole::User);
            let node_id = node.id;
            self.nodes.insert(node_id, node);

            self.rewire_insert(pred_id, matrix_id, node_id);
        }

        log::info!(
            "Loaded plugin graph from {} ({} plugins, {} skipped)",
            full_path.display(),
            self.len(),
            warnings.len()
        );
        Ok(warnings)
    }

    // =========================================================================
    // ASCII Diagram (for TUI display of non-linear graphs)
    // =========================================================================

    /// Render the graph as an ASCII topology diagram.
    ///
    /// Linear graphs: `Input → EQ → Comp → Matrix → Output`
    /// Non-linear graphs show branching with column-based layout.
    pub fn to_ascii_diagram(&self) -> Vec<String> {
        let Ok(sorted) = self.topological_sort() else {
            return vec!["(empty graph)".to_string()];
        };
        if sorted.is_empty() {
            return vec!["(empty graph)".to_string()];
        }

        // If linear, simple arrow chain
        if let Some(order) = self.linear_order() {
            let names: Vec<String> = order.iter().map(|id| self.node_display_name(*id)).collect();
            // Split into lines of ~80 chars
            let mut lines = Vec::new();
            let mut line = String::new();
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    line.push_str(" → ");
                }
                if line.len() + name.len() > 76 && !line.is_empty() {
                    lines.push(line);
                    line = String::from("  → ");
                }
                line.push_str(name);
            }
            if !line.is_empty() {
                lines.push(line);
            }
            return lines;
        }

        // Non-linear: assign depth (longest path from any root)
        let logical_edges: HashSet<(GraphNodeId, GraphNodeId)> = self
            .connections
            .iter()
            .map(|c| (c.from_node, c.to_node))
            .collect();

        let mut depth: HashMap<GraphNodeId, usize> = HashMap::new();
        for &id in &sorted {
            let d = logical_edges
                .iter()
                .filter(|(_, t)| *t == id)
                .filter_map(|(f, _)| depth.get(f).map(|d| d + 1))
                .max()
                .unwrap_or(0);
            depth.insert(id, d);
        }

        // Group by depth
        let max_depth = depth.values().copied().max().unwrap_or(0);
        let mut columns: Vec<Vec<GraphNodeId>> = vec![Vec::new(); max_depth + 1];
        for &id in &sorted {
            if let Some(&d) = depth.get(&id) {
                columns[d].push(id);
            }
        }

        // Render columns
        let max_rows = columns.iter().map(|c| c.len()).max().unwrap_or(1);
        let mut lines = Vec::new();

        for row in 0..max_rows {
            let mut parts = Vec::new();
            for col in &columns {
                if let Some(&id) = col.get(row) {
                    parts.push(format!("[{:^14}]", self.node_display_name(id)));
                } else {
                    parts.push(format!("{:16}", ""));
                }
            }
            lines.push(parts.join("  "));
        }

        // Add connection arrows between columns
        let mut arrow_line = String::new();
        for (i, col) in columns.iter().enumerate() {
            if i > 0 {
                arrow_line.push_str("→ ");
            }
            let count = col.len();
            arrow_line.push_str(&format!("{:^16}", if count > 1 { "┤" } else { "─" }));
        }
        lines.insert(0, arrow_line);
        lines.insert(
            0,
            "(non-linear graph — use GUI for full editing)".to_string(),
        );

        lines
    }

    /// Get a display name for any node (plugin or special).
    fn node_display_name(&self, id: GraphNodeId) -> String {
        if let Some(node) = self.nodes.get(&id) {
            let enabled = if node.plugin.enabled { "" } else { " [off]" };
            format!("{}{}", node.plugin.plugin_type().name(), enabled)
        } else if let Some(special) = self.special_nodes.get(&id) {
            special.display_name()
        } else {
            "?".to_string()
        }
    }

    // =========================================================================
    // Engine Serialization
    // =========================================================================

    /// Serialize the plugin graph to engine configs.
    ///
    /// Order: input monitor (if enabled) → processing plugins → output analyzers.
    /// This matches the ordering produced by `PluginChain::to_plugin_configs`.
    pub fn to_plugin_configs(&self, sample_rate: f64) -> Vec<PluginConfig> {
        let mut input_monitor: Option<PluginConfig> = None;
        let mut processing = Vec::new();
        let mut analyzers = Vec::new();

        // Use linear order if available, otherwise topological sort
        let ordered_ids = self
            .linear_order()
            .or_else(|| self.topological_sort().ok())
            .unwrap_or_default();

        for id in ordered_ids {
            let Some(node) = self.nodes.get(&id) else {
                continue; // skip special nodes
            };
            let Some(config) = node.plugin.to_plugin_config(sample_rate) else {
                continue; // skip disabled/suspended
            };

            match node.role {
                NodeRole::InputMonitor => input_monitor = Some(config),
                NodeRole::OutputMonitor => analyzers.push(config),
                _ => processing.push(config),
            }
        }

        let mut result = Vec::with_capacity(1 + processing.len() + analyzers.len());
        result.extend(input_monitor);
        result.extend(processing);
        result.extend(analyzers);
        result
    }

    /// Map a graph node ID to its index in the `to_plugin_configs()` output.
    ///
    /// Returns None if the node is disabled/suspended or not found.
    pub fn get_engine_index(&self, node_id: GraphNodeId) -> Option<usize> {
        let node = self.nodes.get(&node_id)?;
        if !node.plugin.enabled || node.plugin.suspended {
            return None;
        }

        let ordered_ids = self
            .linear_order()
            .or_else(|| self.topological_sort().ok())
            .unwrap_or_default();

        // Build the same three-category ordering as to_plugin_configs
        let mut input_monitor_id: Option<GraphNodeId> = None;
        let mut processing_ids = Vec::new();
        let mut analyzer_ids = Vec::new();

        for id in ordered_ids {
            let Some(n) = self.nodes.get(&id) else {
                continue;
            };
            if !n.plugin.enabled || n.plugin.suspended {
                continue;
            }

            match n.role {
                NodeRole::InputMonitor => input_monitor_id = Some(id),
                NodeRole::OutputMonitor => analyzer_ids.push(id),
                _ => processing_ids.push(id),
            }
        }

        // Search: input monitor at index 0, then processing, then analyzers
        let offset = if input_monitor_id.is_some() { 1 } else { 0 };

        if input_monitor_id == Some(node_id) {
            return Some(0);
        }
        if let Some(pos) = processing_ids.iter().position(|&id| id == node_id) {
            return Some(offset + pos);
        }
        if let Some(pos) = analyzer_ids.iter().position(|&id| id == node_id) {
            return Some(offset + processing_ids.len() + pos);
        }

        None
    }

    /// Map a linear position index (as shown in the rack) to an engine index.
    ///
    /// The linear position is the index within `plugins_linear()`.
    pub fn get_engine_index_by_linear_position(&self, linear_idx: usize) -> Option<usize> {
        let order = self.linear_order()?;
        // Filter to plugin nodes only (skip special nodes)
        let plugin_ids: Vec<GraphNodeId> = order
            .into_iter()
            .filter(|id| self.nodes.contains_key(id))
            .collect();
        let node_id = plugin_ids.get(linear_idx)?;
        self.get_engine_index(*node_id)
    }

    /// Get the engine index of the input monitor.
    pub fn input_monitor_engine_index(&self) -> Option<usize> {
        self.node_id_for_role(NodeRole::InputMonitor)
            .and_then(|id| self.get_engine_index(id))
    }

    /// Get the engine index of the output monitor.
    pub fn output_monitor_engine_index(&self) -> Option<usize> {
        self.node_id_for_role(NodeRole::OutputMonitor)
            .and_then(|id| self.get_engine_index(id))
    }

    /// Get the engine index of the matrix.
    pub fn matrix_engine_index(&self) -> Option<usize> {
        self.node_id_for_role(NodeRole::Matrix)
            .and_then(|id| self.get_engine_index(id))
    }

    /// Get the engine index of the first enabled spectrum analyzer.
    pub fn spectrum_engine_index(&self) -> Option<usize> {
        self.nodes
            .values()
            .find(|n| {
                n.plugin.enabled && matches!(n.plugin.plugin_type(), PluginType::SpectrumAnalyzer)
            })
            .and_then(|n| self.get_engine_index(n.id))
    }

    /// Get the engine index of the first enabled compressor.
    pub fn compressor_engine_index(&self) -> Option<usize> {
        self.nodes
            .values()
            .find(|n| n.plugin.enabled && matches!(n.plugin.plugin_type(), PluginType::Compressor))
            .and_then(|n| self.get_engine_index(n.id))
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    /// Remove connections from `pred` to `succ`, insert `new_id` between them.
    fn rewire_insert(&mut self, pred_id: GraphNodeId, succ_id: GraphNodeId, new_id: GraphNodeId) {
        // Remove pred→succ connections
        self.connections
            .retain(|c| !(c.from_node == pred_id && c.to_node == succ_id));

        // Wire pred→new
        let ch1 = self
            .node_output_channels(pred_id)
            .min(self.node_input_channels(new_id))
            .max(1);
        for port in 0..ch1 {
            let _ = self.add_connection(pred_id, port, new_id, port);
        }

        // Wire new→succ
        let ch2 = self
            .node_output_channels(new_id)
            .min(self.node_input_channels(succ_id))
            .max(1);
        for port in 0..ch2 {
            let _ = self.add_connection(new_id, port, succ_id, port);
        }
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
    use math_audio_iir_fir::BiquadFilterType;
    use sotf_audio::plugins::EQFilter;

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

    #[test]
    fn test_default_rack_is_linear() {
        let g = PluginGraph::with_default_rack();
        assert!(g.is_linear());

        let order = g.linear_order().unwrap();
        // Input + 4 plugins + Output = 6 nodes
        assert_eq!(order.len(), 6);

        // First node is Input, last is Output
        assert!(g.special_nodes.get(&order[0]).is_some());
        assert!(g.special_nodes.get(&order[5]).is_some());

        // Roles are in order
        let plugins: Vec<_> = g.plugins_linear().unwrap();
        assert_eq!(plugins.len(), 4);
        assert_eq!(plugins[0].role, NodeRole::InputMonitor);
        assert_eq!(plugins[1].role, NodeRole::ReplayGain);
        assert_eq!(plugins[2].role, NodeRole::Matrix);
        assert_eq!(plugins[3].role, NodeRole::OutputMonitor);
    }

    #[test]
    fn test_add_user_plugin() {
        let mut g = PluginGraph::with_default_rack();
        let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

        assert!(g.is_linear());
        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins.len(), 5);

        // EQ should be between ReplayGain and Matrix
        assert_eq!(plugins[2].role, NodeRole::User);
        assert_eq!(plugins[2].id, eq_id);
        assert_eq!(plugins[3].role, NodeRole::Matrix);
    }

    #[test]
    fn test_remove_user_plugin() {
        let mut g = PluginGraph::with_default_rack();
        let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

        g.remove_user_plugin(eq_id).unwrap();
        assert!(g.is_linear());
        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins.len(), 4); // Back to default
    }

    #[test]
    fn test_cannot_remove_permanent() {
        let mut g = PluginGraph::with_default_rack();
        let matrix_id = g.node_id_for_role(NodeRole::Matrix).unwrap();
        assert!(g.remove_user_plugin(matrix_id).is_err());
    }

    #[test]
    fn test_move_user_plugin() {
        let mut g = PluginGraph::with_default_rack();
        let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
        let comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

        // Order: InputMon, ReplayGain, EQ, Comp, Matrix, OutputMon
        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins[2].id, eq_id);
        assert_eq!(plugins[3].id, comp_id);

        // Move comp up (swap with EQ)
        g.move_user_plugin_up(comp_id).unwrap();

        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins[2].id, comp_id);
        assert_eq!(plugins[3].id, eq_id);
    }

    #[test]
    fn test_non_linear_graph() {
        let mut g = PluginGraph::new();
        let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
        let a = g.add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0));
        let b = g.add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0));
        let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 50.0), 2);

        // Input splits to both A and B (non-linear)
        g.add_connection(input, 0, a, 0).unwrap();
        g.add_connection(input, 1, b, 0).unwrap();
        g.add_connection(a, 0, output, 0).unwrap();
        g.add_connection(b, 0, output, 1).unwrap();

        assert!(!g.is_linear());
        assert!(g.linear_order().is_none());
    }

    #[test]
    fn test_toggle_plugin() {
        let mut g = PluginGraph::with_default_rack();
        let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

        assert!(g.nodes.get(&eq_id).unwrap().plugin.enabled);
        g.toggle_plugin(eq_id).unwrap();
        assert!(!g.nodes.get(&eq_id).unwrap().plugin.enabled);
    }

    #[test]
    fn test_to_plugin_configs_ordering() {
        let mut g = PluginGraph::with_default_rack();
        let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
        let _comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

        let configs = g.to_plugin_configs(48000.0);

        // Input monitor first, then processing (ReplayGain disabled so skipped,
        // EQ, Compressor, Matrix), then output monitor
        // ReplayGain is disabled → not in configs
        // Order: InputMonitor, EQ, Compressor, Matrix, OutputMonitor
        let types: Vec<&str> = configs.iter().map(|c| c.plugin_type.as_str()).collect();

        // First should be loudness_monitor (input)
        assert_eq!(types[0], "loudness_monitor");
        // Processing plugins in order
        assert!(types.contains(&"eq"));
        assert!(types.contains(&"compressor"));
        assert!(types.contains(&"matrix"));
        // Last should be loudness_monitor (output, analyzer)
        assert_eq!(types[types.len() - 1], "loudness_monitor");

        // EQ should come before compressor
        let eq_pos = types.iter().position(|&t| t == "eq").unwrap();
        let comp_pos = types.iter().position(|&t| t == "compressor").unwrap();
        assert!(eq_pos < comp_pos);
    }

    #[test]
    fn test_get_engine_index() {
        let mut g = PluginGraph::with_default_rack();
        let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

        // Input monitor at engine index 0
        let im_id = g.node_id_for_role(NodeRole::InputMonitor).unwrap();
        assert_eq!(g.get_engine_index(im_id), Some(0));

        // EQ is the first processing plugin → index 1
        assert_eq!(g.get_engine_index(eq_id), Some(1));

        // Disabled plugin returns None
        g.toggle_plugin(eq_id).unwrap();
        assert_eq!(g.get_engine_index(eq_id), None);
    }

    #[test]
    fn test_get_engine_index_by_linear_position() {
        let mut g = PluginGraph::with_default_rack();
        let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

        // Linear position 0 = InputMonitor → engine index 0
        assert_eq!(g.get_engine_index_by_linear_position(0), Some(0));

        // Linear position 1 = ReplayGain (disabled) → None
        assert_eq!(g.get_engine_index_by_linear_position(1), None);

        // Linear position 2 = EQ → engine index 1
        assert_eq!(g.get_engine_index_by_linear_position(2), Some(1));
    }

    #[test]
    fn test_role_engine_index_helpers() {
        let g = PluginGraph::with_default_rack();

        assert_eq!(g.input_monitor_engine_index(), Some(0));
        assert!(g.output_monitor_engine_index().is_some());
        assert!(g.matrix_engine_index().is_some());

        // Output monitor is an analyzer → comes after processing plugins
        let matrix_idx = g.matrix_engine_index().unwrap();
        let output_idx = g.output_monitor_engine_index().unwrap();
        assert!(output_idx > matrix_idx);
    }

    #[test]
    fn test_move_plugin_drag_down() {
        // Regression: move_plugin must track by node_id, not re-lookup by index
        let mut g = PluginGraph::with_default_rack();
        let a_id = g.add_user_plugin(&PluginType::EQ).unwrap();
        let b_id = g.add_user_plugin(&PluginType::Compressor).unwrap();
        let c_id = g.add_user_plugin(&PluginType::Gain).unwrap();

        // Indices: ..., EQ(2), Comp(3), Gain(4), Matrix(5), ...
        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins[2].id, a_id);
        assert_eq!(plugins[3].id, b_id);
        assert_eq!(plugins[4].id, c_id);

        // Move EQ from index 2 to index 4
        g.move_plugin(2, 4);

        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins[2].id, b_id);
        assert_eq!(plugins[3].id, c_id);
        assert_eq!(plugins[4].id, a_id);
    }

    #[test]
    fn test_move_plugin_drag_up() {
        let mut g = PluginGraph::with_default_rack();
        let a_id = g.add_user_plugin(&PluginType::EQ).unwrap();
        let b_id = g.add_user_plugin(&PluginType::Compressor).unwrap();
        let c_id = g.add_user_plugin(&PluginType::Gain).unwrap();

        // Move Gain from index 4 to index 2
        g.move_plugin(4, 2);

        let plugins = g.plugins_linear().unwrap();
        assert_eq!(plugins[2].id, c_id);
        assert_eq!(plugins[3].id, a_id);
        assert_eq!(plugins[4].id, b_id);
    }

    #[test]
    fn test_user_spectrum_analyzer_stays_in_position() {
        // Regression: user-placed analyzers should not be moved to end
        let mut g = PluginGraph::with_default_rack();
        g.add_user_plugin(&PluginType::SpectrumAnalyzer).unwrap();
        g.add_user_plugin(&PluginType::Compressor).unwrap();

        let configs = g.to_plugin_configs(48000.0);
        let types: Vec<&str> = configs.iter().map(|c| c.plugin_type.as_str()).collect();

        // Spectrum should come before compressor (user ordering preserved)
        let spec_pos = types
            .iter()
            .position(|&t| t == "spectrum_analyzer")
            .unwrap();
        let comp_pos = types.iter().position(|&t| t == "compressor").unwrap();
        assert!(
            spec_pos < comp_pos,
            "spectrum_analyzer should come before compressor but got {:?}",
            types
        );
    }

    #[test]
    fn test_channel_states_resized_on_update() {
        let mut g = PluginGraph::with_default_rack();
        g.add_user_plugin(&PluginType::Upmixer).unwrap();

        // After update, matrix should have 6 channel states (5.1)
        g.update_channel_dependent_plugins();

        let matrix = g.node_for_role(NodeRole::Matrix).unwrap();
        if let PluginSettings::Matrix {
            channel_states,
            input_channels,
            ..
        } = &matrix.plugin.settings
        {
            assert_eq!(*input_channels, 6);
            assert_eq!(
                channel_states.len(),
                6,
                "channel_states should be resized to match input_channels"
            );
        } else {
            panic!("Expected Matrix settings");
        }
    }

    #[test]
    fn test_ascii_diagram_linear() {
        let g = PluginGraph::with_default_rack();
        let lines = g.to_ascii_diagram();
        assert!(!lines.is_empty());
        let joined = lines.join(" ");
        assert!(
            joined.contains("→"),
            "Linear diagram should contain arrows: {}",
            joined
        );
        assert!(joined.contains("Audio Input"), "Should show Input node");
        assert!(joined.contains("Audio Output"), "Should show Output node");
    }

    #[test]
    fn test_ascii_diagram_non_linear() {
        let mut g = PluginGraph::new();
        let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
        let a = g.add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0));
        let b = g.add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0));
        let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 50.0), 2);
        g.add_connection(input, 0, a, 0).unwrap();
        g.add_connection(input, 1, b, 0).unwrap();
        g.add_connection(a, 0, output, 0).unwrap();
        g.add_connection(b, 0, output, 1).unwrap();

        let lines = g.to_ascii_diagram();
        assert!(!lines.is_empty());
        let joined = lines.join("\n");
        assert!(
            joined.contains("non-linear"),
            "Non-linear diagram should indicate non-linearity"
        );
    }

    // =========================================================================
    // Save-to-rack tests
    // =========================================================================

    #[test]
    fn test_find_plugin_index_no_eq() {
        let g = PluginGraph::with_default_rack();
        assert!(g.find_plugin_index(&PluginType::EQ).is_none());
    }

    #[test]
    fn test_insert_eq_and_configure_per_channel() {
        let mut g = PluginGraph::with_default_rack();
        let insert_idx = g.user_plugin_insert_index();
        let node_id = g.insert_plugin(insert_idx, &PluginType::EQ).unwrap();
        assert!(!node_id.is_nil());

        // Configure with per-channel settings
        let ch0_filters = vec![EQFilter::new(BiquadFilterType::Peak, 100.0, 1.5, -3.0)];
        let ch1_filters = vec![EQFilter::new(BiquadFilterType::Peak, 200.0, 2.0, -5.0)];
        let eq_plugin = g.get_plugin_mut(insert_idx).unwrap();
        eq_plugin.settings = PluginSettings::EQ {
            channels: 2,
            filters: ch0_filters.clone(),
            channel_filters: Some(vec![ch0_filters.clone(), ch1_filters.clone()]),
            per_channel_mode: true,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };

        // Verify the graph is still linear
        assert!(g.is_linear());

        // Verify settings are preserved
        let eq = g.get_plugin(insert_idx).unwrap();
        if let PluginSettings::EQ { channels, per_channel_mode, channel_filters, .. } = &eq.settings {
            assert_eq!(*channels, 2);
            assert!(*per_channel_mode);
            let cf = channel_filters.as_ref().unwrap();
            assert_eq!(cf.len(), 2);
            assert_eq!(cf[0][0].frequency, 100.0);
            assert_eq!(cf[1][0].frequency, 200.0);
        } else {
            panic!("Expected EQ settings");
        }

        // Verify EQ is before Matrix
        let eq_idx = g.find_plugin_index(&PluginType::EQ).unwrap();
        let matrix_idx = g.find_plugin_index(&PluginType::Matrix).unwrap();
        assert!(eq_idx < matrix_idx, "EQ should be before Matrix");
    }

    #[test]
    fn test_update_existing_eq_preserves_position() {
        let mut g = PluginGraph::with_default_rack();
        // Add EQ then Compressor
        let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
        let _comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

        let eq_idx = g.find_plugin_index(&PluginType::EQ).unwrap();
        let comp_idx = g.find_plugin_index(&PluginType::Compressor).unwrap();
        assert!(eq_idx < comp_idx);

        // Update EQ with new per-channel settings
        let new_filters = vec![EQFilter::new(BiquadFilterType::Peak, 500.0, 3.0, -8.0)];
        let eq_plugin = g.get_plugin_mut(eq_idx).unwrap();
        eq_plugin.settings = PluginSettings::EQ {
            channels: 2,
            filters: new_filters.clone(),
            channel_filters: Some(vec![new_filters.clone(), new_filters.clone()]),
            per_channel_mode: true,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };

        // Verify position unchanged
        let eq_idx_after = g.find_plugin_index(&PluginType::EQ).unwrap();
        let comp_idx_after = g.find_plugin_index(&PluginType::Compressor).unwrap();
        assert_eq!(eq_idx, eq_idx_after, "EQ position should not change");
        assert_eq!(comp_idx, comp_idx_after, "Compressor position should not change");

        // Verify new settings
        let eq = g.get_plugin(eq_idx).unwrap();
        if let PluginSettings::EQ { filters, per_channel_mode, .. } = &eq.settings {
            assert!(*per_channel_mode);
            assert_eq!(filters[0].frequency, 500.0);
        } else {
            panic!("Expected EQ settings");
        }
    }

    #[test]
    fn test_to_plugin_configs_per_channel_eq() {
        let mut g = PluginGraph::with_default_rack();
        let insert_idx = g.user_plugin_insert_index();
        g.insert_plugin(insert_idx, &PluginType::EQ).unwrap();

        let ch0 = vec![EQFilter::new(BiquadFilterType::Peak, 100.0, 1.5, -3.0)];
        let ch1 = vec![EQFilter::new(BiquadFilterType::Peak, 200.0, 2.0, -5.0)];
        let eq = g.get_plugin_mut(insert_idx).unwrap();
        eq.settings = PluginSettings::EQ {
            channels: 2,
            filters: ch0.clone(),
            channel_filters: Some(vec![ch0, ch1]),
            per_channel_mode: true,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };

        let configs = g.to_plugin_configs(48000.0);
        let eq_config = configs.iter().find(|c| c.plugin_type == "eq").expect("EQ config should be present");
        let params = &eq_config.parameters;

        // per_channel_mode should produce "channel_filters" key
        assert!(params.get("channel_filters").is_some(), "Should have channel_filters key");
        let ch_filters = params["channel_filters"].as_array().unwrap();
        assert_eq!(ch_filters.len(), 2);

        // Verify different frequencies per channel
        let ch0_freq = ch_filters[0][0]["freq"].as_f64().unwrap();
        let ch1_freq = ch_filters[1][0]["freq"].as_f64().unwrap();
        assert!((ch0_freq - 100.0).abs() < 0.1);
        assert!((ch1_freq - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_to_plugin_configs_global_eq() {
        let mut g = PluginGraph::with_default_rack();
        let insert_idx = g.user_plugin_insert_index();
        g.insert_plugin(insert_idx, &PluginType::EQ).unwrap();

        let filters = vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.4, 3.0)];
        let eq = g.get_plugin_mut(insert_idx).unwrap();
        eq.settings = PluginSettings::EQ {
            channels: 2,
            filters,
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };

        let configs = g.to_plugin_configs(48000.0);
        let eq_config = configs.iter().find(|c| c.plugin_type == "eq").expect("EQ config");
        let params = &eq_config.parameters;

        // Global mode should produce "filters" key, NOT "channel_filters"
        assert!(params.get("filters").is_some(), "Should have filters key");
        assert!(params.get("channel_filters").is_none(), "Should NOT have channel_filters key");
    }

    #[test]
    fn test_disabled_eq_excluded_from_configs() {
        let mut g = PluginGraph::with_default_rack();
        let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
        g.toggle_plugin(eq_id).unwrap(); // disable it

        let configs = g.to_plugin_configs(48000.0);
        assert!(
            configs.iter().all(|c| c.plugin_type != "eq"),
            "Disabled EQ should not appear in configs"
        );
    }
}
