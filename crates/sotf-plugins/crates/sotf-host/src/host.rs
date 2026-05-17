// ============================================================================
// Plugin Host Trait - Common interface for plugin hosts
// ============================================================================

use crate::automation::{ParameterAutomation, automation_utils};
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{Plugin, ProcessContext};
use arc_swap::ArcSwap;
use rayon::prelude::*;
use rtrb::{Consumer, Producer, RingBuffer};
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::AddAssign;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const PARAMETER_EVENT_QUEUE_CAPACITY: usize = 1024;
const GRAPH_MUTATION_QUEUE_CAPACITY: usize = 128;

// ============================================================================
// Node Buffer - Simple non-thread-safe buffer for audio data (zero-allocation)
// ============================================================================

trait AudioSample: Copy + Default + AddAssign + Send + Sync + 'static {
    fn scale_add(dst: &mut [Self], src: &[Self]);
}

impl AudioSample for f32 {
    fn scale_add(dst: &mut [Self], src: &[Self]) {
        crate::simd::scale_add_simd(dst, src, 1.0);
    }
}

impl AudioSample for f64 {
    fn scale_add(dst: &mut [Self], src: &[Self]) {
        for (dst, &src) in dst.iter_mut().zip(src.iter()) {
            *dst += src;
        }
    }
}

struct NodeBuffer<T: AudioSample> {
    data: Vec<T>,
    actual_len: usize,
    num_channels: usize,
}

impl<T: AudioSample> NodeBuffer<T> {
    fn new(num_frames: usize, num_channels: usize) -> Self {
        Self {
            data: vec![T::default(); num_frames * num_channels],
            actual_len: 0,
            num_channels,
        }
    }
    fn write(&mut self, data: &[T]) {
        ensure_len(&mut self.data, data.len());
        self.data[..data.len()].copy_from_slice(data);
        self.actual_len = data.len();
    }
    fn read(&self) -> &[T] {
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
        ensure_len(&mut self.data, required);
    }
}

fn ensure_len<T: AudioSample>(buffer: &mut Vec<T>, len: usize) {
    if buffer.len() < len {
        buffer.resize(len, T::default());
    }
}

struct ProcessBuffers<T: AudioSample> {
    node_buffers: Vec<Option<NodeBuffer<T>>>,
    scratch_input: Vec<T>,
    scratch_output: Vec<T>,
    merge_buffer: Vec<T>,
    channel_map_buffer: Vec<T>,
    /// Per-edge latency compensation delay buffers, indexed by `GraphEdge::id`.
    /// `None` means the edge is already aligned and needs no delay.
    compensation_delays: CompensationDelays<T>,
    /// Scratch buffer for frame-by-frame delay processing (avoids per-frame allocation).
    delay_scratch: Vec<T>,
    /// Per-node scratch buffers for parallel stage processing.
    /// Each entry: (scratch_input, scratch_output, merge_buffer).
    /// Only allocated for nodes in stages with 2+ nodes.
    #[allow(dead_code)]
    parallel_scratch: Vec<(Vec<T>, Vec<T>, Vec<T>)>,
    parallel_results: Vec<Result<usize, String>>,
}

struct DelayBuffer<T: AudioSample> {
    buffer: Vec<T>,
    pos: usize,
    delay: usize,
    channels: usize,
}

impl<T: AudioSample> DelayBuffer<T> {
    fn new(max_delay_samples: usize, channels: usize) -> Self {
        let max_delay = max_delay_samples.max(1);
        Self {
            buffer: vec![T::default(); max_delay * channels],
            pos: 0,
            delay: max_delay,
            channels,
        }
    }

    #[inline]
    fn process_frame(&mut self, input: &[T], output: &mut [T]) {
        debug_assert_eq!(input.len(), self.channels);
        debug_assert_eq!(output.len(), self.channels);

        let base = self.pos * self.channels;
        let buf_slice = &mut self.buffer[base..base + self.channels];
        output[..self.channels].copy_from_slice(buf_slice);
        buf_slice.copy_from_slice(&input[..self.channels]);
        self.pos = (self.pos + 1) % self.delay;
    }

    #[cfg(test)]
    fn delay(&self) -> usize {
        self.delay
    }
}

struct CompensationDelays<T: AudioSample> {
    #[allow(dead_code)]
    edge_keys: Vec<(NodeId, NodeId)>,
    delays: Vec<Option<DelayBuffer<T>>>,
}

impl<T: AudioSample> CompensationDelays<T> {
    fn new(edges: &[GraphEdge]) -> Self {
        Self {
            edge_keys: edges.iter().map(|e| (e.from_node, e.to_node)).collect(),
            delays: (0..edges.len()).map(|_| None).collect(),
        }
    }

    #[allow(dead_code)]
    fn empty() -> Self {
        Self {
            edge_keys: Vec::new(),
            delays: Vec::new(),
        }
    }

    fn set(&mut self, edge_id: usize, delay: DelayBuffer<T>) {
        if edge_id < self.delays.len() {
            self.delays[edge_id] = Some(delay);
        }
    }

    fn get_mut_edge(&mut self, edge_id: usize) -> Option<&mut DelayBuffer<T>> {
        self.delays.get_mut(edge_id).and_then(Option::as_mut)
    }

    #[cfg(test)]
    fn contains_key(&self, key: &(NodeId, NodeId)) -> bool {
        self.edge_keys
            .iter()
            .position(|candidate| candidate == key)
            .is_some_and(|idx| self.delays.get(idx).is_some_and(Option::is_some))
    }

    #[cfg(test)]
    fn get(&self, key: &(NodeId, NodeId)) -> Option<&DelayBuffer<T>> {
        let idx = self
            .edge_keys
            .iter()
            .position(|candidate| candidate == key)?;
        self.delays.get(idx)?.as_ref()
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.delays.iter().all(Option::is_none)
    }
}

// ============================================================================
// Buffer Safety Guard - ensures process_buffers are put back on early return
// ============================================================================

/// RAII guard that ensures `ProcessBuffers` are returned to the `DawHost`
/// even if processing exits early (via `?` or error return).
struct BufferGuard<'a, T: AudioSample> {
    slot: &'a mut Option<ProcessBuffers<T>>,
    buffers: Option<ProcessBuffers<T>>,
}

impl<'a, T: AudioSample> BufferGuard<'a, T> {
    fn take(slot: &'a mut Option<ProcessBuffers<T>>) -> Self {
        let buffers = slot.take();
        Self { slot, buffers }
    }

    fn get_mut(&mut self) -> &mut ProcessBuffers<T> {
        self.buffers
            .as_mut()
            .expect("ProcessBuffers missing from guard")
    }
}

impl<'a, T: AudioSample> Drop for BufferGuard<'a, T> {
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
    fn process_f64(&mut self, input: &[f64], output: &mut [f64]) -> Result<usize, String>;
    fn reset(&mut self);
    fn total_latency_samples(&self) -> usize;
    /// RT diagnostics: collect cache contention stats from all analyzer plugins.
    /// Returns Vec of (plugin_index, contention_count, update_count).
    fn take_analyzer_contention_stats(&mut self) -> Vec<(usize, u64, u64)> {
        Vec::new()
    }
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
    id: usize,
}

impl GraphEdge {
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
            edge_type: EdgeType::Audio,
            id: usize::MAX,
        }
    }
    pub fn with_channels(from: NodeId, to: NodeId, channels: Vec<usize>) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: Some(channels),
            edge_type: EdgeType::Audio,
            id: usize::MAX,
        }
    }
    pub fn sidechain(from: NodeId, to: NodeId) -> Self {
        Self {
            from_node: from,
            to_node: to,
            channel_map: None,
            edge_type: EdgeType::Sidechain,
            id: usize::MAX,
        }
    }

    pub fn id(&self) -> usize {
        self.id
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

struct AutomationSlot {
    node_id: NodeId,
    param_id: ParameterId,
    automation: ParameterAutomation,
}

struct ParameterEvent {
    node_id: NodeId,
    param_id: ParameterId,
    value: ParameterValue,
    sample_offset: usize,
}

impl ParameterEvent {
    fn new(
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Self {
        Self {
            node_id,
            param_id,
            value,
            sample_offset,
        }
    }
}

/// Single-producer handle for lock-free parameter updates into `DawHost`.
///
/// Move this to the control/UI thread and call `queue_node_parameter()` there;
/// the host drains events during `process()`.
pub struct ParameterEventSender {
    producer: Producer<ParameterEvent>,
    dropped_events: u64,
}

impl ParameterEventSender {
    /// Queue a parameter update at the start of the next processed block.
    pub fn queue_node_parameter(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), String> {
        self.queue_node_parameter_at(node_id, param_id, value, 0)
    }

    /// Queue a parameter update for `sample_offset` frames into the next block.
    ///
    /// Offsets beyond the current block are applied after that block, before
    /// the next one begins.
    pub fn queue_node_parameter_at(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        let event = ParameterEvent::new(node_id, param_id, value, sample_offset);
        self.producer.push(event).map_err(|err| {
            self.dropped_events = self.dropped_events.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: external parameter event queue full; dropped {} events",
                self.dropped_events
            );
            format!("parameter event queue full: {err:?}")
        })
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

enum GraphMutation {
    AddNode {
        id: NodeId,
        name: String,
        plugin: Box<dyn Plugin>,
    },
    AddPlugin {
        id: NodeId,
        plugin: Box<dyn Plugin>,
    },
    AddEdge(GraphEdge),
    RemovePlugin {
        index: usize,
    },
}

/// Single-producer handle for lock-free graph mutations into `DawHost`.
///
/// Move this to the control/UI thread and queue graph changes there.
///
/// The queue itself is lock-free, but applying graph mutations may initialize
/// plugins and rebuild host buffers. Use it at graph sync points, not inside a
/// hard real-time callback that cannot tolerate rebuild work.
pub struct GraphMutationSender {
    producer: Producer<GraphMutation>,
    next_node_id: Arc<AtomicUsize>,
    dropped_mutations: u64,
}

impl GraphMutationSender {
    /// Reserve a node id and queue a named node insertion.
    ///
    /// The returned `NodeId` can be used when queueing edges before the audio
    /// side applies the mutation.
    pub fn queue_add_node(
        &mut self,
        name: String,
        plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let id = self.next_node_id.fetch_add(1, Ordering::AcqRel);
        let mutation = GraphMutation::AddNode { id, name, plugin };
        self.push_mutation(mutation).map(|()| id)
    }

    /// Reserve a node id and queue a plugin append for the linear chain host API.
    pub fn queue_add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<NodeId, String> {
        let id = self.next_node_id.fetch_add(1, Ordering::AcqRel);
        self.push_mutation(GraphMutation::AddPlugin { id, plugin })
            .map(|()| id)
    }

    /// Queue an edge insertion between existing or pre-reserved nodes.
    pub fn queue_add_edge(&mut self, edge: GraphEdge) -> Result<(), String> {
        self.push_mutation(GraphMutation::AddEdge(edge))
    }

    /// Queue a plugin removal by linear chain index.
    pub fn queue_remove_plugin(&mut self, index: usize) -> Result<(), String> {
        self.push_mutation(GraphMutation::RemovePlugin { index })
    }

    /// Number of graph mutations dropped because the RT queue was full.
    pub fn dropped_mutations(&self) -> u64 {
        self.dropped_mutations
    }

    fn push_mutation(&mut self, mutation: GraphMutation) -> Result<(), String> {
        self.producer.push(mutation).map_err(|err| {
            self.dropped_mutations = self.dropped_mutations.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: graph mutation queue full; dropped {} mutations",
                self.dropped_mutations
            );
            format!("graph mutation queue full: {err:?}")
        })
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
    process_buffers: Option<ProcessBuffers<f32>>,
    process_buffers_f64: Option<ProcessBuffers<f64>>,
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
    /// Flat cache of bypass state for O(1) audio-thread lookup. **Mirror** of
    /// the authoritative `GraphNode::bypassed`; rebuilt in `build()` and kept
    /// in sync exclusively through `Self::set_bypass_state` so the two flags
    /// can never disagree.
    bypassed: Vec<bool>,
    /// Per-node cumulative latency from graph inputs, computed during build().
    /// Used to calculate compensation delays at merge points.
    node_latency_from_input: Vec<usize>,
    /// Parameter automation state. Key = (NodeId, ParameterId).
    /// Evaluated before each processing stage.
    automation: Vec<AutomationSlot>,
    /// Control-thread lookup for automation slots. The audio path iterates
    /// `automation` by index and never hashes `(NodeId, ParameterId)`.
    automation_index: HashMap<(NodeId, ParameterId), usize>,
    /// Current playback position in samples, advanced each process() call.
    playback_position: usize,
    /// Pre-allocated scratch buffer for automation updates (avoids per-process() heap allocation).
    automation_scratch: Vec<(usize, f32)>,
    /// Current immutable topology snapshot, published with ArcSwap after build.
    topology: Arc<ArcSwap<GraphTopology>>,
    f64_input_scratch: Vec<f32>,
    f64_output_scratch: Vec<f32>,
    f64_chain_scratch: Vec<f64>,
    f64_chain_scratch_alt: Vec<f64>,
    parameter_event_tx: Option<Producer<ParameterEvent>>,
    parameter_event_rx: Consumer<ParameterEvent>,
    parameter_event_scratch: Vec<ParameterEvent>,
    dropped_parameter_events: u64,
    graph_mutation_tx: Option<Producer<GraphMutation>>,
    graph_mutation_rx: Consumer<GraphMutation>,
    graph_next_node_id: Arc<AtomicUsize>,
    dropped_graph_mutations: u64,
}

impl DawHost {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let (parameter_event_tx, parameter_event_rx) =
            RingBuffer::new(PARAMETER_EVENT_QUEUE_CAPACITY);
        let (graph_mutation_tx, graph_mutation_rx) = RingBuffer::new(GRAPH_MUTATION_QUEUE_CAPACITY);
        let graph_next_node_id = Arc::new(AtomicUsize::new(0));
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
            process_buffers_f64: None,
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
            automation: Vec::new(),
            automation_index: HashMap::new(),
            playback_position: 0,
            automation_scratch: Vec::new(),
            topology: Arc::new(ArcSwap::from_pointee(GraphTopology::empty())),
            f64_input_scratch: Vec::new(),
            f64_output_scratch: Vec::new(),
            f64_chain_scratch: Vec::new(),
            f64_chain_scratch_alt: Vec::new(),
            parameter_event_tx: Some(parameter_event_tx),
            parameter_event_rx,
            parameter_event_scratch: Vec::with_capacity(PARAMETER_EVENT_QUEUE_CAPACITY),
            dropped_parameter_events: 0,
            graph_mutation_tx: Some(graph_mutation_tx),
            graph_mutation_rx,
            graph_next_node_id,
            dropped_graph_mutations: 0,
        }
    }
    pub fn new_default(sr: u32) -> Self {
        Self::new(2, sr)
    }
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
        let key = (node_id, param_id.clone());
        if let Some(&idx) = self.automation_index.get(&key) {
            self.automation[idx].automation = auto;
            return;
        }
        let idx = self.automation.len();
        self.automation.push(AutomationSlot {
            node_id,
            param_id,
            automation: auto,
        });
        self.automation_index.insert(key, idx);
    }

    /// Remove automation for a specific parameter on a node.
    pub fn clear_automation(&mut self, node_id: NodeId, param_id: &ParameterId) {
        let key = (node_id, param_id.clone());
        let Some(idx) = self.automation_index.remove(&key) else {
            return;
        };
        self.automation.swap_remove(idx);
        if idx < self.automation.len() {
            let moved = &self.automation[idx];
            self.automation_index
                .insert((moved.node_id, moved.param_id.clone()), idx);
        }
    }

    /// Remove all automation.
    pub fn clear_all_automation(&mut self) {
        self.automation.clear();
        self.automation_index.clear();
    }

    /// Reset playback position to 0.
    pub fn reset_playback_position(&mut self) {
        self.playback_position = 0;
        for slot in &mut self.automation {
            slot.automation.position = 0;
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

    /// Returns a lock-free handle to the current immutable topology snapshot.
    pub fn topology_handle(&self) -> Arc<ArcSwap<GraphTopology>> {
        Arc::clone(&self.topology)
    }

    /// Atomically load the current topology snapshot.
    pub fn current_topology(&self) -> Arc<GraphTopology> {
        self.topology.load_full()
    }

    fn publish_topology_snapshot(&self) {
        self.topology.store(Arc::new(self.topology_snapshot()));
    }

    fn reserve_node_id(&mut self) -> NodeId {
        let externally_reserved = self.graph_next_node_id.load(Ordering::Acquire);
        if externally_reserved > self.next_node_id {
            self.next_node_id = externally_reserved;
        }
        let id = self.next_node_id;
        self.next_node_id += 1;
        self.graph_next_node_id
            .store(self.next_node_id, Ordering::Release);
        id
    }

    pub fn add_node(&mut self, name: String, plugin: Box<dyn Plugin>) -> Result<NodeId, String> {
        let id = self.reserve_node_id();
        self.add_node_with_id(id, name, plugin)?;
        Ok(id)
    }

    fn add_node_with_id(
        &mut self,
        id: NodeId,
        name: String,
        mut plugin: Box<dyn Plugin>,
    ) -> Result<(), String> {
        if self.nodes.contains_key(&id) {
            return Err(format!("Node {id} already exists"));
        }
        if id >= self.next_node_id {
            self.next_node_id = id + 1;
            self.graph_next_node_id
                .store(self.next_node_id, Ordering::Release);
        }
        plugin = Self::auto_oversample_plugin(plugin)?;
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
        self.built = false;
        self.cached_latency = None;
        Ok(())
    }

    pub fn add_edge(&mut self, mut edge: GraphEdge) -> Result<(), String> {
        if !self.nodes.contains_key(&edge.from_node) || !self.nodes.contains_key(&edge.to_node) {
            return Err("Node not found".into());
        }
        if edge.from_node == edge.to_node {
            return Err("Self-loop".into());
        }
        edge.id = self.edges.len();
        self.edges.push(edge);
        self.built = false;
        self.cached_latency = None;
        Ok(())
    }

    fn auto_oversample_plugin(plugin: Box<dyn Plugin>) -> Result<Box<dyn Plugin>, String> {
        let Some(factor) = plugin.preferred_oversampling() else {
            return Ok(plugin);
        };
        if factor != 2 && factor != 4 {
            return Err(format!(
                "Invalid preferred oversampling factor {factor}: expected 2 or 4"
            ));
        }
        if plugin.input_channels() != plugin.output_channels() {
            crate::rate_limited_log!(
                warn,
                5,
                "host: plugin '{}' requested {}x oversampling but has mismatched I/O channels ({} -> {}); leaving unwrapped",
                plugin.info().name,
                factor,
                plugin.input_channels(),
                plugin.output_channels()
            );
            return Ok(plugin);
        }
        Ok(Box::new(crate::oversampling::AutoOversampledPlugin::new(
            plugin, factor,
        )?))
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
        let mut node_buffers_f64 = (0..num_slots).map(|_| None).collect::<Vec<_>>();
        for (&id, node) in &self.nodes {
            node_buffers[id] = Some(NodeBuffer::<f32>::new(4096, node.output_channels()));
            node_buffers_f64[id] = Some(NodeBuffer::<f64>::new(4096, node.output_channels()));
        }
        // Cache per-node bypass flags before computing compensation delays
        // (compensation needs to know which nodes are bypassed for latency calculation)
        self.bypassed = vec![false; num_slots];
        for (&id, node) in &self.nodes {
            self.bypassed[id] = node.bypassed;
        }
        // Compute per-node cumulative latency from inputs and compensation delays
        let compensation_delays = self.compute_compensation_delays::<f32>(num_slots);
        let compensation_delays_f64 = self.compute_compensation_delays::<f64>(num_slots);

        self.process_buffers = Some(ProcessBuffers {
            node_buffers,
            scratch_input: vec![0.0f32; 4096 * 32],
            scratch_output: vec![0.0f32; 4096 * 32],
            merge_buffer: vec![0.0f32; 4096 * 32],
            channel_map_buffer: vec![0.0f32; 4096 * 32],
            compensation_delays,
            delay_scratch: vec![0.0f32; 4096 * 32],
            parallel_scratch: (0..num_slots)
                .map(|_| (Vec::new(), Vec::new(), Vec::new()))
                .collect(),
            parallel_results: Vec::with_capacity(
                self.stages
                    .iter()
                    .map(|stage| stage.nodes.len())
                    .max()
                    .unwrap_or(0),
            ),
        });
        self.process_buffers_f64 = Some(ProcessBuffers {
            node_buffers: node_buffers_f64,
            scratch_input: vec![0.0f64; 4096 * 32],
            scratch_output: vec![0.0f64; 4096 * 32],
            merge_buffer: vec![0.0f64; 4096 * 32],
            channel_map_buffer: vec![0.0f64; 4096 * 32],
            compensation_delays: compensation_delays_f64,
            delay_scratch: vec![0.0f64; 4096 * 32],
            parallel_scratch: Vec::new(),
            parallel_results: Vec::new(),
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
        self.publish_topology_snapshot();
        Ok(())
    }

    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let id = self.reserve_node_id();
        self.add_plugin_with_id(id, plugin).map(|_| ())
    }

    fn add_plugin_with_id(
        &mut self,
        id: NodeId,
        plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let expected = if self.chain_nodes.is_empty() {
            self.initial_input_channels
        } else {
            self.nodes[self.chain_nodes.last().unwrap()].output_channels()
        };
        if plugin.input_channels() != expected {
            return Err("mismatch".into());
        }
        let name = format!("plugin_{id}");
        self.add_node_with_id(id, name, plugin)?;
        if let Some(&prev) = self.chain_nodes.last() {
            self.add_edge(GraphEdge::new(prev, id))?;
        }
        self.chain_nodes.push(id);
        self.built = false;
        Ok(id)
    }

    pub fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String> {
        if index >= self.chain_nodes.len() {
            return Err("oob".into());
        }
        let id = self.chain_nodes.remove(index);
        self.edges.retain(|e| e.from_node != id && e.to_node != id);
        self.renumber_edges();
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

    fn renumber_edges(&mut self) {
        for (idx, edge) in self.edges.iter_mut().enumerate() {
            edge.id = idx;
        }
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
        self.queue_node_parameter(nid, super::parameters::ParameterId(id.to_string()), val)
    }

    /// Queue a parameter change for audio-thread application at `sample_offset`
    /// frames into the next `process()` call.
    pub fn set_plugin_parameter_at(
        &mut self,
        index: usize,
        id: &str,
        val: super::parameters::ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.queue_node_parameter_at(
            nid,
            super::parameters::ParameterId(id.to_string()),
            val,
            sample_offset,
        )
    }

    /// Queue a parameter change for audio-thread application at the start of
    /// the next `process()` call.
    pub fn queue_node_parameter(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
    ) -> Result<(), String> {
        self.queue_node_parameter_at(node_id, param_id, value, 0)
    }

    /// Queue a parameter change for audio-thread application at `sample_offset`
    /// frames into the next `process()` call.
    pub fn queue_node_parameter_at(
        &mut self,
        node_id: NodeId,
        param_id: ParameterId,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        if !self.nodes.contains_key(&node_id) {
            return Err("Node not found".into());
        }

        let event = ParameterEvent::new(node_id, param_id, value, sample_offset);
        let producer = self.parameter_event_tx.as_mut().ok_or_else(|| {
            "parameter event sender has been taken; use the returned ParameterEventSender"
                .to_string()
        })?;
        producer.push(event).map_err(|err| {
            self.dropped_parameter_events = self.dropped_parameter_events.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: parameter event queue full; dropped {} events",
                self.dropped_parameter_events
            );
            format!("parameter event queue full: {err:?}")
        })
    }

    /// Move the parameter-event producer out of the host.
    ///
    /// After this is called, use the returned `ParameterEventSender` from the
    /// control/UI side. `DawHost` keeps the consumer and continues draining
    /// events in `process()`.
    pub fn take_parameter_event_sender(&mut self) -> Option<ParameterEventSender> {
        self.parameter_event_tx
            .take()
            .map(|producer| ParameterEventSender {
                producer,
                dropped_events: 0,
            })
    }

    /// Move the graph-mutation producer out of the host.
    ///
    /// After this is called, use the returned `GraphMutationSender` from the
    /// control/UI side. `DawHost` keeps the consumer, applies queued graph
    /// changes before processing, and publishes the rebuilt topology snapshot.
    pub fn take_graph_mutation_sender(&mut self) -> Option<GraphMutationSender> {
        self.graph_mutation_tx
            .take()
            .map(|producer| GraphMutationSender {
                producer,
                next_node_id: Arc::clone(&self.graph_next_node_id),
                dropped_mutations: 0,
            })
    }

    /// Apply a parameter immediately on the calling thread.
    ///
    /// This is intended for offline setup, tests, and migration code. Real-time
    /// control paths should use `set_plugin_parameter()` / `queue_node_parameter()`.
    pub fn set_plugin_parameter_immediate(
        &mut self,
        index: usize,
        id: &str,
        val: super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.apply_parameter_event(ParameterEvent {
            node_id: nid,
            param_id: super::parameters::ParameterId(id.to_string()),
            value: val,
            sample_offset: 0,
        })
    }

    fn drain_parameter_events_into(&mut self, events: &mut Vec<ParameterEvent>) {
        events.clear();
        while let Ok(event) = self.parameter_event_rx.pop() {
            events.push(event);
        }
    }

    fn apply_parameter_event(&mut self, event: ParameterEvent) -> Result<(), String> {
        let ParameterEvent {
            node_id,
            param_id,
            value,
            sample_offset: _,
        } = event;
        let node = self
            .nodes
            .get(&node_id)
            .ok_or_else(|| format!("Node {node_id} not found"))?;
        let plugin = self
            .plugins
            .get_mut(node_id)
            .and_then(Option::as_mut)
            .ok_or_else(|| format!("Plugin for node {node_id} not found"))?;
        plugin.set_parameter(param_id, value).map_err(|err| {
            crate::rate_limited_log!(
                warn,
                5,
                "host: queued parameter event failed for node {} '{}': {err}",
                node_id,
                node.name
            );
            err
        })
    }

    /// Number of parameter events dropped because the RT queue was full.
    pub fn dropped_parameter_events(&self) -> u64 {
        self.dropped_parameter_events
    }

    /// Queue a graph mutation through the host-owned producer.
    ///
    /// This is useful before handing the producer to another thread. After
    /// `take_graph_mutation_sender()` is called, use that sender instead.
    fn queue_graph_mutation(&mut self, mutation: GraphMutation) -> Result<(), String> {
        let producer = self.graph_mutation_tx.as_mut().ok_or_else(|| {
            "graph mutation sender has been taken; use the returned GraphMutationSender".to_string()
        })?;
        producer.push(mutation).map_err(|err| {
            self.dropped_graph_mutations = self.dropped_graph_mutations.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: graph mutation queue full; dropped {} mutations",
                self.dropped_graph_mutations
            );
            format!("graph mutation queue full: {err:?}")
        })
    }

    /// Queue a linear-chain plugin append for audio-thread application.
    pub fn queue_add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<NodeId, String> {
        let id = self.reserve_node_id();
        self.queue_graph_mutation(GraphMutation::AddPlugin { id, plugin })
            .map(|()| id)
    }

    /// Reserve a node id and queue a named node insertion for audio-thread application.
    pub fn queue_add_node(
        &mut self,
        name: String,
        plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let id = self.reserve_node_id();
        self.queue_graph_mutation(GraphMutation::AddNode { id, name, plugin })
            .map(|()| id)
    }

    /// Queue an edge insertion for audio-thread application.
    pub fn queue_add_edge(&mut self, edge: GraphEdge) -> Result<(), String> {
        self.queue_graph_mutation(GraphMutation::AddEdge(edge))
    }

    /// Queue a linear-chain plugin removal for audio-thread application.
    pub fn queue_remove_plugin(&mut self, index: usize) -> Result<(), String> {
        self.queue_graph_mutation(GraphMutation::RemovePlugin { index })
    }

    /// Number of graph mutations dropped because the RT queue was full.
    pub fn dropped_graph_mutations(&self) -> u64 {
        self.dropped_graph_mutations
    }

    fn drain_graph_mutations(&mut self) -> Result<(), String> {
        while let Ok(mutation) = self.graph_mutation_rx.pop() {
            self.apply_graph_mutation(mutation)?;
        }
        Ok(())
    }

    fn apply_graph_mutation(&mut self, mutation: GraphMutation) -> Result<(), String> {
        match mutation {
            GraphMutation::AddNode { id, name, plugin } => self.add_node_with_id(id, name, plugin),
            GraphMutation::AddPlugin { id, plugin } => {
                self.add_plugin_with_id(id, plugin).map(|_| ())
            }
            GraphMutation::AddEdge(edge) => self.add_edge(edge),
            GraphMutation::RemovePlugin { index } => self.remove_plugin(index).map(|_| ()),
        }
    }

    /// Bypass a node so its plugin is skipped during processing.
    /// When bypassed, input is passed directly to output.
    /// Only works for nodes with matching input/output channel counts.
    pub fn bypass_node(&mut self, id: NodeId) -> Result<(), String> {
        {
            let node = self.nodes.get(&id).ok_or("Node not found")?;
            if node.input_channels != node.output_channels {
                return Err(format!(
                    "Cannot bypass node '{}': input channels ({}) != output channels ({})",
                    node.name, node.input_channels, node.output_channels
                ));
            }
        }
        self.set_bypass_state(id, true);
        Ok(())
    }

    /// Unbypass a node so its plugin resumes processing.
    pub fn unbypass_node(&mut self, id: NodeId) -> Result<(), String> {
        if !self.nodes.contains_key(&id) {
            return Err("Node not found".into());
        }
        self.set_bypass_state(id, false);
        Ok(())
    }

    /// Single write-through helper that keeps `GraphNode::bypassed` (the
    /// authoritative model) and the flat `self.bypassed[id]` cache in sync.
    /// All bypass mutations must funnel through here so the two views can
    /// never diverge.
    fn set_bypass_state(&mut self, id: NodeId, bypassed: bool) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.bypassed = bypassed;
        }
        if id < self.bypassed.len() {
            self.bypassed[id] = bypassed;
        }
        self.cached_latency = None;
        self.built = false;
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
        self.drain_graph_mutations()?;
        if !self.built {
            self.build()?;
        }
        let mut events = std::mem::take(&mut self.parameter_event_scratch);
        self.drain_parameter_events_into(&mut events);
        let result = self.process_with_parameter_events(input, output, &mut events);
        self.parameter_event_scratch = events;
        result
    }

    fn process_with_parameter_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        events: &mut Vec<ParameterEvent>,
    ) -> Result<usize, String> {
        if events.is_empty() {
            return self.process_block_without_parameter_events(input, output);
        }

        if events.iter().any(|event| event.sample_offset > 0)
            && self.can_split_parameter_event_block(input, output)
        {
            return self.process_split_parameter_events(input, output, events);
        }

        for event in events.drain(..) {
            self.apply_parameter_event(event)?;
        }
        self.process_block_without_parameter_events(input, output)
    }

    fn can_split_parameter_event_block(&self, input: &[f32], output: &[f32]) -> bool {
        if !self.automation.is_empty()
            || self.has_variable_frame_plugin
            || !self.cached_frames_identity
            || !self.cached_rate_identity
        {
            return false;
        }
        let input_channels = self.input_channels();
        if input_channels == 0 || !input.len().is_multiple_of(input_channels) {
            return false;
        }
        let frames = input.len() / input_channels;
        output.len() >= frames * self.output_channels()
    }

    fn process_split_parameter_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        events: &mut Vec<ParameterEvent>,
    ) -> Result<usize, String> {
        let input_channels = self.input_channels();
        let output_channels = self.output_channels();
        let frames = input.len() / input_channels;
        events.sort_by_key(|event| event.sample_offset);
        events.reverse();

        let mut frame_cursor = 0;
        let mut processed_frames = 0;

        while events.last().is_some_and(|event| event.sample_offset == 0) {
            let event = events.pop().unwrap();
            self.apply_parameter_event(event)?;
        }

        while frame_cursor < frames {
            let next_event_frame = events
                .last()
                .map_or(frames, |event| event.sample_offset.min(frames));

            if next_event_frame > frame_cursor {
                let in_start = frame_cursor * input_channels;
                let in_end = next_event_frame * input_channels;
                let out_start = frame_cursor * output_channels;
                let out_end = next_event_frame * output_channels;
                let segment_frames = self.process_block_without_parameter_events(
                    &input[in_start..in_end],
                    &mut output[out_start..out_end],
                )?;
                processed_frames += segment_frames;
                frame_cursor = next_event_frame;
            }

            while events
                .last()
                .is_some_and(|event| event.sample_offset <= frame_cursor)
            {
                let event = events.pop().unwrap();
                self.apply_parameter_event(event)?;
            }
        }

        while let Some(event) = events.pop() {
            self.apply_parameter_event(event)?;
        }

        Ok(processed_frames)
    }

    fn process_block_without_parameter_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<usize, String> {
        if self.nodes.is_empty() {
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels());
        }
        let nf = input.len() / self.input_channels();
        let max_of = self.output_frames_for_input(nf);
        let out_ch = self.output_channels();
        self.apply_automation_for_block(nf);

        // Use BufferGuard to guarantee process_buffers are returned even on early ?-return
        let mut guard = BufferGuard::take(&mut self.process_buffers);
        let bufs = guard.get_mut();
        for nb in bufs.node_buffers.iter_mut().flatten() {
            nb.ensure_capacity(nf.max(max_of));
            nb.clear();
        }
        ensure_len(&mut bufs.scratch_input, input.len());
        let mut cf = nf;

        for stage in &self.stages {
            if let Some(parallel_result) = Self::process_stage_parallel(
                self.parallel_enabled,
                stage,
                input,
                self.sample_rate,
                cf,
                &mut self.plugins,
                &self.nodes,
                &self.predecessors,
                &self.is_input_node,
                &self.bypassed,
                bufs,
            ) {
                cf = parallel_result?;
                continue;
            }

            let mut stage_cf: Option<usize> = None;
            for &nid in &stage.nodes {
                let node = &self.nodes[&nid];
                let in_len = if self.is_input_node[nid] {
                    ensure_len(&mut bufs.scratch_input, input.len());
                    bufs.scratch_input[..input.len()].copy_from_slice(input);
                    input.len()
                } else {
                    let il = Self::merge_inputs_into(
                        node,
                        &self.predecessors,
                        &bufs.node_buffers,
                        cf,
                        &mut bufs.merge_buffer,
                        &mut bufs.channel_map_buffer,
                        &mut bufs.delay_scratch,
                        &mut bufs.compensation_delays,
                    )
                    .map_err(|e| {
                        crate::rate_limited_log!(
                            error,
                            5,
                            "host: merge_inputs_into failed for node {} '{}': {e}",
                            nid,
                            node.name
                        );
                        e
                    })?;
                    ensure_len(&mut bufs.scratch_input, il);
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
                    let context = ProcessContext {
                        sample_rate: self.sample_rate,
                        num_frames: cf,
                    };
                    let mof = p.output_frames_for_input(cf);
                    let ol = mof * node.output_channels();
                    let needs_extended_in_place_buffer = node.input_channels()
                        > node.output_channels()
                        && self.predecessors[nid]
                            .iter()
                            .any(|e| e.edge_type == EdgeType::Sidechain);
                    let process_output_len = if needs_extended_in_place_buffer {
                        ol.max(in_len)
                    } else {
                        ol
                    };
                    ensure_len(&mut bufs.scratch_output, process_output_len);
                    let out_frames = p
                        .process(
                            &bufs.scratch_input[..in_len],
                            &mut bufs.scratch_output[..process_output_len],
                            &context,
                        )
                        .map_err(|e| {
                            crate::rate_limited_log!(
                                error,
                                5,
                                "host: plugin '{}' (node {}) process failed: {e}",
                                node.name,
                                nid
                            );
                            e
                        })?;
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
        Self::collect_output_from_buffers(&self.output_nodes, &bufs.node_buffers, output, cf)
            .map_err(|e| {
                crate::rate_limited_log!(error, 5, "host: collect_output_from_buffers failed: {e}");
                e
            })?;
        if cf < nf && self.has_variable_frame_plugin {
            output[cf * out_ch..].fill(0.0);
            cf = nf;
        }
        // Advance playback position for automation
        self.playback_position += nf;

        // BufferGuard's Drop impl returns bufs to self.process_buffers when
        // `guard` falls out of scope here.
        Ok(cf)
    }

    fn apply_automation_for_block(&mut self, nf: usize) {
        // Apply automation: evaluate curves at current position and set parameters.
        // `eval_curve` interprets (sample, num_frames) as a position within a window,
        // so we use each automation's relative position and advance it by nf each call.
        if self.automation.is_empty() {
            return;
        }

        self.automation_scratch.clear();
        for (idx, slot) in self.automation.iter().enumerate() {
            let auto = &slot.automation;
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
                let pos = auto.position.min(total_frames.saturating_sub(1));
                let val = automation_utils::eval_curve(curve, pos, total_frames);
                self.automation_scratch.push((idx, val));
            }
        }
        for i in 0..self.automation_scratch.len() {
            let (idx, val) = self.automation_scratch[i];
            let slot = &mut self.automation[idx];
            if let Some(p) = self.plugins[slot.node_id].as_mut() {
                let _ = p.set_parameter(slot.param_id.clone(), ParameterValue::Float(val));
            }
            slot.automation.last_value = val;
            slot.automation.position += nf;
        }
    }

    /// Process an f64 buffer through the host.
    ///
    /// Native f64 simple-chain and DAG paths are used when every active plugin
    /// declares `supports_f64()`. Graphs containing f32-only plugins use a
    /// scratch-backed f32 compatibility bridge.
    pub fn process_f64(&mut self, input: &[f64], output: &mut [f64]) -> Result<usize, String> {
        self.drain_graph_mutations()?;
        if !self.built {
            self.build()?;
        }
        let mut events = std::mem::take(&mut self.parameter_event_scratch);
        self.drain_parameter_events_into(&mut events);
        let result = self.process_f64_with_parameter_events(input, output, &mut events);
        self.parameter_event_scratch = events;
        result
    }

    fn process_f64_with_parameter_events(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        events: &mut Vec<ParameterEvent>,
    ) -> Result<usize, String> {
        if events.is_empty() {
            return self.process_f64_without_parameter_events(input, output);
        }

        if events.iter().any(|event| event.sample_offset > 0)
            && self.can_split_parameter_event_block_f64(input, output)
        {
            return self.process_f64_split_parameter_events(input, output, events);
        }

        for event in events.drain(..) {
            self.apply_parameter_event(event)?;
        }
        self.process_f64_without_parameter_events(input, output)
    }

    fn can_split_parameter_event_block_f64(&self, input: &[f64], output: &[f64]) -> bool {
        if !self.automation.is_empty() || !self.cached_frames_identity || !self.cached_rate_identity
        {
            return false;
        }
        let input_channels = self.input_channels();
        if input_channels == 0 || !input.len().is_multiple_of(input_channels) {
            return false;
        }
        let frames = input.len() / input_channels;
        output.len() >= frames * self.output_channels()
    }

    fn process_f64_split_parameter_events(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        events: &mut Vec<ParameterEvent>,
    ) -> Result<usize, String> {
        let input_channels = self.input_channels();
        let output_channels = self.output_channels();
        let frames = input.len() / input_channels;
        events.sort_by_key(|event| event.sample_offset);
        events.reverse();

        let mut frame_cursor = 0;
        let mut processed_frames = 0;

        while events.last().is_some_and(|event| event.sample_offset == 0) {
            let event = events.pop().unwrap();
            self.apply_parameter_event(event)?;
        }

        while frame_cursor < frames {
            let next_event_frame = events
                .last()
                .map_or(frames, |event| event.sample_offset.min(frames));

            if next_event_frame > frame_cursor {
                let in_start = frame_cursor * input_channels;
                let in_end = next_event_frame * input_channels;
                let out_start = frame_cursor * output_channels;
                let out_end = next_event_frame * output_channels;
                let segment_frames = self.process_f64_without_parameter_events(
                    &input[in_start..in_end],
                    &mut output[out_start..out_end],
                )?;
                processed_frames += segment_frames;
                frame_cursor = next_event_frame;
            }

            while events
                .last()
                .is_some_and(|event| event.sample_offset <= frame_cursor)
            {
                let event = events.pop().unwrap();
                self.apply_parameter_event(event)?;
            }
        }

        while let Some(event) = events.pop() {
            self.apply_parameter_event(event)?;
        }

        Ok(processed_frames)
    }

    fn process_f64_without_parameter_events(
        &mut self,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<usize, String> {
        if self.nodes.is_empty() {
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels());
        }
        if self.can_process_f64_chain_native() {
            return self.process_f64_chain_native(input, output);
        }
        if self.can_process_f64_graph_native() {
            return self.process_f64_graph_native(input, output);
        }
        let mut input_scratch = std::mem::take(&mut self.f64_input_scratch);
        let mut output_scratch = std::mem::take(&mut self.f64_output_scratch);

        ensure_len(&mut input_scratch, input.len());
        for (dst, &src) in input_scratch[..input.len()].iter_mut().zip(input.iter()) {
            *dst = src as f32;
        }

        ensure_len(&mut output_scratch, output.len());
        let in_len = input.len();
        let out_len = output.len();
        let result = self.process_block_without_parameter_events(
            &input_scratch[..in_len],
            &mut output_scratch[..out_len],
        );
        let frames = match result {
            Ok(frames) => frames,
            Err(err) => {
                self.f64_input_scratch = input_scratch;
                self.f64_output_scratch = output_scratch;
                return Err(err);
            }
        };
        for (dst, &src) in output.iter_mut().zip(output_scratch[..out_len].iter()) {
            *dst = src as f64;
        }

        self.f64_input_scratch = input_scratch;
        self.f64_output_scratch = output_scratch;
        Ok(frames)
    }

    fn can_process_f64_graph_native(&self) -> bool {
        !self.nodes.is_empty()
            && self.nodes.keys().copied().all(|nid| {
                self.bypassed.get(nid).copied().unwrap_or(false)
                    || self.plugins[nid]
                        .as_ref()
                        .is_some_and(|plugin| plugin.supports_f64())
            })
    }

    fn can_process_f64_chain_native(&self) -> bool {
        if self.chain_nodes.is_empty() || self.chain_nodes.len() != self.nodes.len() {
            return false;
        }
        if self.edges.len() != self.chain_nodes.len().saturating_sub(1) {
            return false;
        }
        for pair in self.chain_nodes.windows(2) {
            let from = pair[0];
            let to = pair[1];
            let Some(edge) = self
                .edges
                .iter()
                .find(|e| e.from_node == from && e.to_node == to)
            else {
                return false;
            };
            if edge.edge_type != EdgeType::Audio || edge.channel_map.is_some() {
                return false;
            }
        }
        self.chain_nodes.iter().all(|&nid| {
            self.bypassed.get(nid).copied().unwrap_or(false)
                || self.plugins[nid]
                    .as_ref()
                    .is_some_and(|plugin| plugin.supports_f64())
        })
    }

    fn process_f64_graph_native(
        &mut self,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<usize, String> {
        let nf = input.len() / self.input_channels();
        let max_of = self.output_frames_for_input(nf);
        let out_ch = self.output_channels();
        self.apply_automation_for_block(nf);

        let mut guard = BufferGuard::take(&mut self.process_buffers_f64);
        let bufs = guard.get_mut();
        for nb in bufs.node_buffers.iter_mut().flatten() {
            nb.ensure_capacity(nf.max(max_of));
            nb.clear();
        }
        ensure_len(&mut bufs.scratch_input, input.len());
        let mut cf = nf;

        for stage in &self.stages {
            let mut stage_cf: Option<usize> = None;
            for &nid in &stage.nodes {
                let node = &self.nodes[&nid];
                let in_len = if self.is_input_node[nid] {
                    ensure_len(&mut bufs.scratch_input, input.len());
                    bufs.scratch_input[..input.len()].copy_from_slice(input);
                    input.len()
                } else {
                    let il = Self::merge_inputs_into(
                        node,
                        &self.predecessors,
                        &bufs.node_buffers,
                        cf,
                        &mut bufs.merge_buffer,
                        &mut bufs.channel_map_buffer,
                        &mut bufs.delay_scratch,
                        &mut bufs.compensation_delays,
                    )
                    .map_err(|e| {
                        crate::rate_limited_log!(
                            error,
                            5,
                            "host: f64 merge_inputs_into failed for node {} '{}': {e}",
                            nid,
                            node.name
                        );
                        e
                    })?;
                    ensure_len(&mut bufs.scratch_input, il);
                    bufs.scratch_input[..il].copy_from_slice(&bufs.merge_buffer[..il]);
                    il
                };
                let actual_output_frames = if self.bypassed[nid] {
                    bufs.node_buffers[nid]
                        .as_mut()
                        .unwrap()
                        .write(&bufs.scratch_input[..in_len]);
                    cf
                } else {
                    let plugin = self.plugins[nid].as_mut().unwrap();
                    let context = ProcessContext {
                        sample_rate: self.sample_rate,
                        num_frames: cf,
                    };
                    let max_output_frames = plugin.output_frames_for_input(cf);
                    let output_len = max_output_frames * node.output_channels();
                    let needs_extended_in_place_buffer = node.input_channels()
                        > node.output_channels()
                        && self.predecessors[nid]
                            .iter()
                            .any(|e| e.edge_type == EdgeType::Sidechain);
                    let process_output_len = if needs_extended_in_place_buffer {
                        output_len.max(in_len)
                    } else {
                        output_len
                    };
                    ensure_len(&mut bufs.scratch_output, process_output_len);
                    let frames = plugin
                        .process_f64(
                            &bufs.scratch_input[..in_len],
                            &mut bufs.scratch_output[..process_output_len],
                            &context,
                        )
                        .map_err(|e| {
                            crate::rate_limited_log!(
                                error,
                                5,
                                "host: plugin '{}' (node {}) f64 process failed: {e}",
                                node.name,
                                nid
                            );
                            e
                        })?;
                    bufs.node_buffers[nid]
                        .as_mut()
                        .unwrap()
                        .write(&bufs.scratch_output[..frames * node.output_channels()]);
                    frames
                };
                stage_cf = Some(match stage_cf {
                    Some(prev) => prev.min(actual_output_frames),
                    None => actual_output_frames,
                });
            }
            if let Some(scf) = stage_cf {
                cf = scf;
            }
        }

        Self::collect_output_from_buffers(&self.output_nodes, &bufs.node_buffers, output, cf)
            .map_err(|e| {
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: f64 collect_output_from_buffers failed: {e}"
                );
                e
            })?;
        if cf < nf && self.has_variable_frame_plugin {
            output[cf * out_ch..].fill(0.0);
            cf = nf;
        }
        self.playback_position += nf;

        Ok(cf)
    }

    fn process_f64_chain_native(
        &mut self,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<usize, String> {
        let mut scratch_a = std::mem::take(&mut self.f64_chain_scratch);
        let mut scratch_b = std::mem::take(&mut self.f64_chain_scratch_alt);

        ensure_len(&mut scratch_a, input.len());
        scratch_a[..input.len()].copy_from_slice(input);

        let mut current_in_a = true;
        let mut current_len = input.len();
        let mut current_frames = input.len() / self.input_channels();
        let mut current_rate = self.sample_rate;

        for idx in 0..self.chain_nodes.len() {
            let nid = self.chain_nodes[idx];
            let node = self
                .nodes
                .get(&nid)
                .ok_or_else(|| format!("Missing node {nid} during f64 processing"))?;
            let is_last = idx + 1 == self.chain_nodes.len();
            let output_frames = self.plugins[nid]
                .as_ref()
                .unwrap()
                .output_frames_for_input(current_frames);
            let output_len = output_frames * node.output_channels();

            let frames = if is_last {
                if output.len() < output_len {
                    self.f64_chain_scratch = scratch_a;
                    self.f64_chain_scratch_alt = scratch_b;
                    return Err(format!(
                        "f64 output too small: need {output_len} samples, got {}",
                        output.len()
                    ));
                }
                if current_in_a {
                    Self::process_f64_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &scratch_a[..current_len],
                        &mut output[..output_len],
                        current_rate,
                        current_frames,
                    )?
                } else {
                    Self::process_f64_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &scratch_b[..current_len],
                        &mut output[..output_len],
                        current_rate,
                        current_frames,
                    )?
                }
            } else if current_in_a {
                ensure_len(&mut scratch_b, output_len);
                let frames = Self::process_f64_node(
                    self.plugins[nid].as_mut().unwrap().as_mut(),
                    self.bypassed.get(nid).copied().unwrap_or(false),
                    &scratch_a[..current_len],
                    &mut scratch_b[..output_len],
                    current_rate,
                    current_frames,
                )?;
                current_in_a = false;
                frames
            } else {
                ensure_len(&mut scratch_a, output_len);
                let frames = Self::process_f64_node(
                    self.plugins[nid].as_mut().unwrap().as_mut(),
                    self.bypassed.get(nid).copied().unwrap_or(false),
                    &scratch_b[..current_len],
                    &mut scratch_a[..output_len],
                    current_rate,
                    current_frames,
                )?;
                current_in_a = true;
                frames
            };

            current_frames = frames;
            current_len = frames * node.output_channels();
            current_rate = self.plugins[nid]
                .as_ref()
                .unwrap()
                .output_sample_rate(current_rate);
        }

        self.f64_chain_scratch = scratch_a;
        self.f64_chain_scratch_alt = scratch_b;
        self.playback_position += input.len() / self.input_channels();
        Ok(current_frames)
    }

    fn process_f64_node(
        plugin: &mut dyn Plugin,
        bypassed: bool,
        input: &[f64],
        output: &mut [f64],
        sample_rate: u32,
        num_frames: usize,
    ) -> Result<usize, String> {
        if bypassed {
            output[..input.len()].copy_from_slice(input);
            return Ok(num_frames);
        }
        let context = ProcessContext {
            sample_rate,
            num_frames,
        };
        plugin.process_f64(input, output, &context)
    }

    fn process_stage_parallel(
        parallel_enabled: bool,
        stage: &ProcessingStage,
        input: &[f32],
        sample_rate: u32,
        cf: usize,
        plugins: &mut [Option<Box<dyn Plugin>>],
        nodes: &HashMap<NodeId, GraphNode>,
        predecessors: &[Vec<GraphEdge>],
        is_input_node: &[bool],
        bypassed: &[bool],
        bufs: &mut ProcessBuffers<f32>,
    ) -> Option<Result<usize, String>> {
        if !parallel_enabled || stage.nodes.len() < 2 {
            return None;
        }

        for &nid in &stage.nodes {
            if nid >= plugins.len() || nid >= bufs.node_buffers.len() {
                return None;
            }
            if !is_input_node.get(nid).copied().unwrap_or(false) {
                let preds = predecessors.get(nid)?;
                if preds.len() != 1 {
                    return None;
                }
                let edge = &preds[0];
                if edge.edge_type != EdgeType::Audio || edge.channel_map.is_some() {
                    return None;
                }
            }
        }

        if bufs.parallel_scratch.len() < plugins.len()
            || bufs.parallel_results.capacity() < stage.nodes.len()
        {
            return None;
        }

        let plugins_addr = plugins.as_mut_ptr() as usize;
        let node_buffers_addr = bufs.node_buffers.as_mut_ptr() as usize;
        let scratch_addr = bufs.parallel_scratch.as_mut_ptr() as usize;
        stage
            .nodes
            .par_iter()
            .map(|&nid| {
                let node = nodes
                    .get(&nid)
                    .ok_or_else(|| format!("Missing node {nid} during parallel stage"))?;
                // SAFETY: `stage.nodes` is produced by topological sorting and contains
                // each node at most once. This closure mutates only the plugin, output
                // buffer, and scratch slot for its own `nid`. Reads are limited to
                // predecessor node buffers from earlier stages; this fast path rejects
                // merge nodes, so it never reads a buffer written by the same stage.
                unsafe {
                    let plugin_slot =
                        &mut *((plugins_addr as *mut Option<Box<dyn Plugin>>).add(nid));
                    let node_buffer_slot =
                        &mut *((node_buffers_addr as *mut Option<NodeBuffer<f32>>).add(nid));
                    let (scratch_input, scratch_output, merge_buffer) =
                        &mut *((scratch_addr as *mut (Vec<f32>, Vec<f32>, Vec<f32>)).add(nid));

                    let in_len = if is_input_node.get(nid).copied().unwrap_or(false) {
                        ensure_len(scratch_input, input.len());
                        scratch_input[..input.len()].copy_from_slice(input);
                        input.len()
                    } else {
                        let edge = &predecessors[nid][0];
                        let source = (&*((node_buffers_addr as *const Option<NodeBuffer<f32>>)
                            .add(edge.from_node)))
                            .as_ref()
                            .ok_or_else(|| {
                                format!("Missing predecessor buffer for node {}", edge.from_node)
                            })?;
                        let source_data = source.read();
                        let input_len = cf * node.input_channels();
                        ensure_len(merge_buffer, input_len);
                        merge_buffer[..input_len].fill(0.0);
                        let route_channels = source.num_channels.min(node.input_channels());
                        for frame in 0..cf {
                            let src = frame * source.num_channels;
                            let dst = frame * node.input_channels();
                            let src_end = (src + route_channels).min(source_data.len());
                            let copied = src_end.saturating_sub(src);
                            if copied > 0 {
                                merge_buffer[dst..dst + copied]
                                    .copy_from_slice(&source_data[src..src_end]);
                            }
                        }
                        ensure_len(scratch_input, input_len);
                        scratch_input[..input_len].copy_from_slice(&merge_buffer[..input_len]);
                        input_len
                    };

                    let out_frames = if bypassed.get(nid).copied().unwrap_or(false) {
                        node_buffer_slot
                            .as_mut()
                            .unwrap()
                            .write(&scratch_input[..in_len]);
                        cf
                    } else {
                        let plugin = plugin_slot.as_mut().unwrap();
                        let context = ProcessContext {
                            sample_rate,
                            num_frames: cf,
                        };
                        let max_output_frames = plugin.output_frames_for_input(cf);
                        let output_len = max_output_frames * node.output_channels();
                        ensure_len(scratch_output, output_len);
                        let frames = plugin
                            .process(
                                &scratch_input[..in_len],
                                &mut scratch_output[..output_len],
                                &context,
                            )
                            .map_err(|e| {
                                crate::rate_limited_log!(
                                    error,
                                    5,
                                    "host: plugin '{}' (node {}) parallel process failed: {e}",
                                    node.name,
                                    nid
                                );
                                e
                            })?;
                        node_buffer_slot
                            .as_mut()
                            .unwrap()
                            .write(&scratch_output[..frames * node.output_channels()]);
                        frames
                    };
                    Ok::<usize, String>(out_frames)
                }
            })
            .collect_into_vec(&mut bufs.parallel_results);

        let mut stage_cf = None;
        for result in bufs.parallel_results.drain(..) {
            match result {
                Ok(frames) => {
                    stage_cf = Some(stage_cf.map_or(frames, |prev: usize| prev.min(frames)));
                }
                Err(err) => return Some(Err(err)),
            }
        }

        stage_cf.map(Ok)
    }

    fn merge_inputs_into<T: AudioSample>(
        n: &GraphNode,
        preds: &[Vec<GraphEdge>],
        nbs: &[Option<NodeBuffer<T>>],
        nf: usize,
        mb: &mut Vec<T>,
        cmb: &mut Vec<T>,
        delay_scratch: &mut Vec<T>,
        compensation_delays: &mut CompensationDelays<T>,
    ) -> Result<usize, String> {
        let is = nf * n.input_channels();
        ensure_len(mb, is);
        mb[..is].fill(T::default());
        let has_sidechain = preds[n.id]
            .iter()
            .any(|e| e.edge_type == EdgeType::Sidechain);
        let primary_channels = if has_sidechain && n.input_channels() > n.output_channels() {
            n.output_channels()
        } else {
            n.input_channels()
        };
        let mut sidechain_offset = primary_channels;
        for e in &preds[n.id] {
            let sb = nbs[e.from_node].as_ref().unwrap();
            let sd = sb.read();
            let dest_offset = match e.edge_type {
                EdgeType::Audio => 0,
                EdgeType::Sidechain => {
                    if sidechain_offset >= n.input_channels() {
                        continue;
                    }
                    let offset = sidechain_offset;
                    let requested = e
                        .channel_map
                        .as_ref()
                        .map_or(sb.num_channels, |cm| cm.len());
                    sidechain_offset = (sidechain_offset + requested).min(n.input_channels());
                    offset
                }
            };
            let available_dest_channels = match e.edge_type {
                EdgeType::Audio => primary_channels,
                EdgeType::Sidechain => n.input_channels().saturating_sub(dest_offset),
            };
            if available_dest_channels == 0 {
                continue;
            }
            if let Some(ref cm) = e.channel_map {
                let mapped_channels = cm.len().min(available_dest_channels);
                let ms = nf * mapped_channels;
                ensure_len(cmb, ms);
                for f in 0..nf {
                    for (di, &si) in cm.iter().take(mapped_channels).enumerate() {
                        let dst = f * mapped_channels + di;
                        let src = f * sb.num_channels + si;
                        cmb[dst] = sd.get(src).copied().unwrap_or_default();
                    }
                }
                Self::apply_compensation_and_sum_at(
                    e,
                    mapped_channels,
                    nf,
                    &cmb[..ms],
                    mb,
                    n.input_channels(),
                    dest_offset,
                    delay_scratch,
                    compensation_delays,
                );
            } else {
                let route_channels = sb.num_channels.min(available_dest_channels);
                let ms = nf * route_channels;
                if route_channels == sb.num_channels && sd.len() >= ms {
                    Self::apply_compensation_and_sum_at(
                        e,
                        route_channels,
                        nf,
                        &sd[..ms],
                        mb,
                        n.input_channels(),
                        dest_offset,
                        delay_scratch,
                        compensation_delays,
                    );
                } else {
                    ensure_len(cmb, ms);
                    for f in 0..nf {
                        let src = f * sb.num_channels;
                        let dst = f * route_channels;
                        let src_end = (src + route_channels).min(sd.len());
                        let copied = src_end.saturating_sub(src);
                        if copied > 0 {
                            cmb[dst..dst + copied].copy_from_slice(&sd[src..src_end]);
                        }
                        if copied < route_channels {
                            cmb[dst + copied..dst + route_channels].fill(T::default());
                        }
                    }
                    Self::apply_compensation_and_sum_at(
                        e,
                        route_channels,
                        nf,
                        &cmb[..ms],
                        mb,
                        n.input_channels(),
                        dest_offset,
                        delay_scratch,
                        compensation_delays,
                    );
                }
            }
        }
        Ok(is)
    }

    /// Apply latency compensation delay (if any) to `src_data` for the given edge,
    /// then sum the result into `dest`. If no compensation is needed, sums directly.
    fn apply_compensation_and_sum_at<T: AudioSample>(
        edge: &GraphEdge,
        channels: usize,
        num_frames: usize,
        src_data: &[T],
        dest: &mut [T],
        dest_channels: usize,
        dest_offset: usize,
        delay_scratch: &mut Vec<T>,
        compensation_delays: &mut CompensationDelays<T>,
    ) {
        if let Some(delay_buf) = compensation_delays.get_mut_edge(edge.id) {
            // Process frame-by-frame through the compensation delay.
            // delay_scratch is split: first `total` samples for output,
            // next `channels` samples as a reusable silence frame.
            let total = num_frames * channels;
            let needed = total + channels;
            if delay_scratch.len() < needed {
                delay_scratch.resize(needed, T::default());
            }
            // Zero the silence frame region
            delay_scratch[total..total + channels].fill(T::default());
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
            Self::sum_interleaved_at(
                &delay_scratch[..total],
                dest,
                channels,
                dest_channels,
                dest_offset,
                num_frames,
            );
        } else {
            Self::sum_interleaved_at(
                src_data,
                dest,
                channels,
                dest_channels,
                dest_offset,
                num_frames,
            );
        }
    }

    fn sum_interleaved_at<T: AudioSample>(
        src_data: &[T],
        dest: &mut [T],
        channels: usize,
        dest_channels: usize,
        dest_offset: usize,
        num_frames: usize,
    ) {
        let total = num_frames * channels;
        if dest_offset == 0
            && channels == dest_channels
            && src_data.len() >= total
            && dest.len() >= total
        {
            T::scale_add(&mut dest[..total], &src_data[..total]);
            return;
        }

        for frame in 0..num_frames {
            let src = frame * channels;
            let dst = frame * dest_channels + dest_offset;
            for ch in 0..channels {
                if src + ch < src_data.len() && dst + ch < dest.len() {
                    dest[dst + ch] += src_data[src + ch];
                }
            }
        }
    }

    fn collect_output_from_buffers<T: AudioSample>(
        ons: &[NodeId],
        nbs: &[Option<NodeBuffer<T>>],
        out: &mut [T],
        _nf: usize,
    ) -> Result<(), String> {
        if ons.len() == 1 {
            let d = nbs[ons[0]].as_ref().unwrap().read();
            let l = d.len().min(out.len());
            out[..l].copy_from_slice(&d[..l]);
        } else {
            out.fill(T::default());
            for &id in ons {
                let d = nbs[id].as_ref().unwrap().read();
                let l = d.len().min(out.len());
                T::scale_add(&mut out[..l], &d[..l]);
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
    fn compute_compensation_delays<T: AudioSample>(
        &mut self,
        num_slots: usize,
    ) -> CompensationDelays<T> {
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
        let mut delays = CompensationDelays::<T>::new(&self.edges);

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
                let dest_node = self
                    .nodes
                    .get(&nid)
                    .expect("stage node should exist in graph");
                let has_sidechain = preds.iter().any(|e| e.edge_type == EdgeType::Sidechain);
                let primary_channels =
                    if has_sidechain && dest_node.input_channels() > dest_node.output_channels() {
                        dest_node.output_channels()
                    } else {
                        dest_node.input_channels()
                    };
                let mut sidechain_offset = primary_channels;
                for edge in preds {
                    let pred_latency = self.node_latency_from_input[edge.from_node];
                    let compensation = max_pred_latency - pred_latency;
                    if compensation > 0 {
                        let pred_channels = self
                            .nodes
                            .get(&edge.from_node)
                            .map(|n| n.output_channels())
                            .unwrap_or(2);
                        let delay_channels = Self::routed_channel_count(
                            dest_node,
                            edge,
                            pred_channels,
                            primary_channels,
                            &mut sidechain_offset,
                        );
                        if delay_channels == 0 {
                            continue;
                        }
                        delays.set(edge.id, DelayBuffer::new(compensation, delay_channels));
                    } else if edge.edge_type == EdgeType::Sidechain {
                        let pred_channels = self
                            .nodes
                            .get(&edge.from_node)
                            .map(|n| n.output_channels())
                            .unwrap_or(2);
                        let _ = Self::routed_channel_count(
                            dest_node,
                            edge,
                            pred_channels,
                            primary_channels,
                            &mut sidechain_offset,
                        );
                    }
                }
            }
        }

        delays
    }

    fn routed_channel_count(
        dest_node: &GraphNode,
        edge: &GraphEdge,
        source_channels: usize,
        primary_channels: usize,
        sidechain_offset: &mut usize,
    ) -> usize {
        let available_dest_channels = match edge.edge_type {
            EdgeType::Audio => primary_channels,
            EdgeType::Sidechain => {
                if *sidechain_offset >= dest_node.input_channels() {
                    return 0;
                }
                let available = dest_node.input_channels().saturating_sub(*sidechain_offset);
                let requested = edge
                    .channel_map
                    .as_ref()
                    .map_or(source_channels, |cm| cm.len());
                *sidechain_offset = (*sidechain_offset + requested).min(dest_node.input_channels());
                available
            }
        };

        if available_dest_channels == 0 {
            return 0;
        }

        edge.channel_map.as_ref().map_or_else(
            || source_channels.min(available_dest_channels),
            |cm| cm.len().min(available_dest_channels),
        )
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
    fn process_f64(&mut self, i: &[f64], o: &mut [f64]) -> Result<usize, String> {
        self.process_f64(i, o)
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
    fn take_analyzer_contention_stats(&mut self) -> Vec<(usize, u64, u64)> {
        let mut stats = Vec::new();
        for (i, &node_id) in self.chain_nodes.iter().enumerate() {
            if let Some(Some(plugin)) = self.plugins.get_mut(node_id) {
                let (contention, updates) = plugin.take_cache_contention_stats();
                if updates > 0 {
                    stats.push((i, contention, updates));
                }
            }
        }
        stats
    }
}

// Note: Host integration tests that use plugin types (GainPlugin, UpmixerPlugin, etc.)
// live in the `sotf-plugins` facade crate's tests/ directory since those plugin types
// are not available in `sotf-host`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{InPlacePlugin, InPlacePluginAdapter, Plugin};

    #[test]
    fn test_pluginhost_api_empty_graph() {
        let mut g = DawHost::new(2, 48000);
        let i = vec![1.0; 96];
        let mut o = vec![0.0; 96];
        assert!(g.process(&i, &mut o).is_ok());
        assert_eq!(o, i);
    }

    #[test]
    fn test_process_f64_empty_graph() {
        let mut g = DawHost::new(2, 48000);
        let input = vec![0.25_f64, -0.5, 1.0, -1.0];
        let mut output = vec![0.0_f64; input.len()];
        let frames = g.process_f64(&input, &mut output).unwrap();
        assert_eq!(frames, 2);
        assert_eq!(output, input);
    }

    struct F64ScalePlugin {
        channels: usize,
        factor: f64,
    }

    impl Plugin for F64ScalePlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("F64Scale", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<crate::parameters::Parameter> {
            vec![crate::parameters::Parameter::new_float(
                "factor", "Factor", 1.0, 0.0, 8.0,
            )]
        }
        fn set_parameter(
            &mut self,
            id: crate::parameters::ParameterId,
            value: crate::parameters::ParameterValue,
        ) -> Result<(), String> {
            if id.0 == "factor"
                && let crate::parameters::ParameterValue::Float(value) = value
            {
                self.factor = value as f64;
                return Ok(());
            }
            Err(format!("unknown parameter: {}", id.0))
        }
        fn get_parameter(
            &self,
            id: &crate::parameters::ParameterId,
        ) -> Option<crate::parameters::ParameterValue> {
            if id.0 == "factor" {
                Some(crate::parameters::ParameterValue::Float(self.factor as f32))
            } else {
                None
            }
        }
        fn process(
            &mut self,
            _: &[f32],
            _: &mut [f32],
            _: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            Err("f32 path should not be used".into())
        }
        fn process_f64(
            &mut self,
            input: &[f64],
            output: &mut [f64],
            ctx: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i * self.factor;
            }
            Ok(ctx.num_frames)
        }
        fn supports_f64(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_process_f64_uses_native_chain_when_supported() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(F64ScalePlugin {
            channels: 2,
            factor: 2.0,
        }))
        .unwrap();
        g.add_plugin(Box::new(F64ScalePlugin {
            channels: 2,
            factor: 3.0,
        }))
        .unwrap();

        let input = vec![0.25_f64, -0.5, 1.0, -1.0];
        let mut output = vec![0.0_f64; input.len()];
        let frames = g.process_f64(&input, &mut output).unwrap();

        assert_eq!(frames, 2);
        assert_eq!(output, vec![1.5, -3.0, 6.0, -6.0]);
    }

    #[test]
    fn test_process_f64_sample_offset_parameter_event_splits_native_chain() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(F64ScalePlugin {
            channels: 2,
            factor: 1.0,
        }))
        .unwrap();
        g.build().unwrap();

        g.set_plugin_parameter_at(
            0,
            "factor",
            crate::parameters::ParameterValue::Float(0.5),
            2,
        )
        .unwrap();

        let input = vec![1.0_f64; 8];
        let mut output = vec![0.0_f64; 8];
        let frames = g.process_f64(&input, &mut output).unwrap();

        assert_eq!(frames, 4);
        assert_eq!(output, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_process_f64_sample_offset_parameter_event_splits_f32_bridge() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        g.set_plugin_parameter_at(0, "gain", crate::parameters::ParameterValue::Float(0.5), 2)
            .unwrap();

        let input = vec![1.0_f64; 8];
        let mut output = vec![0.0_f64; 8];
        let frames = g.process_f64(&input, &mut output).unwrap();

        assert_eq!(frames, 4);
        assert_eq!(output, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_process_f64_uses_native_dag_when_supported() {
        let mut g = DawHost::new(1, 48000);
        let a = g
            .add_node(
                "a".into(),
                Box::new(F64ScalePlugin {
                    channels: 1,
                    factor: 2.0,
                }),
            )
            .unwrap();
        let b = g
            .add_node(
                "b".into(),
                Box::new(F64ScalePlugin {
                    channels: 1,
                    factor: 3.0,
                }),
            )
            .unwrap();
        let c = g
            .add_node(
                "c".into(),
                Box::new(F64ScalePlugin {
                    channels: 1,
                    factor: 5.0,
                }),
            )
            .unwrap();
        let d = g
            .add_node(
                "d".into(),
                Box::new(F64ScalePlugin {
                    channels: 1,
                    factor: 1.0,
                }),
            )
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(a, c)).unwrap();
        g.add_edge(GraphEdge::new(b, d)).unwrap();
        g.add_edge(GraphEdge::new(c, d)).unwrap();
        g.build().unwrap();

        let input = vec![1.0_f64, 2.0, 3.0, 4.0];
        let mut output = vec![0.0_f64; input.len()];
        let frames = g.process_f64(&input, &mut output).unwrap();

        assert_eq!(frames, 4);
        assert_eq!(output, vec![16.0, 32.0, 48.0, 64.0]);
    }

    #[test]
    fn test_topology_handle_updates_after_build() {
        let mut g = DawHost::new(2, 48000);
        let topology = g.topology_handle();
        assert!(topology.load().nodes.is_empty());

        g.add_plugin(Box::new(ScalerPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        let snapshot = topology.load_full();
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.stages.len(), 1);
    }

    #[test]
    fn test_graph_mutation_sender_appends_plugin_before_process() {
        let mut g = DawHost::new(2, 48000);
        let topology = g.topology_handle();
        let mut sender = g
            .take_graph_mutation_sender()
            .expect("graph sender should be available once");
        assert!(g.take_graph_mutation_sender().is_none());

        sender
            .queue_add_plugin(Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        let frames = g.process(&input, &mut output).unwrap();

        assert_eq!(frames, 2);
        assert_eq!(output, vec![2.0, 4.0, 6.0, 8.0]);
        assert_eq!(sender.dropped_mutations(), 0);

        let snapshot = topology.load_full();
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.stages.len(), 1);
    }

    #[test]
    fn test_graph_mutation_sender_reserves_node_ids_for_edges() {
        let mut g = DawHost::new(2, 48000);
        let topology = g.topology_handle();
        let mut sender = g.take_graph_mutation_sender().unwrap();

        let first = sender
            .queue_add_node("first".into(), Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();
        let second = sender
            .queue_add_node("second".into(), Box::new(ScalerPlugin::new(2, 3.0)))
            .unwrap();
        sender
            .queue_add_edge(GraphEdge::new(first, second))
            .unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        let frames = g.process(&input, &mut output).unwrap();

        assert_eq!(frames, 2);
        assert_eq!(output, vec![6.0, 12.0, 18.0, 24.0]);

        let snapshot = topology.load_full();
        assert!(snapshot.nodes.contains_key(&first));
        assert!(snapshot.nodes.contains_key(&second));
        assert_eq!(snapshot.edges.len(), 1);
    }

    #[test]
    fn test_graph_mutation_sender_reserves_add_plugin_ids_in_queue_order() {
        let mut g = DawHost::new(2, 48000);
        let mut sender = g.take_graph_mutation_sender().unwrap();

        let plugin_node = sender
            .queue_add_plugin(Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();
        let named_node = sender
            .queue_add_node("side".into(), Box::new(ScalerPlugin::new(2, 3.0)))
            .unwrap();

        assert_eq!(plugin_node, 0);
        assert_eq!(named_node, 1);

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        g.process(&input, &mut output).unwrap();

        assert_eq!(g.chain_nodes, vec![plugin_node]);
        assert!(g.nodes.contains_key(&named_node));
    }

    #[test]
    fn test_parameter_event_scratch_is_preallocated_to_queue_capacity() {
        let g = DawHost::new(2, 48000);
        assert!(
            g.parameter_event_scratch.capacity() >= PARAMETER_EVENT_QUEUE_CAPACITY,
            "parameter event scratch should not allocate while draining a full ring"
        );
    }

    #[test]
    fn test_parallel_scratch_is_prepared_during_build() {
        let mut g = DawHost::new(2, 48000);
        let a = g
            .add_node("a".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let b = g
            .add_node("b".into(), Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();
        let c = g
            .add_node("c".into(), Box::new(ScalerPlugin::new(2, 3.0)))
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(a, c)).unwrap();
        g.build().unwrap();

        let bufs = g.process_buffers.as_ref().unwrap();
        assert!(bufs.parallel_scratch.len() >= g.plugins.len());
        assert!(bufs.parallel_results.capacity() >= 2);
    }

    struct PrefersOversamplingPlugin {
        inner: ScalerPlugin,
        factor: u32,
    }

    impl Plugin for PrefersOversamplingPlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("PrefersOversampling", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            self.inner.channels
        }
        fn output_channels(&self) -> usize {
            self.inner.channels
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
            self.inner.process(input, output, ctx)
        }
        fn preferred_oversampling(&self) -> Option<u32> {
            Some(self.factor)
        }
    }

    #[test]
    fn test_add_node_auto_wraps_preferred_oversampling_plugin() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(PrefersOversamplingPlugin {
            inner: ScalerPlugin::new(2, 1.0),
            factor: 2,
        }))
        .unwrap();

        let info = g.get_plugin(0).unwrap().info();
        assert_eq!(info.name, "PrefersOversampling(2x)");
        assert_eq!(g.get_plugin(0).unwrap().preferred_oversampling(), None);
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

    struct SidechainInPlacePlugin {
        channels: usize,
    }

    impl InPlacePlugin for SidechainInPlacePlugin {
        fn info(&self) -> crate::plugin::PluginInfo {
            crate::plugin::PluginInfo::new("SidechainInPlace", "0.1", "test")
        }

        fn channels(&self) -> usize {
            self.channels
        }

        fn input_channels(&self) -> usize {
            self.channels * 2
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

        fn process_in_place(
            &mut self,
            buffer: &mut [f32],
            context: &crate::plugin::ProcessContext,
        ) -> Result<usize, String> {
            let stride = self.channels * 2;
            for frame in 0..context.num_frames {
                let off = frame * stride;
                for ch in 0..self.channels {
                    buffer[off + ch] += buffer[off + self.channels + ch] * 10.0;
                }
            }
            Ok(context.num_frames)
        }
    }

    #[test]
    fn test_sidechain_edge_appends_extended_input_for_in_place_adapter() {
        let mut g = DawHost::new(2, 48000);
        let audio = g
            .add_node("audio".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let sidechain = g
            .add_node("sidechain".into(), Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();
        let processor = g
            .add_node(
                "processor".into(),
                Box::new(InPlacePluginAdapter::new(SidechainInPlacePlugin {
                    channels: 2,
                })),
            )
            .unwrap();

        g.add_edge(GraphEdge::new(audio, processor)).unwrap();
        g.add_sidechain_edge(sidechain, processor).unwrap();
        g.build().unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        g.process(&input, &mut output).unwrap();

        assert_eq!(output, vec![21.0, 42.0, 63.0, 84.0]);
    }

    #[test]
    fn test_large_input_block_grows_scratch_before_copy() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        let frames = 5000;
        let input = vec![0.25; frames * 2];
        let mut output = vec![0.0; frames * 2];
        let processed = g.process(&input, &mut output).unwrap();

        assert_eq!(processed, frames);
        assert_eq!(output, input);
    }

    #[test]
    fn test_direct_dag_mutation_after_build_rebuilds_before_processing() {
        let mut g = DawHost::new(2, 48000);
        let first = g
            .add_node("first".into(), Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();
        g.build().unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 4];
        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![2.0, 4.0, 6.0, 8.0]);

        let second = g
            .add_node("second".into(), Box::new(ScalerPlugin::new(2, 3.0)))
            .unwrap();
        g.add_edge(GraphEdge::new(first, second)).unwrap();

        g.process(&input, &mut output).unwrap();
        assert_eq!(output, vec![6.0, 12.0, 18.0, 24.0]);
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
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 42)))
            .unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 42);
    }

    #[test]
    fn test_cached_latency_chain_sums() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 30)))
            .unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 60);
    }

    #[test]
    fn test_cached_latency_invalidated_on_add_node() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10)))
            .unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 10);
        // Adding another plugin invalidates the cache
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20)))
            .unwrap();
        g.build().unwrap();
        assert_eq!(g.total_latency_samples(), 30);
    }

    #[test]
    fn test_cached_latency_invalidated_on_remove() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20)))
            .unwrap();
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
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 10)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(2, 1.0, 20)))
            .unwrap();
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
                Box::new(ChannelChangingPlugin {
                    in_ch: 2,
                    out_ch: 5,
                }),
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

        let inp = g
            .add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 2.0, 10)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 3.0, 0)))
            .unwrap();
        let out = g
            .add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

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

        let inp = g
            .add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 5)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 5)))
            .unwrap();
        let out = g
            .add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

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
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 10)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 20)))
            .unwrap();
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

        let inp = g
            .add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 2)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 0)))
            .unwrap();
        let out = g
            .add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

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
        assert_eq!(
            output[0], 1.0,
            "Frame 0: only A contributes (B is compensated/delayed)"
        );
        assert_eq!(
            output[1], 1.0,
            "Frame 1: only A contributes (B is compensated/delayed)"
        );
        assert_eq!(output[2], 2.0, "Frame 2: both A and delayed B contribute");
        assert_eq!(output[7], 2.0, "Frame 7: both A and delayed B contribute");
    }

    #[test]
    fn test_latency_compensation_handles_channel_mapped_edge() {
        let mut g = DawHost::new(2, 48000);

        let inp = g
            .add_node("input".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let long_path = g
            .add_node(
                "long".into(),
                Box::new(ScalerPlugin::with_latency(2, 1.0, 2)),
            )
            .unwrap();
        let mapped_path = g
            .add_node("mapped".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let out = g
            .add_node("output".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();

        g.add_edge(GraphEdge::new(inp, long_path)).unwrap();
        g.add_edge(GraphEdge::new(inp, mapped_path)).unwrap();
        g.add_edge(GraphEdge::new(long_path, out)).unwrap();
        g.add_edge(GraphEdge::with_channels(mapped_path, out, vec![0]))
            .unwrap();
        g.build().unwrap();

        let input = vec![
            1.0, 10.0, //
            2.0, 20.0, //
            3.0, 30.0, //
            4.0, 40.0,
        ];
        let mut output = vec![0.0; input.len()];

        g.process(&input, &mut output).unwrap();

        assert_eq!(output[0], 1.0);
        assert_eq!(output[1], 10.0);
        assert_eq!(output[4], 4.0);
        assert_eq!(output[5], 30.0);
        assert_eq!(output[6], 6.0);
        assert_eq!(output[7], 40.0);
    }

    #[test]
    fn test_latency_compensation_asymmetric_three_paths() {
        // Graph: input -> [A (lat=20), B (lat=10), C (lat=0)] -> output
        // Compensation: B delayed by 10, C delayed by 20
        let mut g = DawHost::new(1, 48000);

        let inp = g
            .add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 20)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 10)))
            .unwrap();
        let c = g
            .add_node("C".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 0)))
            .unwrap();
        let out = g
            .add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

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

        let inp = g
            .add_node("input".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 10)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::with_latency(1, 1.0, 10)))
            .unwrap();
        let out = g
            .add_node("output".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();

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
        let mut slot: Option<ProcessBuffers<f32>> = Some(ProcessBuffers {
            node_buffers: vec![],
            scratch_input: vec![],
            scratch_output: vec![],
            merge_buffer: vec![],
            channel_map_buffer: vec![],
            compensation_delays: CompensationDelays::empty(),
            delay_scratch: vec![],
            parallel_scratch: Vec::new(),
            parallel_results: Vec::new(),
        });

        {
            let _guard = BufferGuard::take(&mut slot);
            // guard holds &mut slot, so we can't check slot here,
            // but on drop it must restore the buffers.
        }
        assert!(
            slot.is_some(),
            "Slot should be restored after guard dropped"
        );
    }

    #[test]
    fn test_buffer_guard_survives_simulated_early_return() {
        let mut slot: Option<ProcessBuffers<f32>> = Some(ProcessBuffers {
            node_buffers: vec![],
            scratch_input: vec![],
            scratch_output: vec![],
            merge_buffer: vec![],
            channel_map_buffer: vec![],
            compensation_delays: CompensationDelays::empty(),
            delay_scratch: vec![],
            parallel_scratch: Vec::new(),
            parallel_results: Vec::new(),
        });

        // Simulate early return inside a scope
        let result: Result<(), String> = {
            let _guard = BufferGuard::take(&mut slot);
            // Simulate error that would cause early return
            Err("simulated error".into())
        };

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
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 5)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 10)))
            .unwrap();
        g.add_plugin(Box::new(ScalerPlugin::with_latency(1, 1.0, 3)))
            .unwrap();
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
            if id.0 == "gain"
                && let crate::parameters::ParameterValue::Float(v) = val
            {
                self.gain = v;
                return Ok(());
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

    #[test]
    fn test_set_plugin_parameter_queues_until_next_process() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        let param_id = crate::parameters::ParameterId::from("gain");
        g.set_plugin_parameter(0, "gain", crate::parameters::ParameterValue::Float(0.5))
            .unwrap();

        let queued_value = g
            .get_plugin(0)
            .unwrap()
            .get_parameter(&param_id)
            .and_then(|v| v.as_float())
            .unwrap();
        assert_eq!(queued_value, 1.0);

        let input = vec![1.0f32; 4];
        let mut output = vec![0.0f32; 4];
        g.process(&input, &mut output).unwrap();

        assert_eq!(output, vec![0.5; 4]);
    }

    #[test]
    fn test_external_parameter_sender_queues_without_host_borrow() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        let node_id = g.chain_nodes[0];
        let mut sender = g
            .take_parameter_event_sender()
            .expect("sender should be available once");
        assert!(g.take_parameter_event_sender().is_none());

        sender
            .queue_node_parameter(
                node_id,
                crate::parameters::ParameterId::from("gain"),
                crate::parameters::ParameterValue::Float(0.25),
            )
            .unwrap();

        let input = vec![1.0f32; 4];
        let mut output = vec![0.0f32; 4];
        g.process(&input, &mut output).unwrap();

        assert_eq!(output, vec![0.25; 4]);
        assert_eq!(sender.dropped_events(), 0);
    }

    #[test]
    fn test_parameter_event_sample_offset_splits_block() {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
        g.build().unwrap();

        g.set_plugin_parameter_at(0, "gain", crate::parameters::ParameterValue::Float(0.5), 2)
            .unwrap();

        let input = vec![1.0f32; 8];
        let mut output = vec![0.0f32; 8];
        let frames = g.process(&input, &mut output).unwrap();

        assert_eq!(frames, 4);
        assert_eq!(output, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
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

    // ── DAG topology tests ──

    #[test]
    fn test_cycle_detection_self_loop() {
        let mut g = DawHost::new(2, 48000);
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        // Self-loops are rejected at add_edge level
        assert!(g.add_edge(GraphEdge::new(a, a)).is_err());
    }

    #[test]
    fn test_cycle_detection_two_node_cycle() {
        let mut g = DawHost::new(2, 48000);
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(b, a)).unwrap();
        assert!(g.build().is_err());
    }

    #[test]
    fn test_cycle_detection_three_node_cycle() {
        let mut g = DawHost::new(2, 48000);
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        let c = g
            .add_node("C".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(b, c)).unwrap();
        g.add_edge(GraphEdge::new(c, a)).unwrap();
        assert!(g.build().is_err());
    }

    #[test]
    fn test_diamond_topology_processes_correctly() {
        // Diamond: A -> B, A -> C, B -> D, C -> D
        // A scales by 2, B scales by 3, C scales by 5, D scales by 1
        // D merges (sums) B and C outputs: input * 2 * 3 + input * 2 * 5 = input * 16
        let mut g = DawHost::new(2, 48000);
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::new(2, 2.0)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::new(2, 3.0)))
            .unwrap();
        let c = g
            .add_node("C".into(), Box::new(ScalerPlugin::new(2, 5.0)))
            .unwrap();
        let d = g
            .add_node("D".into(), Box::new(ScalerPlugin::new(2, 1.0)))
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(a, c)).unwrap();
        g.add_edge(GraphEdge::new(b, d)).unwrap();
        g.add_edge(GraphEdge::new(c, d)).unwrap();
        g.build().unwrap();

        let nf = 4;
        let input = vec![1.0_f32; nf * 2];
        let mut output = vec![0.0_f32; nf * 2];
        g.process(&input, &mut output).unwrap();

        // D receives sum of B and C: (1*2*3) + (1*2*5) = 16
        for &s in &output {
            assert!(
                (s - 16.0).abs() < 1e-4,
                "Diamond should produce 16.0, got {}",
                s
            );
        }
    }

    #[test]
    fn test_diamond_latency_compensation() {
        // Diamond with asymmetric latency:
        // A -> B (latency 10), A -> C (latency 30), B -> D, C -> D
        // B path total: 10, C path total: 30
        // B should be delayed by 20 samples to align with C
        let mut g = DawHost::new(2, 48000);
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::with_latency(2, 1.0, 0)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::with_latency(2, 1.0, 10)))
            .unwrap();
        let c = g
            .add_node("C".into(), Box::new(ScalerPlugin::with_latency(2, 1.0, 30)))
            .unwrap();
        let d = g
            .add_node("D".into(), Box::new(ScalerPlugin::with_latency(2, 1.0, 0)))
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(a, c)).unwrap();
        g.add_edge(GraphEdge::new(b, d)).unwrap();
        g.add_edge(GraphEdge::new(c, d)).unwrap();
        g.build().unwrap();

        // Total latency should be max path: 30
        assert_eq!(g.total_latency_samples(), 30);
    }

    #[test]
    fn test_diamond_with_valid_dag_builds_ok() {
        // Ensure diamond topology (not a cycle) is accepted
        let mut g = DawHost::new(1, 44100);
        let a = g
            .add_node("A".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let b = g
            .add_node("B".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let c = g
            .add_node("C".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        let d = g
            .add_node("D".into(), Box::new(ScalerPlugin::new(1, 1.0)))
            .unwrap();
        g.add_edge(GraphEdge::new(a, b)).unwrap();
        g.add_edge(GraphEdge::new(a, c)).unwrap();
        g.add_edge(GraphEdge::new(b, d)).unwrap();
        g.add_edge(GraphEdge::new(c, d)).unwrap();
        assert!(
            g.build().is_ok(),
            "Diamond DAG should not be rejected as cycle"
        );
    }
}
