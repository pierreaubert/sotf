use super::Host;
use super::audio_sample::AudioSample;
use super::audio_sample::ensure_len;
use super::audio_sample::write_plugin_failure_passthrough;
use super::buffer_guard::BufferGuard;
use super::compensation_delays::CompensationDelays;
use super::compiled_plan::{
    CompiledGraphPlan, CompiledLinearPlan, CompiledOp, CompiledOpKind, CompiledRenderPlan,
    build_segments,
};
use super::delay_buffer::DelayBuffer;
use super::graph_edge::GraphEdge;
use super::graph_mutation_sender::GraphMutationSender;
use super::graph_node::GraphNode;
use super::graph_topology::GraphTopology;
use super::misc::DEFAULT_PARALLEL_NODE_COST;
use super::misc::GRAPH_MUTATION_QUEUE_CAPACITY;
use super::misc::HEAVY_PARALLEL_NODE_COST;
use super::misc::MIN_PARALLEL_STAGE_WORK_UNITS;
use super::misc::MODERATE_PARALLEL_NODE_COST;
use super::misc::PARAMETER_EVENT_QUEUE_CAPACITY;
use super::misc::panic_payload_description;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::misc::sandbox_reason_text;
use super::node_buffer::NodeBuffer;
use super::parameter_event::ParameterEvent;
use super::parameter_event_sender::ParameterEventSender;
use super::processing_stage::ProcessingStage;
use super::types::AutomationSlot;
use super::types::EdgeType;
use super::types::GraphMutation;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::types::IsolatedExternalPluginWorkerReport;
use super::types::NodeId;
use super::types::ProcessBuffers;
use crate::automation::{ParameterAutomation, automation_utils};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::external_plugin_isolated::IsolatedExternalPlugin;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::external_plugin_process::ExternalPluginProcessEvent;
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginCompiledOp, PluginCostClass, ProcessContext};
use arc_swap::ArcSwap;
use rayon::prelude::*;
use rtrb::{Consumer, Producer, RingBuffer};
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompiledLinearSource {
    ExternalInput,
    ScratchInput,
    ScratchOutput,
}

fn plugin_compiled_op(kind: CompiledOpKind) -> Option<PluginCompiledOp> {
    match kind {
        CompiledOpKind::ApplyGain => Some(PluginCompiledOp::ApplyGain),
        CompiledOpKind::EqBiquadBank => Some(PluginCompiledOp::EqBiquadBank),
        CompiledOpKind::ChannelMuteSolo => Some(PluginCompiledOp::ChannelMuteSolo),
        CompiledOpKind::Limiter => Some(PluginCompiledOp::Limiter),
        CompiledOpKind::MultibandCompressor => Some(PluginCompiledOp::MultibandCompressor),
        CompiledOpKind::AnalyzerTap => Some(PluginCompiledOp::AnalyzerTap),
        _ => None,
    }
}

pub(super) struct DawQueueEndpoints {
    pub(super) parameter_event_tx: Option<Producer<ParameterEvent>>,
    pub(super) parameter_event_rx: Consumer<ParameterEvent>,
    pub(super) parameter_event_scratch: Vec<ParameterEvent>,
    pub(super) graph_mutation_tx: Option<Producer<GraphMutation>>,
    pub(super) graph_mutation_rx: Consumer<GraphMutation>,
}

pub(super) struct DawQueueState {
    pub(super) graph_next_node_id: Arc<AtomicUsize>,
    pub(super) dropped_parameter_events: u64,
    pub(super) dropped_graph_mutations: u64,
}

pub(super) struct DawAutomationState {
    /// Parameter automation state. Key = (NodeId, ParameterId).
    /// Evaluated before each processing stage.
    pub(super) automation: Vec<AutomationSlot>,
    /// Control-thread lookup for automation slots. The audio path iterates
    /// `automation` by index and never hashes `(NodeId, ParameterId)`.
    pub(super) automation_index: HashMap<(NodeId, ParameterId), usize>,
    /// Current playback position in samples, advanced each process() call.
    pub(super) playback_position: usize,
}

pub(super) struct DawConfig {
    pub(super) sample_rate: u32,
    pub(super) parallel_enabled: bool,
    pub(super) compiled_linear_enabled: bool,
    pub(super) initial_input_channels: usize,
    /// Whether plugins may request their own preferred oversampling wrapper.
    pub(super) plugin_preferred_oversampling_enabled: bool,
    /// Force an oversampling wrapper around all same-I/O plugins when set.
    pub(super) forced_oversampling_factor: Option<u32>,
    /// Current immutable topology snapshot, published with ArcSwap after build.
    pub(super) topology: Arc<ArcSwap<GraphTopology>>,
    pub(super) f64_input_scratch: Vec<f32>,
    pub(super) f64_output_scratch: Vec<f32>,
    pub(super) f64_chain_scratch: Vec<f64>,
    pub(super) f64_chain_scratch_alt: Vec<f64>,
}

pub struct DawHost {
    pub(super) nodes: HashMap<NodeId, GraphNode>,
    /// Plugin storage indexed by NodeId — disjoint from `nodes` for borrow checker.
    /// `process()` can borrow `&self.nodes` (topology) and `&mut self.plugins[nid]` (plugin)
    /// without conflict.
    pub(super) plugins: Vec<Option<Box<dyn Plugin>>>,
    pub(super) edges: Vec<GraphEdge>,
    pub(super) stages: Vec<ProcessingStage>,
    pub(super) input_nodes: Vec<NodeId>,
    pub(super) output_nodes: Vec<NodeId>,
    pub(super) next_node_id: NodeId,
    pub(super) chain_nodes: Vec<NodeId>,
    pub(super) built: bool,
    pub(super) process_buffers: Option<ProcessBuffers<f32>>,
    pub(super) process_buffers_f64: Option<ProcessBuffers<f64>>,
    pub(super) predecessors: Vec<Vec<GraphEdge>>,
    pub(super) is_input_node: Vec<bool>,
    pub(super) is_output_node: Vec<bool>,
    pub(super) has_variable_frame_plugin: bool,
    /// True if all plugins return input_frames unchanged from output_frames_for_input()
    pub(super) cached_frames_identity: bool,
    /// True if all plugins return input_rate unchanged from output_sample_rate()
    pub(super) cached_rate_identity: bool,
    /// Cached per-node output frame ratios for non-identity chains (rare)
    /// Only populated when cached_frames_identity is false
    pub(super) cached_output_frame_ratios: Vec<(NodeId, f64)>,
    /// Indices of plugins in chain_nodes that have analyzer data (get_data() returns Some)
    pub(super) analyzer_indices: Vec<usize>,
    /// Cached total latency in samples, computed during build() and invalidated on graph changes
    pub(super) cached_latency: Option<usize>,
    pub(super) compiled_plan: CompiledRenderPlan,
    /// Per-node cost estimate used to decide whether a parallel stage has enough
    /// work to amortize scheduler overhead. Built off the audio path.
    pub(super) cached_parallel_node_costs: Vec<u32>,
    /// Flat cache of bypass state for O(1) audio-thread lookup. **Mirror** of
    /// the authoritative `GraphNode::bypassed`; rebuilt in `build()` and kept
    /// in sync exclusively through `Self::set_bypass_state` so the two flags
    /// can never disagree.
    pub(super) bypassed: Vec<bool>,
    /// Per-node cumulative latency from graph inputs, computed during build().
    /// Used to calculate compensation delays at merge points.
    pub(super) node_latency_from_input: Vec<usize>,
    /// Pre-allocated scratch buffer for automation updates (avoids per-process() heap allocation).
    pub(super) automation_scratch: Vec<(usize, f32)>,
    pub(super) queues: DawQueueEndpoints,
    pub(super) queue_state: DawQueueState,
    pub(super) automation_state: DawAutomationState,
    pub(super) config: DawConfig,
}

impl DawHost {
    /// Maximum block size the host pre-sizes its internal scratch and node
    /// buffers for. Covers all current SOTF engine block sizes; larger blocks
    /// still work but may trigger a one-time allocation on the audio thread.
    const MAX_BLOCK_FRAMES: usize = 8192;

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
            next_node_id: 0,
            chain_nodes: Vec::new(),
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
            compiled_plan: CompiledRenderPlan::default(),
            cached_parallel_node_costs: Vec::new(),
            bypassed: Vec::new(),
            node_latency_from_input: Vec::new(),
            automation_scratch: Vec::new(),
            queues: DawQueueEndpoints {
                parameter_event_tx: Some(parameter_event_tx),
                parameter_event_rx,
                parameter_event_scratch: Vec::with_capacity(PARAMETER_EVENT_QUEUE_CAPACITY),
                graph_mutation_tx: Some(graph_mutation_tx),
                graph_mutation_rx,
            },
            queue_state: DawQueueState {
                graph_next_node_id,
                dropped_parameter_events: 0,
                dropped_graph_mutations: 0,
            },
            automation_state: DawAutomationState {
                automation: Vec::new(),
                automation_index: HashMap::new(),
                playback_position: 0,
            },
            config: DawConfig {
                sample_rate,
                parallel_enabled: true,
                compiled_linear_enabled: true,
                initial_input_channels: channels,
                plugin_preferred_oversampling_enabled: true,
                forced_oversampling_factor: None,
                topology: Arc::new(ArcSwap::from_pointee(GraphTopology::empty())),
                f64_input_scratch: Vec::new(),
                f64_output_scratch: Vec::new(),
                f64_chain_scratch: Vec::new(),
                f64_chain_scratch_alt: Vec::new(),
            },
        }
    }
    pub fn new_default(sr: u32) -> Self {
        Self::new(2, sr)
    }
    pub fn set_parallel_enabled(&mut self, e: bool) {
        self.config.parallel_enabled = e;
    }

    pub fn set_compiled_linear_enabled(&mut self, enabled: bool) {
        if self.config.compiled_linear_enabled != enabled {
            self.config.compiled_linear_enabled = enabled;
            self.built = false;
        }
    }

    /// Enable or disable plugins' `preferred_oversampling()` requests.
    pub fn set_plugin_preferred_oversampling_enabled(&mut self, enabled: bool) {
        self.config.plugin_preferred_oversampling_enabled = enabled;
    }

    /// Force a host oversampling wrapper around same-I/O plugins.
    pub fn set_forced_oversampling_factor(&mut self, factor: Option<u32>) -> Result<(), String> {
        if let Some(factor) = factor
            && factor != 2
            && factor != 4
        {
            return Err(format!(
                "Invalid forced oversampling factor {factor}: expected 2 or 4"
            ));
        }
        self.config.forced_oversampling_factor = factor;
        Ok(())
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
        if let Some(&idx) = self.automation_state.automation_index.get(&key) {
            self.automation_state.automation[idx].automation = auto;
            return;
        }
        let idx = self.automation_state.automation.len();
        self.automation_state.automation.push(AutomationSlot {
            node_id,
            param_id,
            automation: auto,
        });
        self.automation_state.automation_index.insert(key, idx);
    }

    /// Remove automation for a specific parameter on a node.
    pub fn clear_automation(&mut self, node_id: NodeId, param_id: &ParameterId) {
        let key = (node_id, param_id.clone());
        let Some(idx) = self.automation_state.automation_index.remove(&key) else {
            return;
        };
        self.automation_state.automation.swap_remove(idx);
        if idx < self.automation_state.automation.len() {
            let moved = &self.automation_state.automation[idx];
            self.automation_state
                .automation_index
                .insert((moved.node_id, moved.param_id.clone()), idx);
        }
    }

    /// Remove all automation.
    pub fn clear_all_automation(&mut self) {
        self.automation_state.automation.clear();
        self.automation_state.automation_index.clear();
    }

    /// Reset playback position to 0.
    pub fn reset_playback_position(&mut self) {
        self.automation_state.playback_position = 0;
        for slot in &mut self.automation_state.automation {
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
        Arc::clone(&self.config.topology)
    }

    /// Atomically load the current topology snapshot.
    pub fn current_topology(&self) -> Arc<GraphTopology> {
        self.config.topology.load_full()
    }

    pub(super) fn publish_topology_snapshot(&self) {
        self.config
            .topology
            .store(Arc::new(self.topology_snapshot()));
    }

    pub(super) fn reserve_node_id(&mut self) -> NodeId {
        let externally_reserved = self.queue_state.graph_next_node_id.load(Ordering::Acquire);
        if externally_reserved > self.next_node_id {
            self.next_node_id = externally_reserved;
        }
        let id = self.next_node_id;
        self.next_node_id += 1;
        self.queue_state
            .graph_next_node_id
            .store(self.next_node_id, Ordering::Release);
        id
    }

    pub fn add_node(&mut self, name: String, plugin: Box<dyn Plugin>) -> Result<NodeId, String> {
        let id = self.reserve_node_id();
        self.add_node_with_id(id, name, plugin)?;
        Ok(id)
    }

    pub(super) fn add_node_with_id(
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
            self.queue_state
                .graph_next_node_id
                .store(self.next_node_id, Ordering::Release);
        }
        plugin = self.auto_oversample_plugin(plugin)?;
        plugin.initialize(self.config.sample_rate)?;
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

    pub(super) fn auto_oversample_plugin(
        &self,
        plugin: Box<dyn Plugin>,
    ) -> Result<Box<dyn Plugin>, String> {
        let factor = self.config.forced_oversampling_factor.or_else(|| {
            if self.config.plugin_preferred_oversampling_enabled {
                plugin.preferred_oversampling()
            } else {
                None
            }
        });

        let Some(factor) = factor else {
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
        Ok(Box::new(
            crate::oversampling::AutoOversampledPlugin::new_with_max_frames(
                plugin,
                factor,
                Self::MAX_BLOCK_FRAMES,
            )?,
        ))
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
            node_buffers[id] = Some(NodeBuffer::<f32>::new(
                Self::MAX_BLOCK_FRAMES,
                node.output_channels(),
            ));
            node_buffers_f64[id] = Some(NodeBuffer::<f64>::new(
                Self::MAX_BLOCK_FRAMES,
                node.output_channels(),
            ));
        }
        // Cache per-node bypass flags before computing compensation delays
        // (compensation needs to know which nodes are bypassed for latency calculation)
        self.bypassed = vec![false; num_slots];
        self.cached_parallel_node_costs = vec![DEFAULT_PARALLEL_NODE_COST; num_slots];
        for (&id, node) in &self.nodes {
            self.bypassed[id] = node.bypassed;
            let plugin = self.plugins[id].as_ref().unwrap();
            self.cached_parallel_node_costs[id] =
                Self::estimate_parallel_node_cost(plugin.as_ref(), id, &node.name);
        }
        // Compute per-node cumulative latency from inputs and compensation delays
        let compensation_delays = self.compute_compensation_delays::<f32>(num_slots)?;
        let compensation_delays_f64 = self.compute_compensation_delays::<f64>(num_slots)?;

        self.process_buffers = Some(ProcessBuffers {
            node_buffers,
            scratch_input: vec![0.0f32; Self::MAX_BLOCK_FRAMES * 32],
            scratch_output: vec![0.0f32; Self::MAX_BLOCK_FRAMES * 32],
            merge_buffer: vec![0.0f32; Self::MAX_BLOCK_FRAMES * 32],
            channel_map_buffer: vec![0.0f32; Self::MAX_BLOCK_FRAMES * 32],
            compensation_delays,
            delay_scratch: vec![0.0f32; Self::MAX_BLOCK_FRAMES * 32],
            parallel_scratch: (0..num_slots)
                .map(|id| {
                    if let Some(node) = self.nodes.get(&id) {
                        (
                            vec![0.0f32; Self::MAX_BLOCK_FRAMES * node.input_channels()],
                            vec![0.0f32; Self::MAX_BLOCK_FRAMES * node.output_channels()],
                            vec![0.0f32; Self::MAX_BLOCK_FRAMES * node.input_channels()],
                        )
                    } else {
                        (Vec::new(), Vec::new(), Vec::new())
                    }
                })
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
            scratch_input: vec![0.0f64; Self::MAX_BLOCK_FRAMES * 32],
            scratch_output: vec![0.0f64; Self::MAX_BLOCK_FRAMES * 32],
            merge_buffer: vec![0.0f64; Self::MAX_BLOCK_FRAMES * 32],
            channel_map_buffer: vec![0.0f64; Self::MAX_BLOCK_FRAMES * 32],
            compensation_delays: compensation_delays_f64,
            delay_scratch: vec![0.0f64; Self::MAX_BLOCK_FRAMES * 32],
            parallel_scratch: (0..num_slots)
                .map(|id| {
                    if let Some(node) = self.nodes.get(&id) {
                        (
                            vec![0.0f64; Self::MAX_BLOCK_FRAMES * node.input_channels()],
                            vec![0.0f64; Self::MAX_BLOCK_FRAMES * node.output_channels()],
                            vec![0.0f64; Self::MAX_BLOCK_FRAMES * node.input_channels()],
                        )
                    } else {
                        (Vec::new(), Vec::new(), Vec::new())
                    }
                })
                .collect(),
            parallel_results: Vec::with_capacity(
                self.stages
                    .iter()
                    .map(|stage| stage.nodes.len())
                    .max()
                    .unwrap_or(0),
            ),
        });
        // Cache per-frame properties to avoid mutex locks during process()
        self.cached_frames_identity = true;
        self.cached_rate_identity = true;
        self.cached_output_frame_ratios.clear();
        self.analyzer_indices.clear();

        for (chain_idx, &id) in self.chain_nodes.iter().enumerate() {
            let p = self.plugins[id].as_ref().unwrap();
            let node = &self.nodes[&id];
            if Self::plugin_output_frames_for_input_isolated(p.as_ref(), id, &node.name, 100) != 100
            {
                self.cached_frames_identity = false;
            }
            if Self::plugin_output_sample_rate_isolated(p.as_ref(), id, &node.name, 48000) != 48000
            {
                self.cached_rate_identity = false;
            }
            if p.get_data().is_some() {
                self.analyzer_indices.push(chain_idx);
            }
        }

        self.has_variable_frame_plugin = self.chain_nodes.iter().any(|&id| {
            let p = self.plugins[id].as_ref().unwrap();
            let node = &self.nodes[&id];
            Self::plugin_output_frames_for_input_isolated(p.as_ref(), id, &node.name, 100) != 100
                || p.latency_samples() > 0
        });
        // Cache total latency so total_latency_samples() is O(1)
        self.cached_latency = Some(self.compute_latency());
        self.compiled_plan = self.compile_render_plan();
        self.built = true;
        self.publish_topology_snapshot();
        Ok(())
    }

    pub(super) fn compile_render_plan(&self) -> CompiledRenderPlan {
        if self.nodes.is_empty() {
            CompiledRenderPlan::EmptyPassthrough
        } else if self.config.compiled_linear_enabled && self.can_process_f32_linear_chain() {
            CompiledRenderPlan::LinearF32(self.compile_linear_f32_plan())
        } else {
            CompiledRenderPlan::Graph(self.compile_graph_plan())
        }
    }

    pub(super) fn compile_linear_f32_plan(&self) -> CompiledLinearPlan {
        let ops = self
            .chain_nodes
            .iter()
            .map(|&id| self.compile_op(id))
            .collect::<Vec<_>>();
        CompiledLinearPlan {
            input_channels: self.input_channels(),
            output_channels: self.output_channels(),
            segments: build_segments(&ops),
            ops,
        }
    }

    pub(super) fn compile_graph_plan(&self) -> CompiledGraphPlan {
        let ops = self
            .stages
            .iter()
            .flat_map(|stage| stage.nodes.iter().copied())
            .map(|id| self.compile_op(id))
            .collect::<Vec<_>>();
        CompiledGraphPlan {
            segments: build_segments(&ops),
        }
    }

    pub(super) fn compile_op(&self, id: NodeId) -> CompiledOp {
        let plugin = self.plugins[id].as_ref().unwrap();
        let node = &self.nodes[&id];
        let metadata = plugin.compile_metadata();
        CompiledOp::from_plugin(
            node,
            metadata,
            plugin.supports_f64(),
            Self::plugin_output_frames_for_input_isolated(plugin.as_ref(), id, &node.name, 100)
                == 100,
            Self::plugin_output_sample_rate_isolated(plugin.as_ref(), id, &node.name, 48_000)
                == 48_000,
        )
    }

    pub(super) fn can_process_f32_linear_chain(&self) -> bool {
        if self.chain_nodes.is_empty() || self.chain_nodes.len() != self.nodes.len() {
            return false;
        }
        if !self.cached_frames_identity
            || !self.cached_rate_identity
            || self.has_variable_frame_plugin
        {
            return false;
        }
        if self.input_nodes.len() != 1 || self.output_nodes.len() != 1 {
            return false;
        }
        if self.input_nodes[0] != self.chain_nodes[0]
            || self.output_nodes[0] != *self.chain_nodes.last().unwrap()
        {
            return false;
        }
        if self.edges.len() != self.chain_nodes.len().saturating_sub(1) {
            return false;
        }
        for &nid in &self.chain_nodes {
            let Some(node) = self.nodes.get(&nid) else {
                return false;
            };
            if node.input_channels() != node.output_channels() || self.plugins[nid].is_none() {
                return false;
            }
        }
        for pair in self.chain_nodes.windows(2) {
            let from = pair[0];
            let to = pair[1];
            let Some(edge) = self
                .edges
                .iter()
                .find(|edge| edge.from_node == from && edge.to_node == to)
            else {
                return false;
            };
            if edge.edge_type != EdgeType::Audio
                || edge.channel_map.is_some()
                || edge.destination_offset != 0
            {
                return false;
            }
        }
        true
    }

    pub(super) fn estimate_parallel_node_cost(
        plugin: &dyn Plugin,
        node_id: NodeId,
        node_name: &str,
    ) -> u32 {
        let metadata = plugin.compile_metadata();
        let has_latency_or_variable_frames = metadata.latency_samples > 0
            || Self::plugin_output_frames_for_input_isolated(plugin, node_id, node_name, 100)
                != 100
            || Self::plugin_output_sample_rate_isolated(plugin, node_id, node_name, 48_000)
                != 48_000;
        if has_latency_or_variable_frames {
            return HEAVY_PARALLEL_NODE_COST;
        }

        match metadata.cost_class {
            PluginCostClass::Scalar => {}
            PluginCostClass::Analyzer => return DEFAULT_PARALLEL_NODE_COST,
            PluginCostClass::Iir | PluginCostClass::Dynamics => {
                return MODERATE_PARALLEL_NODE_COST;
            }
            PluginCostClass::Fft | PluginCostClass::Convolution | PluginCostClass::External => {
                return HEAVY_PARALLEL_NODE_COST;
            }
        }

        let name = plugin.info().name.to_ascii_lowercase();
        if name.contains("spectral")
            || name.contains("linear phase")
            || name.contains("convolution")
            || name.contains("denoiser")
            || name.contains("declick")
            || name.contains("hiss")
            || name.contains("fir")
            || name.contains("fft")
        {
            HEAVY_PARALLEL_NODE_COST
        } else if name.contains("eq")
            || name.contains("compressor")
            || name.contains("limiter")
            || name.contains("expander")
            || name.contains("saturation")
            || name.contains("transient")
            || name.contains("delay")
            || name.contains("external")
        {
            MODERATE_PARALLEL_NODE_COST
        } else {
            DEFAULT_PARALLEL_NODE_COST
        }
    }

    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let id = self.reserve_node_id();
        self.add_plugin_with_id(id, plugin).map(|_| ())
    }

    pub(super) fn add_plugin_with_id(
        &mut self,
        id: NodeId,
        plugin: Box<dyn Plugin>,
    ) -> Result<NodeId, String> {
        let expected = if self.chain_nodes.is_empty() {
            self.config.initial_input_channels
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

    pub(super) fn renumber_edges(&mut self) {
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

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn poll_isolated_external_plugin_workers(
        &mut self,
    ) -> Vec<IsolatedExternalPluginWorkerReport> {
        let mut reports = Vec::new();
        for (plugin_index, node_id) in self.worker_report_nodes() {
            let Some(Some(plugin)) = self.plugins.get_mut(node_id) else {
                continue;
            };
            let Some(isolated) = plugin
                .as_any_mut()
                .and_then(|plugin| plugin.downcast_mut::<IsolatedExternalPlugin>())
            else {
                continue;
            };

            let (event, error) = match isolated.poll_worker() {
                Ok(event) => (event, None),
                Err(err) => (None, Some(err)),
            };
            reports.push(Self::isolated_external_plugin_report(
                plugin_index,
                node_id,
                isolated,
                event,
                error,
            ));
        }
        reports
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn ensure_isolated_external_plugin_workers_running(
        &mut self,
    ) -> Vec<IsolatedExternalPluginWorkerReport> {
        let mut reports = Vec::new();
        for (plugin_index, node_id) in self.worker_report_nodes() {
            let Some(Some(plugin)) = self.plugins.get_mut(node_id) else {
                continue;
            };
            let Some(isolated) = plugin
                .as_any_mut()
                .and_then(|plugin| plugin.downcast_mut::<IsolatedExternalPlugin>())
            else {
                continue;
            };

            let (event, error) = match isolated.ensure_worker_running_event() {
                Ok(event) => (Some(event), None),
                Err(err) => (None, Some(err)),
            };
            reports.push(Self::isolated_external_plugin_report(
                plugin_index,
                node_id,
                isolated,
                event,
                error,
            ));
        }
        reports
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(super) fn isolated_external_plugin_report(
        plugin_index: usize,
        node_id: NodeId,
        plugin: &IsolatedExternalPlugin,
        event: Option<ExternalPluginProcessEvent>,
        error: Option<String>,
    ) -> IsolatedExternalPluginWorkerReport {
        let sandbox = plugin.worker_sandbox_status();
        let sandbox_reason = sandbox_reason_text(sandbox.status, sandbox.backend, error.as_deref());
        IsolatedExternalPluginWorkerReport {
            plugin_index,
            node_id,
            plugin_instance_id: plugin.plugin_instance_id(),
            event,
            error,
            worker_start_count: plugin.worker_start_count(),
            worker_exit_count: plugin.worker_exit_count(),
            worker_launch_failure_count: plugin.worker_launch_failure_count(),
            block_timeout_count: plugin.block_timeout_count(),
            block_worker_failure_count: plugin.block_worker_failure_count(),
            block_wrong_sequence_count: plugin.block_wrong_sequence_count(),
            sandbox_status: sandbox.status,
            sandbox_backend: sandbox.backend,
            sandbox_reason,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn worker_report_nodes(&self) -> Vec<(usize, NodeId)> {
        let mut node_ids = self.nodes.keys().copied().collect::<Vec<_>>();
        node_ids.sort_unstable();
        node_ids
            .into_iter()
            .map(|node_id| {
                let plugin_index = self
                    .chain_nodes
                    .iter()
                    .position(|&chain_node_id| chain_node_id == node_id)
                    .unwrap_or(node_id);
                (plugin_index, node_id)
            })
            .collect()
    }

    pub fn input_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            self.config.initial_input_channels
        } else {
            self.nodes[&self.chain_nodes[0]].input_channels()
        }
    }
    pub fn output_channels(&self) -> usize {
        if self.chain_nodes.is_empty() {
            self.config.initial_input_channels
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
            let plugin = self.plugins[id].as_ref().unwrap();
            let node = &self.nodes[&id];
            result = Self::plugin_output_frames_for_input_isolated(
                plugin.as_ref(),
                id,
                &node.name,
                result,
            );
        }
        result
    }
    pub fn output_sample_rate(&self, r: u32) -> u32 {
        if self.cached_rate_identity {
            return r;
        }
        let mut result = r;
        for &id in &self.chain_nodes {
            let plugin = self.plugins[id].as_ref().unwrap();
            let node = &self.nodes[&id];
            result =
                Self::plugin_output_sample_rate_isolated(plugin.as_ref(), id, &node.name, result);
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
        val: super::super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.queue_node_parameter(nid, super::super::parameters::ParameterId::from(id), val)
    }

    /// Queue a parameter change for audio-thread application at `sample_offset`
    /// frames into the next `process()` call.
    pub fn set_plugin_parameter_at(
        &mut self,
        index: usize,
        id: &str,
        val: super::super::parameters::ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.queue_node_parameter_at(
            nid,
            super::super::parameters::ParameterId::from(id),
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
        let producer = self.queues.parameter_event_tx.as_mut().ok_or_else(|| {
            "parameter event sender has been taken; use the returned ParameterEventSender"
                .to_string()
        })?;
        producer.push(event).map_err(|err| {
            self.queue_state.dropped_parameter_events =
                self.queue_state.dropped_parameter_events.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: parameter event queue full; dropped {} events",
                self.queue_state.dropped_parameter_events
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
        self.queues
            .parameter_event_tx
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
        self.queues
            .graph_mutation_tx
            .take()
            .map(|producer| GraphMutationSender {
                producer,
                next_node_id: Arc::clone(&self.queue_state.graph_next_node_id),
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
        val: super::super::parameters::ParameterValue,
    ) -> Result<(), String> {
        let &nid = self.chain_nodes.get(index).ok_or("oob")?;
        self.apply_parameter_event(ParameterEvent {
            node_id: nid,
            param_id: super::super::parameters::ParameterId::from(id),
            value: val,
            sample_offset: 0,
        })
    }

    pub(super) fn drain_parameter_events_into(&mut self, events: &mut Vec<ParameterEvent>) {
        events.clear();
        while let Ok(event) = self.queues.parameter_event_rx.pop() {
            events.push(event);
        }
    }

    pub(super) fn plugin_supports_f64_isolated(
        plugin: &dyn Plugin,
        node_id: NodeId,
        node_name: &str,
    ) -> bool {
        match catch_unwind(AssertUnwindSafe(|| plugin.supports_f64())) {
            Ok(supports_f64) => supports_f64,
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked in supports_f64: {}; using f32 bridge",
                    node_name,
                    node_id,
                    reason
                );
                false
            }
        }
    }

    pub(super) fn plugin_output_frames_for_input_isolated(
        plugin: &dyn Plugin,
        node_id: NodeId,
        node_name: &str,
        input_frames: usize,
    ) -> usize {
        match catch_unwind(AssertUnwindSafe(|| {
            plugin.output_frames_for_input(input_frames)
        })) {
            Ok(output_frames) => output_frames,
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked in output_frames_for_input: {}; assuming identity frame count",
                    node_name,
                    node_id,
                    reason
                );
                input_frames
            }
        }
    }

    pub(super) fn plugin_output_sample_rate_isolated(
        plugin: &dyn Plugin,
        node_id: NodeId,
        node_name: &str,
        input_rate: u32,
    ) -> u32 {
        match catch_unwind(AssertUnwindSafe(|| plugin.output_sample_rate(input_rate))) {
            Ok(output_rate) => output_rate,
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked in output_sample_rate: {}; assuming identity sample rate",
                    node_name,
                    node_id,
                    reason
                );
                input_rate
            }
        }
    }

    pub(super) fn process_plugin_f32_isolated(
        plugin: &mut dyn Plugin,
        node: &GraphNode,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext<'_>,
    ) -> usize {
        let fallback = |output: &mut [f32]| {
            write_plugin_failure_passthrough(
                input,
                output,
                context.num_frames,
                node.input_channels(),
                node.output_channels(),
            )
        };

        match catch_unwind(AssertUnwindSafe(|| plugin.process(input, output, context))) {
            Ok(Ok(frames)) => {
                if frames.saturating_mul(node.output_channels()) <= output.len() {
                    frames
                } else {
                    crate::rate_limited_log!(
                        error,
                        5,
                        "host: plugin '{}' (node {}) returned {} frames but output buffer holds {} samples; using passthrough for this block",
                        node.name,
                        node.id,
                        frames,
                        output.len()
                    );
                    fallback(output)
                }
            }
            Ok(Err(err)) => {
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) process failed: {err}; using passthrough for this block",
                    node.name,
                    node.id
                );
                fallback(output)
            }
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked in process: {}; using passthrough for this block",
                    node.name,
                    node.id,
                    reason
                );
                fallback(output)
            }
        }
    }

    pub(super) fn process_compiled_plugin_f32_isolated(
        plugin: &mut dyn Plugin,
        node: &GraphNode,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext<'_>,
    ) -> Option<usize> {
        let result = match catch_unwind(AssertUnwindSafe(|| {
            plugin.process_compiled_f32(op, input, output, context)
        })) {
            Ok(result) => result?,
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked in compiled {:?}: {}; using regular process for this block",
                    node.name,
                    node.id,
                    op,
                    reason
                );
                return None;
            }
        };

        match result {
            Ok(frames) => {
                if frames.saturating_mul(node.output_channels()) <= output.len() {
                    Some(frames)
                } else {
                    crate::rate_limited_log!(
                        error,
                        5,
                        "host: plugin '{}' (node {}) compiled {:?} returned {} frames but output buffer holds {} samples; using regular process for this block",
                        node.name,
                        node.id,
                        op,
                        frames,
                        output.len()
                    );
                    None
                }
            }
            Err(err) => {
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) compiled {:?} failed: {err}; using regular process for this block",
                    node.name,
                    node.id,
                    op
                );
                None
            }
        }
    }

    pub(super) fn process_plugin_f64_isolated(
        plugin: &mut dyn Plugin,
        node: &GraphNode,
        input: &[f64],
        output: &mut [f64],
        context: &ProcessContext<'_>,
    ) -> usize {
        let fallback = |output: &mut [f64]| {
            write_plugin_failure_passthrough(
                input,
                output,
                context.num_frames,
                node.input_channels(),
                node.output_channels(),
            )
        };

        match catch_unwind(AssertUnwindSafe(|| {
            plugin.process_f64(input, output, context)
        })) {
            Ok(Ok(frames)) => {
                if frames.saturating_mul(node.output_channels()) <= output.len() {
                    frames
                } else {
                    crate::rate_limited_log!(
                        error,
                        5,
                        "host: plugin '{}' (node {}) returned {} f64 frames but output buffer holds {} samples; using passthrough for this block",
                        node.name,
                        node.id,
                        frames,
                        output.len()
                    );
                    fallback(output)
                }
            }
            Ok(Err(err)) => {
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) f64 process failed: {err}; using passthrough for this block",
                    node.name,
                    node.id
                );
                fallback(output)
            }
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked in f64 process: {}; using passthrough for this block",
                    node.name,
                    node.id,
                    reason
                );
                fallback(output)
            }
        }
    }

    pub(super) fn apply_parameter_event(&mut self, event: ParameterEvent) -> Result<(), String> {
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
        match catch_unwind(AssertUnwindSafe(|| plugin.set_parameter(param_id, value))) {
            Ok(result) => result.map_err(|err| {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "host: queued parameter event failed for node {} '{}': {err}",
                    node_id,
                    node.name
                );
                err
            }),
            Err(payload) => {
                let reason = panic_payload_description(payload.as_ref());
                crate::rate_limited_log!(
                    error,
                    5,
                    "host: plugin '{}' (node {}) panicked while applying parameter: {}",
                    node.name,
                    node_id,
                    reason
                );
                Err(format!(
                    "plugin '{}' (node {}) panicked while applying parameter: {}",
                    node.name, node_id, reason
                ))
            }
        }
    }

    /// Number of parameter events dropped because the RT queue was full.
    pub fn dropped_parameter_events(&self) -> u64 {
        self.queue_state.dropped_parameter_events
    }

    /// Queue a graph mutation through the host-owned producer.
    ///
    /// This is useful before handing the producer to another thread. After
    /// `take_graph_mutation_sender()` is called, use that sender instead.
    pub(super) fn queue_graph_mutation(&mut self, mutation: GraphMutation) -> Result<(), String> {
        let producer = self.queues.graph_mutation_tx.as_mut().ok_or_else(|| {
            "graph mutation sender has been taken; use the returned GraphMutationSender".to_string()
        })?;
        producer.push(mutation).map_err(|err| {
            self.queue_state.dropped_graph_mutations =
                self.queue_state.dropped_graph_mutations.saturating_add(1);
            crate::rate_limited_log!(
                warn,
                5,
                "host: graph mutation queue full; dropped {} mutations",
                self.queue_state.dropped_graph_mutations
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
        self.queue_state.dropped_graph_mutations
    }

    pub(super) fn drain_graph_mutations(&mut self) -> Result<(), String> {
        while let Ok(mutation) = self.queues.graph_mutation_rx.pop() {
            self.apply_graph_mutation(mutation)?;
        }
        Ok(())
    }

    pub(super) fn apply_graph_mutation(&mut self, mutation: GraphMutation) -> Result<(), String> {
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
    pub(super) fn set_bypass_state(&mut self, id: NodeId, bypassed: bool) {
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
        if let Some((index, sample)) = input
            .iter()
            .copied()
            .enumerate()
            .find(|(_, sample)| !sample.is_finite())
        {
            output.fill(0.0);
            return Err(format!(
                "host input contains non-finite sample at index {index}: {sample}"
            ));
        }
        let mut events = std::mem::take(&mut self.queues.parameter_event_scratch);
        self.drain_parameter_events_into(&mut events);
        let result = self.process_with_parameter_events(input, output, &mut events);
        self.queues.parameter_event_scratch = events;
        result
    }

    pub(super) fn process_with_parameter_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        events: &mut Vec<ParameterEvent>,
    ) -> Result<usize, String> {
        if events.is_empty() {
            return self.process_block_without_parameter_events(
                input,
                output,
                self.automation_state.playback_position as u64,
            );
        }

        if events.iter().any(|event| event.sample_offset > 0)
            && self.can_split_parameter_event_block(input, output)
        {
            return self.process_split_parameter_events(
                input,
                output,
                events,
                self.automation_state.playback_position as u64,
            );
        }

        for event in events.drain(..) {
            self.apply_parameter_event(event)?;
        }
        self.process_block_without_parameter_events(
            input,
            output,
            self.automation_state.playback_position as u64,
        )
    }

    pub(super) fn can_split_parameter_event_block(&self, input: &[f32], output: &[f32]) -> bool {
        if !self.automation_state.automation.is_empty()
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

    pub(super) fn process_split_parameter_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        events: &mut Vec<ParameterEvent>,
        block_start_sample: u64,
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
                    block_start_sample + frame_cursor as u64,
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

    pub(super) fn process_block_without_parameter_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        block_start_sample: u64,
    ) -> Result<usize, String> {
        if self.nodes.is_empty() {
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels());
        }
        let nf = input.len() / self.input_channels();
        let max_of = self.output_frames_for_input(nf);
        let out_ch = self.output_channels();
        self.apply_automation_for_block(nf);
        if let Some(plan) = self.compiled_plan.linear_f32().cloned() {
            return self.process_compiled_linear_f32_plan(&plan, input, output, block_start_sample);
        }
        let stage_block_start_sample = block_start_sample;

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
                self.config.parallel_enabled,
                stage,
                input,
                self.config.sample_rate,
                cf,
                stage_block_start_sample,
                &mut self.plugins,
                &self.nodes,
                &self.predecessors,
                &self.is_input_node,
                &self.bypassed,
                &self.cached_parallel_node_costs,
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
                    let context = ProcessContext::new(self.config.sample_rate, cf)
                        .with_sample_position(stage_block_start_sample);
                    let mof = Self::plugin_output_frames_for_input_isolated(
                        p.as_ref(),
                        nid,
                        &node.name,
                        cf,
                    );
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
                    let out_frames = Self::process_plugin_f32_isolated(
                        p.as_mut(),
                        node,
                        &bufs.scratch_input[..in_len],
                        &mut bufs.scratch_output[..process_output_len],
                        &context,
                    );
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
        self.automation_state.playback_position += nf;

        // BufferGuard's Drop impl returns bufs to self.process_buffers when
        // `guard` falls out of scope here.
        Ok(cf)
    }

    pub(super) fn process_compiled_linear_f32_plan(
        &mut self,
        plan: &CompiledLinearPlan,
        input: &[f32],
        output: &mut [f32],
        block_start_sample: u64,
    ) -> Result<usize, String> {
        let input_channels = plan.input_channels;
        let linear_gain_regions = self.compiled_linear_gain_regions(plan);
        let static_gains = (0..plan.ops.len())
            .map(|idx| self.compiled_op_static_gain(plan, idx))
            .collect::<Vec<_>>();
        let mut guard = BufferGuard::take(&mut self.process_buffers);
        let bufs = guard.get_mut();

        let mut current_source = CompiledLinearSource::ExternalInput;
        let mut current_len = input.len();
        let mut current_frames = input.len() / input_channels;
        let mut active_gain_region: Option<(usize, f32)> = None;

        let mut idx = 0;
        while idx < plan.ops.len() {
            if active_gain_region.is_none() {
                active_gain_region = linear_gain_regions[idx];
            }

            if active_gain_region.is_some() && static_gains[idx].is_some() {
                idx += 1;
                if let Some((region_end, gain)) = active_gain_region
                    && idx == region_end
                {
                    current_source = Self::apply_compiled_region_gain(
                        bufs,
                        input,
                        output,
                        current_source,
                        current_len,
                        gain,
                        region_end == plan.ops.len(),
                    )?;
                    active_gain_region = None;
                }
                continue;
            }

            let op = &plan.ops[idx];
            let nid = op.node_id;
            let node = self
                .nodes
                .get(&nid)
                .ok_or_else(|| format!("Missing node {nid} during compiled f32 processing"))?;
            let region_writes_final_gain = active_gain_region
                .is_some_and(|(region_end, gain)| region_end == plan.ops.len() && gain != 1.0);
            let is_last = idx + 1 == plan.ops.len() && !region_writes_final_gain;
            let output_frames = {
                let plugin = self.plugins[nid].as_ref().unwrap();
                Self::plugin_output_frames_for_input_isolated(
                    plugin.as_ref(),
                    nid,
                    &node.name,
                    current_frames,
                )
            };
            let output_len = output_frames * node.output_channels();

            let frames = match (is_last, current_source) {
                (true, CompiledLinearSource::ExternalInput) => {
                    if output.len() < output_len {
                        return Err(format!(
                            "f32 output too small: need {output_len} samples, got {}",
                            output.len()
                        ));
                    }
                    Self::process_f32_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        op.kind,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &input[..current_len],
                        &mut output[..output_len],
                        self.config.sample_rate,
                        block_start_sample,
                        current_frames,
                    )?
                }
                (true, CompiledLinearSource::ScratchInput) => {
                    if output.len() < output_len {
                        return Err(format!(
                            "f32 output too small: need {output_len} samples, got {}",
                            output.len()
                        ));
                    }
                    Self::process_f32_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        op.kind,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &bufs.scratch_input[..current_len],
                        &mut output[..output_len],
                        self.config.sample_rate,
                        block_start_sample,
                        current_frames,
                    )?
                }
                (true, CompiledLinearSource::ScratchOutput) => {
                    if output.len() < output_len {
                        return Err(format!(
                            "f32 output too small: need {output_len} samples, got {}",
                            output.len()
                        ));
                    }
                    Self::process_f32_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        op.kind,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &bufs.scratch_output[..current_len],
                        &mut output[..output_len],
                        self.config.sample_rate,
                        block_start_sample,
                        current_frames,
                    )?
                }
                (false, CompiledLinearSource::ExternalInput) => {
                    ensure_len(&mut bufs.scratch_output, output_len);
                    let frames = Self::process_f32_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        op.kind,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &input[..current_len],
                        &mut bufs.scratch_output[..output_len],
                        self.config.sample_rate,
                        block_start_sample,
                        current_frames,
                    )?;
                    current_source = CompiledLinearSource::ScratchOutput;
                    frames
                }
                (false, CompiledLinearSource::ScratchInput) => {
                    ensure_len(&mut bufs.scratch_output, output_len);
                    let frames = Self::process_f32_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        op.kind,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &bufs.scratch_input[..current_len],
                        &mut bufs.scratch_output[..output_len],
                        self.config.sample_rate,
                        block_start_sample,
                        current_frames,
                    )?;
                    current_source = CompiledLinearSource::ScratchOutput;
                    frames
                }
                (false, CompiledLinearSource::ScratchOutput) => {
                    ensure_len(&mut bufs.scratch_input, output_len);
                    let frames = Self::process_f32_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        op.kind,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &bufs.scratch_output[..current_len],
                        &mut bufs.scratch_input[..output_len],
                        self.config.sample_rate,
                        block_start_sample,
                        current_frames,
                    )?;
                    current_source = CompiledLinearSource::ScratchInput;
                    frames
                }
            };

            current_frames = frames;
            current_len = frames * node.output_channels();
            idx += 1;
            if let Some((region_end, gain)) = active_gain_region
                && idx == region_end
            {
                current_source = Self::apply_compiled_region_gain(
                    bufs,
                    input,
                    output,
                    current_source,
                    current_len,
                    gain,
                    region_end == plan.ops.len(),
                )?;
                active_gain_region = None;
            }
        }

        self.automation_state.playback_position += input.len() / input_channels;
        Ok(current_frames)
    }

    fn compiled_linear_gain_regions(&self, plan: &CompiledLinearPlan) -> Vec<Option<(usize, f32)>> {
        (0..plan.ops.len())
            .map(|start| self.compiled_linear_gain_region(plan, start))
            .collect()
    }

    fn compiled_linear_gain_region(
        &self,
        plan: &CompiledLinearPlan,
        start: usize,
    ) -> Option<(usize, f32)> {
        let first = plan.ops.get(start)?;
        if first.barrier.is_some()
            || first.metadata_boundary
            || !first.linear
            || !first.time_invariant_for_block
            || !first.can_absorb_input_gain
            || !first.can_absorb_output_gain
            || first.input_channels != first.output_channels
        {
            return None;
        }

        let mut end = start;
        let mut gain = 1.0_f32;
        let mut saw_static_gain = false;
        while let Some(op) = plan.ops.get(end) {
            if op.barrier.is_some()
                || op.metadata_boundary
                || !op.linear
                || !op.time_invariant_for_block
                || !op.can_absorb_input_gain
                || !op.can_absorb_output_gain
                || op.input_channels != op.output_channels
            {
                break;
            }
            if let Some(op_gain) = self.compiled_op_static_gain(plan, end) {
                saw_static_gain = true;
                gain *= op_gain;
            } else if op.kind == CompiledOpKind::ApplyGain {
                break;
            }
            end += 1;
        }
        (saw_static_gain && end > start).then_some((end, gain))
    }

    fn compiled_op_static_gain(&self, plan: &CompiledLinearPlan, idx: usize) -> Option<f32> {
        let op = plan.ops.get(idx)?;
        if op.kind != CompiledOpKind::ApplyGain {
            return None;
        }
        let nid = op.node_id;
        if self.bypassed.get(nid).copied().unwrap_or(false) {
            return Some(1.0);
        }
        self.plugins
            .get(nid)
            .and_then(|plugin| plugin.as_ref())
            .and_then(|plugin| plugin.compile_metadata().static_gain)
    }

    fn apply_compiled_region_gain(
        bufs: &mut ProcessBuffers<f32>,
        input: &[f32],
        output: &mut [f32],
        current_source: CompiledLinearSource,
        current_len: usize,
        gain: f32,
        is_final: bool,
    ) -> Result<CompiledLinearSource, String> {
        if is_final {
            if output.len() < current_len {
                return Err(format!(
                    "f32 output too small: need {current_len} samples, got {}",
                    output.len()
                ));
            }
            match current_source {
                CompiledLinearSource::ExternalInput => {
                    Self::write_scaled_f32(&input[..current_len], &mut output[..current_len], gain)
                }
                CompiledLinearSource::ScratchInput => Self::write_scaled_f32(
                    &bufs.scratch_input[..current_len],
                    &mut output[..current_len],
                    gain,
                ),
                CompiledLinearSource::ScratchOutput => Self::write_scaled_f32(
                    &bufs.scratch_output[..current_len],
                    &mut output[..current_len],
                    gain,
                ),
            }
            return Ok(current_source);
        }

        match current_source {
            CompiledLinearSource::ExternalInput => {
                ensure_len(&mut bufs.scratch_output, current_len);
                Self::write_scaled_f32(
                    &input[..current_len],
                    &mut bufs.scratch_output[..current_len],
                    gain,
                );
                Ok(CompiledLinearSource::ScratchOutput)
            }
            CompiledLinearSource::ScratchInput => {
                ensure_len(&mut bufs.scratch_output, current_len);
                Self::write_scaled_f32(
                    &bufs.scratch_input[..current_len],
                    &mut bufs.scratch_output[..current_len],
                    gain,
                );
                Ok(CompiledLinearSource::ScratchOutput)
            }
            CompiledLinearSource::ScratchOutput => {
                ensure_len(&mut bufs.scratch_input, current_len);
                Self::write_scaled_f32(
                    &bufs.scratch_output[..current_len],
                    &mut bufs.scratch_input[..current_len],
                    gain,
                );
                Ok(CompiledLinearSource::ScratchInput)
            }
        }
    }

    fn write_scaled_f32(input: &[f32], output: &mut [f32], gain: f32) {
        debug_assert!(output.len() >= input.len());
        if (gain - 1.0).abs() <= f32::EPSILON {
            output[..input.len()].copy_from_slice(input);
        } else {
            for (dst, src) in output.iter_mut().zip(input.iter()) {
                *dst = *src * gain;
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "plugin process signature mirrors the underlying audio graph API"
    )]
    pub(super) fn process_f32_node(
        plugin: &mut dyn Plugin,
        node: &GraphNode,
        op_kind: CompiledOpKind,
        bypassed: bool,
        input: &[f32],
        output: &mut [f32],
        sample_rate: u32,
        sample_position: u64,
        num_frames: usize,
    ) -> Result<usize, String> {
        if bypassed {
            output[..input.len()].copy_from_slice(input);
            return Ok(num_frames);
        }
        let context =
            ProcessContext::new(sample_rate, num_frames).with_sample_position(sample_position);
        if let Some(frames) = plugin_compiled_op(op_kind).and_then(|compiled_op| {
            Self::process_compiled_plugin_f32_isolated(
                plugin,
                node,
                compiled_op,
                input,
                output,
                &context,
            )
        }) {
            return Ok(frames);
        }
        Ok(Self::process_plugin_f32_isolated(
            plugin, node, input, output, &context,
        ))
    }

    pub(super) fn apply_automation_for_block(&mut self, nf: usize) {
        // Apply automation: evaluate curves at current position and set parameters.
        // `eval_curve` interprets (sample, num_frames) as a position within a window,
        // so we use each automation's relative position and advance it by nf each call.
        if self.automation_state.automation.is_empty() {
            return;
        }

        self.automation_scratch.clear();
        for (idx, slot) in self.automation_state.automation.iter().enumerate() {
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
            let slot = &mut self.automation_state.automation[idx];
            if let Some(p) = self.plugins[slot.node_id].as_mut() {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    p.set_parameter(slot.param_id.clone(), ParameterValue::Float(val))
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => {
                        crate::rate_limited_log!(
                            warn,
                            5,
                            "host: automation parameter update failed for node {}: {err}",
                            slot.node_id
                        );
                    }
                    Err(payload) => {
                        let reason = panic_payload_description(payload.as_ref());
                        crate::rate_limited_log!(
                            error,
                            5,
                            "host: plugin at node {} panicked during automation parameter update: {}",
                            slot.node_id,
                            reason
                        );
                    }
                }
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
        if let Some((index, sample)) = input
            .iter()
            .copied()
            .enumerate()
            .find(|(_, sample)| !sample.is_finite())
        {
            output.fill(0.0);
            return Err(format!(
                "host input contains non-finite sample at index {index}: {sample}"
            ));
        }
        let mut events = std::mem::take(&mut self.queues.parameter_event_scratch);
        self.drain_parameter_events_into(&mut events);
        let result = self.process_f64_with_parameter_events(input, output, &mut events);
        self.queues.parameter_event_scratch = events;
        result
    }

    pub(super) fn process_f64_with_parameter_events(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        events: &mut Vec<ParameterEvent>,
    ) -> Result<usize, String> {
        if events.is_empty() {
            return self.process_f64_without_parameter_events(
                input,
                output,
                self.automation_state.playback_position as u64,
            );
        }

        if events.iter().any(|event| event.sample_offset > 0)
            && self.can_split_parameter_event_block_f64(input, output)
        {
            return self.process_f64_split_parameter_events(
                input,
                output,
                events,
                self.automation_state.playback_position as u64,
            );
        }

        for event in events.drain(..) {
            self.apply_parameter_event(event)?;
        }
        self.process_f64_without_parameter_events(
            input,
            output,
            self.automation_state.playback_position as u64,
        )
    }

    pub(super) fn can_split_parameter_event_block_f64(
        &self,
        input: &[f64],
        output: &[f64],
    ) -> bool {
        if !self.automation_state.automation.is_empty()
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

    pub(super) fn process_f64_split_parameter_events(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        events: &mut Vec<ParameterEvent>,
        block_start_sample: u64,
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
                    block_start_sample + frame_cursor as u64,
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

    pub(super) fn process_f64_without_parameter_events(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        block_start_sample: u64,
    ) -> Result<usize, String> {
        if self.nodes.is_empty() {
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels());
        }
        if self.can_process_f64_chain_native() {
            return self.process_f64_chain_native(input, output, block_start_sample);
        }
        if self.can_process_f64_graph_native() {
            return self.process_f64_graph_native(input, output, block_start_sample);
        }
        let mut input_scratch = std::mem::take(&mut self.config.f64_input_scratch);
        let mut output_scratch = std::mem::take(&mut self.config.f64_output_scratch);

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
            block_start_sample,
        );
        let frames = match result {
            Ok(frames) => frames,
            Err(err) => {
                self.config.f64_input_scratch = input_scratch;
                self.config.f64_output_scratch = output_scratch;
                return Err(err);
            }
        };
        for (dst, &src) in output.iter_mut().zip(output_scratch[..out_len].iter()) {
            *dst = src as f64;
        }

        self.config.f64_input_scratch = input_scratch;
        self.config.f64_output_scratch = output_scratch;
        Ok(frames)
    }

    pub(super) fn can_process_f64_graph_native(&self) -> bool {
        !self.nodes.is_empty()
            && self.nodes.keys().copied().all(|nid| {
                self.bypassed.get(nid).copied().unwrap_or(false)
                    || self.plugins[nid].as_ref().is_some_and(|plugin| {
                        let node = &self.nodes[&nid];
                        Self::plugin_supports_f64_isolated(plugin.as_ref(), nid, &node.name)
                    })
            })
    }

    pub(super) fn can_process_f64_chain_native(&self) -> bool {
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
            if edge.edge_type != EdgeType::Audio
                || edge.channel_map.is_some()
                || edge.destination_offset != 0
            {
                return false;
            }
        }
        self.chain_nodes.iter().all(|&nid| {
            self.bypassed.get(nid).copied().unwrap_or(false)
                || self.plugins[nid].as_ref().is_some_and(|plugin| {
                    let node = &self.nodes[&nid];
                    Self::plugin_supports_f64_isolated(plugin.as_ref(), nid, &node.name)
                })
        })
    }

    pub(super) fn process_f64_graph_native(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        block_start_sample: u64,
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
                    let context = ProcessContext::new(self.config.sample_rate, cf)
                        .with_sample_position(block_start_sample);
                    let max_output_frames = Self::plugin_output_frames_for_input_isolated(
                        plugin.as_ref(),
                        nid,
                        &node.name,
                        cf,
                    );
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
                    let frames = Self::process_plugin_f64_isolated(
                        plugin.as_mut(),
                        node,
                        &bufs.scratch_input[..in_len],
                        &mut bufs.scratch_output[..process_output_len],
                        &context,
                    );
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
        self.automation_state.playback_position += nf;

        Ok(cf)
    }

    pub(super) fn process_f64_chain_native(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        block_start_sample: u64,
    ) -> Result<usize, String> {
        let mut scratch_a = std::mem::take(&mut self.config.f64_chain_scratch);
        let mut scratch_b = std::mem::take(&mut self.config.f64_chain_scratch_alt);

        ensure_len(&mut scratch_a, input.len());
        scratch_a[..input.len()].copy_from_slice(input);

        let mut current_in_a = true;
        let mut current_len = input.len();
        let mut current_frames = input.len() / self.input_channels();
        let mut current_rate = self.config.sample_rate;

        for idx in 0..self.chain_nodes.len() {
            let nid = self.chain_nodes[idx];
            let node = self
                .nodes
                .get(&nid)
                .ok_or_else(|| format!("Missing node {nid} during f64 processing"))?;
            let is_last = idx + 1 == self.chain_nodes.len();
            let output_frames = {
                let plugin = self.plugins[nid].as_ref().unwrap();
                Self::plugin_output_frames_for_input_isolated(
                    plugin.as_ref(),
                    nid,
                    &node.name,
                    current_frames,
                )
            };
            let output_len = output_frames * node.output_channels();

            let frames = if is_last {
                if output.len() < output_len {
                    self.config.f64_chain_scratch = scratch_a;
                    self.config.f64_chain_scratch_alt = scratch_b;
                    return Err(format!(
                        "f64 output too small: need {output_len} samples, got {}",
                        output.len()
                    ));
                }
                if current_in_a {
                    Self::process_f64_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &scratch_a[..current_len],
                        &mut output[..output_len],
                        current_rate,
                        block_start_sample,
                        current_frames,
                    )?
                } else {
                    Self::process_f64_node(
                        self.plugins[nid].as_mut().unwrap().as_mut(),
                        node,
                        self.bypassed.get(nid).copied().unwrap_or(false),
                        &scratch_b[..current_len],
                        &mut output[..output_len],
                        current_rate,
                        block_start_sample,
                        current_frames,
                    )?
                }
            } else if current_in_a {
                ensure_len(&mut scratch_b, output_len);
                let frames = Self::process_f64_node(
                    self.plugins[nid].as_mut().unwrap().as_mut(),
                    node,
                    self.bypassed.get(nid).copied().unwrap_or(false),
                    &scratch_a[..current_len],
                    &mut scratch_b[..output_len],
                    current_rate,
                    block_start_sample,
                    current_frames,
                )?;
                current_in_a = false;
                frames
            } else {
                ensure_len(&mut scratch_a, output_len);
                let frames = Self::process_f64_node(
                    self.plugins[nid].as_mut().unwrap().as_mut(),
                    node,
                    self.bypassed.get(nid).copied().unwrap_or(false),
                    &scratch_b[..current_len],
                    &mut scratch_a[..output_len],
                    current_rate,
                    block_start_sample,
                    current_frames,
                )?;
                current_in_a = true;
                frames
            };

            current_frames = frames;
            current_len = frames * node.output_channels();
            current_rate = {
                let plugin = self.plugins[nid].as_ref().unwrap();
                Self::plugin_output_sample_rate_isolated(
                    plugin.as_ref(),
                    nid,
                    &node.name,
                    current_rate,
                )
            };
        }

        self.config.f64_chain_scratch = scratch_a;
        self.config.f64_chain_scratch_alt = scratch_b;
        self.automation_state.playback_position += input.len() / self.input_channels();
        Ok(current_frames)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "plugin process signature mirrors the underlying audio graph API"
    )]
    pub(super) fn process_f64_node(
        plugin: &mut dyn Plugin,
        node: &GraphNode,
        bypassed: bool,
        input: &[f64],
        output: &mut [f64],
        sample_rate: u32,
        sample_position: u64,
        num_frames: usize,
    ) -> Result<usize, String> {
        if bypassed {
            output[..input.len()].copy_from_slice(input);
            return Ok(num_frames);
        }
        let context =
            ProcessContext::new(sample_rate, num_frames).with_sample_position(sample_position);
        Ok(Self::process_plugin_f64_isolated(
            plugin, node, input, output, &context,
        ))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "parallel stage dispatch needs the full graph context; a struct would just move the fields"
    )]
    pub(super) fn process_stage_parallel(
        parallel_enabled: bool,
        stage: &ProcessingStage,
        input: &[f32],
        sample_rate: u32,
        cf: usize,
        sample_position: u64,
        plugins: &mut [Option<Box<dyn Plugin>>],
        nodes: &HashMap<NodeId, GraphNode>,
        predecessors: &[Vec<GraphEdge>],
        is_input_node: &[bool],
        bypassed: &[bool],
        parallel_node_costs: &[u32],
        bufs: &mut ProcessBuffers<f32>,
    ) -> Option<Result<usize, String>> {
        if !parallel_enabled
            || !Self::should_parallelize_stage(stage, cf, nodes, parallel_node_costs)
        {
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
                if edge.edge_type != EdgeType::Audio
                    || edge.channel_map.is_some()
                    || edge.destination_offset != 0
                {
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
                        let context = ProcessContext::new(sample_rate, cf)
                            .with_sample_position(sample_position);
                        let max_output_frames = Self::plugin_output_frames_for_input_isolated(
                            plugin.as_ref(),
                            nid,
                            &node.name,
                            cf,
                        );
                        let output_len = max_output_frames * node.output_channels();
                        ensure_len(scratch_output, output_len);
                        let frames = Self::process_plugin_f32_isolated(
                            plugin.as_mut(),
                            node,
                            &scratch_input[..in_len],
                            &mut scratch_output[..output_len],
                            &context,
                        );
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

    pub(super) fn should_parallelize_stage(
        stage: &ProcessingStage,
        frames: usize,
        nodes: &HashMap<NodeId, GraphNode>,
        parallel_node_costs: &[u32],
    ) -> bool {
        if stage.nodes.len() < 2 {
            return false;
        }
        Self::stage_work_units(stage, frames, nodes, parallel_node_costs)
            >= MIN_PARALLEL_STAGE_WORK_UNITS
    }

    pub(super) fn stage_work_units(
        stage: &ProcessingStage,
        frames: usize,
        nodes: &HashMap<NodeId, GraphNode>,
        parallel_node_costs: &[u32],
    ) -> usize {
        stage.nodes.iter().fold(0usize, |total, &nid| {
            let channels = nodes
                .get(&nid)
                .map(|node| node.input_channels().max(node.output_channels()).max(1))
                .unwrap_or(1);
            let cost = parallel_node_costs
                .get(nid)
                .copied()
                .unwrap_or(DEFAULT_PARALLEL_NODE_COST) as usize;
            total.saturating_add(frames.saturating_mul(channels).saturating_mul(cost))
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "internal graph wiring helper: all arguments are distinct scratch buffers"
    )]
    pub(super) fn merge_inputs_into<T: AudioSample>(
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
                EdgeType::Audio => e.destination_offset,
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
                EdgeType::Audio => primary_channels.saturating_sub(dest_offset),
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
    #[allow(
        clippy::too_many_arguments,
        reason = "internal latency-compensation helper: scratch buffers and destination slices are separate concerns"
    )]
    pub(super) fn apply_compensation_and_sum_at<T: AudioSample>(
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

    pub(super) fn sum_interleaved_at<T: AudioSample>(
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

    pub(super) fn collect_output_from_buffers<T: AudioSample>(
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
    pub(super) fn compute_latency(&self) -> usize {
        self.output_nodes
            .iter()
            .map(|&id| self.path_latency(id))
            .max()
            .unwrap_or(0)
    }
    pub(super) fn path_latency(&self, id: NodeId) -> usize {
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
    pub(super) fn compute_compensation_delays<T: AudioSample>(
        &mut self,
        num_slots: usize,
    ) -> Result<CompensationDelays<T>, String> {
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
                        delays.set(edge.id, DelayBuffer::new(compensation, delay_channels))?;
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

        Ok(delays)
    }

    pub(super) fn routed_channel_count(
        dest_node: &GraphNode,
        edge: &GraphEdge,
        source_channels: usize,
        primary_channels: usize,
        sidechain_offset: &mut usize,
    ) -> usize {
        let available_dest_channels = match edge.edge_type {
            EdgeType::Audio => primary_channels.saturating_sub(edge.destination_offset),
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

    pub(super) fn has_cycle(&self) -> bool {
        let mut v = HashSet::new();
        let mut r = HashSet::new();
        for &id in self.nodes.keys() {
            if self.cycle_util(id, &mut v, &mut r) {
                return true;
            }
        }
        false
    }
    pub(super) fn cycle_util(
        &self,
        id: NodeId,
        v: &mut HashSet<NodeId>,
        r: &mut HashSet<NodeId>,
    ) -> bool {
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
    pub(super) fn compute_io_nodes(&mut self) {
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
    pub(super) fn compute_stages(&mut self) -> Result<(), String> {
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
    fn set_plugin_parameter(
        &mut self,
        i: usize,
        param_id: &str,
        value: super::super::parameters::ParameterValue,
    ) -> Result<(), String> {
        DawHost::set_plugin_parameter(self, i, param_id, value)
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

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn poll_isolated_external_plugin_workers(&mut self) -> Vec<IsolatedExternalPluginWorkerReport> {
        DawHost::poll_isolated_external_plugin_workers(self)
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn ensure_isolated_external_plugin_workers_running(
        &mut self,
    ) -> Vec<IsolatedExternalPluginWorkerReport> {
        DawHost::ensure_isolated_external_plugin_workers_running(self)
    }
}
