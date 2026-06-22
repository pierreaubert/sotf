use super::graph_node::GraphNode;
use super::types::NodeId;
use crate::plugin::{PluginCompileMetadata, PluginCompiledOp, PluginCostClass};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CompiledRenderPlan {
    EmptyPassthrough,
    LinearF32(CompiledLinearPlan),
    Graph(CompiledGraphPlan),
}

impl Default for CompiledRenderPlan {
    fn default() -> Self {
        Self::Graph(CompiledGraphPlan::default())
    }
}

impl CompiledRenderPlan {
    pub(super) fn linear_f32(&self) -> Option<&CompiledLinearPlan> {
        match self {
            Self::LinearF32(plan) => Some(plan),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct CompiledGraphPlan {
    pub(super) segments: Vec<CompiledSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompiledLinearPlan {
    pub(super) input_channels: usize,
    pub(super) output_channels: usize,
    pub(super) ops: Vec<CompiledOp>,
    pub(super) segments: Vec<CompiledSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompiledOp {
    pub(super) node_id: NodeId,
    pub(super) input_channels: usize,
    pub(super) output_channels: usize,
    pub(super) kind: CompiledOpKind,
    pub(super) cost_class: PluginCostClass,
    pub(super) linear: bool,
    pub(super) time_invariant_for_block: bool,
    pub(super) channel_mixing: bool,
    pub(super) stateful: bool,
    pub(super) can_absorb_input_gain: bool,
    pub(super) can_absorb_output_gain: bool,
    pub(super) can_merge_with_eq: bool,
    pub(super) metadata_boundary: bool,
    pub(super) latency_samples: usize,
    pub(super) supports_f64: bool,
    pub(super) same_frame_count: bool,
    pub(super) same_sample_rate: bool,
    pub(super) barrier: Option<CompiledBarrierKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompiledOpKind {
    PluginFallback,
    ApplyGain,
    EqBiquadBank,
    ChannelMuteSolo,
    Limiter,
    MultibandCompressor,
    AnalyzerTap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompiledBarrierKind {
    ChannelCountChange,
    VariableFrames,
    SampleRateChange,
    Latency,
    Dynamics,
    Analyzer,
    Fft,
    Convolution,
    External,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompiledRegionKind {
    Scalar,
    LinearIir,
    Dynamics,
    Stft,
    Convolution,
    Analyzer,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompiledSegment {
    pub(super) start_op: usize,
    pub(super) end_op: usize,
    pub(super) cost_class: PluginCostClass,
    pub(super) region_kind: CompiledRegionKind,
    pub(super) barrier_after: Option<CompiledBarrierKind>,
}

impl CompiledOp {
    pub(super) fn from_plugin(
        node: &GraphNode,
        metadata: PluginCompileMetadata,
        supports_f64: bool,
        same_frame_count: bool,
        same_sample_rate: bool,
    ) -> Self {
        let barrier = Self::barrier_for(node, &metadata, same_frame_count, same_sample_rate);
        Self {
            node_id: node.id,
            input_channels: node.input_channels(),
            output_channels: node.output_channels(),
            kind: metadata
                .compiled_op
                .map(Self::kind_from_plugin_op)
                .unwrap_or(CompiledOpKind::PluginFallback),
            cost_class: metadata.cost_class,
            linear: metadata.linear,
            time_invariant_for_block: metadata.time_invariant_for_block,
            channel_mixing: metadata.channel_mixing,
            stateful: metadata.stateful,
            can_absorb_input_gain: metadata.can_absorb_input_gain,
            can_absorb_output_gain: metadata.can_absorb_output_gain,
            can_merge_with_eq: metadata.can_merge_with_eq,
            metadata_boundary: metadata.boundary,
            latency_samples: metadata.latency_samples,
            supports_f64,
            same_frame_count,
            same_sample_rate,
            barrier,
        }
    }

    fn kind_from_plugin_op(op: PluginCompiledOp) -> CompiledOpKind {
        match op {
            PluginCompiledOp::ApplyGain => CompiledOpKind::ApplyGain,
            PluginCompiledOp::EqBiquadBank => CompiledOpKind::EqBiquadBank,
            PluginCompiledOp::ChannelMuteSolo => CompiledOpKind::ChannelMuteSolo,
            PluginCompiledOp::Limiter => CompiledOpKind::Limiter,
            PluginCompiledOp::MultibandCompressor => CompiledOpKind::MultibandCompressor,
            PluginCompiledOp::AnalyzerTap => CompiledOpKind::AnalyzerTap,
        }
    }

    fn barrier_for(
        node: &GraphNode,
        metadata: &PluginCompileMetadata,
        same_frame_count: bool,
        same_sample_rate: bool,
    ) -> Option<CompiledBarrierKind> {
        if node.input_channels() != node.output_channels() {
            return Some(CompiledBarrierKind::ChannelCountChange);
        }
        if !same_frame_count {
            return Some(CompiledBarrierKind::VariableFrames);
        }
        if !same_sample_rate {
            return Some(CompiledBarrierKind::SampleRateChange);
        }
        if metadata.latency_samples > 0 {
            return Some(CompiledBarrierKind::Latency);
        }
        if !metadata.boundary {
            return None;
        }
        match metadata.cost_class {
            PluginCostClass::Analyzer => Some(CompiledBarrierKind::Analyzer),
            PluginCostClass::Dynamics => Some(CompiledBarrierKind::Dynamics),
            PluginCostClass::Fft => Some(CompiledBarrierKind::Fft),
            PluginCostClass::Convolution => Some(CompiledBarrierKind::Convolution),
            PluginCostClass::External => Some(CompiledBarrierKind::External),
            _ => Some(CompiledBarrierKind::Metadata),
        }
    }
}

pub(super) fn build_segments(ops: &[CompiledOp]) -> Vec<CompiledSegment> {
    let mut segments = Vec::new();
    if ops.is_empty() {
        return segments;
    }

    let mut start_op = 0;
    let mut cost_class = ops[0].cost_class;
    for (idx, op) in ops.iter().enumerate() {
        cost_class = merge_cost_class(cost_class, op.cost_class);
        if let Some(barrier) = op.barrier {
            segments.push(CompiledSegment {
                start_op,
                end_op: idx + 1,
                cost_class,
                region_kind: region_kind_for(cost_class),
                barrier_after: Some(barrier),
            });
            start_op = idx + 1;
            if let Some(next) = ops.get(start_op) {
                cost_class = next.cost_class;
            }
        }
    }

    if start_op < ops.len() {
        segments.push(CompiledSegment {
            start_op,
            end_op: ops.len(),
            cost_class,
            region_kind: region_kind_for(cost_class),
            barrier_after: None,
        });
    }
    segments
}

pub(super) fn region_kind_for(class: PluginCostClass) -> CompiledRegionKind {
    match class {
        PluginCostClass::Scalar => CompiledRegionKind::Scalar,
        PluginCostClass::Iir => CompiledRegionKind::LinearIir,
        PluginCostClass::Dynamics => CompiledRegionKind::Dynamics,
        PluginCostClass::Fft => CompiledRegionKind::Stft,
        PluginCostClass::Convolution => CompiledRegionKind::Convolution,
        PluginCostClass::Analyzer => CompiledRegionKind::Analyzer,
        PluginCostClass::External => CompiledRegionKind::External,
    }
}

fn merge_cost_class(left: PluginCostClass, right: PluginCostClass) -> PluginCostClass {
    if cost_rank(right) > cost_rank(left) {
        right
    } else {
        left
    }
}

fn cost_rank(class: PluginCostClass) -> u8 {
    match class {
        PluginCostClass::Scalar => 0,
        PluginCostClass::Analyzer => 1,
        PluginCostClass::Iir => 2,
        PluginCostClass::Dynamics => 3,
        PluginCostClass::Fft => 4,
        PluginCostClass::Convolution => 5,
        PluginCostClass::External => 6,
    }
}
