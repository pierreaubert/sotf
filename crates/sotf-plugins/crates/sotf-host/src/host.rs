// ============================================================================
// Plugin Host Trait - Common interface for plugin hosts
// ============================================================================

use crate::automation::{automation_utils, ParameterAutomation};
use crate::lookahead::LookaheadBuffer;
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{Plugin, ProcessContext};
#[allow(unused_imports)]
use rayon::prelude::*;
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

// ============================================================================
// Node Buffer - Simple non-thread-safe buffer for audio data (zero-allocation)
// ============================================================================

struct NodeBuffer {
    data: Vec<f32>,
    actual_len: usize,
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
            &[]
        } else {
            &self.data[..self.actual_len]
        }
    }
    fn clear(&mut self) {
        self.actual_len = 0;
    }
    fn ensure_capacity(&mut self, num_frames: usize) {
        let required = num_frames * self.num_channels;
        if self.data.len() < required {
            self.data.resize(required, 0.0);
        }
    }
}

struct ProcessBuffers {
    node_buffers: Vec<Option<NodeBuffer>>,
    scratch_input: Vec<f32>,
    scratch_output: Vec<f32>,
    merge_buffer: Vec<f32>,
    channel_map_buffer: Vec<f32>,
    /// Per-edge latency compensation delay buffers.
    /// Keyed by (from_node, to_node). Only present for edges that need compensation.
    compensation_delays: HashMap<(NodeId, NodeId), LookaheadBuffer>,
    /// Scratch buffer for frame-by-frame delay processing (avoids per-frame allocation).
    delay_scratch: Vec<f32>,
    /// Per-node scratch buffers for parallel stage processing.
    /// Each entry: (scratch_input, scratch_output, merge_buffer).
    /// Only allocated for nodes in stages with 2+ nodes.
    #[allow(dead_code)]
    parallel_scratch: Vec<(Vec<f32>, Vec<f32>, Vec<f32>)>,
}

// ============================================================================
// Buffer Safety Guard - ensures process_buffers are put back on early return
// ============================================================================

/// RAII guard that ensures `ProcessBuffers` are returned to the `DawHost`
/// even if processing exits early (via `?` or error return).
struct BufferGuard<'a> {
    slot: &'a mut Option<ProcessBuffers>,
    buffers: Option<ProcessBuffers>,
}

impl<'a> BufferGuard<'a> {
    fn take(slot: &'a mut Option<ProcessBuffers>) -> Self {
        let buffers = slot.take();
        Self { slot, buffers }
    }

    fn get_mut(&mut self) -> &mut ProcessBuffers {
        self.buffers.as_mut().expect("ProcessBuffers missing from guard")
    }
}

impl<'a> Drop for BufferGuard<'a> {
    fn drop(&mut self) {
        *self.slot = self.buffers.take();
    }
}

pub trait Host {
    fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String>;
    fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String>;
    fn plugin_count(&self) -> usize;
    fn get_plugin(&self, index: usize) -> Option<&dyn Plugin>;
    fn input_channels(&self) -> usize;
    fn output_channels(&self) -> usize;
    fn get_plugin_data(&self, _index: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
    fn set_plugin_parameter(
        &mut self,
        index: usize,
        param_id: &str,
        value: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let _ = (index, param_id, value);
        Err("set_plugin_parameter not implemented for this host".to_string())
    }
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String>;
    fn reset(&mut self);
    fn total_latency_samples(&self) -> usize;
}

pub type NodeId = usize;

#[derive(Clone)]
pub struct GraphNode {
    pub id: NodeId,
    pub name: String,
    input_channels: usize,
    output_channels: usize,
    bypassed: bool,
}

impl GraphNode {
    pub fn new(id: NodeId, name: String, input_channels: usize, output_channels: usize) -> Self {
        Self {
            id,
            name,
            input_channels,
            output_channels,
            bypassed: false,
        }
    }
    pub fn input_channels(&self) -> usize {
        self.input_channels
    }
    pub fn output_channels(&self) -> usize {
        self.output_channels
    }
}

/// Type of connection between nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeType {
    /// Normal audio connection (fills primary input channels).
    #[default]
    Audio,
    /// Sidechain connection (fills extended input channels after primary audio).
    Sidechain,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub channel_map: Option<Vec<usize>>,
    pub edge_type: EdgeType,
}

impl GraphEdge {
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
            edge_type: EdgeType::Audio,
        }
    }
    pub fn with_channels(from: NodeId, to: NodeId, channels: Vec<usize>) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: Some(channels),
            edge_type: EdgeType::Audio,
        }
    }
    pub fn sidechain(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
            edge_type: EdgeType::Sidechain,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessingStage {
    pub nodes: Vec<NodeId>,
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

/// Immutable snapshot of graph topology for lock-free graph updates.
/// The control thread builds a new `GraphTopology` and swaps it via `ArcSwap`.
/// The audio thread loads the current snapshot atomically.
#[derive(Clone)]
pub struct GraphTopology {
    pub nodes: HashMap<NodeId, GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub stages: Vec<ProcessingStage>,
    pub input_nodes: Vec<NodeId>,
    pub output_nodes: Vec<NodeId>,
    pub predecessors: Vec<Vec<GraphEdge>>,
    pub is_input_node: Vec<bool>,
    pub is_output_node: Vec<bool>,
}

impl GraphTopology {
    pub fn empty() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            stages: Vec::new(),
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            predecessors: Vec::new(),
            is_input_node: Vec::new(),
            is_output_node: Vec::new(),
        }
    }
}

pub struct DawHost {
    nodes: HashMap<NodeId, GraphNode>,
    /// Plugin storage indexed by NodeId — disjoint from `nodes` for borrow checker.
    /// `process()` can borrow `&self.nodes` (topology) and `&mut self.plugins[nid]` (plugin)
    /// without conflict.
    plugins: Vec<Option<Box<dyn Plugin>>>,
    edges: Vec<GraphEdge>,
    stages: Vec<ProcessingStage>,
    input_nodes: Vec<NodeId>,
    output_nodes: Vec<NodeId>,
    sample_rate: u32,
    next_node_id: NodeId,
    parallel_enabled: bool,
    chain_nodes: Vec<NodeId>,
    initial_input_channels: usize,
    built: bool,
    process_buffers: Option<ProcessBuffers>,
    predecessors: Vec<Vec<GraphEdge>>,
    is_input_node: Vec<bool>,
    is_output_node: Vec<bool>,
    has_variable_frame_plugin: bool,
    /// True if all plugins return input_frames unchanged from output_frames_for_input()
    cached_frames_identity: bool,
    /// True if all plugins return input_rate unchanged from output_sample_rate()
    cached_rate_identity: bool,
    /// Cached per-node output frame ratios for non-identity chains (rare)
    /// Only populated when cached_frames_identity is false
    cached_output_frame_ratios: Vec<(NodeId, f64)>,
    /// Indices of plugins in chain_nodes that have analyzer data (get_data() returns Some)
    analyzer_indices: Vec<usize>,
    /// Cached total latency in samples, computed during build() and invalidated on graph changes
    cached_latency: Option<usize>,
    /// Per-node bypass flags, indexed by NodeId. When true, the node's plugin is skipped
    /// and input is passed directly to output during processing.
    bypassed: Vec<bool>,
    /// Per-node cumulative latency from graph inputs, computed during build().
    /// Used to calculate compensation delays at merge points.
    node_latency_from_input: Vec<usize>,
    /// Per-node reported latency cached during build(), indexed by NodeId.
    /// Used to detect runtime latency changes in set_plugin_parameter().
    node_latency_cache: Vec<usize>,
    /// True when a plugin's latency_samples() has changed since last build/rebuild.
    /// Triggers compensation delay recomputation at the start of next process().
    latency_dirty: bool,
    /// Parameter automation state. Key = (NodeId, ParameterId).
    /// Evaluated before each processing stage.
    automation: HashMap<(NodeId, ParameterId), ParameterAutomation>,
    /// Current playback position in samples, advanced each process() call.
    playback_position: usize,
    /// Pre-allocated scratch buffer for automation updates (avoids per-process() heap allocation).
    automation_scratch: Vec<(NodeId, ParameterId, f32)>,
    /// Per-node parameter ramps for the current block, indexed by NodeId.
    /// Pre-allocated during build(), cleared (not deallocated) each process() call.
    node_ramps: Vec<Vec<crate::plugin::ParameterRamp>>,
    /// Maps (NodeId, ParameterId) -> param_index (position in plugin's parameters() vec).
    /// Built during build() for O(1) lookup when creating ParameterRamp structs.
    param_index_cache: HashMap<(NodeId, ParameterId), u16>,
    /// Per-node count of primary (non-sidechain) input channels, indexed by NodeId.
    /// primary_input_channels[nid] = input_channels - sidechain_channels.
    /// Computed during build() from sidechain edge predecessors.
    primary_input_channels: Vec<usize>,
}

impl DawHost {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            nodes: HashMap::new(),
            plugins: Vec::new(),
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
            predecessors: Vec::new(),
            is_input_node: Vec::new(),
            is_output_node: Vec::new(),
            has_variable_frame_plugin: false,
            cached_frames_identity: true,
            cached_rate_identity: true,
            cached_output_frame_ratios: Vec::new(),
            analyzer_indices: Vec::new(),
            cached_latency: None,
            bypassed: Vec::new(),
            node_latency_from_input: Vec::new(),
            node_latency_cache: Vec::new(),
            latency_dirty: false,
            automation: HashMap::new(),
            playback_position: 0,
            automation_scratch: Vec::new(),
            node_ramps: Vec::new(),
            param_index_cache: HashMap::new(),
            primary_input_channels: Vec::new(),
        }
    }
    pub fn new_default(sr: u32) -> Self {
        Self::new(2, sr)
    }
    #[deprecated(note = "parallel execution is not yet implemented; this flag has no effect")]
    pub fn set_parallel_enabled(&mut self, e: bool) {
        self.parallel_enabled = e;
    }

    /// Set an automation curve for a parameter on a specific node.
    /// The curve is evaluated during `process()` before each stage.
    pub fn set_automation(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        curve: crate::automation::AutomationCurve,
    ) {
        let auto = ParameterAutomation {
            param_id: param_id.clone(),
            mode: crate::automation::AutomationMode::Host,
            curve: Some(curve),
            position: 0,
            base_value: 0.0,
            last_value: 0.0,
        };
        self.automation.insert((node_id, param_id), auto);
    }

    /// Remove automation for a specific parameter on a node.
    pub fn clear_automation(&mut self, node_id: NodeId, param_id: &ParameterId) {
        self.automation.remove(&(node_id, param_id.clone()));
    }

    /// Remove all automation.
    pub fn clear_all_automation(&mut self) {
        self.automation.clear();
    }

    /// Reset playback position to 0.
    pub fn reset_playback_position(&mut self) {
        self.playback_position = 0;
        for auto in self.automation.values_mut() {
            auto.position = 0;
        }
    }

    /// Take a snapshot of the current graph topology.
    /// Can be used with `ArcSwap` for lock-free graph updates from a control thread.
    pub fn topology_snapshot(&self) -> GraphTopology {
        GraphTopology {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            stages: self.stages.clone(),
            input_nodes: self.input_nodes.clone(),
            output_nodes: self.output_nodes.clone(),
            predecessors: self.predecessors.clone(),
            is_input_node: self.is_input_node.clone(),
            is_output_node: self.is_output_node.clone(),
        }
    }

    pub fn add_node(
        &mut self,
        name: String,
        mut plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let id = self.next_node_id;
        self.next_node_id += 1;
        plugin.initialize(self.sample_rate)?;
        let input_channels = plugin.input_channels();
        let output_channels = plugin.output_channels();
        self.nodes.insert(
            id,
            GraphNode::new(id, name, input_channels, output_channels),
        );
        // Grow plugins vec to accommodate the new id
        if id >= self.plugins.len() {
            self.plugins.resize_with(id + 1, || None);
        }
        self.plugins[id] = Some(plugin);
        self.cached_latency = None;
        Ok(id)
    }

    pub fn add_edge(&mut self, edge: GraphEdge) -> Result<(), String> {
        if !self.nodes.contains_key(&edge.from_node) || !self.nodes.contains_key(&edge.to_node) {
            return Err("Node not found".into());
        }
        if edge.from_node == edge.to_node {
            return Err("Self-loop".into());
        }
        self.edges.push(edge);
        self.cached_latency = None;
        Ok(())
    }

    /// Add a sidechain edge: the output of `from` is routed as sidechain input
    /// to `to`. During processing, sidechain data is appended after the node's
    /// primary audio input channels.
    pub fn add_sidechain_edge(&mut self, from: NodeId, to: NodeId) -> Result<(), String> {
        self.add_edge(GraphEdge::sidechain(from, to))
    }

    pub fn build(&mut self) -> Result<(), String> {
        if self.has_cycle() {
            return Err("Cycle".into());
        }
        self.compute_io_nodes();
        self.compute_stages()?;
        let max_id = self.nodes.keys().copied().max().unwrap_or(0);
        let num_slots = if self.nodes.is_empty() { 0 } else { max_id + 1 };
        self.predecessors = vec![Vec::new(); num_slots];
        self.is_input_node = vec![false; num_slots];
        self.is_output_node = vec![false; num_slots];
        for edge in &self.edges {
            self.predecessors[edge.to_node].push(edge.clone());
        }
        for &id in &self.input_nodes {
            self.is_input_node[id] = true;
        }
        for &id in &self.output_nodes {
            self.is_output_node[id] = true;
        }
        let mut node_buffers = (0..num_slots).map(|_| None).collect::<Vec<_>>();
        for (&id, node) in &self.nodes {
            node_buffers[id] = Some(NodeBuffer::new(4096, node.output_channels()));
        }
        // Cache per-node bypass flags before computing compensation delays
        // (compensation needs to know which nodes are bypassed for latency calculation)
        self.bypassed = vec![false; num_slots];
        for (&id, node) in &self.nodes {
            self.bypassed[id] = node.bypassed;
        }
        // Compute per-node cumulative latency from inputs and compensation delays
        let compensation_delays = self.compute_compensation_delays(num_slots);

        // Cache per-node reported latency for runtime change detection
        self.node_latency_cache = vec![0; num_slots];
        for &nid in self.nodes.keys() {
            if let Some(p) = self.plugins.get(nid).and_then(|p| p.as_ref()) {
                self.node_latency_cache[nid] = p.latency_samples();
            }
        }
        self.latency_dirty = false;

        // Build param_index_cache and pre-allocate node_ramps for automation
        self.param_index_cache.clear();
        self.node_ramps = vec![Vec::new(); num_slots];
        for &nid in self.nodes.keys() {
            if let Some(p) = self.plugins.get(nid).and_then(|p| p.as_ref()) {
                for (idx, param) in p.parameters().iter().enumerate() {
                    self.param_index_cache
                        .insert((nid, param.id.clone()), idx as u16);
                }
            }
        }

        // Compute primary (non-sidechain) input channels per node.
        // For nodes with sidechain edges, subtract sidechain source channels.
        self.primary_input_channels = vec![0; num_slots];
        for (&id, node) in &self.nodes {
            let sc_ch: usize = self.predecessors[id]
                .iter()
                .filter(|e| e.edge_type == EdgeType::Sidechain)
                .filter_map(|e| self.nodes.get(&e.from_node))
                .map(|n| n.output_channels())
                .sum();
            self.primary_input_channels[id] = node.input_channels().saturating_sub(sc_ch);
        }

        self.process_buffers = Some(ProcessBuffers {
            node_buffers,
            scratch_input: vec![0.0; 4096 * 32],
            scratch_output: vec![0.0; 4096 * 32],
            merge_buffer: vec![0.0; 4096 * 32],
            channel_map_buffer: vec![0.0; 4096 * 32],
            compensation_delays,
            delay_scratch: vec![0.0; 4096 * 32],
            parallel_scratch: Vec::new(),
        });
        // Cache per-frame properties to avoid mutex locks during process()
        self.cached_frames_identity = true;
        self.cached_rate_identity = true;
        self.cached_output_frame_ratios.clear();
        self.analyzer_indices.clear();

        for (chain_idx, &id) in self.chain_nodes.iter().enumerate() {
            let p = self.plugins[id].as_ref().unwrap();
            if p.output_frames_for_input(100) != 100 {
                self.cached_frames_identity = false;
            }
            if p.output_sample_rate(48000) != 48000 {
                self.cached_rate_identity = false;
            }
            if p.get_data().is_some() {
                self.analyzer_indices.push(chain_idx);
            }
        }

        self.has_variable_frame_plugin = self.chain_nodes.iter().any(|&id| {
            let p = self.plugins[id].as_ref().unwrap();
            p.output_frames_for_input(100) != 100 || p.latency_samples() > 0
        });
        // Cache total latency so total_latency_samples() is O(1)
        self.cached_latency = Some(self.compute_latency());
        self.built = true;
        Ok(())
    }

    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let expected = if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[self.chain_nodes.last().unwrap()].output_channels()
        };
        if plugin.input_channels() != expected {
            return Err("mismatch".into());
        }
        let name = format!("plugin_{}", self.next_node_id);
        let id = self.add_node(name, plugin)?;
        if let Some(&prev) = self.chain_nodes.last() {
            self.add_edge(GraphEdge::new(prev, id))?;
        }
        self.chain_nodes.push(id);
        self.built = false;
        Ok(())
    }

    pub fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String> {
        if index >= self.chain_nodes.len() {
            return Err("oob".into());
        }
        let id = self.chain_nodes.remove(index);
        self.edges.retain(|e| e.from_node != id && e.to_node != id);
        if index > 0 && index < self.chain_nodes.len() {
            self.add_edge(GraphEdge::new(
                self.chain_nodes[index - 1],
                self.chain_nodes[index],
            ))?;
        }
        self.nodes.remove(&id).unwrap();
        self.built = false;
        self.cached_latency = None;
        Ok(self.plugins[id].take().unwrap())
    }

    pub fn plugin_count(&self) -> usize {
        self.chain_nodes.len()
    }
    pub fn get_plugin(&self, index: usize) -> Option<&dyn Plugin> {
        let &nid = self.chain_nodes.get(index)?;
        self.plugins.get(nid)?.as_deref()
    }
    pub fn input_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[&self.chain_nodes[0]].input_channels()
        }
    }
    pub fn output_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[self.chain_nodes.last().unwrap()].output_channels()
        }
    }
    pub fn output_frames_for_input(&self, f: usize) -> usize {
        if self.cached_frames_identity {
            return f;
        }
        let mut result = f;
        for &id in &self.chain_nodes {
            result = self.plugins[id]
                .as_ref()
                .unwrap()
                .output_frames_for_input(result);
        }
        result
    }
    pub fn output_sample_rate(&self, r: u32) -> u32 {
        if self.cached_rate_identity {
            return r;
        }
        let mut result = r;
        for &id in &self.chain_nodes {
            result = self.plugins[id]
                .as_ref()
                .unwrap()
                .output_sample_rate(result);
        }
        result
    }
    pub fn last_output_frames(&self) -> Option<usize> {
        for &id in self.chain_nodes.iter().rev() {
            if let Some(f) = self.plugins[id].as_ref().unwrap().last_output_frames() {
                return Some(f);
            }
        }
        None
    }
    pub fn set_plugin_parameter(
        &mut self,
        index: usize,
        id: &str,
        val: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        let plugin = self.plugins[nid].as_mut().unwrap();
        plugin.set_parameter(super::parameters::ParameterId(id.to_string()), val)?;
        // Detect runtime latency changes: if the plugin's reported latency differs
        // from the cached value, mark for compensation rebuild at next process().
        let new_latency = plugin.latency_samples();
        if nid < self.node_latency_cache.len()
            && self.node_latency_cache[nid] != new_latency
        {
            self.node_latency_cache[nid] = new_latency;
            self.latency_dirty = true;
        }
        Ok(())
    }

    /// Bypass a node so its plugin is skipped during processing.
    /// When bypassed, input is passed directly to output.
    /// Only works for nodes with matching input/output channel counts.
    pub fn bypass_node(&mut self, id: NodeId) -> Result<(), String> {
        let node = self.nodes.get_mut(&id).ok_or("Node not found")?;
        if node.input_channels != node.output_channels {
            return Err(format!(
                "Cannot bypass node '{}': input channels ({}) != output channels ({})",
                node.name, node.input_channels, node.output_channels
            ));
        }
        node.bypassed = true;
        if id < self.bypassed.len() {
            self.bypassed[id] = true;
        }
        self.cached_latency = None;
        self.built = false;
        Ok(())
    }

    /// Unbypass a node so its plugin resumes processing.
    pub fn unbypass_node(&mut self, id: NodeId) -> Result<(), String> {
        let node = self.nodes.get_mut(&id).ok_or("Node not found")?;
        node.bypassed = false;
        if id < self.bypassed.len() {
            self.bypassed[id] = false;
        }
        self.cached_latency = None;
        self.built = false;
        Ok(())
    }

    /// Returns true if the given node is bypassed.
    pub fn is_node_bypassed(&self, id: NodeId) -> Result<bool, String> {
        let node = self.nodes.get(&id).ok_or("Node not found")?;
        Ok(node.bypassed)
    }

    /// Bypass a plugin by chain index (0-based index into the plugin chain).
    pub fn bypass_plugin(&mut self, index: usize) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.bypass_node(nid)
    }

    /// Unbypass a plugin by chain index.
    pub fn unbypass_plugin(&mut self, index: usize) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.unbypass_node(nid)
    }

    /// Returns true if the plugin at the given chain index is bypassed.
    pub fn is_plugin_bypassed(&self, index: usize) -> Result<bool, String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.is_node_bypassed(nid)
    }

    /// Returns indices of plugins that have analyzer data (get_data() returns Some).
    /// Computed during build() to avoid per-frame discovery.
    pub fn analyzer_indices(&self) -> &[usize] {
        &self.analyzer_indices
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        if !self.built {
            self.build()?;
        }
        // Rebuild compensation delays if a plugin's latency changed at runtime
        if self.latency_dirty {
            self.rebuild_compensation();
        }
        if self.nodes.is_empty() {
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels());
        }
        let nf = input.len() / self.input_channels();
        let max_of = self.output_frames_for_input(nf);
        let out_ch = self.output_channels();
        // Use BufferGuard to guarantee process_buffers are returned even on early ?-return
        let mut guard = BufferGuard::take(&mut self.process_buffers);
        let bufs = guard.get_mut();
        for nb in bufs.node_buffers.iter_mut().flatten() {
            nb.ensure_capacity(nf.max(max_of));
            nb.clear();
        }
        let mut cf = nf;

        // Apply automation: evaluate curves at block start AND end for sample-accurate ramps.
        // start_value = curve at current position, end_value = curve at position + nf.
        // Plugins receive both via ProcessContext.ramps for per-sample interpolation.
        // set_parameter(end_value) is called for backward compatibility.
        for ramps in self.node_ramps.iter_mut() {
            ramps.clear();
        }
        if !self.automation.is_empty() {
            self.automation_scratch.clear();
            for ((nid, _pid), auto) in &self.automation {
                if let Some(curve) = auto.curve.as_ref() {
                    let total_frames = match curve {
                        crate::automation::AutomationCurve::Step {
                            values,
                            samples_per_step,
                        } => {
                            if *samples_per_step > 0 {
                                values.len() * *samples_per_step
                            } else {
                                values.len() * nf
                            }
                        }
                        crate::automation::AutomationCurve::Linear { values } => {
                            values.len().max(1) * nf
                        }
                        crate::automation::AutomationCurve::Bezier { points } => {
                            points.last().map_or(nf, |p| p.position.max(nf))
                        }
                        crate::automation::AutomationCurve::Exponential { values, .. } => {
                            values.len().max(1) * nf
                        }
                    };
                    let pos_start = auto.position.min(total_frames.saturating_sub(1));
                    let pos_end = (auto.position + nf).min(total_frames.saturating_sub(1));
                    let start_val =
                        automation_utils::eval_curve(curve, pos_start, total_frames);
                    let end_val = automation_utils::eval_curve(curve, pos_end, total_frames);
                    // set_parameter uses start_val for backward compat: plugins
                    // that don't read ramps will process the block at the start value.
                    // Ramp-aware plugins interpolate from start_val to end_val.
                    self.automation_scratch
                        .push((*nid, auto.param_id.clone(), start_val));

                    // Build ParameterRamp if the value changes across the block
                    if (start_val - end_val).abs() > 1e-9 {
                        if let Some(&param_idx) =
                            self.param_index_cache.get(&(*nid, auto.param_id.clone()))
                        {
                            if *nid < self.node_ramps.len() {
                                self.node_ramps[*nid].push(crate::plugin::ParameterRamp {
                                    param_index: param_idx,
                                    start_value: start_val,
                                    end_value: end_val,
                                });
                            }
                        }
                    }
                }
            }
            for i in 0..self.automation_scratch.len() {
                let (nid, ref pid, val) = self.automation_scratch[i];
                if let Some(p) = self.plugins[nid].as_mut() {
                    let _ = p.set_parameter(pid.clone(), ParameterValue::Float(val));
                }
                if let Some(auto) = self.automation.get_mut(&(nid, pid.clone())) {
                    auto.last_value = val;
                    auto.position += nf;
                }
            }
        }

        for stage in &self.stages {
            let mut stage_cf: Option<usize> = None;
            for &nid in &stage.nodes {
                let node = &self.nodes[&nid];
                let in_len = if self.is_input_node[nid] {
                    bufs.scratch_input[..input.len()].copy_from_slice(input);
                    input.len()
                } else {
                    let primary_ch = if nid < self.primary_input_channels.len() {
                        self.primary_input_channels[nid]
                    } else {
                        node.input_channels()
                    };
                    let il = Self::merge_inputs_into(
                        node,
                        &self.predecessors,
                        &bufs.node_buffers,
                        cf,
                        &mut bufs.merge_buffer,
                        &mut bufs.channel_map_buffer,
                        &mut bufs.delay_scratch,
                        &mut bufs.compensation_delays,
                        primary_ch,
                    )?;
                    bufs.scratch_input[..il].copy_from_slice(&bufs.merge_buffer[..il]);
                    il
                };
                let aof = if self.bypassed[nid] {
                    // Bypassed: pass input directly to output buffer
                    bufs.node_buffers[nid]
                        .as_mut()
                        .unwrap()
                        .write(&bufs.scratch_input[..in_len]);
                    cf
                } else {
                    let p = self.plugins[nid].as_mut().unwrap();
                    let ramps = if nid < self.node_ramps.len() {
                        std::mem::take(&mut self.node_ramps[nid])
                    } else {
                        Vec::new()
                    };
                    let context = ProcessContext {
                        sample_rate: self.sample_rate,
                        num_frames: cf,
                        ramps,
                    };
                    let mof = p.output_frames_for_input(cf);
                    let ol = mof * node.output_channels();
                    if bufs.scratch_output.len() < ol {
                        bufs.scratch_output.resize(ol, 0.0);
                    }
                    let out_frames = p.process(
                        &bufs.scratch_input[..in_len],
                        &mut bufs.scratch_output[..ol],
                        &context,
                    )?;
                    // Return ramps Vec to avoid re-allocation on next process()
                    if nid < self.node_ramps.len() {
                        let mut ramps = context.ramps;
                        ramps.clear();
                        self.node_ramps[nid] = ramps;
                    }
                    bufs.node_buffers[nid]
                        .as_mut()
                        .unwrap()
                        .write(&bufs.scratch_output[..out_frames * node.output_channels()]);
                    out_frames
                };
                stage_cf = Some(match stage_cf {
                    Some(prev) => prev.min(aof),
                    None => aof,
                });
            }
            if let Some(scf) = stage_cf {
                cf = scf;
            }
        }
        Self::collect_output_from_buffers(&self.output_nodes, &bufs.node_buffers, output, cf)?;
        if cf < nf && self.has_variable_frame_plugin {
            output[cf * out_ch..].fill(0.0);
            cf = nf;
        }
        // Advance playback position for automation
        self.playback_position += nf;

        // BufferGuard's Drop impl returns bufs to self.process_buffers
        drop(guard);
        Ok(cf)
    }

    fn merge_inputs_into(
        n: &GraphNode,
        preds: &[Vec<GraphEdge>],
        nbs: &[Option<NodeBuffer>],
        nf: usize,
        mb: &mut [f32],
        cmb: &mut [f32],
        delay_scratch: &mut Vec<f32>,
        compensation_delays: &mut HashMap<(NodeId, NodeId), LookaheadBuffer>,
        primary_ch: usize,
    ) -> Result<usize, String> {
        let node_in_ch = n.input_channels();
        let is = nf * node_in_ch;
        if mb.len() < is {
            return Err(format!("Merge buffer too small: {} < {}", mb.len(), is));
        }
        mb[..is].fill(0.0);

        // Phase 1: Sum Audio edges into primary channels (0..primary_ch)
        for e in &preds[n.id] {
            if e.edge_type == EdgeType::Sidechain {
                continue;
            }
            let sb = nbs[e.from_node].as_ref().unwrap();
            let sd = sb.read();
            if let Some(ref cm) = e.channel_map {
                let ms = nf * cm.len();
                if cmb.len() < ms {
                    return Err(format!(
                        "Channel map buffer too small: {} < {}",
                        cmb.len(),
                        ms
                    ));
                }
                for f in 0..nf {
                    for (di, &si) in cm.iter().enumerate() {
                        cmb[f * cm.len() + di] = sd[f * sb.num_channels + si];
                    }
                }
                Self::apply_compensation_and_sum(
                    e, cm.len(), nf, &cmb[..ms], mb, delay_scratch, compensation_delays,
                );
            } else if sb.num_channels == node_in_ch {
                // Source channels match node input channels: bulk sum (common case)
                let len = is.min(sd.len());
                Self::apply_compensation_and_sum(
                    e, node_in_ch, nf, &sd[..len], mb, delay_scratch, compensation_delays,
                );
            } else {
                // Source has fewer channels than node (sidechain extends input).
                // Copy per-frame, placing source data into channels 0..src_ch
                // with stride = node_in_ch in the merge buffer.
                let src_ch = sb.num_channels;
                for f in 0..nf {
                    for ch in 0..src_ch.min(primary_ch) {
                        let src_idx = f * src_ch + ch;
                        let dst_idx = f * node_in_ch + ch;
                        if src_idx < sd.len() {
                            mb[dst_idx] += sd[src_idx];
                        }
                    }
                }
            }
        }

        // Phase 2: Route Sidechain edges into extended channels (primary_ch..node_in_ch)
        let mut sc_ch_offset = primary_ch;
        for e in &preds[n.id] {
            if e.edge_type != EdgeType::Sidechain {
                continue;
            }
            let sb = nbs[e.from_node].as_ref().unwrap();
            let sd = sb.read();
            let sc_ch = sb.num_channels;
            for f in 0..nf {
                for ch in 0..sc_ch {
                    let src_idx = f * sc_ch + ch;
                    let dst_idx = f * node_in_ch + sc_ch_offset + ch;
                    if src_idx < sd.len() && dst_idx < is {
                        mb[dst_idx] = sd[src_idx];
                    }
                }
            }
            sc_ch_offset += sc_ch;
        }

        Ok(is)
    }

    /// Apply latency compensation delay (if any) to `src_data` for the given edge,
    /// then sum the result into `dest`. If no compensation is needed, sums directly.
    fn apply_compensation_and_sum(
        edge: &GraphEdge,
        channels: usize,
        num_frames: usize,
        src_data: &[f32],
        dest: &mut [f32],
        delay_scratch: &mut Vec<f32>,
        compensation_delays: &mut HashMap<(NodeId, NodeId), LookaheadBuffer>,
    ) {
        let key = (edge.from_node, edge.to_node);
        if let Some(delay_buf) = compensation_delays.get_mut(&key) {
            // Process frame-by-frame through the compensation delay.
            // delay_scratch is split: first `total` samples for output,
            // next `channels` samples as a reusable silence frame.
            let total = num_frames * channels;
            let needed = total + channels;
            if delay_scratch.len() < needed {
                delay_scratch.resize(needed, 0.0);
            }
            // Zero the silence frame region
            delay_scratch[total..total + channels].fill(0.0);
            for f in 0..num_frames {
                let start = f * channels;
                let end = start + channels;
                if end <= src_data.len() {
                    // Split delay_scratch so we can read silence region while writing frame region
                    let (frame_part, silence_part) = delay_scratch.split_at_mut(total);
                    let _ = silence_part; // unused in this branch
                    delay_buf.process_frame(&src_data[start..end], &mut frame_part[start..end]);
                } else {
                    // Partial/missing frame: feed silence from the tail of delay_scratch
                    let (frame_part, silence_part) = delay_scratch.split_at_mut(total);
                    delay_buf.process_frame(&silence_part[..channels], &mut frame_part[start..end]);
                }
            }
            for (d, &s) in dest[..total].iter_mut().zip(delay_scratch[..total].iter()) {
                *d += s;
            }
        } else {
            // No compensation, sum directly
            let len = src_data.len().min(dest.len());
            for (d, &s) in dest[..len].iter_mut().zip(src_data[..len].iter()) {
                *d += s;
            }
        }
    }

    fn collect_output_from_buffers(
        ons: &[NodeId],
        nbs: &[Option<NodeBuffer>],
        out: &mut [f32],
        _nf: usize,
    ) -> Result<(), String> {
        if ons.len() == 1 {
            let d = nbs[ons[0]].as_ref().unwrap().read();
            let l = d.len().min(out.len());
            out[..l].copy_from_slice(&d[..l]);
        } else {
            out.fill(0.0);
            for &id in ons {
                let d = nbs[id].as_ref().unwrap().read();
                let l = d.len().min(out.len());
                for (dst, &s) in out[..l].iter_mut().zip(d[..l].iter()) {
                    *dst += s;
                }
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        for &id in self.nodes.keys() {
            if let Some(p) = self.plugins[id].as_mut() {
                p.reset();
            }
        }
    }
    pub fn total_latency_samples(&self) -> usize {
        if let Some(cached) = self.cached_latency {
            return cached;
        }
        self.compute_latency()
    }
    fn compute_latency(&self) -> usize {
        self.output_nodes
            .iter()
            .map(|&id| self.path_latency(id))
            .max()
            .unwrap_or(0)
    }
    fn path_latency(&self, id: NodeId) -> usize {
        // Bypassed nodes contribute zero latency
        let l = if self.nodes.get(&id).is_some_and(|n| n.bypassed) {
            0
        } else {
            self.plugins[id].as_ref().unwrap().latency_samples()
        };
        // Use pre-built predecessors list (no heap allocation, O(n) instead of O(2^n) for diamonds)
        let preds = if id < self.predecessors.len() {
            &self.predecessors[id]
        } else {
            // Fallback for nodes added after build() — scan edges
            return l + self
                .edges
                .iter()
                .filter(|e| e.to_node == id)
                .map(|e| self.path_latency(e.from_node))
                .max()
                .unwrap_or(0);
        };
        if preds.is_empty() {
            l
        } else {
            l + preds
                .iter()
                .map(|e| self.path_latency(e.from_node))
                .max()
                .unwrap_or(0)
        }
    }
    /// Compute the cumulative latency from graph inputs to each node, then create
    /// compensation delay buffers for edges feeding into merge points where path
    /// latencies differ. This ensures all paths through the DAG are time-aligned
    /// at merge points.
    fn compute_compensation_delays(&mut self, num_slots: usize) -> HashMap<(NodeId, NodeId), LookaheadBuffer> {
        // Step 1: Compute cumulative latency from inputs to each node using topological order.
        // For each node, the cumulative latency is:
        //   node's own latency + max(cumulative latency of predecessors)
        self.node_latency_from_input = vec![0; num_slots];

        // Process in topological order (stages are already computed)
        for stage in &self.stages {
            for &nid in &stage.nodes {
                let own_latency = if self.bypassed.get(nid).copied().unwrap_or(false) {
                    0
                } else {
                    self.plugins[nid]
                        .as_ref()
                        .map(|p| p.latency_samples())
                        .unwrap_or(0)
                };

                let max_pred_latency = self.predecessors[nid]
                    .iter()
                    .map(|e| self.node_latency_from_input[e.from_node])
                    .max()
                    .unwrap_or(0);

                self.node_latency_from_input[nid] = own_latency + max_pred_latency;
            }
        }

        // Step 2: For each merge point (node with multiple predecessors), compute
        // compensation delays for shorter paths.
        let mut delays = HashMap::new();

        for stage in &self.stages {
            for &nid in &stage.nodes {
                let preds = &self.predecessors[nid];
                if preds.len() < 2 {
                    continue; // Not a merge point
                }

                // Find the max cumulative latency among all predecessors
                let max_pred_latency = preds
                    .iter()
                    .map(|e| self.node_latency_from_input[e.from_node])
                    .max()
                    .unwrap_or(0);

                // For each predecessor with lower latency, create a compensation delay
                for edge in preds {
                    let pred_latency = self.node_latency_from_input[edge.from_node];
                    let compensation = max_pred_latency - pred_latency;
                    if compensation > 0 {
                        let pred_channels = self.nodes.get(&edge.from_node)
                            .map(|n| n.output_channels())
                            .unwrap_or(2);
                        delays.insert(
                            (edge.from_node, edge.to_node),
                            LookaheadBuffer::new(compensation, pred_channels),
                        );
                    }
                }
            }
        }

        delays
    }

    /// Recompute compensation delays after a plugin's latency changed at runtime.
    /// Only recalculates latency and delay buffers — does not do a full build().
    fn rebuild_compensation(&mut self) {
        let num_slots = self.plugins.len();
        let new_delays = self.compute_compensation_delays(num_slots);
        if let Some(bufs) = self.process_buffers.as_mut() {
            bufs.compensation_delays = new_delays;
        }
        self.cached_latency = Some(self.compute_latency());
        self.latency_dirty = false;
    }

    fn has_cycle(&self) -> bool {
        let mut v = HashSet::new();
        let mut r = HashSet::new();
        for &id in self.nodes.keys() {
            if self.cycle_util(id, &mut v, &mut r) {
                return true;
            }
        }
        false
    }
    fn cycle_util(&self, id: NodeId, v: &mut HashSet<NodeId>, r: &mut HashSet<NodeId>) -> bool {
        if r.contains(&id) {
            return true;
        }
        if v.contains(&id) {
            return false;
        }
        v.insert(id);
        r.insert(id);
        for e in &self.edges {
            if e.from_node == id && self.cycle_util(e.to_node, v, r) {
                return true;
            }
        }
        r.remove(&id);
        false
    }
    fn compute_io_nodes(&mut self) {
        let mut hi = HashSet::new();
        let mut ho = HashSet::new();
        for e in &self.edges {
            hi.insert(e.to_node);
            ho.insert(e.from_node);
        }
        self.input_nodes = self
            .nodes
            .keys()
            .filter(|id| !hi.contains(id))
            .copied()
            .collect();
        self.output_nodes = self
            .nodes
            .keys()
            .filter(|id| !ho.contains(id))
            .copied()
            .collect();
    }
    fn compute_stages(&mut self) -> Result<(), String> {
        let mut deg: HashMap<NodeId, usize> = self.nodes.keys().map(|&id| (id, 0)).collect();
        for e in &self.edges {
            *deg.get_mut(&e.to_node).unwrap() += 1;
        }
        let mut q: VecDeque<NodeId> = deg
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        self.stages.clear();
        let mut count = 0;
        while !q.is_empty() {
            let mut s = ProcessingStage::new();
            for _ in 0..q.len() {
                let id = q.pop_front().unwrap();
                s.add_node(id);
                count += 1;
                for e in &self.edges {
                    if e.from_node == id {
                        let d = deg.get_mut(&e.to_node).unwrap();
                        *d -= 1;
                        if *d == 0 {
                            q.push_back(e.to_node);
                        }
                    }
                }
            }
            if !s.is_empty() {
                self.stages.push(s);
            }
        }
        if count != self.nodes.len() {
            Err("Cycle".into())
        } else {
            Ok(())
        }
    }
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }
    pub fn stage_info(&self, idx: usize) -> Option<Vec<String>> {
        self.stages.get(idx).map(|s| {
            s.nodes
                .iter()
                .filter_map(|id| self.nodes.get(id))
                .map(|n| n.name.clone())
                .collect()
        })
    }
}

impl Host for DawHost {
    fn add_plugin(&mut self, p: Box<dyn Plugin>) -> Result<(), String> {
        self.add_plugin(p)
    }
    fn remove_plugin(&mut self, i: usize) -> Result<Box<dyn Plugin>, String> {
        self.remove_plugin(i)
    }
    fn plugin_count(&self) -> usize {
        self.plugin_count()
    }
    fn get_plugin(&self, i: usize) -> Option<&dyn Plugin> {
        self.get_plugin(i)
    }
    fn input_channels(&self) -> usize {
        self.input_channels()
    }
    fn output_channels(&self) -> usize {
        self.output_channels()
    }
    fn process(&mut self, i: &[f32], o: &mut [f32]) -> Result<usize, String> {
        self.process(i, o)
    }
    fn reset(&mut self) {
        self.reset()
    }
    fn total_latency_samples(&self) -> usize {
        self.total_latency_samples()
    }
    fn get_plugin_data(&self, i: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        let &node_id = self.chain_nodes.get(i)?;
        self.plugins.get(node_id)?.as_ref()?.get_data()
    }
}

// Note: Host integration tests that use plugin types (GainPlugin, UpmixerPlugin, etc.)
// live in the `sotf-plugins` facade crate's tests/ directory since those plugin types
// are not available in `sotf-host`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Plugin;

    #[test]
    fn test_pluginhost_api_empty_graph() {
        let mut g = DawHost::new(2, 48000);
        let i = vec![1.0; 96];
        let mut o = vec![0.0; 96];
        assert!(g.process(&i, &mut o).is_ok());
        assert_eq!(o, i);
    }

    /// Mock variable-frame plugin that returns a configurable output frame count.
    struct VariableFramePlugin {
        channels: usize,
        output_frames: usize,
    }
    impl VariableFramePlugin {
        fn new(channels: usize, output_frames: usize) -> Self {
            Self {
                channels,
                output_frames,
            }
        }
    }
    impl Plugin for VariableFramePlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("VariableFrame", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![]
        }
        fn set_parameter(
            &mut self,
            _: crate::parameters::ParameterId,
            _: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(
            &self,
            _: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            None
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            _ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            let out_len = self.output_frames * self.channels;
            for (o, &i) in output[..out_len].iter_mut().zip(input.iter().cycle()) {
                *o = i;
            }
            Ok(self.output_frames)
        }
        fn output_frames_for_input(&self, _: usize) -> usize {
            self.output_frames
        }
        fn latency_samples(&self) -> usize {
            1
        }
    }

    /// Mock plugin that records the ProcessContext.num_frames it receives.
    struct FrameRecorderPlugin {
        channels: usize,
        last_num_frames: std::cell::Cell<usize>,
    }
    impl FrameRecorderPlugin {
        fn new(channels: usize) -> Self {
            Self {
                channels,
                last_num_frames: std::cell::Cell::new(0),
            }
        }
    }
    impl Plugin for FrameRecorderPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("FrameRecorder", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![]
        }
        fn set_parameter(
            &mut self,
            _: crate::parameters::ParameterId,
            _: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(
            &self,
            _: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            None
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            self.last_num_frames.set(ctx.num_frames);
            let len = input.len().min(output.len());
            output[..len].copy_from_slice(&input[..len]);
            Ok(ctx.num_frames)
        }
        fn get_data(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
            Some(Arc::new(self.last_num_frames.get()))
        }
    }

    #[test]
    fn test_parallel_variable_frame_consistent_cf() {
        let mut g = DawHost::new(2, 48000);

        let input_node = g
            .add_node("input".into(), Box::new(VariableFramePlugin::new(2, 256)))
            .unwrap();
        let vf_a = g
            .add_node("vf_a".into(), Box::new(VariableFramePlugin::new(2, 100)))
            .unwrap();
        let vf_b = g
            .add_node("vf_b".into(), Box::new(VariableFramePlugin::new(2, 200)))
            .unwrap();
        let output_node = g
            .add_node("recorder".into(), Box::new(FrameRecorderPlugin::new(2)))
            .unwrap();

        g.add_edge(GraphEdge::new(input_node, vf_a)).unwrap();
        g.add_edge(GraphEdge::new(input_node, vf_b)).unwrap();
        g.add_edge(GraphEdge::new(vf_a, output_node)).unwrap();
        g.add_edge(GraphEdge::new(vf_b, output_node)).unwrap();
        g.build().unwrap();

        let nf = 256;
        let input = vec![0.5_f32; nf * 2];
        let mut output = vec![0.0_f32; nf * 2];
        g.process(&input, &mut output).unwrap();

        let recorded_cf = g.plugins[output_node]
            .as_ref()
            .unwrap()
            .get_data()
            .unwrap()
            .downcast_ref::<usize>()
            .copied()
            .unwrap();

        assert_eq!(
            recorded_cf, 100,
            "Downstream received cf={}, expected 100 (min of parallel outputs)",
            recorded_cf,
        );
    }

    /// Mock plugin that scales all samples by a factor. Used to detect bypass vs. active processing.
    struct ScalerPlugin {
        channels: usize,
        factor: f32,
        latency: usize,
    }
    impl ScalerPlugin {
        fn new(channels: usize, factor: f32) -> Self {
            Self {
                channels,
                factor,
                latency: 0,
            }
        }
        fn with_latency(channels: usize, factor: f32, latency: usize) -> Self {
            Self {
                channels,
                factor,
                latency,
            }
        }
    }
    impl Plugin for ScalerPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("Scaler", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![]
        }
        fn set_parameter(
            &mut self,
            _: crate::parameters::ParameterId,
            _: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(
            &self,
            _: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            None
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i * self.factor;
            }
            Ok(ctx.num_frames)
        }
        fn latency_samples(&self) -> usize {
            self.latency
        }
    }

    // ---- Cached latency tests ----

    #[test]
    fn test_cached_latency_empty_graph() {
        let mut g = DawHost::new(2, 48000);
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 0);
    }

    #[test]
    fn test_cached_latency_single_plugin() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 42))).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 42);
    }

    #[test]
    fn test_cached_latency_chain_sums() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 30))).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 60);
    }

    #[test]
    fn test_cached_latency_invalidated_on_add_node() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10))).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 10);
        // Adding another plugin invalidates the cache
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20))).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 30);
    }

    #[test]
    fn test_cached_latency_invalidated_on_remove() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20))).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 30);
        g.remove_plugin(0).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 20);
    }

    // ---- Bypass tests ----

    #[test]
    fn test_bypass_plugin_passes_input_through() {
        let mut g = DawHost::new(2, 48000);
        // Plugin that doubles all samples
        g.add_plugin(Box::new(ScalerPlugin::new(2, 2.0))).unwrap();
        g.build().unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let mut output = vec![0.0; 4];

        // Without bypass: output should be doubled
        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![2.0, 4.0, 6.0, 8.0]);

        // Bypass the plugin
        g.bypass_plugin(0).unwrap();
        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![1.0, 2.0, 3.0, 4.0]);

        // Unbypass: should double again
        g.unbypass_plugin(0).unwrap();
        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn test_bypass_middle_of_chain() {
        let mut g = DawHost::new(2, 48000);
        // Chain: x2 -> x3 -> x5
        g.add_plugin(Box::new(ScalerPlugin::new(2, 2.0))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::new(2, 3.0))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::new(2, 5.0))).unwrap();
        g.build().unwrap();

        let input = vec![1.0, 1.0]; // 1 frame, 2 channels
        let mut output = vec![0.0; 2];

        // All active: 1 * 2 * 3 * 5 = 30
        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![30.0, 30.0]);

        // Bypass middle (x3): 1 * 2 * 5 = 10
        g.bypass_plugin(1).unwrap();
        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![10.0, 10.0]);
    }

    #[test]
    fn test_bypass_reduces_latency() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20))).unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 30);

        // Bypass second plugin: latency drops to 10
        g.bypass_plugin(1).unwrap();
        // Cache was invalidated, need rebuild for latency recalc
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 10);
    }

    #[test]
    fn test_bypass_node_query() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        assert!(!g.is_plugin_bypassed(0).unwrap());
        g.bypass_plugin(0).unwrap();
        assert!(g.is_plugin_bypassed(0).unwrap());
        g.unbypass_plugin(0).unwrap();
        assert!(!g.is_plugin_bypassed(0).unwrap());
    }

    #[test]
    fn test_bypass_oob_returns_error() {
        let mut g = DawHost::new(2, 48000);
        assert!(g.bypass_plugin(0).is_err());
        assert!(g.unbypass_plugin(0).is_err());
        assert!(g.is_plugin_bypassed(0).is_err());
    }

    #[test]
    fn test_bypass_channel_mismatch_rejected() {
        let mut g = DawHost::new(2, 48000);
        // Use VariableFramePlugin as it has same in/out channels, but let's
        // create a channel-changing scenario using add_node directly
        let id = g
            .add_node(
                "upmix".into(),
                Box::new(ChannelChangingPlugin { in_ch: 2, out_ch: 5 }),
            )
            .unwrap();
        let result = g.bypass_node(id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot bypass"));
    }

    /// Mock plugin with different input/output channel counts (e.g., upmixer).
    struct ChannelChangingPlugin {
        in_ch: usize,
        out_ch: usize,
    }
    impl Plugin for ChannelChangingPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("ChannelChanger", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.in_ch
        }
        fn output_channels(&self) -> usize {
            self.out_ch
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![]
        }
        fn set_parameter(
            &mut self,
            _: crate::parameters::ParameterId,
            _: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(
            &self,
            _: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            None
        }
        fn process(
            &mut self,
            _input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            output.fill(0.0);
            Ok(ctx.num_frames)
        }
    }

    #[test]
    fn node_buffer_read_returns_empty_after_clear() {
        let mut buf = NodeBuffer::new(128, 2);
        // Write some data
        buf.write(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(buf.read().len(), 4);
        // After clear, read must return empty slice (not stale data)
        buf.clear();
        assert!(
            buf.read().is_empty(),
            "read() after clear() must return empty slice"
        );
    }

    // ---- Latency compensation tests ----

    #[test]
    fn test_latency_compensation_parallel_paths() {
        // Graph: input -> [A (latency=10), B (latency=0)] -> output
        // Path A has 10 samples of latency, path B has 0.
        // Compensation should delay path B by 10 samples so they align at the merge.
        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let a = g.add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 2.0, 10))).unwrap();
        let b = g.add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 3.0, 0))).unwrap();
        let out = g.add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, a)).unwrap();
        g.add_edge(GraphEdge::new(inp, b)).unwrap();
        g.add_edge(GraphEdge::new(a, out)).unwrap();
        g.add_edge(GraphEdge::new(b, out)).unwrap();
        g.build().unwrap();

        // Check that compensation delay was created for edge B->output
        let bufs = g.process_buffers.as_ref().unwrap();
        assert!(
            bufs.compensation_delays.contains_key(&(b, out)),
            "Should have compensation delay on shorter path (B->output)"
        );
        assert!(
            !bufs.compensation_delays.contains_key(&(a, out)),
            "Should NOT have compensation delay on longer path (A->output)"
        );

        // The compensation delay for B->output should be 10 samples
        let delay = bufs.compensation_delays.get(&(b, out)).unwrap();
        assert_eq!(delay.delay(), 10);
    }

    #[test]
    fn test_latency_compensation_equal_paths_no_delay() {
        // Graph: input -> [A (latency=5), B (latency=5)] -> output
        // Both paths have equal latency, no compensation needed.
        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let a = g.add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 5))).unwrap();
        let b = g.add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 5))).unwrap();
        let out = g.add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, a)).unwrap();
        g.add_edge(GraphEdge::new(inp, b)).unwrap();
        g.add_edge(GraphEdge::new(a, out)).unwrap();
        g.add_edge(GraphEdge::new(b, out)).unwrap();
        g.build().unwrap();

        let bufs = g.process_buffers.as_ref().unwrap();
        assert!(
            bufs.compensation_delays.is_empty(),
            "No compensation needed when paths have equal latency"
        );
    }

    #[test]
    fn test_latency_compensation_chain_no_merge() {
        // Simple chain: A -> B -> C, no parallel paths, no compensation needed.
        let mut g = DawHost::new(1, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 10))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 20))).unwrap();
        g.build().unwrap();

        let bufs = g.process_buffers.as_ref().unwrap();
        assert!(
            bufs.compensation_delays.is_empty(),
            "No compensation needed in a simple chain"
        );
    }

    #[test]
    fn test_latency_compensation_delays_audio() {
        // Verify that compensation actually delays the audio data.
        // Graph: input -> [A (latency=2, factor=1.0), B (latency=0, factor=1.0)] -> output
        // B should be delayed by 2 frames to align with A.
        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let a = g.add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 2))).unwrap();
        let b = g.add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 0))).unwrap();
        let out = g.add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, a)).unwrap();
        g.add_edge(GraphEdge::new(inp, b)).unwrap();
        g.add_edge(GraphEdge::new(a, out)).unwrap();
        g.add_edge(GraphEdge::new(b, out)).unwrap();
        g.build().unwrap();

        // Process a block: first 2 frames of B's contribution should be silence (delay)
        let nf = 8;
        let input = vec![1.0f32; nf];
        let mut output = vec![0.0f32; nf];
        g.process(&input, &mut output).unwrap();

        // Without compensation: B contributes immediately, A contributes immediately.
        // With compensation: B's output is delayed by 2 samples (silence for first 2 frames).
        // So at merge: frame 0-1 get only A's contribution (1.0 each).
        // frame 2-7 get A + delayed B (1.0 + 1.0 = 2.0 each).
        // A also contributes directly (ScalerPlugin doesn't actually delay internally,
        // it just reports latency). So both contribute 1.0, but B is delayed by 2.
        // Frames 0-1: A=1.0 + B_delayed=0.0 = 1.0
        // Frames 2-7: A=1.0 + B_delayed=1.0 = 2.0
        assert_eq!(output[0], 1.0, "Frame 0: only A contributes (B is compensated/delayed)");
        assert_eq!(output[1], 1.0, "Frame 1: only A contributes (B is compensated/delayed)");
        assert_eq!(output[2], 2.0, "Frame 2: both A and delayed B contribute");
        assert_eq!(output[7], 2.0, "Frame 7: both A and delayed B contribute");
    }

    #[test]
    fn test_latency_compensation_asymmetric_three_paths() {
        // Graph: input -> [A (lat=20), B (lat=10), C (lat=0)] -> output
        // Compensation: B delayed by 10, C delayed by 20
        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let a = g.add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 20))).unwrap();
        let b = g.add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 10))).unwrap();
        let c = g.add_node("C".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 0))).unwrap();
        let out = g.add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, a)).unwrap();
        g.add_edge(GraphEdge::new(inp, b)).unwrap();
        g.add_edge(GraphEdge::new(inp, c)).unwrap();
        g.add_edge(GraphEdge::new(a, out)).unwrap();
        g.add_edge(GraphEdge::new(b, out)).unwrap();
        g.add_edge(GraphEdge::new(c, out)).unwrap();
        g.build().unwrap();

        let bufs = g.process_buffers.as_ref().unwrap();
        // A has max latency (20), no compensation
        assert!(!bufs.compensation_delays.contains_key(&(a, out)));
        // B needs 10 samples of compensation (20 - 10)
        assert_eq!(bufs.compensation_delays.get(&(b, out)).unwrap().delay(), 10);
        // C needs 20 samples of compensation (20 - 0)
        assert_eq!(bufs.compensation_delays.get(&(c, out)).unwrap().delay(), 20);
    }

    #[test]
    fn test_latency_compensation_bypassed_node_zero_latency() {
        // Bypassed nodes contribute zero latency, affecting compensation
        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let a = g.add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 10))).unwrap();
        let b = g.add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 10))).unwrap();
        let out = g.add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, a)).unwrap();
        g.add_edge(GraphEdge::new(inp, b)).unwrap();
        g.add_edge(GraphEdge::new(a, out)).unwrap();
        g.add_edge(GraphEdge::new(b, out)).unwrap();

        // Bypass A: its latency drops to 0. B still has 10.
        g.bypass_node(a).unwrap();
        g.build().unwrap();

        let bufs = g.process_buffers.as_ref().unwrap();
        // A (bypassed, latency=0) needs 10 samples of compensation
        assert_eq!(bufs.compensation_delays.get(&(a, out)).unwrap().delay(), 10);
        // B (latency=10) is the longest path, no compensation
        assert!(!bufs.compensation_delays.contains_key(&(b, out)));
    }

    // ---- Buffer safety guard tests ----

    #[test]
    fn test_buffer_guard_returns_buffers_on_drop() {
        let mut slot: Option<ProcessBuffers> = Some(ProcessBuffers {
            node_buffers: vec![],
            scratch_input: vec![],
            scratch_output: vec![],
            merge_buffer: vec![],
            channel_map_buffer: vec![],
            compensation_delays: HashMap::new(),
            delay_scratch: vec![],
            parallel_scratch: Vec::new(),
        });

        {
            let _guard = BufferGuard::take(&mut slot);
            // guard holds &mut slot, so we can't check slot here,
            // but on drop it must restore the buffers.
        }
        assert!(slot.is_some(), "Slot should be restored after guard dropped");
    }

    #[test]
    fn test_buffer_guard_survives_simulated_early_return() {
        let mut slot: Option<ProcessBuffers> = Some(ProcessBuffers {
            node_buffers: vec![],
            scratch_input: vec![],
            scratch_output: vec![],
            merge_buffer: vec![],
            channel_map_buffer: vec![],
            compensation_delays: HashMap::new(),
            delay_scratch: vec![],
            parallel_scratch: Vec::new(),
        });

        // Simulate early return inside a scope
        let result: Result<(), String> = (|| {
            let _guard = BufferGuard::take(&mut slot);
            // Simulate error that would cause early return
            Err("simulated error".into())
        })();

        assert!(result.is_err());
        assert!(
            slot.is_some(),
            "Slot must be restored even after error return"
        );
    }

    #[test]
    fn test_node_latency_from_input_computed() {
        // Chain: A(lat=5) -> B(lat=10) -> C(lat=3)
        // Cumulative: A=5, B=15, C=18
        let mut g = DawHost::new(1, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 5))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 10))).unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 3))).unwrap();
        g.build().unwrap();

        let id_a = g.chain_nodes[0];
        let id_b = g.chain_nodes[1];
        let id_c = g.chain_nodes[2];
        assert_eq!(g.node_latency_from_input[id_a], 5);
        assert_eq!(g.node_latency_from_input[id_b], 15);
        assert_eq!(g.node_latency_from_input[id_c], 18);
    }

    /// Mock plugin that applies a gain parameter to all samples.
    /// Supports `set_parameter`/`get_parameter` for the "gain" parameter so
    /// automation tests can verify the value was written.
    struct GainPlugin {
        channels: usize,
        gain: f32,
    }
    impl GainPlugin {
        fn new(channels: usize, initial_gain: f32) -> Self {
            Self {
                channels,
                gain: initial_gain,
            }
        }
    }
    impl Plugin for GainPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("Gain", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![crate::parameters::Parameter::new_float(
                "gain", "Gain", 1.0, 0.0, 4.0,
            )]
        }
        fn set_parameter(
            &mut self,
            id: crate::parameters::ParameterId,
            val: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            if id.0 == "gain" {
                if let crate::parameters::ParameterValue::Float(v) = val {
                    self.gain = v;
                    return Ok(());
                }
            }
            Err(format!("unknown parameter: {}", id.0))
        }
        fn get_parameter(
            &self,
            id: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            if id.0 == "gain" {
                Some(crate::parameters::ParameterValue::Float(self.gain))
            } else {
                None
            }
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i * self.gain;
            }
            Ok(ctx.num_frames)
        }
    }

    // ---- Automation tests ----

    #[test]
    fn test_automation_basic() {
        // Create a host with a single GainPlugin starting at gain=1.0.
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        // The chain_nodes[0] is the NodeId assigned to our GainPlugin.
        let node_id = g.chain_nodes[0];
        let param_id = crate::parameters::ParameterId::from("gain");

        // 2 channels × 48 frames = 96 samples.
        let num_frames = 48;

        // Step automation: each step lasts exactly num_frames samples so each
        // call to process() advances to the next step value.
        // Step 0 (samples 0..47)  → gain = 0.25
        // Step 1 (samples 48..95) → gain = 0.75
        g.set_automation(
            node_id,
            param_id.clone(),
            crate::automation::AutomationCurve::Step {
                values: vec![0.25, 0.75],
                samples_per_step: num_frames,
            },
        );
        let input = vec![1.0f32; num_frames * 2];
        let mut output = vec![0.0f32; num_frames * 2];

        // First process(): automation evaluates at position=0, step=0 → gain=0.25.
        g.process(&input, &mut output).unwrap();

        let gain_after_first_block = g
            .get_plugin(0)
            .unwrap()
            .get_parameter(&param_id)
            .and_then(|v| v.as_float())
            .expect("gain parameter must be readable");

        assert!(
            (gain_after_first_block - 0.25).abs() < 1e-6,
            "After first process(), gain should be 0.25 (step 0), got {}",
            gain_after_first_block
        );

        // Verify the audio was actually scaled by 0.25.
        assert!(
            output.iter().all(|&s| (s - 0.25).abs() < 1e-6),
            "Output samples should be input * 0.25"
        );

        // Second process(): automation position = num_frames, step=1 → gain=0.75.
        g.process(&input, &mut output).unwrap();

        let gain_after_second_block = g
            .get_plugin(0)
            .unwrap()
            .get_parameter(&param_id)
            .and_then(|v| v.as_float())
            .expect("gain parameter must be readable");

        assert!(
            (gain_after_second_block - 0.75).abs() < 1e-6,
            "After second process(), gain should be 0.75 (step 1), got {}",
            gain_after_second_block
        );

        // Verify audio was scaled by 0.75.
        assert!(
            output.iter().all(|&s| (s - 0.75).abs() < 1e-6),
            "Output samples should be input * 0.75"
        );
    }

    // ---- PDC (Plugin Delay Compensation) tests ----

    /// A plugin that actually delays audio by `delay` samples using a LookaheadBuffer,
    /// and correctly reports latency_samples(). Used to test PDC end-to-end.
    struct TrueDelayPlugin {
        channels: usize,
        delay: usize,
        buf: crate::lookahead::LookaheadBuffer,
    }
    impl TrueDelayPlugin {
        fn new(channels: usize, delay_samples: usize) -> Self {
            Self {
                channels,
                delay: delay_samples,
                buf: crate::lookahead::LookaheadBuffer::new(delay_samples, channels),
            }
        }
    }
    impl Plugin for TrueDelayPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("TrueDelay", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![]
        }
        fn set_parameter(
            &mut self,
            _: crate::parameters::ParameterId,
            _: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(
            &self,
            _: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            None
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            let ch = self.channels;
            for f in 0..ctx.num_frames {
                let start = f * ch;
                let end = start + ch;
                self.buf
                    .process_frame(&input[start..end], &mut output[start..end]);
            }
            Ok(ctx.num_frames)
        }
        fn latency_samples(&self) -> usize {
            self.delay
        }
    }

    #[test]
    fn test_pdc_diamond_time_alignment() {
        // Diamond graph:
        //   inp -> [passthrough (lat=0), delay (lat=16)] -> out
        //
        // Without PDC, the passthrough path would arrive 16 samples early.
        // With PDC, both paths should be time-aligned at the merge (out) node:
        // the passthrough path gets a compensation delay of 16 samples.
        let delay_samples = 16;
        let num_frames = 64;

        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("inp".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let pass = g.add_node("pass".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let delay = g.add_node("delay".into(), Box::new(TrueDelayPlugin::new(1, delay_samples))).unwrap();
        let out = g.add_node("out".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, pass)).unwrap();
        g.add_edge(GraphEdge::new(inp, delay)).unwrap();
        g.add_edge(GraphEdge::new(pass, out)).unwrap();
        g.add_edge(GraphEdge::new(delay, out)).unwrap();
        g.build().unwrap();

        assert_eq!(g.total_latency_samples(), delay_samples);

        // Create an impulse at frame 0
        let mut input = vec![0.0f32; num_frames];
        input[0] = 1.0;
        let mut output = vec![0.0f32; num_frames];

        g.process(&input, &mut output).unwrap();

        // Both paths should arrive at the same time (frame = delay_samples).
        // passthrough path: delayed by PDC compensation (16 samples)
        // delay path: delayed by TrueDelayPlugin (16 samples)
        // At the merge, they sum: output[delay_samples] should be 2.0 (1.0 + 1.0)
        assert!(
            (output[delay_samples] - 2.0).abs() < 1e-6,
            "Impulse should be doubled at frame {} due to PDC alignment, got {}",
            delay_samples,
            output[delay_samples]
        );

        // No energy before the aligned position
        for i in 0..delay_samples {
            assert!(
                output[i].abs() < 1e-6,
                "Frame {} should be silent before aligned impulse, got {}",
                i,
                output[i]
            );
        }

        // No energy after the aligned impulse
        for i in (delay_samples + 1)..num_frames {
            assert!(
                output[i].abs() < 1e-6,
                "Frame {} should be silent after aligned impulse, got {}",
                i,
                output[i]
            );
        }
    }

    #[test]
    fn test_pdc_diamond_multi_block() {
        // Same diamond as above, but process the impulse across multiple blocks.
        // The impulse is in the first block; alignment should still work.
        let delay_samples = 8;
        let block_size = 4; // Smaller than delay
        let total_frames = 32;

        let mut g = DawHost::new(1, 48000);

        let inp = g.add_node("inp".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let pass = g.add_node("pass".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();
        let delay = g.add_node("delay".into(), Box::new(TrueDelayPlugin::new(1, delay_samples))).unwrap();
        let out = g.add_node("out".into(), Box::new(ScalerPlugin::new(1, 1.0))).unwrap();

        g.add_edge(GraphEdge::new(inp, pass)).unwrap();
        g.add_edge(GraphEdge::new(inp, delay)).unwrap();
        g.add_edge(GraphEdge::new(pass, out)).unwrap();
        g.add_edge(GraphEdge::new(delay, out)).unwrap();
        g.build().unwrap();

        // Collect output across multiple blocks
        let mut all_output = Vec::new();
        for block_idx in 0..(total_frames / block_size) {
            let mut input = vec![0.0f32; block_size];
            if block_idx == 0 {
                input[0] = 1.0; // Impulse in first frame of first block
            }
            let mut output = vec![0.0f32; block_size];
            g.process(&input, &mut output).unwrap();
            all_output.extend_from_slice(&output);
        }

        // Impulse should appear at sample = delay_samples, doubled
        assert!(
            (all_output[delay_samples] - 2.0).abs() < 1e-6,
            "Multi-block: impulse should be doubled at frame {}, got {}",
            delay_samples,
            all_output[delay_samples]
        );

        // Verify silence elsewhere
        for (i, &s) in all_output.iter().enumerate() {
            if i != delay_samples {
                assert!(
                    s.abs() < 1e-6,
                    "Multi-block: frame {} should be silent, got {}",
                    i,
                    s
                );
            }
        }
    }

    #[test]
    fn test_pdc_reported_vs_actual() {
        // Verify that total_latency_samples() matches the actual measured delay.
        // Linear chain: input -> delay(32) -> output
        let delay_samples = 32;
        let channels = 1;
        let num_frames = 128;

        let mut g = DawHost::new(channels, 48000);
        g.add_plugin(Box::new(TrueDelayPlugin::new(channels, delay_samples)))
            .unwrap();
        g.build().unwrap();

        let reported = g.total_latency_samples();
        assert_eq!(reported, delay_samples);

        // Measure actual delay via impulse
        let mut input = vec![0.0f32; num_frames * channels];
        input[0] = 1.0;
        let mut output = vec![0.0f32; num_frames * channels];
        g.process(&input, &mut output).unwrap();

        // Find the first non-zero sample in output
        let actual = output
            .iter()
            .position(|&s| s.abs() > 0.5)
            .expect("impulse should appear in output");

        assert_eq!(
            reported, actual,
            "Reported latency ({}) should match actual measured delay ({})",
            reported, actual
        );
    }

    /// A plugin whose latency changes when the "latency" parameter is set.
    /// Used to test runtime latency rebuild.
    struct VariableLatencyPlugin {
        channels: usize,
        latency: usize,
    }
    impl VariableLatencyPlugin {
        fn new(channels: usize, initial_latency: usize) -> Self {
            Self {
                channels,
                latency: initial_latency,
            }
        }
    }
    impl Plugin for VariableLatencyPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("VarLatency", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![crate::parameters::Parameter::new_float(
                "latency", "Latency", 0.0, 0.0, 1000.0,
            )]
        }
        fn set_parameter(
            &mut self,
            id: crate::parameters::ParameterId,
            val: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            if id.0 == "latency" {
                if let crate::parameters::ParameterValue::Float(v) = val {
                    self.latency = v as usize;
                    return Ok(());
                }
            }
            Err(format!("unknown parameter: {}", id.0))
        }
        fn get_parameter(
            &self,
            id: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            if id.0 == "latency" {
                Some(crate::parameters::ParameterValue::Float(
                    self.latency as f32,
                ))
            } else {
                None
            }
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            output[..input.len()].copy_from_slice(input);
            Ok(ctx.num_frames)
        }
        fn latency_samples(&self) -> usize {
            self.latency
        }
    }

    #[test]
    fn test_runtime_latency_change_triggers_rebuild() {
        let mut g = DawHost::new(1, 48000);
        g.add_plugin(Box::new(VariableLatencyPlugin::new(1, 10)))
            .unwrap();
        g.build().unwrap();

        assert_eq!(g.total_latency_samples(), 10);
        assert!(!g.latency_dirty);

        // Change latency via parameter
        g.set_plugin_parameter(
            0,
            "latency",
            crate::parameters::ParameterValue::Float(20.0),
        )
        .unwrap();

        assert!(g.latency_dirty, "latency_dirty should be set after latency change");

        // Process to trigger rebuild
        let input = vec![0.0f32; 64];
        let mut output = vec![0.0f32; 64];
        g.process(&input, &mut output).unwrap();

        assert_eq!(
            g.total_latency_samples(),
            20,
            "Total latency should update after process() triggers rebuild"
        );
        assert!(!g.latency_dirty, "latency_dirty should be cleared after rebuild");
    }

    // ---- Sample-accurate automation ramp tests ----

    /// A plugin that captures the ramps received in ProcessContext.
    /// Used to verify that DawHost delivers correct ramp data.
    struct RampCapturingPlugin {
        channels: usize,
        gain: f32,
        captured_ramps: Vec<Vec<crate::plugin::ParameterRamp>>,
    }
    impl RampCapturingPlugin {
        fn new(channels: usize) -> Self {
            Self {
                channels,
                gain: 1.0,
                captured_ramps: Vec::new(),
            }
        }
    }
    impl Plugin for RampCapturingPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("RampCapture", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![crate::parameters::Parameter::new_float(
                "gain", "Gain", 1.0, 0.0, 4.0,
            )]
        }
        fn set_parameter(
            &mut self,
            id: crate::parameters::ParameterId,
            val: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            if id.0 == "gain" {
                if let crate::parameters::ParameterValue::Float(v) = val {
                    self.gain = v;
                    return Ok(());
                }
            }
            Err(format!("unknown parameter: {}", id.0))
        }
        fn get_parameter(
            &self,
            id: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            if id.0 == "gain" {
                Some(crate::parameters::ParameterValue::Float(self.gain))
            } else {
                None
            }
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            // Capture ramps for test inspection
            self.captured_ramps.push(ctx.ramps.clone());
            // Apply gain to pass audio through
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i * self.gain;
            }
            Ok(ctx.num_frames)
        }
    }

    #[test]
    fn test_automation_linear_ramp_values() {
        // Linear automation ramp from 0.0 to 1.0 over 4 blocks.
        // Each block should receive a ramp with interpolated start/end values.
        let mut g = DawHost::new(1, 48000);
        g.add_plugin(Box::new(RampCapturingPlugin::new(1))).unwrap();
        g.build().unwrap();

        let node_id = g.chain_nodes[0];
        let param_id = crate::parameters::ParameterId::from("gain");
        let num_frames = 48;

        // Linear curve: 4 values [0.0, 0.333, 0.667, 1.0]
        // With 4 values and total_frames = 4 * nf, each value corresponds to one block.
        g.set_automation(
            node_id,
            param_id,
            crate::automation::AutomationCurve::Linear {
                values: vec![0.0, 0.333, 0.667, 1.0],
            },
        );

        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames];

        // Process 4 blocks
        for _ in 0..4 {
            g.process(&input, &mut output).unwrap();
        }

        // Get captured ramps from the plugin
        let plugin = g.get_plugin(0).unwrap();
        let plugin: &RampCapturingPlugin =
            unsafe { &*(plugin as *const dyn Plugin as *const RampCapturingPlugin) };

        // Every block should have received ramps (values change each block)
        let non_empty_ramps: Vec<_> = plugin
            .captured_ramps
            .iter()
            .filter(|r| !r.is_empty())
            .collect();
        assert!(
            !non_empty_ramps.is_empty(),
            "Linear automation should produce ramps"
        );

        // Each ramp should have param_index=0 (gain is the first parameter)
        for ramps in &non_empty_ramps {
            assert_eq!(ramps[0].param_index, 0);
            // start and end should differ (linear interpolation changes across block)
            assert!(
                (ramps[0].start_value - ramps[0].end_value).abs() > 1e-6,
                "Ramp should have different start ({}) and end ({}) values",
                ramps[0].start_value,
                ramps[0].end_value
            );
        }
    }

    #[test]
    fn test_automation_step_no_ramp() {
        // Step automation: value is constant within each step.
        // No ramp should be generated within a step.
        let mut g = DawHost::new(1, 48000);
        g.add_plugin(Box::new(RampCapturingPlugin::new(1))).unwrap();
        g.build().unwrap();

        let node_id = g.chain_nodes[0];
        let param_id = crate::parameters::ParameterId::from("gain");
        let num_frames = 48;

        // Step: hold 0.5 for 2 blocks, then 0.8 for 2 blocks
        g.set_automation(
            node_id,
            param_id,
            crate::automation::AutomationCurve::Step {
                values: vec![0.5, 0.8],
                samples_per_step: num_frames * 2, // Each step lasts 2 blocks
            },
        );

        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames];

        // First block: within step 0 (0.5) — no ramp expected
        g.process(&input, &mut output).unwrap();

        let plugin = g.get_plugin(0).unwrap();
        let plugin: &RampCapturingPlugin =
            unsafe { &*(plugin as *const dyn Plugin as *const RampCapturingPlugin) };

        assert!(
            plugin.captured_ramps[0].is_empty(),
            "Step automation within a step should not produce ramps, got {:?}",
            plugin.captured_ramps[0]
        );
    }

    #[test]
    fn test_automation_ramp_empty_when_idle() {
        // No automation set — ramps should be empty.
        let mut g = DawHost::new(1, 48000);
        g.add_plugin(Box::new(RampCapturingPlugin::new(1))).unwrap();
        g.build().unwrap();

        let input = vec![1.0f32; 48];
        let mut output = vec![0.0f32; 48];

        g.process(&input, &mut output).unwrap();

        let plugin = g.get_plugin(0).unwrap();
        let plugin: &RampCapturingPlugin =
            unsafe { &*(plugin as *const dyn Plugin as *const RampCapturingPlugin) };

        assert!(
            plugin.captured_ramps[0].is_empty(),
            "No automation should produce empty ramps"
        );
    }

    // ---- Sidechain routing tests ----

    /// A plugin that receives N primary channels + N sidechain channels.
    /// Copies sidechain data to output for verification.
    struct SidechainReceiver {
        primary_ch: usize,
        /// Captured sidechain samples from the last process() call
        last_sidechain: Vec<f32>,
    }
    impl SidechainReceiver {
        fn new(primary_ch: usize) -> Self {
            Self {
                primary_ch,
                last_sidechain: Vec::new(),
            }
        }
    }
    impl Plugin for SidechainReceiver {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("SidechainReceiver", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.primary_ch * 2 // primary + sidechain
        }
        fn output_channels(&self) -> usize {
            self.primary_ch
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![]
        }
        fn set_parameter(
            &mut self,
            _: crate::parameters::ParameterId,
            _: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            Err("none".into())
        }
        fn get_parameter(
            &self,
            _: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            None
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            let in_ch = self.primary_ch * 2;
            let out_ch = self.primary_ch;
            self.last_sidechain.clear();
            for f in 0..ctx.num_frames {
                // Copy primary audio to output
                for ch in 0..out_ch {
                    output[f * out_ch + ch] = input[f * in_ch + ch];
                }
                // Capture sidechain channels
                for ch in 0..self.primary_ch {
                    self.last_sidechain
                        .push(input[f * in_ch + self.primary_ch + ch]);
                }
            }
            Ok(ctx.num_frames)
        }
    }

    #[test]
    fn test_sidechain_routing_basic() {
        // Graph: inp -> receiver (Audio), kick -> receiver (Sidechain)
        // inp sends primary audio (1.0), kick sends sidechain signal (0.5)
        // Receiver should see both in its extended input channels.
        let mut g = DawHost::new(1, 48000);

        let inp = g
            .add_node("inp".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let kick = g
            .add_node("kick".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let recv = g
            .add_node("recv".into(), Box::new(SidechainReceiver::new(1)))
            .unwrap();
        let out = g
            .add_node("out".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

        // Audio: inp -> recv, Sidechain: kick -> recv, recv -> out
        g.add_edge(GraphEdge::new(inp, recv)).unwrap();
        g.add_edge(GraphEdge::sidechain(kick, recv)).unwrap();
        g.add_edge(GraphEdge::new(recv, out)).unwrap();
        g.build().unwrap();

        // Verify primary_input_channels
        assert_eq!(
            g.primary_input_channels[recv], 1,
            "Receiver should have 1 primary channel"
        );

        let nf = 16;
        // inp gets 1.0, kick gets 0.5
        let mut input = vec![1.0f32; nf];
        let mut output = vec![0.0f32; nf];

        // We need to feed different signals to inp and kick.
        // Since inp is an input node and kick is also an input node (no predecessors),
        // both receive the same input buffer from process().
        // For this test, use a workaround: set kick's gain to 0.5 so it scales the input.
        // Actually, both inp and kick are input nodes — they both receive the same
        // `input` slice in process(). Let's use ScalerPlugin with factor=0.5 for kick.
        drop(g);

        let mut g = DawHost::new(1, 48000);
        let inp = g
            .add_node("inp".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let kick = g
            .add_node("kick".into(), Box::new(ScalerPlugin::new(1, 0.5)))
            .unwrap();
        let recv = g
            .add_node("recv".into(), Box::new(SidechainReceiver::new(1)))
            .unwrap();
        let out = g
            .add_node("out".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

        g.add_edge(GraphEdge::new(inp, recv)).unwrap();
        g.add_edge(GraphEdge::sidechain(kick, recv)).unwrap();
        g.add_edge(GraphEdge::new(recv, out)).unwrap();
        g.build().unwrap();

        input.fill(1.0);
        g.process(&input, &mut output).unwrap();

        // Get the SidechainReceiver's captured sidechain data
        let plugin: &SidechainReceiver = unsafe {
            &*(g.plugins[recv].as_ref().unwrap().as_ref() as *const dyn Plugin
                as *const SidechainReceiver)
        };

        // Primary audio should be 1.0 (from inp with factor 1.0)
        // Output should be the primary audio passed through
        for &s in &output {
            assert!(
                (s - 1.0).abs() < 1e-6,
                "Primary audio should be 1.0 in output, got {}",
                s
            );
        }

        // Sidechain should be 0.5 (from kick with factor 0.5)
        assert_eq!(plugin.last_sidechain.len(), nf);
        for &s in &plugin.last_sidechain {
            assert!(
                (s - 0.5).abs() < 1e-6,
                "Sidechain should be 0.5, got {}",
                s
            );
        }
    }

    #[test]
    fn test_sidechain_primary_channels_computation() {
        let mut g = DawHost::new(2, 48000);

        // Node with no sidechain: primary_ch == input_ch
        let a = g
            .add_node("a".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        // Node with sidechain: SidechainReceiver has input_ch=4, output_ch=2
        let b = g
            .add_node("b".into(), Box::new(SidechainReceiver::new(2)))
            .unwrap();

        g.add_edge(GraphEdge::new(a, b)).unwrap();
        // Add a sidechain source (another 2ch node)
        let sc = g
            .add_node("sc".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        g.add_edge(GraphEdge::sidechain(sc, b)).unwrap();
        g.build().unwrap();

        assert_eq!(g.primary_input_channels[a], 2, "Node a: no sidechain");
        assert_eq!(
            g.primary_input_channels[b], 2,
            "Node b: 4 input - 2 sidechain = 2 primary"
        );
    }
}
