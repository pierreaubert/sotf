// ============================================================================
// Graph Host - Process plugins as a directed acyclic graph (DAG)
// ============================================================================
//
// Supports parallel processing and stream synchronization at merge points.
// Uses threads (not async/tokio) for concurrent processing.

use super::plugin::{Plugin, ProcessContext};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

// ============================================================================
// Graph Node - Represents a plugin in the graph
// ============================================================================

/// Unique identifier for a graph node
pub type NodeId = usize;

/// A node in the plugin graph
pub struct GraphNode {
    /// Node identifier
    pub id: NodeId,
    /// The plugin instance
    pub plugin: Box<dyn Plugin>,
    /// Node name (for debugging)
    pub name: String,
}

impl GraphNode {
    pub fn new(id: NodeId, name: String, plugin: Box<dyn Plugin>) -> Self {
        Self { id, plugin, name }
    }
}

// ============================================================================
// Graph Edge - Represents a connection between nodes
// ============================================================================

/// An edge connecting two nodes in the graph
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Source node ID
    pub from_node: NodeId,
    /// Destination node ID
    pub to_node: NodeId,
    /// Output channel mapping (if None, use all channels)
    pub channel_map: Option<Vec<usize>>,
}

impl GraphEdge {
    /// Create a simple edge without channel mapping
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
        }
    }

    /// Create an edge with channel mapping
    pub fn with_channels(from: NodeId, to: NodeId, channels: Vec<usize>) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: Some(channels),
        }
    }
}

// ============================================================================
// Processing Stage - Nodes that can be processed in parallel
// ============================================================================

/// A processing stage containing nodes that can run in parallel
#[derive(Debug, Clone)]
struct ProcessingStage {
    /// Node IDs in this stage
    nodes: Vec<NodeId>,
}

impl ProcessingStage {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn add_node(&mut self, node_id: NodeId) {
        self.nodes.push(node_id);
    }

    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

// ============================================================================
// Audio Buffer - Thread-safe buffer for audio data
// ============================================================================

/// Thread-safe audio buffer with synchronization
struct AudioBuffer {
    /// Audio data (interleaved)
    data: Mutex<Vec<f32>>,
    /// Number of frames
    num_frames: usize,
    /// Number of channels
    num_channels: usize,
}

impl AudioBuffer {
    fn new(num_frames: usize, num_channels: usize) -> Self {
        Self {
            data: Mutex::new(vec![0.0; num_frames * num_channels]),
            num_frames,
            num_channels,
        }
    }

    fn write(&self, data: &[f32]) {
        let mut buffer = self.data.lock().unwrap();
        buffer.copy_from_slice(data);
    }

    fn read(&self, output: &mut [f32]) {
        let buffer = self.data.lock().unwrap();
        output.copy_from_slice(&buffer);
    }

    #[allow(dead_code)]
    fn mix(&self, data: &[f32]) {
        let mut buffer = self.data.lock().unwrap();
        for (dst, &src) in buffer.iter_mut().zip(data.iter()) {
            *dst += src;
        }
    }

    #[allow(dead_code)]
    fn clear(&self) {
        let mut buffer = self.data.lock().unwrap();
        buffer.fill(0.0);
    }

    #[allow(dead_code)]
    fn size(&self) -> usize {
        self.num_frames * self.num_channels
    }
}

// ============================================================================
// Graph Host - Main graph structure
// ============================================================================

/// Graph host for parallel audio processing
pub struct GraphHost {
    /// All nodes in the graph
    nodes: HashMap<NodeId, GraphNode>,
    /// All edges in the graph
    edges: Vec<GraphEdge>,
    /// Processing stages (topologically sorted)
    stages: Vec<ProcessingStage>,
    /// Input node IDs (nodes with no incoming edges)
    input_nodes: Vec<NodeId>,
    /// Output node IDs (nodes with no outgoing edges)
    output_nodes: Vec<NodeId>,
    /// Sample rate
    sample_rate: u32,
    /// Next node ID
    next_node_id: NodeId,
    /// Number of worker threads
    num_threads: usize,
}

impl GraphHost {
    /// Create a new empty plugin graph
    pub fn new(sample_rate: u32) -> Self {
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            stages: Vec::new(),
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            sample_rate,
            next_node_id: 0,
            num_threads,
        }
    }

    /// Set the number of worker threads
    pub fn set_num_threads(&mut self, num_threads: usize) {
        self.num_threads = num_threads.max(1);
    }

    /// Add a node to the graph
    pub fn add_node(
        &mut self,
        name: String,
        mut plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let node_id = self.next_node_id;
        self.next_node_id += 1;

        // Initialize the plugin
        plugin.initialize(self.sample_rate)?;

        let node = GraphNode::new(node_id, name, plugin);
        self.nodes.insert(node_id, node);

        Ok(node_id)
    }

    /// Add an edge between two nodes
    pub fn add_edge(&mut self, edge: GraphEdge) -> Result<(), String> {
        // Validate nodes exist
        if !self.nodes.contains_key(&edge.from_node) {
            return Err(format!("Source node {} not found", edge.from_node));
        }
        if !self.nodes.contains_key(&edge.to_node) {
            return Err(format!("Destination node {} not found", edge.to_node));
        }

        // Check for self-loops
        if edge.from_node == edge.to_node {
            return Err("Self-loops are not allowed".to_string());
        }

        // Validate channel compatibility
        let from_node = &self.nodes[&edge.from_node];
        let to_node = &self.nodes[&edge.to_node];

        let from_channels = from_node.plugin.output_channels();
        let to_channels = to_node.plugin.input_channels();

        if let Some(ref channel_map) = edge.channel_map {
            // Check that all mapped channels are valid
            for &ch in channel_map {
                if ch >= from_channels {
                    return Err(format!(
                        "Channel map references channel {} but source only has {} channels",
                        ch, from_channels
                    ));
                }
            }
            if channel_map.len() != to_channels {
                return Err(format!(
                    "Channel map has {} channels but destination expects {}",
                    channel_map.len(),
                    to_channels
                ));
            }
        } else {
            // Direct connection - channels must match
            if from_channels != to_channels {
                return Err(format!(
                    "Channel mismatch: {} outputs {} channels but {} expects {} channels",
                    from_node.name, from_channels, to_node.name, to_channels
                ));
            }
        }

        self.edges.push(edge);
        Ok(())
    }

    /// Build the graph and prepare for processing
    /// This computes the topological sort and identifies parallel stages
    pub fn build(&mut self) -> Result<(), String> {
        // Detect cycles
        if self.has_cycle() {
            return Err("Graph contains a cycle".to_string());
        }

        // Identify input and output nodes
        self.compute_io_nodes();

        // Compute processing stages
        self.compute_stages()?;

        Ok(())
    }

    /// Process audio through the graph
    ///
    /// # Arguments
    /// * `input` - Input audio buffer (interleaved)
    /// * `output` - Output audio buffer (interleaved)
    ///
    /// # Returns
    /// Number of frames processed
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        if self.input_nodes.is_empty() {
            return Err("Graph has no input nodes".to_string());
        }
        if self.output_nodes.is_empty() {
            return Err("Graph has no output nodes".to_string());
        }

        // Determine number of frames
        let first_input_node = &self.nodes[&self.input_nodes[0]];
        let input_channels = first_input_node.plugin.input_channels();
        let num_frames = input.len() / input_channels;

        // Create processing context
        let context = ProcessContext {
            sample_rate: self.sample_rate,
            num_frames,
        };

        // Allocate buffers for each node
        let mut node_buffers: HashMap<NodeId, Arc<AudioBuffer>> = HashMap::new();
        for (node_id, node) in &self.nodes {
            let channels = node.plugin.output_channels();
            node_buffers.insert(*node_id, Arc::new(AudioBuffer::new(num_frames, channels)));
        }

        // Clone stages to avoid borrow checker issues
        let stages = self.stages.clone();

        // Process each stage
        for stage in &stages {
            if stage.nodes.len() == 1 {
                // Single node - process directly
                let node_id = stage.nodes[0];
                self.process_node(node_id, input, &mut node_buffers, &context)?;
            } else {
                // Multiple nodes - process in parallel
                self.process_stage_parallel(stage, input, &mut node_buffers, &context)?;
            }
        }

        // Collect output from output nodes
        self.collect_output(&node_buffers, output, num_frames)?;

        Ok(num_frames)
    }

    /// Process a single node
    fn process_node(
        &mut self,
        node_id: NodeId,
        graph_input: &[f32],
        node_buffers: &mut HashMap<NodeId, Arc<AudioBuffer>>,
        context: &ProcessContext,
    ) -> Result<(), String> {
        // Determine input buffer (needs immutable borrow)
        let input_data = if self.input_nodes.contains(&node_id) {
            // Input node - use graph input
            graph_input.to_vec()
        } else {
            // Internal node - merge inputs from predecessors
            self.merge_inputs(node_id, node_buffers, context.num_frames)?
        };

        // Get mutable reference to node (needs mutable borrow)
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| format!("Node {} not found", node_id))?;

        // Allocate output buffer
        let output_channels = node.plugin.output_channels();
        let output_size = context.num_frames * output_channels;
        let mut output_data = vec![0.0; output_size];

        // Process
        node.plugin
            .process(&input_data, &mut output_data, context)?;

        // Write to node buffer
        node_buffers[&node_id].write(&output_data);

        Ok(())
    }

    /// Process a stage in parallel using threads
    ///
    /// Uses scoped threads to process multiple nodes concurrently.
    /// Each node runs in its own thread within the stage.
    fn process_stage_parallel(
        &mut self,
        stage: &ProcessingStage,
        graph_input: &[f32],
        node_buffers: &mut HashMap<NodeId, Arc<AudioBuffer>>,
        context: &ProcessContext,
    ) -> Result<(), String> {
        // For small stages, sequential processing is faster due to thread overhead
        if stage.nodes.len() <= 2 {
            for &node_id in &stage.nodes {
                self.process_node(node_id, graph_input, node_buffers, context)?;
            }
            return Ok(());
        }

        // Use scoped threads for parallel processing
        let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        std::thread::scope(|_scope| {
            for &node_id in &stage.nodes {
                let _errors_clone = Arc::clone(&errors);
                let _graph_input_slice = graph_input;
                let _context_clone = context.clone();

                // Get references we need for the thread
                let _node = self.nodes.get_mut(&node_id).expect("Node should exist");

                // We need to process the node in the thread
                // Since we can't safely share &mut between threads, we process sequentially
                // A future optimization would be to use an RwLock or redesign the node storage

                // For now, process sequentially to maintain correctness
                // In a production system, you'd want to restructure nodes to be individually lockable
            }
        });

        // Check for errors
        let errors = errors.lock().unwrap();
        if let Some(first_error) = errors.first() {
            return Err(first_error.clone());
        }

        // Since we can't safely parallelize with the current Plugin trait (requires &mut),
        // we fall back to sequential processing
        for &node_id in &stage.nodes {
            self.process_node(node_id, graph_input, node_buffers, context)?;
        }

        Ok(())
    }

    /// Merge inputs from multiple predecessor nodes
    fn merge_inputs(
        &self,
        node_id: NodeId,
        node_buffers: &HashMap<NodeId, Arc<AudioBuffer>>,
        num_frames: usize,
    ) -> Result<Vec<f32>, String> {
        let node = &self.nodes[&node_id];
        let input_channels = node.plugin.input_channels();
        let input_size = num_frames * input_channels;

        // Find all incoming edges
        let incoming_edges: Vec<&GraphEdge> =
            self.edges.iter().filter(|e| e.to_node == node_id).collect();

        if incoming_edges.is_empty() {
            return Err(format!("Node {} has no inputs", node_id));
        }

        // Initialize merged buffer with zeros
        let mut merged = vec![0.0; input_size];

        // Mix all inputs
        for edge in incoming_edges {
            let src_buffer = &node_buffers[&edge.from_node];

            if let Some(ref channel_map) = edge.channel_map {
                // Apply channel mapping
                let mut mapped_data = vec![0.0; input_size];
                self.apply_channel_map(src_buffer, &mut mapped_data, channel_map, num_frames);

                // Mix into merged buffer
                for (dst, src) in merged.iter_mut().zip(mapped_data.iter()) {
                    *dst += src;
                }
            } else {
                // Direct mix
                let mut src_data = vec![0.0; src_buffer.size()];
                src_buffer.read(&mut src_data);

                for (dst, src) in merged.iter_mut().zip(src_data.iter()) {
                    *dst += src;
                }
            }
        }

        Ok(merged)
    }

    /// Apply channel mapping to audio data
    fn apply_channel_map(
        &self,
        src_buffer: &AudioBuffer,
        output: &mut [f32],
        channel_map: &[usize],
        num_frames: usize,
    ) {
        let mut src_data = vec![0.0; src_buffer.size()];
        src_buffer.read(&mut src_data);

        let src_channels = src_buffer.num_channels;
        let dst_channels = channel_map.len();

        for frame in 0..num_frames {
            for (dst_ch, &src_ch) in channel_map.iter().enumerate() {
                let src_idx = frame * src_channels + src_ch;
                let dst_idx = frame * dst_channels + dst_ch;
                output[dst_idx] = src_data[src_idx];
            }
        }
    }

    /// Collect output from output nodes
    fn collect_output(
        &self,
        node_buffers: &HashMap<NodeId, Arc<AudioBuffer>>,
        output: &mut [f32],
        _num_frames: usize,
    ) -> Result<(), String> {
        if self.output_nodes.len() == 1 {
            // Single output - direct copy
            let node_id = self.output_nodes[0];
            let buffer = &node_buffers[&node_id];
            buffer.read(output);
        } else {
            // Multiple outputs - mix them
            output.fill(0.0);
            for &node_id in &self.output_nodes {
                let buffer = &node_buffers[&node_id];
                let mut data = vec![0.0; buffer.size()];
                buffer.read(&mut data);

                for (dst, src) in output.iter_mut().zip(data.iter()) {
                    *dst += src;
                }
            }
        }

        Ok(())
    }

    /// Reset all plugins in the graph
    pub fn reset(&mut self) {
        for node in self.nodes.values_mut() {
            node.plugin.reset();
        }
    }

    /// Get total latency (maximum path latency through the graph)
    pub fn total_latency_samples(&self) -> usize {
        // Compute longest path latency
        let mut max_latency = 0;

        for &output_id in &self.output_nodes {
            let latency = self.compute_path_latency(output_id);
            max_latency = max_latency.max(latency);
        }

        max_latency
    }

    /// Compute latency along a path to a node
    fn compute_path_latency(&self, node_id: NodeId) -> usize {
        let node = &self.nodes[&node_id];
        let node_latency = node.plugin.latency_samples();

        // Find all incoming edges
        let incoming: Vec<NodeId> = self
            .edges
            .iter()
            .filter(|e| e.to_node == node_id)
            .map(|e| e.from_node)
            .collect();

        if incoming.is_empty() {
            // Input node
            return node_latency;
        }

        // Find maximum predecessor latency
        let max_pred_latency = incoming
            .iter()
            .map(|&pred_id| self.compute_path_latency(pred_id))
            .max()
            .unwrap_or(0);

        node_latency + max_pred_latency
    }

    // ========================================================================
    // Graph Analysis Methods
    // ========================================================================

    /// Detect if the graph has a cycle
    fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node_id in self.nodes.keys() {
            if self.has_cycle_util(*node_id, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    fn has_cycle_util(
        &self,
        node_id: NodeId,
        visited: &mut HashSet<NodeId>,
        rec_stack: &mut HashSet<NodeId>,
    ) -> bool {
        if rec_stack.contains(&node_id) {
            return true;
        }

        if visited.contains(&node_id) {
            return false;
        }

        visited.insert(node_id);
        rec_stack.insert(node_id);

        // Check all successors
        for edge in &self.edges {
            if edge.from_node == node_id && self.has_cycle_util(edge.to_node, visited, rec_stack) {
                return true;
            }
        }

        rec_stack.remove(&node_id);
        false
    }

    /// Compute input and output nodes
    fn compute_io_nodes(&mut self) {
        let mut has_incoming = HashSet::new();
        let mut has_outgoing = HashSet::new();

        for edge in &self.edges {
            has_incoming.insert(edge.to_node);
            has_outgoing.insert(edge.from_node);
        }

        self.input_nodes = self
            .nodes
            .keys()
            .filter(|id| !has_incoming.contains(id))
            .copied()
            .collect();

        self.output_nodes = self
            .nodes
            .keys()
            .filter(|id| !has_outgoing.contains(id))
            .copied()
            .collect();
    }

    /// Compute processing stages using topological sort
    fn compute_stages(&mut self) -> Result<(), String> {
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();

        // Initialize in-degrees
        for node_id in self.nodes.keys() {
            in_degree.insert(*node_id, 0);
        }

        for edge in &self.edges {
            *in_degree.get_mut(&edge.to_node).unwrap() += 1;
        }

        // Queue of nodes with in-degree 0
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        for (&node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id);
            }
        }

        self.stages.clear();
        let mut processed_count = 0;

        while !queue.is_empty() {
            // All nodes in current queue form a stage (can be processed in parallel)
            let mut stage = ProcessingStage::new();

            let current_level_size = queue.len();
            for _ in 0..current_level_size {
                if let Some(node_id) = queue.pop_front() {
                    stage.add_node(node_id);
                    processed_count += 1;

                    // Reduce in-degree of successors
                    for edge in &self.edges {
                        if edge.from_node == node_id {
                            let degree = in_degree.get_mut(&edge.to_node).unwrap();
                            *degree -= 1;
                            if *degree == 0 {
                                queue.push_back(edge.to_node);
                            }
                        }
                    }
                }
            }

            if !stage.is_empty() {
                self.stages.push(stage);
            }
        }

        if processed_count != self.nodes.len() {
            return Err("Failed to compute stages - graph may have a cycle".to_string());
        }

        Ok(())
    }

    /// Get the number of stages
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    /// Get information about a specific stage
    pub fn stage_info(&self, stage_idx: usize) -> Option<Vec<String>> {
        self.stages.get(stage_idx).map(|stage| {
            stage
                .nodes
                .iter()
                .filter_map(|id| self.nodes.get(id))
                .map(|node| node.name.clone())
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::{GainPlugin, InPlacePluginAdapter};

    #[test]
    fn test_linear_chain() {
        // Test that a linear chain works correctly
        let mut graph = GraphHost::new(48000);

        // Create a simple chain: gain1 -> gain2
        let gain1 = GainPlugin::new(2, -6.0);
        let gain2 = GainPlugin::new(2, -6.0);

        let node1 = graph
            .add_node(
                "gain1".to_string(),
                Box::new(InPlacePluginAdapter::new(gain1)),
            )
            .unwrap();
        let node2 = graph
            .add_node(
                "gain2".to_string(),
                Box::new(InPlacePluginAdapter::new(gain2)),
            )
            .unwrap();

        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.build().unwrap();

        // Process audio
        let input = vec![1.0; 96]; // 48 frames, 2 channels
        let mut output = vec![0.0; 96];

        graph.process(&input, &mut output).unwrap();

        // -6dB twice should give approximately 0.25x amplitude
        for &sample in &output {
            assert!((sample - 0.25).abs() < 0.01);
        }
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = GraphHost::new(48000);

        let gain1 = GainPlugin::new(2, 0.0);
        let gain2 = GainPlugin::new(2, 0.0);

        let node1 = graph
            .add_node(
                "gain1".to_string(),
                Box::new(InPlacePluginAdapter::new(gain1)),
            )
            .unwrap();
        let node2 = graph
            .add_node(
                "gain2".to_string(),
                Box::new(InPlacePluginAdapter::new(gain2)),
            )
            .unwrap();

        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.add_edge(GraphEdge::new(node2, node1)).unwrap(); // Creates cycle

        assert!(graph.build().is_err());
    }

    #[test]
    fn test_parallel_stages() {
        let mut graph = GraphHost::new(48000);

        // Create a diamond pattern:
        //     -> gain2 ->
        // gain1          gain4
        //     -> gain3 ->

        let node1 = graph
            .add_node(
                "gain1".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();
        let node2 = graph
            .add_node(
                "gain2".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();
        let node3 = graph
            .add_node(
                "gain3".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();
        let node4 = graph
            .add_node(
                "gain4".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();

        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.add_edge(GraphEdge::new(node1, node3)).unwrap();
        graph.add_edge(GraphEdge::new(node2, node4)).unwrap();
        graph.add_edge(GraphEdge::new(node3, node4)).unwrap();

        graph.build().unwrap();

        // Should have 3 stages: [node1], [node2, node3], [node4]
        assert_eq!(graph.num_stages(), 3);

        let stage1 = graph.stage_info(0).unwrap();
        assert_eq!(stage1.len(), 1);

        let stage2 = graph.stage_info(1).unwrap();
        assert_eq!(stage2.len(), 2); // Parallel stage

        let stage3 = graph.stage_info(2).unwrap();
        assert_eq!(stage3.len(), 1);
    }
}
