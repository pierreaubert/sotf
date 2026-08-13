use super::super::*;
use sotf_host::host::DawHost;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, ProcessContext};

/// Pass-through plugin that reports a fixed latency for testing.
struct LatencyPassthrough {
    pub(super) channels: usize,
    pub(super) latency: usize,
}

impl Plugin for LatencyPassthrough {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("LatencyPassthrough", "0.1", "test")
    }
    fn input_channels(&self) -> usize {
        self.channels
    }
    fn output_channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        vec![]
    }
    fn set_parameter(
        &mut self,
        _: sotf_host::parameters::ParameterId,
        _: sotf_host::parameters::ParameterValue,
    ) -> Result<(), String> {
        Err("none".into())
    }
    fn get_parameter(
        &self,
        _: &sotf_host::parameters::ParameterId,
    ) -> Option<sotf_host::parameters::ParameterValue> {
        None
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output[..input.len()].copy_from_slice(input);
        Ok(context.num_frames)
    }
    fn latency_samples(&self) -> usize {
        self.latency
    }
}

#[test]
fn test_latency_compensation() {
    // Path A: passthrough (0 latency)
    // Path B: passthrough reporting 64 samples latency
    // The plugin should delay path A by 64 frames to align them.
    let channels = 2;
    let latency_frames = 64;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Replace host_b with one containing a latency-reporting plugin
    let mut host_b = DawHost::new(channels, 48000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: latency_frames,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_b = host_b;
    plugin.update_latency_compensation().unwrap();

    // Verify reported latency = max of both paths
    assert_eq!(plugin.latency_samples(), latency_frames);

    // Verify delay_a compensates the shorter path A
    assert_eq!(plugin.delay_a.len, latency_frames * channels);
    assert_eq!(plugin.delay_b.len, 0);

    // Send an impulse and verify alignment:
    // Path A output should be delayed by latency_frames relative to input.
    let num_frames = 256;
    let mut input = vec![0.0f32; num_frames * channels];
    // Impulse at frame 0
    for sample in input.iter_mut().take(channels) {
        *sample = 1.0;
    }

    let mut output = vec![0.0f32; num_frames * channels];
    let context = ProcessContext::new(48000, num_frames);

    // Use pure-A mode (mix = -1) with auto-gain disabled
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-1.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    // Set very fast transition so smoother doesn't interfere
    plugin
        .set_parameter(
            ParameterId::from("mix_transition_ms"),
            ParameterValue::Float(5.0),
        )
        .unwrap();

    // Process a few silent blocks first to settle the smoother at -1.0
    let silent = vec![0.0f32; num_frames * channels];
    let mut discard = vec![0.0f32; num_frames * channels];
    for _ in 0..10 {
        plugin.process(&silent, &mut discard, &context).unwrap();
    }

    // Now send the impulse
    plugin.process(&input, &mut output, &context).unwrap();

    // Path A's impulse (frame 0) should appear at frame latency_frames in output
    // because delay_a delays it by latency_frames.
    let impulse_idx = latency_frames * channels;
    for ch in 0..channels {
        // Before the delay point: should be ~0
        // (tiny leakage from crossfade smoother is acceptable)
        if latency_frames > 0 {
            assert!(
                output[ch].abs() < 0.001,
                "Frame 0 ch {} should be silent (delayed), got {}",
                ch,
                output[ch]
            );
        }
        // At the delay point: should be ~1.0
        assert!(
            (output[impulse_idx + ch] - 1.0).abs() < 0.01,
            "Frame {} ch {} should be ~1.0 (impulse), got {}",
            latency_frames,
            ch,
            output[impulse_idx + ch]
        );
    }
}

#[test]
fn bypass_preserves_reported_latency() {
    let channels = 1;
    let latency_frames = 8;
    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48_000).unwrap();
    let mut host_b = DawHost::new(channels, 48_000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: latency_frames,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_b = host_b;
    plugin.update_latency_compensation().unwrap();
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();

    let mut input = vec![0.0; 32];
    input[0] = 1.0;
    let mut output = vec![0.0; 32];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 32))
        .unwrap();

    assert_eq!(plugin.latency_samples(), latency_frames);
    assert!(output[..latency_frames].iter().all(|sample| *sample == 0.0));
    assert_eq!(output[latency_frames], 1.0);
}

#[test]
fn test_latency_compensation_reset() {
    let channels = 2;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Add latency to path B
    let mut host_b = DawHost::new(channels, 48000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: 32,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_b = host_b;
    plugin.update_latency_compensation().unwrap();

    // Fill delay with non-zero data
    let num_frames = 64;
    let input = vec![1.0f32; num_frames * channels];
    let mut output = vec![0.0f32; num_frames * channels];
    let context = ProcessContext::new(48000, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    // Reset should clear delay line contents
    plugin.reset();

    // Delay buffer should be all zeros after reset
    assert!(plugin.delay_a.buffer.iter().all(|&s| s == 0.0));
    assert!(plugin.delay_b.buffer.iter().all(|&s| s == 0.0));
}

#[test]
fn test_latency_compensation_equal_latency() {
    let channels = 2;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Both paths: 32 samples latency
    let mut host_a = DawHost::new(channels, 48000);
    host_a
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: 32,
        }))
        .unwrap();
    host_a.build().unwrap();
    let mut host_b = DawHost::new(channels, 48000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: 32,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_a = host_a;
    plugin.host_b = host_b;
    plugin.update_latency_compensation().unwrap();

    // No compensation needed — both delays should be 0
    assert_eq!(plugin.delay_a.len, 0);
    assert_eq!(plugin.delay_b.len, 0);
}

/// Verify that `update_latency_compensation` returns `Err` when a host
/// cannot build (due to a cycle in the graph), and that both delay lines
/// are zeroed as a safe fallback.
///
/// `DawHost::build()` rejects graphs that contain cycles. We create such a
/// cycle by adding two nodes with `add_node` and then wiring them in a loop
/// via `add_edge` (cycle detection only happens inside `build()`).
#[test]
fn test_latency_compensation_returns_error_on_broken_host() {
    use sotf_host::host::GraphEdge;

    let channels = 2;
    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Build a host whose graph has a cycle so that build() will fail.
    let mut cyclic_host = DawHost::new(channels, 48000);

    // Add two passthrough nodes
    let node_a = cyclic_host
        .add_node(
            "a".to_string(),
            Box::new(LatencyPassthrough {
                channels,
                latency: 0,
            }),
        )
        .unwrap();
    let node_b = cyclic_host
        .add_node(
            "b".to_string(),
            Box::new(LatencyPassthrough {
                channels,
                latency: 0,
            }),
        )
        .unwrap();

    // Wire a→b and b→a: this creates a cycle
    cyclic_host
        .add_edge(GraphEdge::new(node_a, node_b))
        .unwrap();
    cyclic_host
        .add_edge(GraphEdge::new(node_b, node_a))
        .unwrap();

    // Replace host_b: update_latency_compensation will try to build this
    // cyclic host, which must fail.
    plugin.host_b = cyclic_host;

    let result = plugin.update_latency_compensation();
    assert!(
        result.is_err(),
        "update_latency_compensation should return Err when a host build fails (cycle)"
    );

    // Both delay lines should be zeroed (safe fallback — plugin stays audible)
    assert_eq!(
        plugin.delay_a.len, 0,
        "delay_a should be zeroed on build failure"
    );
    assert_eq!(
        plugin.delay_b.len, 0,
        "delay_b should be zeroed on build failure"
    );
}
