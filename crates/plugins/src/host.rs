// ============================================================================
// Plugin Host Trait - Common interface for plugin hosts
// ============================================================================

use super::plugin::{Plugin, ProcessContext};
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

// ============================================================================
// Node Buffer - Simple non-thread-safe buffer for audio data (zero-allocation)
// ============================================================================

/// Simple audio buffer without synchronization overhead.
/// Used within `process()` which takes `&mut self` (single-threaded).
struct NodeBuffer {
    /// Audio data (interleaved), pre-allocated to max expected size
    data: Vec<f32>,
    /// Actual number of samples written (may be less than buffer capacity)
    actual_len: usize,
    /// Number of channels
    num_channels: usize,
}

impl NodeBuffer {
    fn new(num_frames: usize, num_channels: usize) -> Self {
        Self {
            data: vec![0.0; num_frames * num_channels],
            actual_len: 0,
            num_channels,
        }
    }

    fn write(&mut self, data: &[f32]) {
        if self.data.len() < data.len() {
            self.data.resize(data.len(), 0.0);
        }
        self.data[..data.len()].copy_from_slice(data);
        self.actual_len = data.len();
    }

    fn read(&self) -> &[f32] {
        if self.actual_len == 0 {
            &self.data
        } else {
            &self.data[..self.actual_len]
        }
    }

    fn clear(&mut self) {
        self.actual_len = 0;
    }

    /// Ensure the buffer can hold at least `num_frames * num_channels` samples.
    fn ensure_capacity(&mut self, num_frames: usize) {
        let required = num_frames * self.num_channels;
        if self.data.len() < required {
            self.data.resize(required, 0.0);
        }
    }
}

/// Pre-allocated processing buffers, extracted from DawHost to avoid borrow conflicts.
/// Taken out of DawHost via `Option::take()` during `process()`, put back after.
struct ProcessBuffers {
    /// Pre-allocated buffer for each node (reused across frames)
    node_buffers: HashMap<NodeId, NodeBuffer>,
    /// Pre-allocated scratch input buffer
    scratch_input: Vec<f32>,
    /// Pre-allocated scratch output buffer
    scratch_output: Vec<f32>,
    /// Pre-allocated merge buffer
    merge_buffer: Vec<f32>,
    /// Pre-allocated channel map buffer
    channel_map_buffer: Vec<f32>,
}

/// Common trait for plugin hosts
///
/// Plugin hosts manage a collection of audio plugins and route audio through them.
/// Different implementations provide different topologies:
/// - DawHost: Thread-safe DAG with parallel processing (DAW-style)
///
/// # Example
/// ```
/// use sotf_plugins::Host;
/// use sotf_plugins::{PluginHost, GainPlugin, InPlacePluginAdapter};
///
/// let mut host = PluginHost::new(2, 44100); // Start with 2 channels
/// let gain = GainPlugin::new(2, -6.0);
/// host.add_plugin(Box::new(InPlacePluginAdapter::new(gain))).unwrap();
///
/// // Process audio
/// let input = vec![1.0; 8]; // 4 frames, 2 channels
/// let mut output = vec![0.0; 8];
/// host.process(&input, &mut output).unwrap();
/// ```
pub trait Host {
    /// Add a plugin to the host
    ///
    /// Returns an error if the plugin's input channels don't match
    /// the current output channels.
    fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String>;

    /// Remove a plugin at the given index
    ///
    /// Returns the removed plugin, or an error if the index is out of bounds.
    fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String>;

    /// Get the number of plugins in the host
    fn plugin_count(&self) -> usize;

    /// Get plugin at index (immutable)
    fn get_plugin(&self, index: usize) -> Option<&dyn Plugin>;

    /// Get input channel count
    fn input_channels(&self) -> usize;

    /// Get output channel count (after all plugins)
    fn output_channels(&self) -> usize;

    /// Get data from a plugin at the given index
    fn get_plugin_data(&self, _index: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }

    /// Set a parameter on a plugin at the given index
    ///
    /// Returns an error if the index is out of bounds or the plugin doesn't support
    /// the parameter ID.
    fn set_plugin_parameter(
        &mut self,
        index: usize,
        param_id: &str,
        value: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let _ = (index, param_id, value);
        Err("set_plugin_parameter not implemented for this host".to_string())
    }

    /// Process audio through the plugin chain/graph
    ///
    /// # Arguments
    /// * `input` - Interleaved input samples (length = num_frames * input_channels)
    /// * `output` - Interleaved output samples (length = num_frames * output_channels)
    ///
    /// # Returns
    /// Number of frames processed, or error message
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String>;

    /// Reset all plugins in the host
    fn reset(&mut self);

    /// Get total latency in samples
    fn total_latency_samples(&self) -> usize;
}

// ============================================================================
// Graph Node - Represents a plugin in the graph (thread-safe)
// ============================================================================

/// Unique identifier for a graph node
pub type NodeId = usize;

/// A node in the plugin graph (thread-safe)
pub struct GraphNode {
    /// Node identifier
    #[allow(dead_code)]
    pub id: NodeId,
    /// The plugin instance (wrapped in Mutex for thread safety)
    pub plugin: Arc<Mutex<Box<dyn Plugin>>>,
    /// Node name (for debugging)
    pub name: String,
    /// Cached input/output channel counts
    input_channels: usize,
    output_channels: usize,
}

impl GraphNode {
    pub fn new(id: NodeId, name: String, plugin: Box<dyn Plugin>) -> Self {
        let input_channels = plugin.input_channels();
        let output_channels = plugin.output_channels();

        Self {
            id,
            plugin: Arc::new(Mutex::new(plugin)),
            name,
            input_channels,
            output_channels,
        }
    }

    pub fn input_channels(&self) -> usize {
        self.input_channels
    }

    pub fn output_channels(&self) -> usize {
        self.output_channels
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

// (AudioBuffer removed - replaced by NodeBuffer above)

// ============================================================================
// DAW Host - Main graph structure
// ============================================================================

/// DAW-style host for parallel audio processing
///
/// Supports two modes:
/// 1. **Chain Mode**: Compatible with PluginHost API - plugins are added linearly
/// 2. **Graph Mode**: Full graph API with nodes and edges
pub struct DawHost {
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
    /// Enable parallel processing (DEPRECATED: scratch buffer reuse always uses sequential processing)
    parallel_enabled: bool,
    /// Linear chain mode: ordered list of node IDs (for PluginHost compatibility)
    chain_nodes: Vec<NodeId>,
    /// Initial input channel count
    initial_input_channels: usize,
    /// Graph has been built
    built: bool,
    /// Pre-allocated processing buffers (taken during process(), put back after)
    process_buffers: Option<ProcessBuffers>,
}

impl DawHost {
    /// Create a new empty plugin graph (defaults to stereo)
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz
    pub fn new_default(sample_rate: u32) -> Self {
        Self::new(2, sample_rate)
    }

    /// Create a new plugin graph with specified input channels
    ///
    /// This is the primary constructor that's compatible with PluginHost API.
    ///
    /// # Arguments
    /// * `channels` - Initial number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            stages: Vec::new(),
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            sample_rate,
            next_node_id: 0,
            parallel_enabled: true,
            chain_nodes: Vec::new(),
            initial_input_channels: channels,
            built: false,
            process_buffers: None,
        }
    }

    /// Enable or disable parallel processing
    pub fn set_parallel_enabled(&mut self, enabled: bool) {
        self.parallel_enabled = enabled;
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

        let from_channels = from_node.output_channels();
        let to_channels = to_node.input_channels();

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
    pub fn build(&mut self) -> Result<(), String> {
        // Detect cycles
        if self.has_cycle() {
            return Err("Graph contains a cycle".to_string());
        }

        // Identify input and output nodes
        self.compute_io_nodes();

        // Compute processing stages
        self.compute_stages()?;

        log::info!(
            "[PluginGraph] Built graph with {} nodes, {} stages",
            self.nodes.len(),
            self.stages.len()
        );

        for (i, stage) in self.stages.iter().enumerate() {
            let node_names: Vec<&str> = stage
                .nodes
                .iter()
                .filter_map(|id| self.nodes.get(id).map(|n| n.name.as_str()))
                .collect();
            log::info!(
                "[PluginGraph] Stage {}: {} nodes: {:?}",
                i,
                stage.nodes.len(),
                node_names
            );
        }

        // Pre-allocate processing buffers based on graph topology
        let max_channels = self
            .nodes
            .values()
            .map(|n| n.output_channels().max(n.input_channels()))
            .max()
            .unwrap_or(2);
        // Use a generous default frame count; will grow if needed
        let default_buffer_frames = 2048;
        let node_buffers: HashMap<NodeId, NodeBuffer> = self
            .nodes
            .iter()
            .map(|(&id, node)| {
                let buf = NodeBuffer::new(default_buffer_frames, node.output_channels());
                (id, buf)
            })
            .collect();
        let scratch_size = default_buffer_frames * max_channels;
        self.process_buffers = Some(ProcessBuffers {
            node_buffers,
            scratch_input: vec![0.0; scratch_size],
            scratch_output: vec![0.0; scratch_size],
            merge_buffer: vec![0.0; scratch_size],
            channel_map_buffer: vec![0.0; scratch_size],
        });

        self.built = true;
        Ok(())
    }

    // ========================================================================
    // PluginHost-Compatible API (Linear Chain Mode)
    // ========================================================================

    /// Add a plugin to the end of the chain (PluginHost-compatible API)
    ///
    /// This automatically creates nodes and edges to form a linear chain.
    /// The first plugin receives input from the graph input, and the last plugin
    /// produces the graph output.
    ///
    /// # Arguments
    /// * `plugin` - The plugin to add to the chain
    ///
    /// # Returns
    /// Ok(()) on success, or an error if the plugin's channels don't match
    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        // Get expected input channels
        let expected_input = if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            // Get output channels of last plugin in chain
            let last_node_id = *self.chain_nodes.last().unwrap();
            self.nodes[&last_node_id].output_channels()
        };

        // Verify channel compatibility
        if plugin.input_channels() != expected_input {
            return Err(format!(
                "Plugin expects {} input channels, but chain provides {}",
                plugin.input_channels(),
                expected_input
            ));
        }

        // Create node
        let plugin_name = format!("plugin_{}", self.next_node_id);
        let node_id = self.add_node(plugin_name, plugin)?;

        // Connect to previous node if this isn't the first plugin
        if let Some(&prev_node_id) = self.chain_nodes.last() {
            self.add_edge(GraphEdge::new(prev_node_id, node_id))?;
        }

        // Add to chain
        self.chain_nodes.push(node_id);

        // Mark as not built (needs rebuild, buffers will be re-created)
        self.built = false;
        self.process_buffers = None;

        Ok(())
    }

    /// Remove a plugin at the given index (PluginHost-compatible API)
    ///
    /// Note: This is only supported in chain mode. If you've manually added
    /// nodes and edges, use the graph API instead.
    ///
    /// # Arguments
    /// * `index` - Index of the plugin to remove (0-based)
    ///
    /// # Returns
    /// The removed plugin, or an error if the index is out of bounds
    pub fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String> {
        if index >= self.chain_nodes.len() {
            return Err(format!("Plugin index {} out of bounds", index));
        }

        let node_id = self.chain_nodes.remove(index);

        // Remove edges connected to this node
        self.edges
            .retain(|e| e.from_node != node_id && e.to_node != node_id);

        // Reconnect the chain if needed
        if index > 0 && index < self.chain_nodes.len() {
            let prev_id = self.chain_nodes[index - 1];
            let next_id = self.chain_nodes[index];
            self.add_edge(GraphEdge::new(prev_id, next_id))?;
        }

        // Remove the node
        let node = self
            .nodes
            .remove(&node_id)
            .ok_or_else(|| format!("Node {} not found", node_id))?;

        // Extract the plugin from the Arc<Mutex<>>
        let plugin = Arc::try_unwrap(node.plugin)
            .map_err(|_| "Cannot remove plugin: still in use")?
            .into_inner()
            .unwrap();

        // Mark as not built (buffers will be re-created)
        self.built = false;
        self.process_buffers = None;

        Ok(plugin)
    }

    /// Get the number of plugins in the chain (PluginHost-compatible API)
    pub fn plugin_count(&self) -> usize {
        self.chain_nodes.len()
    }

    /// Get plugin at index (immutable) (PluginHost-compatible API)
    ///
    /// Returns None if the index is out of bounds or if not in chain mode.
    pub fn get_plugin(&self, index: usize) -> Option<&dyn Plugin> {
        let _node_id = self.chain_nodes.get(index)?;
        // We can't easily return a reference to the plugin inside Arc<Mutex<>>
        // This is a limitation of the graph architecture
        // For now, return None - users should use the graph API for introspection
        None
    }

    /// Get input channel count (PluginHost-compatible API)
    pub fn input_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            // Default to 2 channels for HAL mode when initial_input_channels is 0
            if self.initial_input_channels == 0 { 2 } else { self.initial_input_channels }
        } else {
            // First plugin's input channels
            let first_node_id = self.chain_nodes[0];
            self.nodes[&first_node_id].input_channels()
        }
    }

    /// Get output channel count (PluginHost-compatible API)
    pub fn output_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            // Default to 2 channels for HAL mode when initial_input_channels is 0
            if self.initial_input_channels == 0 { 2 } else { self.initial_input_channels }
        } else {
            // Last plugin's output channels
            let last_node_id = *self.chain_nodes.last().unwrap();
            self.nodes[&last_node_id].output_channels()
        }
    }

    /// Calculate the number of output frames for a given number of input frames.
    /// Accounts for plugins that change frame count (like resamplers).
    pub fn output_frames_for_input(&self, input_frames: usize) -> usize {
        let mut frames = input_frames;
        for node_id in &self.chain_nodes {
            if let Some(node) = self.nodes.get(node_id) {
                let plugin = node.plugin.lock().unwrap();
                frames = plugin.output_frames_for_input(frames);
            }
        }
        frames
    }

    /// Get the output sample rate after all plugins.
    /// Returns input rate if no rate-changing plugins exist.
    pub fn output_sample_rate(&self, input_rate: u32) -> u32 {
        let mut rate = input_rate;
        for node_id in &self.chain_nodes {
            if let Some(node) = self.nodes.get(node_id) {
                let plugin = node.plugin.lock().unwrap();
                rate = plugin.output_sample_rate(rate);
            }
        }
        rate
    }

    /// Get the actual output frames from the last process() call.
    /// This queries the last plugin in the chain that implements last_output_frames().
    /// Returns None if no plugins track output frames or chain is empty.
    pub fn last_output_frames(&self) -> Option<usize> {
        // Iterate through chain in reverse to find a plugin that tracks output frames
        for node_id in self.chain_nodes.iter().rev() {
            if let Some(node) = self.nodes.get(node_id) {
                let plugin = node.plugin.lock().unwrap();
                if let Some(frames) = plugin.last_output_frames() {
                    return Some(frames);
                }
            }
        }
        None
    }

    /// Set a parameter on a plugin at the given index (PluginHost-compatible API)
    pub fn set_plugin_parameter(
        &mut self,
        index: usize,
        param_id: &str,
        value: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let node_id = self
            .chain_nodes
            .get(index)
            .ok_or_else(|| format!("Plugin index {} out of bounds", index))?;

        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| format!("Node {} not found", node_id))?;

        let mut plugin = node
            .plugin
            .lock()
            .map_err(|e| format!("Failed to lock plugin at index {}: {}", index, e))?;

        plugin.set_parameter(super::parameters::ParameterId(param_id.to_string()), value)
    }

    // ========================================================================
    // Processing
    // ========================================================================

    /// Process audio through the graph
    ///
    /// If the graph hasn't been built yet, it will be built automatically.
    /// This provides PluginHost-like behavior where you don't need to manually call build().
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        // Auto-build if needed (PluginHost compatibility)
        if !self.built {
            self.build()?;
        }

        // Handle empty graph (pass-through)
        if self.nodes.is_empty() {
            if input.len() != output.len() {
                return Err(format!(
                    "Input/output size mismatch: {} vs {}",
                    input.len(),
                    output.len()
                ));
            }
            output.copy_from_slice(input);
            // Avoid division by zero when initial_input_channels is 0 (HAL mode)
            // In this case, assume the input contains complete frames
            let channels = if self.initial_input_channels == 0 { 2 } else { self.initial_input_channels };
            return Ok(input.len() / channels);
        }

        if self.input_nodes.is_empty() {
            return Err("Graph has no input nodes".to_string());
        }
        if self.output_nodes.is_empty() {
            return Err("Graph has no output nodes".to_string());
        }

        // Determine number of frames
        let first_input_node = &self.nodes[&self.input_nodes[0]];
        let input_channels = first_input_node.input_channels();
        let num_frames = input.len() / input_channels;

        // Calculate max output frames (accounts for resamplers)
        let max_output_frames = self.output_frames_for_input(num_frames);

        // Use the larger of input or output frames for buffer allocation
        let buffer_frames = num_frames.max(max_output_frames);

        // Take pre-allocated buffers (avoids borrow conflicts with &self for graph topology)
        let mut bufs = self.process_buffers.take().unwrap_or_else(|| {
            // Fallback: create fresh buffers if somehow missing
            let max_ch = self.nodes.values().map(|n| n.output_channels().max(n.input_channels())).max().unwrap_or(2);
            let node_buffers = self.nodes.iter().map(|(&id, node)| {
                (id, NodeBuffer::new(buffer_frames, node.output_channels()))
            }).collect();
            let sz = buffer_frames * max_ch;
            ProcessBuffers {
                node_buffers,
                scratch_input: vec![0.0; sz],
                scratch_output: vec![0.0; sz],
                merge_buffer: vec![0.0; sz],
                channel_map_buffer: vec![0.0; sz],
            }
        });

        // Ensure node buffers are large enough and cleared for this frame
        for (&id, node) in &self.nodes {
            let nb = bufs.node_buffers.entry(id).or_insert_with(|| {
                NodeBuffer::new(buffer_frames, node.output_channels())
            });
            nb.ensure_capacity(buffer_frames);
            nb.clear();
        }

        // Track current frame count through the chain (may change after resamplers)
        let mut current_frames = num_frames;

        // Process each stage (sequential for scratch buffer reuse)
        for stage_idx in 0..self.stages.len() {
            let stage_nodes: Vec<NodeId> = self.stages[stage_idx].nodes.clone();
            for &node_id in &stage_nodes {
                let context = ProcessContext {
                    sample_rate: self.sample_rate,
                    num_frames: current_frames,
                };

                Self::process_node_with_buffers(
                    &self.nodes,
                    &self.input_nodes,
                    &self.edges,
                    node_id,
                    input,
                    &mut bufs,
                    &context,
                    &mut current_frames,
                )?;
            }
        }

        // Collect output from output nodes
        Self::collect_output_from_buffers(
            &self.output_nodes,
            &bufs.node_buffers,
            output,
            current_frames,
        )?;

        // Return actual output frames if available (e.g., from resampler),
        // otherwise return input frames
        let actual_output_frames = self.last_output_frames().unwrap_or(num_frames);

        // Put buffers back for reuse next frame
        self.process_buffers = Some(bufs);

        Ok(actual_output_frames)
    }

    /// Process a single node using pre-allocated buffers (static method to avoid borrow conflicts).
    fn process_node_with_buffers(
        nodes: &HashMap<NodeId, GraphNode>,
        input_nodes: &[NodeId],
        edges: &[GraphEdge],
        node_id: NodeId,
        graph_input: &[f32],
        bufs: &mut ProcessBuffers,
        context: &ProcessContext,
        current_frames: &mut usize,
    ) -> Result<(), String> {
        let node = nodes
            .get(&node_id)
            .ok_or_else(|| format!("Node {} not found", node_id))?;

        // Determine input data
        let input_len = if input_nodes.contains(&node_id) {
            // Input node - copy graph input into scratch_input for uniform handling
            let len = graph_input.len();
            if bufs.scratch_input.len() < len {
                bufs.scratch_input.resize(len, 0.0);
            }
            bufs.scratch_input[..len].copy_from_slice(graph_input);
            len
        } else {
            // Internal node - merge inputs from predecessors into merge_buffer,
            // then copy to scratch_input
            let merged_len = Self::merge_inputs_into(
                nodes,
                edges,
                node_id,
                &bufs.node_buffers,
                context.num_frames,
                &mut bufs.merge_buffer,
                &mut bufs.channel_map_buffer,
            )?;
            if bufs.scratch_input.len() < merged_len {
                bufs.scratch_input.resize(merged_len, 0.0);
            }
            bufs.scratch_input[..merged_len].copy_from_slice(&bufs.merge_buffer[..merged_len]);
            merged_len
        };

        // Query the plugin for max output frames (accounts for resamplers)
        let max_output_frames = {
            let plugin = node
                .plugin
                .lock()
                .map_err(|e| format!("Plugin '{}' lock poisoned: {}", node.name, e))?;
            plugin.output_frames_for_input(context.num_frames)
        };

        // Prepare output scratch buffer
        let output_channels = node.output_channels();
        let output_size = max_output_frames * output_channels;
        if bufs.scratch_output.len() < output_size {
            bufs.scratch_output.resize(output_size, 0.0);
        }
        bufs.scratch_output[..output_size].fill(0.0);

        // Process (lock the plugin for processing)
        let actual_output_frames = {
            let mut plugin = node
                .plugin
                .lock()
                .map_err(|e| format!("Plugin '{}' lock poisoned: {}", node.name, e))?;
            plugin.process(
                &bufs.scratch_input[..input_len],
                &mut bufs.scratch_output[..output_size],
                context,
            )?
        };

        // Write actual output to node buffer
        let actual_output_size = actual_output_frames * output_channels;
        if let Some(nb) = bufs.node_buffers.get_mut(&node_id) {
            nb.write(&bufs.scratch_output[..actual_output_size]);
        }

        // Update frame count for subsequent plugins
        *current_frames = actual_output_frames;

        Ok(())
    }

    /// Merge inputs from predecessor nodes into `merge_buffer` (zero-allocation).
    /// Returns the number of samples written into `merge_buffer`.
    fn merge_inputs_into(
        nodes: &HashMap<NodeId, GraphNode>,
        edges: &[GraphEdge],
        node_id: NodeId,
        node_buffers: &HashMap<NodeId, NodeBuffer>,
        num_frames: usize,
        merge_buffer: &mut Vec<f32>,
        channel_map_buffer: &mut Vec<f32>,
    ) -> Result<usize, String> {
        let node = &nodes[&node_id];
        let input_channels = node.input_channels();
        let input_size = num_frames * input_channels;

        // Ensure merge buffer is large enough and zeroed
        if merge_buffer.len() < input_size {
            merge_buffer.resize(input_size, 0.0);
        }
        merge_buffer[..input_size].fill(0.0);

        // Find and mix all incoming edges
        let mut found_input = false;
        for edge in edges.iter().filter(|e| e.to_node == node_id) {
            found_input = true;
            let src_buffer = &node_buffers[&edge.from_node];

            if let Some(ref channel_map) = edge.channel_map {
                // Apply channel mapping into channel_map_buffer
                let src_data = src_buffer.read();
                let src_channels = src_buffer.num_channels;
                let dst_channels = channel_map.len();
                let mapped_size = num_frames * dst_channels;
                if channel_map_buffer.len() < mapped_size {
                    channel_map_buffer.resize(mapped_size, 0.0);
                }

                for frame in 0..num_frames {
                    for (dst_ch, &src_ch) in channel_map.iter().enumerate() {
                        let src_idx = frame * src_channels + src_ch;
                        let dst_idx = frame * dst_channels + dst_ch;
                        channel_map_buffer[dst_idx] = src_data[src_idx];
                    }
                }

                // Mix into merge buffer
                for (dst, &src) in merge_buffer[..input_size]
                    .iter_mut()
                    .zip(channel_map_buffer[..mapped_size].iter())
                {
                    *dst += src;
                }
            } else {
                // Direct mix from node buffer (no clone - reads a slice)
                let src_data = src_buffer.read();
                for (dst, &src) in merge_buffer[..input_size]
                    .iter_mut()
                    .zip(src_data.iter())
                {
                    *dst += src;
                }
            }
        }

        if !found_input {
            return Err(format!("Node {} has no inputs", node_id));
        }

        Ok(input_size)
    }

    /// Collect output from output nodes (static method, no borrow conflicts).
    fn collect_output_from_buffers(
        output_nodes: &[NodeId],
        node_buffers: &HashMap<NodeId, NodeBuffer>,
        output: &mut [f32],
        _num_frames: usize,
    ) -> Result<(), String> {
        if output_nodes.len() == 1 {
            // Single output - direct copy (slice, no allocation)
            let node_id = output_nodes[0];
            let buffer = &node_buffers[&node_id];
            let data = buffer.read();
            let copy_len = data.len().min(output.len());
            output[..copy_len].copy_from_slice(&data[..copy_len]);
        } else {
            // Multiple outputs - mix them
            output.fill(0.0);
            for &node_id in output_nodes {
                let buffer = &node_buffers[&node_id];
                let data = buffer.read();
                let mix_len = data.len().min(output.len());
                for (dst, &src) in output[..mix_len].iter_mut().zip(data[..mix_len].iter()) {
                    *dst += src;
                }
            }
        }

        Ok(())
    }

    /// Reset all plugins in the graph
    pub fn reset(&self) {
        for node in self.nodes.values() {
            let mut plugin = node.plugin.lock().unwrap();
            plugin.reset();
        }
    }

    /// Get total latency (maximum path latency through the graph)
    pub fn total_latency_samples(&self) -> usize {
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
        let plugin = node.plugin.lock().unwrap();
        let node_latency = plugin.latency_samples();

        // Find all incoming edges
        let incoming: Vec<NodeId> = self
            .edges
            .iter()
            .filter(|e| e.to_node == node_id)
            .map(|e| e.from_node)
            .collect();

        if incoming.is_empty() {
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

        for edge in &self.edges {
            if edge.from_node == node_id && self.has_cycle_util(edge.to_node, visited, rec_stack) {
                return true;
            }
        }

        rec_stack.remove(&node_id);
        false
    }

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

    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

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

// ============================================================================
// Host Trait Implementation
// ============================================================================

impl Host for DawHost {
    fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        DawHost::add_plugin(self, plugin)
    }

    fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String> {
        DawHost::remove_plugin(self, index)
    }

    fn plugin_count(&self) -> usize {
        DawHost::plugin_count(self)
    }

    fn get_plugin(&self, index: usize) -> Option<&dyn Plugin> {
        DawHost::get_plugin(self, index)
    }

    fn get_plugin_data(&self, index: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        let node_id = self.chain_nodes.get(index)?;
        let node = self.nodes.get(node_id)?;
        let plugin = node.plugin.lock().unwrap();
        plugin.get_data()
    }

    fn set_plugin_parameter(
        &mut self,
        index: usize,
        param_id: &str,
        value: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        DawHost::set_plugin_parameter(self, index, param_id, value)
    }

    fn input_channels(&self) -> usize {
        DawHost::input_channels(self)
    }

    fn output_channels(&self) -> usize {
        DawHost::output_channels(self)
    }

    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        DawHost::process(self, input, output)
    }

    fn reset(&mut self) {
        DawHost::reset(self)
    }

    fn total_latency_samples(&self) -> usize {
        DawHost::total_latency_samples(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GainPlugin, InPlacePluginAdapter};

    #[test]
    fn test_linear_chain() {
        // Test that a linear chain works correctly
        let mut graph = DawHost::new(2, 48000);

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
            assert!((sample - 0.25_f32).abs() < 0.01);
        }
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DawHost::new_default(48000);

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
    fn test_parallel_diamond() {
        let mut graph = DawHost::new_default(48000);

        // Create a diamond pattern:
        //     -> gain2 ->
        // gain1          gain4
        //     -> gain3 ->
        //
        // gain2 and gain3 can run in parallel

        let node1 = graph
            .add_node(
                "gain1".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -3.0))),
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

        // Process audio
        let input = vec![1.0; 96]; // 48 frames, 2 channels
        let mut output = vec![0.0; 96];

        graph.process(&input, &mut output).unwrap();

        // gain1 applies -3dB (0.707x)
        // Signals go through gain2 and gain3 (both 0dB)
        // Then merge at gain4 (0dB) - this doubles the signal
        // Expected: 0.707 * 2 = 1.414
        for &sample in &output {
            assert!(
                (sample - 1.414).abs() < 0.05,
                "Expected ~1.414, got {}",
                sample
            );
        }
    }

    #[test]
    fn test_stream_merge() {
        // Test that stream merging works correctly at join points
        let mut graph = DawHost::new_default(48000);

        // Two inputs merge into one output
        //  input1 (gain1 -6dB) \
        //                        > merge (gain3 0dB)
        //  input2 (gain2 -6dB) /
        //
        // Note: We need a single input, so we'll split it

        let node1 = graph
            .add_node(
                "split".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();
        let node2 = graph
            .add_node(
                "gain1".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -6.0))),
            )
            .unwrap();
        let node3 = graph
            .add_node(
                "gain2".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -6.0))),
            )
            .unwrap();
        let node4 = graph
            .add_node(
                "merge".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();

        // Split: node1 -> node2 and node1 -> node3
        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.add_edge(GraphEdge::new(node1, node3)).unwrap();

        // Merge: node2 -> node4 and node3 -> node4
        graph.add_edge(GraphEdge::new(node2, node4)).unwrap();
        graph.add_edge(GraphEdge::new(node3, node4)).unwrap();

        graph.build().unwrap();

        // Process audio
        let input = vec![1.0; 96]; // 48 frames, 2 channels
        let mut output = vec![0.0; 96];

        graph.process(&input, &mut output).unwrap();

        // Input: 1.0
        // After split (0dB): 1.0
        // Both branches apply -6dB: 0.5
        // Merge adds them: 0.5 + 0.5 = 1.0
        // Output through merge (0dB): 1.0
        for &sample in &output {
            assert!(
                (sample - 1.0_f32).abs() < 0.01,
                "Expected ~1.0, got {}",
                sample
            );
        }
    }

    #[test]
    fn test_parallel_processing_enabled() {
        let mut graph = DawHost::new_default(48000);
        graph.set_parallel_enabled(true);

        // Create multiple parallel paths
        let node1 = graph
            .add_node(
                "input".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();

        let mut parallel_nodes = Vec::new();
        for i in 0..4 {
            let node = graph
                .add_node(
                    format!("parallel_{}", i),
                    Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
                )
                .unwrap();
            graph.add_edge(GraphEdge::new(node1, node)).unwrap();
            parallel_nodes.push(node);
        }

        let output_node = graph
            .add_node(
                "output".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();

        for &node in &parallel_nodes {
            graph.add_edge(GraphEdge::new(node, output_node)).unwrap();
        }

        graph.build().unwrap();

        // Verify we have a parallel stage
        assert_eq!(graph.num_stages(), 3);
        let parallel_stage = graph.stage_info(1).unwrap();
        assert_eq!(parallel_stage.len(), 4);

        // Process
        let input = vec![1.0; 96];
        let mut output = vec![0.0; 96];
        graph.process(&input, &mut output).unwrap();

        // 4 parallel paths all at 0dB merge together
        for &sample in &output {
            assert!((sample - 4.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_parallel_processing_disabled() {
        let mut graph = DawHost::new_default(48000);
        graph.set_parallel_enabled(false); // Disable parallel processing

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

        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.add_edge(GraphEdge::new(node1, node3)).unwrap();

        graph.build().unwrap();

        let input = vec![1.0; 96];
        let mut output = vec![0.0; 96];

        // Should still work, just sequentially
        graph.process(&input, &mut output).unwrap();

        // Both paths merge: 1.0 + 1.0 = 2.0
        for &sample in &output {
            assert!((sample - 2.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_latency_calculation() {
        let mut graph = DawHost::new_default(48000);

        // Create a chain with different latencies
        // In real plugins, latency would vary, but GainPlugin has 0 latency
        // This test just verifies the calculation works
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

        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.build().unwrap();

        let latency = graph.total_latency_samples();
        assert_eq!(latency, 0); // GainPlugin has 0 latency
    }

    #[test]
    fn test_reset() {
        let mut graph = DawHost::new_default(48000);

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

        graph.add_edge(GraphEdge::new(node1, node2)).unwrap();
        graph.build().unwrap();

        // Reset should not panic
        graph.reset();
    }

    // ========================================================================
    // PluginHost-Compatible API Tests
    // ========================================================================

    #[test]
    fn test_pluginhost_api_linear_chain() {
        // Test that the PluginHost-compatible API works
        let mut graph = DawHost::new(2, 48000);

        // Add plugins like you would with PluginHost
        let gain1 = GainPlugin::new(2, -6.0);
        let gain2 = GainPlugin::new(2, -6.0);

        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(gain1)))
            .unwrap();
        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(gain2)))
            .unwrap();

        assert_eq!(graph.plugin_count(), 2);
        assert_eq!(graph.input_channels(), 2);
        assert_eq!(graph.output_channels(), 2);

        // Process (auto-builds)
        let input = vec![1.0; 96];
        let mut output = vec![0.0; 96];

        graph.process(&input, &mut output).unwrap();

        // -6dB twice = -12dB ≈ 0.25x
        for &sample in &output {
            assert!((sample - 0.25_f32).abs() < 0.01);
        }
    }

    #[test]
    fn test_pluginhost_api_remove_plugin() {
        let mut graph = DawHost::new(2, 48000);

        // Add three plugins
        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(
                2, -6.0,
            ))))
            .unwrap();
        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(
                2, -6.0,
            ))))
            .unwrap();
        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(
                2, -6.0,
            ))))
            .unwrap();

        assert_eq!(graph.plugin_count(), 3);

        // Remove middle plugin
        let _removed = graph.remove_plugin(1).unwrap();
        assert_eq!(graph.plugin_count(), 2);

        // Process should still work
        let input = vec![1.0; 96];
        let mut output = vec![0.0; 96];
        graph.process(&input, &mut output).unwrap();

        // Two -6dB plugins = -12dB ≈ 0.25x
        for &sample in &output {
            assert!((sample - 0.25_f32).abs() < 0.01);
        }
    }

    #[test]
    fn test_pluginhost_api_empty_graph() {
        let mut graph = DawHost::new(2, 48000);

        assert_eq!(graph.plugin_count(), 0);
        assert_eq!(graph.input_channels(), 2);
        assert_eq!(graph.output_channels(), 2);

        // Processing an empty graph should pass through (PluginHost compatibility)
        let input = vec![1.0; 96];
        let mut output = vec![0.0; 96];
        assert!(graph.process(&input, &mut output).is_ok());
        assert_eq!(output, input);
    }

    #[test]
    fn test_pluginhost_api_channel_mismatch() {
        let mut graph = DawHost::new(2, 48000);

        // Add a 2-channel plugin
        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))))
            .unwrap();

        // Try to add a 5-channel plugin - should fail
        let result = graph.add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(5, 0.0))));
        assert!(result.is_err());
    }

    #[test]
    fn test_mixed_api_usage() {
        // Test that you can mix PluginHost API with graph API
        let mut graph = DawHost::new(2, 48000);

        // Add a plugin using PluginHost API
        graph
            .add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(
                2, -3.0,
            ))))
            .unwrap();

        // Now add some nodes using graph API for a parallel split
        let last_chain_node = *graph.chain_nodes.last().unwrap();

        let branch_a = graph
            .add_node(
                "branch_a".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();
        let branch_b = graph
            .add_node(
                "branch_b".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();
        let merge = graph
            .add_node(
                "merge".to_string(),
                Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))),
            )
            .unwrap();

        // Connect: last chain node -> branch_a -> merge
        //          last chain node -> branch_b -> merge
        graph
            .add_edge(GraphEdge::new(last_chain_node, branch_a))
            .unwrap();
        graph
            .add_edge(GraphEdge::new(last_chain_node, branch_b))
            .unwrap();
        graph.add_edge(GraphEdge::new(branch_a, merge)).unwrap();
        graph.add_edge(GraphEdge::new(branch_b, merge)).unwrap();

        graph.build().unwrap();

        // Process
        let input = vec![1.0; 96];
        let mut output = vec![0.0; 96];
        graph.process(&input, &mut output).unwrap();

        // First plugin: -3dB = 0.707x
        // Split into two branches (0dB each)
        // Merge: 0.707 + 0.707 = 1.414x
        for &sample in &output {
            assert!((sample - 1.414_f32).abs() < 0.05);
        }
    }
}
