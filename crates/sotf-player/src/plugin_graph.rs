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
    ChannelConflict, Plugin, PluginSettings, PluginType, UpmixerOutputSettings, resize_matrix,
    upmixer_output_channels,
};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

mod graph_connection;
mod graph_selection;
mod misc;
mod node_position;
mod plugin_graph_node;
mod special_node;
#[cfg(test)]
mod tests;
mod types;

pub use graph_connection::*;
pub use graph_selection::*;
pub use node_position::*;
pub use plugin_graph_node::*;
pub use special_node::*;
pub use types::*;

use misc::upmixer_settings_output_channels;

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
    /// Hidden rack auto-gain stage, injected just before output analyzers.
    #[serde(skip)]
    chain_auto_gain_db: Option<f64>,
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
            chain_auto_gain_db: None,
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
        self.update_channel_dependent_plugins();

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
        self.update_channel_dependent_plugins();

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
        self.update_channel_dependent_plugins();

        Ok(())
    }

    /// Toggle a plugin's enabled state.
    pub fn toggle_plugin(&mut self, node_id: GraphNodeId) -> Result<(), String> {
        let node = self.nodes.get_mut(&node_id).ok_or("Node not found")?;
        node.plugin.enabled = !node.plugin.enabled;
        self.update_channel_dependent_plugins();
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
        self.update_channel_dependent_plugins();
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
        self.update_channel_dependent_plugins();
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
                PluginSettings::Upmixer {
                    speaker_config,
                    output:
                        UpmixerOutputSettings {
                            binaural_preview, ..
                        },
                    ..
                } => {
                    current_channels =
                        upmixer_settings_output_channels(speaker_config, *binaural_preview);
                }
                PluginSettings::AAE { speaker_config, .. } => {
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

    /// Find the linear index of a plugin by its custom `name` field.
    ///
    /// Used by the room-EQ export flow to upsert distinct EQ instances
    /// ("Room EQ", "Broadband EQ") without accidentally clobbering each
    /// other — a name-agnostic lookup by `PluginType::EQ` would always
    /// find the first match and overwrite it.
    pub fn find_plugin_index_by_name(&self, name: &str) -> Option<usize> {
        self.plugins_linear()?
            .iter()
            .position(|n| n.plugin.name.as_deref() == Some(name))
    }

    /// Set the custom `name` on the plugin at the given linear index.
    pub fn set_plugin_name_by_index(&mut self, index: usize, name: Option<String>) {
        if let Some(plugin) = self.get_plugin_mut(index) {
            plugin.name = name;
        }
    }

    /// Find the linear index of a plugin node by its `GraphNodeId`.
    pub fn linear_index_of_node(&self, node_id: GraphNodeId) -> Option<usize> {
        self.plugins_linear()?.iter().position(|n| n.id == node_id)
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
                PluginSettings::Upmixer {
                    speaker_config,
                    output:
                        UpmixerOutputSettings {
                            binaural_preview, ..
                        },
                    ..
                } => {
                    return upmixer_settings_output_channels(speaker_config, *binaural_preview);
                }
                PluginSettings::AAE { speaker_config, .. } => {
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
                PluginSettings::Upmixer {
                    speaker_config,
                    output:
                        UpmixerOutputSettings {
                            binaural_preview, ..
                        },
                    ..
                } => {
                    return Some(if *binaural_preview {
                        "2.0".to_string()
                    } else {
                        speaker_config.clone()
                    });
                }
                PluginSettings::AAE { speaker_config, .. } => {
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
                PluginSettings::Upmixer {
                    speaker_config,
                    output:
                        UpmixerOutputSettings {
                            binaural_preview, ..
                        },
                    ..
                } => {
                    config = Some(if *binaural_preview {
                        "2.0".to_string()
                    } else {
                        speaker_config.clone()
                    });
                }
                PluginSettings::AAE { speaker_config, .. } => config = Some(speaker_config.clone()),
                PluginSettings::AmbisonicsDecoder { target_layout, .. } => {
                    config = Some(target_layout.clone());
                }
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

    /// Set the hidden rack auto-gain trim. When enabled, this gain stage is
    /// inserted after processing plugins and before output analyzers so the OUT
    /// meter reflects the correction without affecting user plugin drive.
    pub fn set_chain_auto_gain(&mut self, gain_db: Option<f64>) {
        self.chain_auto_gain_db = gain_db;
    }

    /// Read the current hidden rack auto-gain trim.
    pub fn chain_auto_gain_db(&self) -> Option<f64> {
        self.chain_auto_gain_db
    }

    fn chain_auto_gain_config(&self, gain_db: f64) -> PluginConfig {
        PluginConfig::new(
            "gain",
            serde_json::json!({
                "channels": self.output_channels(),
                "gain_db": gain_db,
                "smoothing_ms": 80.0,
            }),
        )
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
                PluginSettings::Upmixer {
                    speaker_config,
                    output:
                        UpmixerOutputSettings {
                            binaural_preview, ..
                        },
                    ..
                } => {
                    running_channels =
                        upmixer_settings_output_channels(speaker_config, *binaural_preview);
                    continue;
                }
                PluginSettings::AAE { speaker_config, .. } => {
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
                    let node = self
                        .nodes
                        .get_mut(&node_id)
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

        let mut current_channels: usize = self.input_channel_count().max(1);

        for &node_id in &plugin_ids {
            let node_input_channels = current_channels;
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
                } if *channels != current_channels => {
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
                PluginSettings::Gain {
                    channels,
                    gain_db,
                    smoothing_ms,
                } if *channels != current_channels => {
                    updated_settings = Some(PluginSettings::Gain {
                        channels: current_channels,
                        gain_db: *gain_db,
                        smoothing_ms: *smoothing_ms,
                    });
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
                } if *input_channels != current_channels => {
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
                PluginSettings::Matrix {
                    input_channels,
                    output_channels,
                    matrix,
                    channel_states,
                } if *input_channels != current_channels => {
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
                } if *input_channels != current_channels => {
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
                PluginSettings::BandSplit {
                    channels,
                    frequency,
                    crossover_type,
                } if *channels != current_channels => {
                    updated_settings = Some(PluginSettings::BandSplit {
                        channels: current_channels,
                        frequency: *frequency,
                        crossover_type: crossover_type.clone(),
                    });
                }
                PluginSettings::BandMerge { channels, bands } if *channels != current_channels => {
                    updated_settings = Some(PluginSettings::BandMerge {
                        channels: current_channels,
                        bands: *bands,
                    });
                }
                _ => {}
            }

            if let Some(new_settings) = updated_settings {
                self.nodes
                    .get_mut(&node_id)
                    .expect("plugin_ids/nodes desync: node missing from map")
                    .plugin
                    .settings = new_settings;
            }

            // Track output channels for the next plugin
            let node = &self.nodes[&node_id];
            if node.plugin.enabled && !node.plugin.suspended {
                match &node.plugin.settings {
                    PluginSettings::Upmixer {
                        speaker_config,
                        output:
                            UpmixerOutputSettings {
                                binaural_preview, ..
                            },
                        ..
                    } => {
                        current_channels =
                            upmixer_settings_output_channels(speaker_config, *binaural_preview);
                    }
                    PluginSettings::AAE { speaker_config, .. } => {
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

            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.input_channels = node_input_channels;
                node.output_channels = current_channels;
            }
        }

        if let Some(output) = self.output_node_mut() {
            output.channels = current_channels.max(1);
        }

        // Channel counts on adjacent nodes have just been re-aligned. The
        // existing per-port connections were established for the *previous*
        // channel counts, so a chain that now carries 8 channels through a
        // node still only has the original 2 wires. Rewire each adjacent
        // pair so every channel that flows through the chain has its own
        // explicit port-to-port connection. This is what the user sees in
        // the graph view and what the engine's connection-driven routing
        // expects.
        self.rewire_linear_chain_per_channel();
    }

    /// Walk the linear chain and ensure every adjacent pair of nodes is
    /// fully connected port-by-port up to the smaller of the two channel
    /// counts. Drops connections whose port indices are now out of range
    /// (e.g., after a Matrix output count is shrunk) and adds connections
    /// that are missing (e.g., after a 2→8 upmixer was introduced and the
    /// downstream plugins propagated to 8 channels). Non-linear graphs are
    /// left untouched — the user is responsible for explicit wiring.
    fn rewire_linear_chain_per_channel(&mut self) {
        let Some(order) = self.linear_order() else {
            return;
        };

        for pair in order.windows(2) {
            let from = pair[0];
            let to = pair[1];
            let target_ports = self
                .node_output_channels(from)
                .min(self.node_input_channels(to));

            // Drop any existing from→to connections whose port indices are
            // now out of range so a previously-wider link stops carrying
            // signal on phantom ports.
            self.connections.retain(|c| {
                !(c.from_node == from
                    && c.to_node == to
                    && (c.from_port >= target_ports || c.to_port >= target_ports))
            });

            for port in 0..target_ports {
                let already = self.connections.iter().any(|c| {
                    c.from_node == from
                        && c.to_node == to
                        && c.from_port == port
                        && c.to_port == port
                });
                if !already {
                    let _ = self.add_connection(from, port, to, port);
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
            format!("{}{}", node.plugin.display_name(), enabled)
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
        if let Some(gain_db) = self.chain_auto_gain_db {
            result.push(self.chain_auto_gain_config(gain_db));
        }
        result.extend(analyzers);
        result
    }

    /// Serialize the plugin graph to an engine `PluginGraphConfig` that
    /// preserves topology (parallel branches, merges, multi-driver routing).
    ///
    /// Use this instead of [`to_plugin_configs`] when the graph is non-linear:
    /// flattening a routed graph into a chain via topological sort silently
    /// drops parallel paths.
    ///
    /// Special I/O nodes (Input/Output/Split/Merge from the UI canvas) are
    /// dropped — the engine handles I/O implicitly via the leaf nodes that
    /// have no incoming or outgoing edges. Disabled or suspended plugin
    /// nodes are also skipped, and edges to/from them are pruned.
    ///
    /// Per-port channel duplicates in `connections` (a stereo pair shows up
    /// as two `(port 0→0, port 1→1)` entries) collapse into a single
    /// logical edge — `PluginGraphConfig::edges` carries no port info.
    pub fn to_plugin_graph_config(
        &self,
        sample_rate: f64,
    ) -> sotf_audio::engine::PluginGraphConfig {
        use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};
        use std::collections::HashSet;

        let mut nodes = Vec::new();
        let mut id_map: HashMap<GraphNodeId, usize> = HashMap::new();
        let mut next_id: usize = 0;

        // Walk plugin nodes in topological order so engine ids match the
        // signal flow direction. Falls back to the natural HashMap order if
        // the graph has cycles (shouldn't happen, but don't drop nodes).
        let ordered_ids = self
            .topological_sort()
            .ok()
            .unwrap_or_else(|| self.nodes.keys().copied().collect());

        for graph_id in ordered_ids {
            let Some(node) = self.nodes.get(&graph_id) else {
                continue;
            };
            let Some(config) = node.plugin.to_plugin_config(sample_rate) else {
                continue;
            };
            let id = next_id;
            next_id += 1;
            id_map.insert(graph_id, id);
            nodes.push(PluginGraphNodeConfig {
                id,
                plugin_type: config.plugin_type,
                parameters: config.parameters,
                input_channels: node.input_channels,
            });
        }

        // Deduplicate per-port edges into logical edges.
        let mut seen_edges: HashSet<(usize, usize)> = HashSet::new();
        let mut edges = Vec::new();
        for conn in &self.connections {
            let (Some(&from), Some(&to)) = (id_map.get(&conn.from_node), id_map.get(&conn.to_node))
            else {
                continue; // edge touches a special node or a skipped plugin
            };
            if seen_edges.insert((from, to)) {
                edges.push(PluginGraphEdgeConfig {
                    from_node: from,
                    to_node: to,
                });
            }
        }

        PluginGraphConfig { nodes, edges }
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
            let chain_gain_slots = usize::from(self.chain_auto_gain_db.is_some());
            return Some(offset + processing_ids.len() + chain_gain_slots + pos);
        }

        None
    }

    /// Get the engine index of the hidden rack auto-gain stage.
    pub fn chain_auto_gain_engine_index(&self) -> Option<usize> {
        self.chain_auto_gain_db?;

        let ordered_ids = self
            .linear_order()
            .or_else(|| self.topological_sort().ok())
            .unwrap_or_default();

        let has_input_monitor = ordered_ids.iter().any(|id| {
            self.nodes.get(id).is_some_and(|n| {
                n.plugin.enabled && !n.plugin.suspended && n.role == NodeRole::InputMonitor
            })
        });
        let processing_count = ordered_ids
            .iter()
            .filter(|id| {
                self.nodes.get(id).is_some_and(|n| {
                    n.plugin.enabled
                        && !n.plugin.suspended
                        && !matches!(n.role, NodeRole::InputMonitor | NodeRole::OutputMonitor)
                })
            })
            .count();

        Some(usize::from(has_input_monitor) + processing_count)
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
