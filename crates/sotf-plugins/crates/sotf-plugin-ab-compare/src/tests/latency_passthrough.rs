use super::super::*;
use sotf_host::host::DawHost;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, ProcessContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

struct StatefulPassthrough {
    channels: usize,
    processed_frames: Arc<AtomicUsize>,
    silent: bool,
}

impl Plugin for StatefulPassthrough {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("StatefulPassthrough", "0.1", "test")
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

    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> Result<(), String> {
        Err("none".into())
    }

    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        self.processed_frames
            .fetch_add(context.num_frames, Ordering::Relaxed);
        if self.silent {
            output.fill(0.0);
        } else {
            output.copy_from_slice(input);
        }
        Ok(context.num_frames)
    }
}

fn render_mono_chunks(plugin: &mut ABComparePlugin, input: &[f32], chunks: &[usize]) -> Vec<f32> {
    assert_eq!(chunks.iter().sum::<usize>(), input.len());
    let mut output = Vec::with_capacity(input.len());
    let mut offset = 0;
    for &frames in chunks {
        let mut block = vec![0.0; frames];
        plugin
            .process(
                &input[offset..offset + frames],
                &mut block,
                &ProcessContext::new(1_000, frames),
            )
            .unwrap();
        output.extend_from_slice(&block);
        offset += frames;
    }
    output
}

fn silent_wet_latency_plugin(latency: usize) -> ABComparePlugin {
    let channels = 1;
    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(1_000).unwrap();

    let mut host_a = DawHost::new(channels, 1_000);
    host_a
        .add_plugin(Box::new(StatefulPassthrough {
            channels,
            processed_frames: Arc::new(AtomicUsize::new(0)),
            silent: true,
        }))
        .unwrap();
    host_a.build().unwrap();
    plugin.host_a = host_a;
    plugin.path_a_config = PathConfig::Plugin {
        plugin_type: "silent-test".to_string(),
        parameters: serde_json::json!({}),
    };

    let mut host_b = DawHost::new(channels, 1_000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough { channels, latency }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_b = host_b;
    plugin.path_b_config = PathConfig::Plugin {
        plugin_type: "latency-test".to_string(),
        parameters: serde_json::json!({}),
    };
    plugin.update_latency_compensation().unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mix_transition_ms"),
            ParameterValue::Float(8.0),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-1.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin.reset();
    plugin
}

fn one_pole_step(current: &mut f32, target: f32, coeff: f32) -> f32 {
    if (*current - target).abs() < 1.0e-5 {
        *current = target;
    } else {
        *current = target + coeff * (*current - target);
    }
    *current
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

    // The test plugin only reports latency; it does not delay its own output.
    // Select the genuinely delay-compensated A path before exercising bypass
    // so both sides of the bypass crossfade share the advertised latency.
    plugin
        .set_parameter(
            ParameterId::from("mix_transition_ms"),
            ParameterValue::Float(1.0),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-1.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    // The host is injected directly, so the serialized path config still
    // looks empty. Disable the empty-path shortcut without changing pure-A
    // output; otherwise that test-only mismatch would bypass both hosts.
    plugin
        .set_parameter(
            ParameterId::from("phase_invert_b"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin.reset();
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(plugin.delay_a.len, latency_frames);
    assert_eq!(plugin.delay_dry.len, latency_frames);
    assert_eq!(plugin.transition_smoothers.mix.current(), -1.0);
    assert_eq!(plugin.transition_smoothers.bypass.current(), 0.0);

    let mut input = vec![0.0; 32];
    input[0] = 1.0;
    let mut output = vec![0.0; 32];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 32))
        .unwrap();

    assert_eq!(plugin.latency_samples(), latency_frames);
    assert!(
        output[..latency_frames].iter().all(|sample| *sample == 0.0),
        "bypass emitted samples before reported latency: {:?}",
        &output[..latency_frames]
    );
    assert_eq!(output[latency_frames], 1.0);
}

#[test]
fn bypass_crossfades_without_freezing_nested_path_state() {
    let channels = 1;
    let frames = 64;
    let processed_frames = Arc::new(AtomicUsize::new(0));
    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48_000).unwrap();

    let mut host_a = DawHost::new(channels, 48_000);
    host_a
        .add_plugin(Box::new(StatefulPassthrough {
            channels,
            processed_frames: Arc::clone(&processed_frames),
            silent: true,
        }))
        .unwrap();
    host_a.build().unwrap();
    plugin.host_a = host_a;
    plugin.path_a_config = PathConfig::Plugin {
        plugin_type: "stateful-test".to_string(),
        parameters: serde_json::json!({}),
    };
    plugin.update_latency_compensation().unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mix_transition_ms"),
            ParameterValue::Float(5.0),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-1.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    let input = vec![1.0; frames];
    let mut output = vec![0.0; frames];
    let context = ProcessContext::new(48_000, frames);
    for _ in 0..64 {
        plugin.process(&input, &mut output, &context).unwrap();
    }
    assert!(output.iter().all(|sample| sample.abs() < 1e-4));
    let before_bypass = processed_frames.load(Ordering::Relaxed);

    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();
    plugin.process(&input, &mut output, &context).unwrap();

    assert!(output[0] > 0.0 && output[0] < 1.0);
    assert!(output.windows(2).all(|window| window[1] >= window[0]));
    assert!(output[frames - 1] > output[0]);
    assert_eq!(
        processed_frames.load(Ordering::Relaxed),
        before_bypass + frames,
        "nested path must keep advancing throughout bypass"
    );
}

#[test]
fn empty_paths_advance_bypass_in_both_directions_across_callbacks() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    let mut whole_plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(48_000).unwrap();
    whole_plugin.initialize(48_000).unwrap();
    for candidate in [&mut plugin, &mut whole_plugin] {
        candidate
            .set_parameter(
                ParameterId::from("mix_transition_ms"),
                ParameterValue::Float(10.0),
            )
            .unwrap();
        candidate
            .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
            .unwrap();
    }

    let input = vec![0.375; 300];
    let mut rendered = 0usize;
    for frames in [1usize, 7, 31, 3, 89, 169] {
        let mut output = vec![0.0; frames];
        plugin
            .process(
                &input[rendered..rendered + frames],
                &mut output,
                &ProcessContext::new(48_000, frames),
            )
            .unwrap();
        assert_eq!(output, input[rendered..rendered + frames]);
        rendered += frames;
    }
    let after_bypass = plugin.transition_smoothers.bypass.current();
    let coeff = (-1.0f32 / (10.0e-3 * 48_000.0)).exp();
    let expected_up = 1.0 - coeff.powi(300);
    assert!((after_bypass - expected_up).abs() < 2.0e-6);
    assert!(after_bypass > 0.0 && after_bypass < 1.0);
    let mut whole_output = vec![0.0; input.len()];
    whole_plugin
        .process(
            &input,
            &mut whole_output,
            &ProcessContext::new(48_000, input.len()),
        )
        .unwrap();
    assert_eq!(whole_output, input);
    assert!(
        (whole_plugin.transition_smoothers.bypass.current() - after_bypass).abs() < 2.0e-6,
        "empty-path bypass-up state changed with callback partitioning"
    );

    for candidate in [&mut plugin, &mut whole_plugin] {
        candidate
            .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(false))
            .unwrap();
    }
    rendered = 0;
    for frames in [113usize, 2, 64, 121] {
        let mut output = vec![0.0; frames];
        plugin
            .process(
                &input[rendered..rendered + frames],
                &mut output,
                &ProcessContext::new(48_000, frames),
            )
            .unwrap();
        assert_eq!(output, input[rendered..rendered + frames]);
        rendered += frames;
    }
    let expected_down = coeff.powi(300) * after_bypass;
    let after_unbypass = plugin.transition_smoothers.bypass.current();
    assert!((after_unbypass - expected_down).abs() < 2.0e-6);
    assert!(after_unbypass > 0.0 && after_unbypass < after_bypass);
    whole_plugin
        .process(
            &input,
            &mut whole_output,
            &ProcessContext::new(48_000, input.len()),
        )
        .unwrap();
    assert_eq!(whole_output, input);
    assert!(
        (whole_plugin.transition_smoothers.bypass.current() - after_unbypass).abs() < 2.0e-6,
        "empty-path bypass-down state changed with callback partitioning"
    );
}

#[test]
fn latency_aligned_bypass_ramp_is_partition_invariant_in_both_directions() {
    let latency = 5;
    let mut whole = silent_wet_latency_plugin(latency);
    let mut split = silent_wet_latency_plugin(latency);
    whole
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();
    split
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();

    let up_input = vec![1.0; 137];
    let up_whole = render_mono_chunks(&mut whole, &up_input, &[137]);
    let up_split = render_mono_chunks(&mut split, &up_input, &[1, 3, 17, 2, 64, 50]);
    assert_eq!(up_whole, up_split);

    let coeff = (-1.0f32 / (8.0e-3 * 1_000.0)).exp();
    let mut expected_mix = 0.0;
    for (frame, &actual) in up_whole.iter().enumerate() {
        let mix = one_pole_step(&mut expected_mix, 1.0, coeff);
        let expected = if frame < latency { 0.0 } else { mix };
        assert!(
            (actual - expected).abs() < 2.0e-6,
            "bypass-up frame {frame}: actual={actual}, expected={expected}"
        );
    }
    assert_eq!(expected_mix, 1.0);

    whole
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(false))
        .unwrap();
    split
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(false))
        .unwrap();
    let down_input = vec![1.0; 111];
    let down_whole = render_mono_chunks(&mut whole, &down_input, &[111]);
    let down_split = render_mono_chunks(&mut split, &down_input, &[5, 1, 29, 7, 69]);
    assert_eq!(down_whole, down_split);
    for (frame, &actual) in down_whole.iter().enumerate() {
        let expected = one_pole_step(&mut expected_mix, 0.0, coeff);
        assert!(
            (actual - expected).abs() < 2.0e-6,
            "bypass-down frame {frame}: actual={actual}, expected={expected}"
        );
    }
    assert_eq!(expected_mix, 0.0);
}

#[test]
fn settled_bypass_impulse_obeys_reported_latency_under_irregular_partitioning() {
    let latency = 11;
    let mut plugin = silent_wet_latency_plugin(latency);
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();
    plugin.reset();

    let mut impulse = vec![0.0; 47];
    impulse[0] = 1.0;
    let output = render_mono_chunks(&mut plugin, &impulse, &[2, 1, 7, 3, 19, 15]);
    assert_eq!(plugin.latency_samples(), latency);
    assert!(output[..latency].iter().all(|&sample| sample == 0.0));
    assert_eq!(output[latency], 1.0);
    assert!(output[latency + 1..].iter().all(|&sample| sample == 0.0));
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
