// ============================================================================
// Graph signal-order regression tests
// ============================================================================
//
// Daemon metering falls back to zeros in graph mode when analyzer discovery
// only scans `chain_nodes`: graph builds (`add_node`/`add_edge`) never
// populate it. The host must derive signal order from the computed stages so
// linear graphs behave exactly like their chain equivalent.
//
// The layout mirrors the daemon's monitored graph: the user node is added
// first (low id) while the daemon-owned monitors take higher ids, yet signal
// order must still be monitor -> user -> monitor.

use sotf_host::{
    DawHost, GraphEdge, Host, LoudnessMonitorPlugin, Parameter, ParameterId, ParameterValue,
    Plugin, PluginInfo, ProcessContext,
};

const SAMPLE_RATE: u32 = 48_000;

/// Stereo passthrough with no analyzer data, standing in for user DSP.
struct PassthroughPlugin;

impl Plugin for PassthroughPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Passthrough", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        2
    }

    fn output_channels(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> Result<(), String> {
        Err("no parameters".into())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
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

fn monitor() -> Box<dyn Plugin> {
    Box::new(LoudnessMonitorPlugin::new(2).expect("monitor builds"))
}

#[test]
fn linear_graph_registers_monitors_in_signal_order() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    // Deliberately non-signal id order: user node first, monitors after.
    let user = host
        .add_node("user".to_string(), Box::new(PassthroughPlugin))
        .unwrap();
    let input_monitor = host.add_node("in".to_string(), monitor()).unwrap();
    let output_monitor = host.add_node("out".to_string(), monitor()).unwrap();
    host.add_edge(GraphEdge::new(input_monitor, user)).unwrap();
    host.add_edge(GraphEdge::new(user, output_monitor)).unwrap();
    host.build().unwrap();

    assert_eq!(host.plugin_count(), 3);
    // Signal order, not id order: monitors first and last.
    assert_eq!(host.analyzer_indices(), &[0, 2]);
    assert!(host.get_plugin_data(0).is_some());
    assert!(host.get_plugin_data(1).is_none());
    assert!(host.get_plugin_data(2).is_some());
}

#[test]
fn chain_built_hosts_keep_incremental_order() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(monitor()).unwrap();
    host.add_plugin(Box::new(PassthroughPlugin)).unwrap();
    host.add_plugin(monitor()).unwrap();
    host.build().unwrap();

    assert_eq!(host.plugin_count(), 3);
    assert_eq!(host.analyzer_indices(), &[0, 2]);
}
