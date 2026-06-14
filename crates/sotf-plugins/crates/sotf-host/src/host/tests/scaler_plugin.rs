use super::super::daw_host::DawHost;
use super::super::graph_edge::GraphEdge;
use crate::plugin::Plugin;

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

#[test]
fn test_forced_oversampling_wraps_same_io_plugins() {
    let mut g = DawHost::new(2, 48000);
    g.set_forced_oversampling_factor(Some(4)).unwrap();
    g.add_plugin(Box::new(ScalerPlugin::new(2, 1.0))).unwrap();

    let plugin = g.get_plugin(0).unwrap();
    assert_eq!(plugin.info().name, "Scaler(4x)");
    assert_eq!(plugin.preferred_oversampling(), None);
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
pub(super) struct ScalerPlugin {
    pub(super) channels: usize,
    pub(super) factor: f32,
    pub(super) latency: usize,
}

impl ScalerPlugin {
    pub(super) fn new(channels: usize, factor: f32) -> Self {
        Self {
            channels,
            factor,
            latency: 0,
        }
    }
    pub(super) fn with_latency(channels: usize, factor: f32, latency: usize) -> Self {
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
