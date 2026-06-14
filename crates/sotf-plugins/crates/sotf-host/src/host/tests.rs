use super::buffer_guard::BufferGuard;
use super::compensation_delays::CompensationDelays;
use super::daw_host::DawHost;
use super::delay_buffer::DelayBuffer;
use super::graph_edge::GraphEdge;
use super::misc::PARAMETER_EVENT_QUEUE_CAPACITY;
use super::node_buffer::NodeBuffer;
use super::types::ProcessBuffers;

use crate::plugin::InPlacePluginAdapter;

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
        g.parameter_event_scratch.capacity() >= PARAMETER_EVENT_QUEUE_CAPACITY,
        "parameter event scratch should not allocate while draining a full ring"
    );
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
