use super::buffer_guard::BufferGuard;
use super::compensation_delays::CompensationDelays;
use super::compiled_plan::{
    CompiledBarrierKind, CompiledOpKind, CompiledRegionKind, CompiledRenderPlan,
};
use super::daw_host::DawHost;
use super::delay_buffer::DelayBuffer;
use super::graph_edge::GraphEdge;
use super::graph_node::GraphNode;
use super::misc::DEFAULT_PARALLEL_NODE_COST;
use super::misc::HEAVY_PARALLEL_NODE_COST;
use super::misc::MODERATE_PARALLEL_NODE_COST;
use super::misc::PARAMETER_EVENT_QUEUE_CAPACITY;
use super::node_buffer::NodeBuffer;
use super::processing_stage::ProcessingStage;
use super::types::ProcessBuffers;

use crate::plugin::{
    InPlacePluginAdapter, Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass,
    PluginInfo, ProcessContext,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

mod channel_changing_plugin;
mod f64_scale_plugin;
mod frame_recorder_plugin;
mod gain_plugin;
mod panicking_process_plugin;
mod playback_context_recorder_plugin;
mod prefers_oversampling_plugin;
mod scaler_plugin;
mod sidechain_in_place_plugin;
mod variable_frame_plugin;

use frame_recorder_plugin::FrameRecorderPlugin;
use prefers_oversampling_plugin::PrefersOversamplingPlugin;
use scaler_plugin::ScalerPlugin;
use sidechain_in_place_plugin::SidechainInPlacePlugin;
use variable_frame_plugin::VariableFramePlugin;

#[test]
fn test_pluginhost_api_empty_graph() {
    let mut g = DawHost::new(2, 48000);
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    assert!(g.process(&i, &mut o).is_ok());
    assert_eq!(o, i);
}

#[test]
fn host_rejects_non_finite_input_without_poisoning_later_blocks() {
    let mut host = DawHost::new(2, 48_000);
    let input = [0.25, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    let mut output = [1.0; 4];

    let error = host.process(&input, &mut output).unwrap_err();
    assert!(error.contains("non-finite sample at index 1"), "{error}");
    assert_eq!(output, [0.0; 4]);

    let recovered_input = [0.25, -0.5, 0.75, -1.0];
    let frames = host.process(&recovered_input, &mut output).unwrap();
    assert_eq!(frames, 2);
    assert_eq!(output, recovered_input);
}

#[test]
fn f64_host_rejects_non_finite_input_with_silent_output() {
    let mut host = DawHost::new(2, 48_000);
    let input = [0.25, f64::NAN, 0.75, -1.0];
    let mut output = [1.0; 4];

    let error = host.process_f64(&input, &mut output).unwrap_err();
    assert!(error.contains("non-finite sample at index 1"), "{error}");
    assert_eq!(output, [0.0; 4]);
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

#[test]
fn test_parameter_event_scratch_is_preallocated_to_queue_capacity() {
    let g = DawHost::new(2, 48000);
    assert!(
        g.queues.parameter_event_scratch.capacity() >= PARAMETER_EVENT_QUEUE_CAPACITY,
        "parameter event scratch should not allocate while draining a full ring"
    );
}

#[test]
fn test_parallel_node_cost_classifies_variable_frame_as_heavy() {
    let mut g = DawHost::new(2, 48000);
    let node = g
        .add_node("variable".into(), Box::new(VariableFramePlugin::new(2, 64)))
        .unwrap();
    g.build().unwrap();

    assert_eq!(g.cached_parallel_node_costs[node], HEAVY_PARALLEL_NODE_COST);
}

#[test]
fn test_parallel_node_cost_uses_plugin_cost_class() {
    assert_eq!(
        DawHost::estimate_parallel_node_cost(
            &CostClassPlugin::new("IIR", PluginCostClass::Iir),
            0,
            "unremarkable"
        ),
        MODERATE_PARALLEL_NODE_COST
    );
    assert_eq!(
        DawHost::estimate_parallel_node_cost(
            &CostClassPlugin::new("FFT", PluginCostClass::Fft),
            0,
            "unremarkable"
        ),
        HEAVY_PARALLEL_NODE_COST
    );
    assert_eq!(
        DawHost::estimate_parallel_node_cost(
            &CostClassPlugin::new("Analyzer", PluginCostClass::Analyzer),
            0,
            "spectral analyzer"
        ),
        DEFAULT_PARALLEL_NODE_COST
    );
}

#[test]
fn test_compiled_plan_empty_graph_is_passthrough() {
    let mut g = DawHost::new(2, 48000);
    g.build().unwrap();

    assert!(matches!(
        g.compiled_plan,
        CompiledRenderPlan::EmptyPassthrough
    ));
}

#[test]
fn test_compiled_plan_linear_f32_tags_specialized_gain_op() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(CostClassPlugin::new(
        "Gain",
        PluginCostClass::Scalar,
    )))
    .unwrap();
    g.build().unwrap();

    let CompiledRenderPlan::LinearF32(plan) = &g.compiled_plan else {
        panic!("expected linear f32 compiled plan");
    };
    assert_eq!(plan.input_channels, 2);
    assert_eq!(plan.output_channels, 2);
    assert_eq!(plan.ops.len(), 1);
    assert_eq!(plan.ops[0].kind, CompiledOpKind::ApplyGain);
    assert_eq!(plan.segments.len(), 1);
    assert_eq!(plan.segments[0].barrier_after, None);
    assert_eq!(plan.segments[0].region_kind, CompiledRegionKind::Scalar);
}

#[test]
fn test_compiled_plan_linear_f32_tags_channel_mute_solo_op() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(CostClassPlugin::new(
        "Channel Mute Solo",
        PluginCostClass::Scalar,
    )))
    .unwrap();
    g.build().unwrap();

    let CompiledRenderPlan::LinearF32(plan) = &g.compiled_plan else {
        panic!("expected linear f32 compiled plan");
    };
    assert_eq!(plan.ops.len(), 1);
    assert_eq!(plan.ops[0].kind, CompiledOpKind::ChannelMuteSolo);
}

#[test]
fn test_compiled_linear_f32_uses_specialized_eq_hook() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(SpecializedEqHookPlugin)).unwrap();
    g.build().unwrap();

    let input = vec![0.25_f32, -0.5, 0.75, -1.0];
    let mut output = vec![0.0_f32; input.len()];
    let frames = g.process(&input, &mut output).unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, vec![0.5, -1.0, 1.5, -2.0]);
}

#[test]
fn test_compiled_linear_f32_uses_other_specialized_hooks() {
    let cases = [
        (
            "Gain",
            PluginCostClass::Scalar,
            PluginCompiledOp::ApplyGain,
            3.0_f32,
        ),
        (
            "Channel Mute Solo",
            PluginCostClass::Scalar,
            PluginCompiledOp::ChannelMuteSolo,
            4.0_f32,
        ),
        (
            "Spectrum Analyzer",
            PluginCostClass::Analyzer,
            PluginCompiledOp::AnalyzerTap,
            5.0_f32,
        ),
        (
            "Limiter",
            PluginCostClass::Dynamics,
            PluginCompiledOp::Limiter,
            6.0_f32,
        ),
        (
            "Multiband Compressor",
            PluginCostClass::Dynamics,
            PluginCompiledOp::MultibandCompressor,
            7.0_f32,
        ),
    ];

    for (name, class, op, scale) in cases {
        let mut g = DawHost::new(2, 48000);
        g.add_plugin(Box::new(SpecializedCompiledHookPlugin {
            name,
            class,
            op,
            scale,
        }))
        .unwrap();
        g.build().unwrap();

        let input = vec![0.25_f32, -0.5, 0.75, -1.0];
        let mut output = vec![0.0_f32; input.len()];
        let frames = g.process(&input, &mut output).unwrap();

        assert_eq!(frames, 2, "{name}");
        assert_eq!(
            output,
            input
                .iter()
                .map(|sample| sample * scale)
                .collect::<Vec<_>>(),
            "{name}"
        );
    }
}

#[test]
fn test_compiled_linear_f32_fuses_static_gain_run() {
    let process_calls = Arc::new(AtomicUsize::new(0));
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(StaticGainFusionPlugin {
        gain: 2.0,
        process_calls: Arc::clone(&process_calls),
    }))
    .unwrap();
    g.add_plugin(Box::new(StaticGainFusionPlugin {
        gain: 3.0,
        process_calls: Arc::clone(&process_calls),
    }))
    .unwrap();
    g.build().unwrap();

    let input = vec![0.25_f32, -0.5, 0.75, -1.0];
    let mut output = vec![0.0_f32; input.len()];
    let frames = g.process(&input, &mut output).unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, vec![1.5, -3.0, 4.5, -6.0]);
    assert_eq!(
        process_calls.load(Ordering::Relaxed),
        0,
        "fused static gain run should bypass regular plugin process calls"
    );
}

#[test]
fn test_compiled_linear_f32_folds_static_gain_across_linear_region() {
    let process_calls = Arc::new(AtomicUsize::new(0));
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(StaticGainFusionPlugin {
        gain: 2.0,
        process_calls: Arc::clone(&process_calls),
    }))
    .unwrap();
    g.add_plugin(Box::new(SpecializedEqHookPlugin)).unwrap();
    g.add_plugin(Box::new(StaticGainFusionPlugin {
        gain: 3.0,
        process_calls: Arc::clone(&process_calls),
    }))
    .unwrap();
    g.build().unwrap();

    let input = vec![0.25_f32, -0.5, 0.75, -1.0];
    let mut output = vec![0.0_f32; input.len()];
    let frames = g.process(&input, &mut output).unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, vec![3.0, -6.0, 9.0, -12.0]);
    assert_eq!(
        process_calls.load(Ordering::Relaxed),
        0,
        "static gain plugins should be folded across the linear EQ region"
    );
}

#[test]
fn test_compiled_plan_segments_fft_barrier_in_linear_chain() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(CostClassPlugin::new(
        "Gain",
        PluginCostClass::Scalar,
    )))
    .unwrap();
    g.add_plugin(Box::new(CostClassPlugin::new(
        "Spectral",
        PluginCostClass::Fft,
    )))
    .unwrap();
    g.add_plugin(Box::new(CostClassPlugin::new(
        "Gain",
        PluginCostClass::Scalar,
    )))
    .unwrap();
    g.build().unwrap();

    let CompiledRenderPlan::LinearF32(plan) = &g.compiled_plan else {
        panic!("expected linear f32 compiled plan");
    };
    assert_eq!(plan.ops.len(), 3);
    assert_eq!(plan.segments.len(), 2);
    assert_eq!(
        plan.segments[0].barrier_after,
        Some(CompiledBarrierKind::Fft)
    );
    assert_eq!(plan.segments[0].region_kind, CompiledRegionKind::Stft);
    assert_eq!(plan.segments[1].barrier_after, None);
}

#[test]
fn test_compiled_plan_graph_fallback_for_dag() {
    let mut g = DawHost::new(2, 48000);
    let a = g
        .add_node(
            "a".into(),
            Box::new(CostClassPlugin::new("Gain", PluginCostClass::Scalar)),
        )
        .unwrap();
    let b = g
        .add_node(
            "b".into(),
            Box::new(CostClassPlugin::new("Gain", PluginCostClass::Scalar)),
        )
        .unwrap();
    let c = g
        .add_node(
            "c".into(),
            Box::new(CostClassPlugin::new("Gain", PluginCostClass::Scalar)),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(a, b)).unwrap();
    g.add_edge(GraphEdge::new(a, c)).unwrap();
    g.build().unwrap();

    let CompiledRenderPlan::Graph(plan) = &g.compiled_plan else {
        panic!("expected graph compiled plan");
    };
    assert!(!plan.segments.is_empty());
}

struct CostClassPlugin {
    name: &'static str,
    class: PluginCostClass,
}

impl CostClassPlugin {
    fn new(name: &'static str, class: PluginCostClass) -> Self {
        Self { name, class }
    }
}

impl Plugin for CostClassPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(self.name, "0.1", "test")
    }

    fn input_channels(&self) -> usize {
        2
    }

    fn output_channels(&self) -> usize {
        2
    }

    fn cost_class(&self) -> PluginCostClass {
        self.class
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let compiled_op = match (self.name, self.class) {
            ("Gain", PluginCostClass::Scalar) => Some(PluginCompiledOp::ApplyGain),
            ("Channel Mute Solo", PluginCostClass::Scalar) => {
                Some(PluginCompiledOp::ChannelMuteSolo)
            }
            _ => None,
        };
        match self.class {
            PluginCostClass::Scalar | PluginCostClass::Iir => {
                PluginCompileMetadata::linear_transform(
                    self.class,
                    compiled_op,
                    0,
                    false,
                    false,
                    self.class == PluginCostClass::Iir,
                )
            }
            PluginCostClass::Analyzer => PluginCompileMetadata::analyzer(compiled_op),
            PluginCostClass::Dynamics => {
                PluginCompileMetadata::nonlinear(self.class, compiled_op, 0, false)
            }
            PluginCostClass::Fft | PluginCostClass::Convolution | PluginCostClass::External => {
                PluginCompileMetadata::boundary(self.class, 0)
            }
        }
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
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        Ok(ctx.num_frames)
    }
}

struct SpecializedEqHookPlugin;

impl Plugin for SpecializedEqHookPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("EQ", "0.1", "test")
    }

    fn input_channels(&self) -> usize {
        2
    }

    fn output_channels(&self) -> usize {
        2
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Iir
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::linear_transform(
            PluginCostClass::Iir,
            Some(PluginCompiledOp::EqBiquadBank),
            0,
            false,
            true,
            true,
        )
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
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        Ok(ctx.num_frames)
    }

    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::EqBiquadBank {
            return None;
        }
        for (dst, src) in output.iter_mut().zip(input.iter()) {
            *dst = *src * 2.0;
        }
        Some(Ok(ctx.num_frames))
    }
}

struct SpecializedCompiledHookPlugin {
    name: &'static str,
    class: PluginCostClass,
    op: PluginCompiledOp,
    scale: f32,
}

impl Plugin for SpecializedCompiledHookPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(self.name, "0.1", "test")
    }

    fn input_channels(&self) -> usize {
        2
    }

    fn output_channels(&self) -> usize {
        2
    }

    fn cost_class(&self) -> PluginCostClass {
        self.class
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        match self.class {
            PluginCostClass::Scalar | PluginCostClass::Iir => {
                PluginCompileMetadata::linear_transform(
                    self.class,
                    Some(self.op),
                    0,
                    false,
                    true,
                    self.class == PluginCostClass::Iir,
                )
            }
            PluginCostClass::Analyzer => PluginCompileMetadata::analyzer(Some(self.op)),
            PluginCostClass::Dynamics => {
                PluginCompileMetadata::nonlinear(self.class, Some(self.op), 0, false)
            }
            PluginCostClass::Fft | PluginCostClass::Convolution | PluginCostClass::External => {
                PluginCompileMetadata::boundary(self.class, 0)
            }
        }
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
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        Ok(ctx.num_frames)
    }

    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != self.op {
            return None;
        }
        for (dst, src) in output.iter_mut().zip(input.iter()) {
            *dst = *src * self.scale;
        }
        Some(Ok(ctx.num_frames))
    }
}

struct StaticGainFusionPlugin {
    gain: f32,
    process_calls: Arc<AtomicUsize>,
}

impl Plugin for StaticGainFusionPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gain", "0.1", "test")
    }

    fn input_channels(&self) -> usize {
        2
    }

    fn output_channels(&self) -> usize {
        2
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Scalar
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

    fn compiled_static_gain(&self) -> Option<f32> {
        Some(self.gain)
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = PluginCompileMetadata::linear_transform(
            PluginCostClass::Scalar,
            Some(PluginCompiledOp::ApplyGain),
            0,
            false,
            false,
            false,
        );
        metadata.static_gain = Some(self.gain);
        metadata
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        self.process_calls.fetch_add(1, Ordering::Relaxed);
        for (dst, src) in output.iter_mut().zip(input.iter()) {
            *dst = *src * self.gain;
        }
        Ok(ctx.num_frames)
    }
}

#[test]
fn test_parallel_stage_work_threshold_distinguishes_heavy_nodes() {
    let mut nodes = HashMap::new();
    nodes.insert(0, GraphNode::new(0, "a".into(), 2, 2));
    nodes.insert(1, GraphNode::new(1, "b".into(), 2, 2));

    let mut stage = ProcessingStage::new();
    stage.add_node(0);
    stage.add_node(1);

    let cheap_costs = vec![DEFAULT_PARALLEL_NODE_COST; 2];
    assert!(!DawHost::should_parallelize_stage(
        &stage,
        128,
        &nodes,
        &cheap_costs
    ));

    let heavy_costs = vec![HEAVY_PARALLEL_NODE_COST; 2];
    assert!(DawHost::should_parallelize_stage(
        &stage,
        128,
        &nodes,
        &heavy_costs
    ));
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

#[test]
fn test_preferred_oversampling_can_be_disabled() {
    let mut g = DawHost::new(2, 48000);
    g.set_plugin_preferred_oversampling_enabled(false);
    g.add_plugin(Box::new(PrefersOversamplingPlugin {
        inner: ScalerPlugin::new(2, 1.0),
        factor: 2,
    }))
    .unwrap();

    let plugin = g.get_plugin(0).unwrap();
    assert_eq!(plugin.info().name, "PrefersOversampling");
    assert_eq!(plugin.preferred_oversampling(), Some(2));
}

#[test]
fn test_forced_oversampling_rejects_invalid_factor() {
    let mut g = DawHost::new(2, 48000);
    let err = g.set_forced_oversampling_factor(Some(3)).unwrap_err();
    assert!(err.contains("Invalid forced oversampling factor"));
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

#[test]
fn test_rate_changing_variable_frame_is_not_zero_padded_to_input_length() {
    let mut host = DawHost::new(2, 44_100);
    host.add_plugin(Box::new(VariableFramePlugin::rate_changing(2, 0, 48_000)))
        .unwrap();
    host.build().unwrap();
    let input = vec![0.5; 256 * 2];
    let mut output = vec![1.0; 256 * 2];
    assert_eq!(host.process(&input, &mut output).unwrap(), 0);
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
fn test_cached_latency_empty_graph() {
    let mut g = DawHost::new(2, 48000);
    g.build().unwrap();
    assert_eq!(g.total_latency_samples(), 0);
}

#[test]
fn test_bypass_oob_returns_error() {
    let mut g = DawHost::new(2, 48000);
    assert!(g.bypass_plugin(0).is_err());
    assert!(g.unbypass_plugin(0).is_err());
    assert!(g.is_plugin_bypassed(0).is_err());
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
fn test_compensation_delays_set_oob_returns_error() {
    let edges = vec![GraphEdge::new(0, 1)];
    let mut delays = CompensationDelays::<f32>::new(&edges);
    let delay = DelayBuffer::new(10, 2);
    let result = delays.set(1, delay);
    assert!(
        result.is_err(),
        "set() with out-of-bounds edge_id should return an error"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("out of bounds"),
        "error should mention out of bounds: {err}"
    );
}
