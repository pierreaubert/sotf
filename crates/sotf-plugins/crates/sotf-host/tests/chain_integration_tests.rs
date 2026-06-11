// ============================================================================
// Host Chain Integration Tests
// ============================================================================
//
// Exercises `sotf-host`'s public `DawHost` / `Host` API for plugin chain
// construction, multi-plugin processing, state transitions, and error paths.
// Simple in-tree `Plugin` implementations are used to keep the tests focused
// on the host API rather than any particular plugin crate.

use sotf_host::{
    DawHost, Host, Parameter, ParameterId, ParameterValue, Plugin, PluginInfo, ProcessContext,
};

const SAMPLE_RATE: u32 = 48_000;

/// A simple stateful gain plugin exposing a "gain" parameter.
struct GainPlugin {
    channels: usize,
    gain: f32,
}

impl GainPlugin {
    fn new(channels: usize, gain: f32) -> Self {
        Self { channels, gain }
    }
}

impl Plugin for GainPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gain", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float("gain", "Gain", 1.0, 0.0, 10.0)]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> Result<(), String> {
        if id.as_str() == "gain" {
            if let ParameterValue::Float(v) = value {
                self.gain = v;
                return Ok(());
            }
            return Err("gain expects a float".into());
        }
        Err(format!("unknown parameter: {}", id.as_str()))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "gain" {
            Some(ParameterValue::Float(self.gain))
        } else {
            None
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        for (out, &sample) in output.iter_mut().zip(input.iter()) {
            *out = sample * self.gain;
        }
        Ok(ctx.num_frames)
    }
}

/// A simple identity plugin used for passthrough / channel-format tests.
struct PassThroughPlugin {
    channels: usize,
}

impl PassThroughPlugin {
    fn new(channels: usize) -> Self {
        Self { channels }
    }
}

impl Plugin for PassThroughPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("PassThrough", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
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
        output[..input.len()].copy_from_slice(input);
        Ok(ctx.num_frames)
    }
}

// ----------------------------------------------------------------------------
// Chain construction
// ----------------------------------------------------------------------------

#[test]
fn host_empty_graph_passthrough() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    let input = vec![1.0_f32, 2.0, 3.0, 4.0];
    let mut output = vec![0.0_f32; 4];

    let frames = host.process(&input, &mut output).unwrap();
    assert_eq!(frames, 2);
    assert_eq!(output, input);
}

#[test]
fn host_chain_two_gains() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 0.5))).unwrap();
    host.add_plugin(Box::new(GainPlugin::new(2, 0.5))).unwrap();

    let input = vec![1.0_f32; 8];
    let mut output = vec![0.0_f32; 8];

    host.process(&input, &mut output).unwrap();
    for &sample in &output {
        assert!(
            (sample - 0.25).abs() < 1e-6,
            "expected 0.25 after two 0.5x gains, got {sample}"
        );
    }
}

#[test]
fn host_chain_three_gains() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 2.0))).unwrap();
    host.add_plugin(Box::new(GainPlugin::new(2, 3.0))).unwrap();
    host.add_plugin(Box::new(GainPlugin::new(2, 4.0))).unwrap();

    let input = vec![1.0_f32; 8];
    let mut output = vec![0.0_f32; 8];

    host.process(&input, &mut output).unwrap();
    for &sample in &output {
        assert!(
            (sample - 24.0).abs() < 1e-5,
            "expected 24.0 after 2*3*4, got {sample}"
        );
    }
}

#[test]
fn host_plugin_count_tracks_chain() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    assert_eq!(host.plugin_count(), 0);

    host.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    assert_eq!(host.plugin_count(), 1);

    host.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    assert_eq!(host.plugin_count(), 2);
}

// ----------------------------------------------------------------------------
// Multi-plugin processing roundtrips
// ----------------------------------------------------------------------------

#[test]
fn host_process_f64_chain() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(PassThroughPlugin::new(2)))
        .unwrap();

    let input = vec![0.25_f64, -0.5, 1.0, -1.0];
    let mut output = vec![0.0_f64; 4];

    let frames = host.process_f64(&input, &mut output).unwrap();
    assert_eq!(frames, 2);
    for (out, inp) in output.iter().zip(input.iter()) {
        assert!((out - inp).abs() < 1e-7, "expected {inp}, got {out}");
    }
}

#[test]
fn host_input_output_roundtrip_with_identity_chain() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(PassThroughPlugin::new(2)))
        .unwrap();
    host.add_plugin(Box::new(PassThroughPlugin::new(2)))
        .unwrap();

    let input = vec![0.1_f32, -0.2, 0.3, -0.4];
    let mut output = vec![0.0_f32; 4];

    host.process(&input, &mut output).unwrap();
    for (out, inp) in output.iter().zip(input.iter()) {
        assert!(
            (out - inp).abs() < 1e-6,
            "identity chain should preserve samples; expected {inp}, got {out}"
        );
    }
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn host_reset_maintains_deterministic_output() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 1.5))).unwrap();

    let input = vec![0.5_f32; 8];
    let mut before = vec![0.0_f32; 8];
    host.process(&input, &mut before).unwrap();

    host.reset();

    let mut after = vec![0.0_f32; 8];
    host.process(&input, &mut after).unwrap();

    for (b, a) in before.iter().zip(after.iter()) {
        assert!(
            (b - a).abs() < 1e-6,
            "reset should not change deterministic behavior; got {b} vs {a}"
        );
    }
}

#[test]
fn host_set_plugin_parameter_via_trait() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();

    {
        let host_trait: &mut dyn Host = &mut host;
        host_trait
            .set_plugin_parameter(0, "gain", ParameterValue::Float(0.5))
            .unwrap();
    }

    let input = vec![1.0_f32; 8];
    let mut output = vec![0.0_f32; 8];
    host.process(&input, &mut output).unwrap();

    for &sample in &output {
        assert!(
            (sample - 0.5).abs() < 1e-6,
            "expected 0.5 after setting gain, got {sample}"
        );
    }
}

#[test]
fn host_remove_plugin_rewires_chain() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 2.0))).unwrap();
    host.add_plugin(Box::new(GainPlugin::new(2, 3.0))).unwrap(); // will be removed
    host.add_plugin(Box::new(GainPlugin::new(2, 4.0))).unwrap();

    host.remove_plugin(1).unwrap();
    assert_eq!(host.plugin_count(), 2);

    let input = vec![1.0_f32; 8];
    let mut output = vec![0.0_f32; 8];
    host.process(&input, &mut output).unwrap();

    for &sample in &output {
        assert!(
            (sample - 8.0).abs() < 1e-5,
            "expected 8.0 after removing middle gain (2*4), got {sample}"
        );
    }
}

// ----------------------------------------------------------------------------
// Edge cases / error paths
// ----------------------------------------------------------------------------

#[test]
fn host_add_plugin_rejects_channel_mismatch() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();

    let result = host.add_plugin(Box::new(GainPlugin::new(5, 1.0)));
    assert!(result.is_err(), "expected channel mismatch error");
}

#[test]
fn host_remove_plugin_out_of_bounds_errors() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    assert!(host.remove_plugin(0).is_err());

    host.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    assert!(host.remove_plugin(1).is_err());
}

#[test]
fn host_set_plugin_parameter_out_of_bounds_errors() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    let result = host.set_plugin_parameter(0, "gain", ParameterValue::Float(0.5));
    assert!(result.is_err(), "expected out-of-bounds error");
}

#[test]
fn host_get_plugin_out_of_bounds_returns_none() {
    let host = DawHost::new(2, SAMPLE_RATE);
    assert!(host.get_plugin(0).is_none());
}

#[test]
fn host_input_channels_reflects_first_plugin() {
    let mut host = DawHost::new(3, SAMPLE_RATE);
    assert_eq!(host.input_channels(), 3);

    host.add_plugin(Box::new(GainPlugin::new(3, 1.0))).unwrap();
    assert_eq!(host.input_channels(), 3);
    assert_eq!(host.output_channels(), 3);
}
