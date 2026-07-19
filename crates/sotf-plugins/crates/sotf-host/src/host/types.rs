use super::audio_sample::AudioSample;
use super::compensation_delays::CompensationDelays;
use super::graph_edge::GraphEdge;
use super::node_buffer::NodeBuffer;
use crate::automation::ParameterAutomation;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::external_plugin_ipc::{PluginSandboxBackendCode, PluginSandboxStatusCode};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::external_plugin_process::ExternalPluginProcessEvent;
use crate::parameters::ParameterId;
use crate::plugin::Plugin;

pub(super) struct ProcessBuffers<T: AudioSample> {
    pub(super) node_buffers: Vec<Option<NodeBuffer<T>>>,
    pub(super) scratch_input: Vec<T>,
    pub(super) scratch_output: Vec<T>,
    pub(super) merge_buffer: Vec<T>,
    pub(super) channel_map_buffer: Vec<T>,
    /// Per-edge latency compensation delay buffers, indexed by `GraphEdge::id`.
    /// `None` means the edge is already aligned and needs no delay.
    pub(super) compensation_delays: CompensationDelays<T>,
    /// Scratch buffer for frame-by-frame delay processing (avoids per-frame allocation).
    pub(super) delay_scratch: Vec<T>,
    /// Per-node scratch buffers for parallel stage processing.
    /// Each entry: (scratch_input, scratch_output, merge_buffer).
    /// Only allocated for nodes in stages with 2+ nodes.
    #[allow(dead_code)]
    pub(super) parallel_scratch: Vec<(Vec<T>, Vec<T>, Vec<T>)>,
    pub(super) parallel_results: Vec<Result<usize, String>>,
}

pub type NodeId = usize;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug)]
pub struct IsolatedExternalPluginWorkerReport {
    pub plugin_index: usize,
    pub node_id: NodeId,
    pub plugin_instance_id: Option<usize>,
    pub event: Option<ExternalPluginProcessEvent>,
    pub error: Option<String>,
    pub worker_start_count: u64,
    pub worker_exit_count: u64,
    pub worker_launch_failure_count: u64,
    pub block_timeout_count: u64,
    pub block_worker_failure_count: u64,
    pub block_wrong_sequence_count: u64,
    pub sandbox_status: PluginSandboxStatusCode,
    pub sandbox_backend: PluginSandboxBackendCode,
    pub sandbox_reason: Option<String>,
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

pub(super) struct AutomationSlot {
    pub(super) node_id: NodeId,
    pub(super) param_id: ParameterId,
    pub(super) automation: ParameterAutomation,
}

pub(super) enum GraphMutation {
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
