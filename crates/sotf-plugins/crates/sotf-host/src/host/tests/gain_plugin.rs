use super::super::Host;
use super::super::daw_host::DawHost;
use crate::plugin::Plugin;

#[test]
fn test_process_f64_sample_offset_parameter_event_splits_f32_bridge() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    g.build().unwrap();

    g.set_plugin_parameter_at(0, "gain", crate::parameters::ParameterValue::Float(0.5), 2)
        .unwrap();

    let input = vec![1.0_f64; 8];
    let mut output = vec![0.0_f64; 8];
    let frames = g.process_f64(&input, &mut output).unwrap();

    assert_eq!(frames, 4);
    assert_eq!(output, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
}

/// Mock plugin that applies a gain parameter to all samples.
/// Supports `set_parameter`/`get_parameter` for the "gain" parameter so
/// automation tests can verify the value was written.
struct GainPlugin {
    pub(super) channels: usize,
    pub(super) gain: f32,
}

impl GainPlugin {
    pub(super) fn new(channels: usize, initial_gain: f32) -> Self {
        Self {
            channels,
            gain: initial_gain,
        }
    }
}

impl Plugin for GainPlugin {
    fn info(&self) -> crate::plugin::PluginInfo {
        crate::plugin::PluginInfo::new("Gain", "0.1", "test")
    }
    fn input_channels(&self) -> usize {
        self.channels
    }
    fn output_channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<crate::parameters::Parameter> {
        vec![crate::parameters::Parameter::new_float(
            "gain", "Gain", 1.0, 0.0, 4.0,
        )]
    }
    fn set_parameter(
        &mut self,
        id: crate::parameters::ParameterId,
        val: crate::parameters::ParameterValue,
    ) -> Result<(), String> {
        if id.as_str() == "gain"
            && let crate::parameters::ParameterValue::Float(v) = val
        {
            self.gain = v;
            return Ok(());
        }
        Err(format!("unknown parameter: {}", id.0))
    }
    fn get_parameter(
        &self,
        id: &crate::parameters::ParameterId,
    ) -> Option<crate::parameters::ParameterValue> {
        if id.as_str() == "gain" {
            Some(crate::parameters::ParameterValue::Float(self.gain))
        } else {
            None
        }
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &crate::plugin::ProcessContext,
    ) -> Result<usize, String> {
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            *o = i * self.gain;
        }
        Ok(ctx.num_frames)
    }
}

#[test]
fn test_set_plugin_parameter_queues_until_next_process() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    g.build().unwrap();

    let param_id = crate::parameters::ParameterId::from("gain");
    g.set_plugin_parameter(0, "gain", crate::parameters::ParameterValue::Float(0.5))
        .unwrap();

    let queued_value = g
        .get_plugin(0)
        .unwrap()
        .get_parameter(&param_id)
        .and_then(|v| v.as_float())
        .unwrap();
    assert_eq!(queued_value, 1.0);

    let input = vec![1.0f32; 4];
    let mut output = vec![0.0f32; 4];
    g.process(&input, &mut output).unwrap();

    assert_eq!(output, vec![0.5; 4]);
}

#[test]
fn test_host_trait_set_plugin_parameter_uses_dawhost_queue() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    g.build().unwrap();

    {
        let host: &mut dyn Host = &mut g;
        host.set_plugin_parameter(0, "gain", crate::parameters::ParameterValue::Float(0.25))
            .unwrap();
    }

    let param_id = crate::parameters::ParameterId::from("gain");
    let queued_value = g
        .get_plugin(0)
        .unwrap()
        .get_parameter(&param_id)
        .and_then(|v| v.as_float())
        .unwrap();
    assert_eq!(queued_value, 1.0);

    let input = vec![1.0f32; 4];
    let mut output = vec![0.0f32; 4];
    g.process(&input, &mut output).unwrap();

    assert_eq!(output, vec![0.25; 4]);
}

#[test]
fn test_external_parameter_sender_queues_without_host_borrow() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    g.build().unwrap();

    let node_id = g.chain_nodes[0];
    let mut sender = g
        .take_parameter_event_sender()
        .expect("sender should be available once");
    assert!(g.take_parameter_event_sender().is_none());

    sender
        .queue_node_parameter(
            node_id,
            crate::parameters::ParameterId::from("gain"),
            crate::parameters::ParameterValue::Float(0.25),
        )
        .unwrap();

    let input = vec![1.0f32; 4];
    let mut output = vec![0.0f32; 4];
    g.process(&input, &mut output).unwrap();

    assert_eq!(output, vec![0.25; 4]);
    assert_eq!(sender.dropped_events(), 0);
}

#[test]
fn test_parameter_event_sample_offset_splits_block() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    g.build().unwrap();

    g.set_plugin_parameter_at(0, "gain", crate::parameters::ParameterValue::Float(0.5), 2)
        .unwrap();

    let input = vec![1.0f32; 8];
    let mut output = vec![0.0f32; 8];
    let frames = g.process(&input, &mut output).unwrap();

    assert_eq!(frames, 4);
    assert_eq!(output, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
}

#[test]
fn test_automation_basic() {
    // Create a host with a single GainPlugin starting at gain=1.0.
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();
    g.build().unwrap();

    // The chain_nodes[0] is the NodeId assigned to our GainPlugin.
    let node_id = g.chain_nodes[0];
    let param_id = crate::parameters::ParameterId::from("gain");

    // 2 channels × 48 frames = 96 samples.
    let num_frames = 48;

    // Step automation: each step lasts exactly num_frames samples so each
    // call to process() advances to the next step value.
    // Step 0 (samples 0..47)  → gain = 0.25
    // Step 1 (samples 48..95) → gain = 0.75
    g.set_automation(
        node_id,
        param_id.clone(),
        crate::automation::AutomationCurve::Step {
            values: vec![0.25, 0.75],
            samples_per_step: num_frames,
        },
    );
    let input = vec![1.0f32; num_frames * 2];
    let mut output = vec![0.0f32; num_frames * 2];

    // First process(): automation evaluates at position=0, step=0 → gain=0.25.
    g.process(&input, &mut output).unwrap();

    let gain_after_first_block = g
        .get_plugin(0)
        .unwrap()
        .get_parameter(&param_id)
        .and_then(|v| v.as_float())
        .expect("gain parameter must be readable");

    assert!(
        (gain_after_first_block - 0.25).abs() < 1e-6,
        "After first process(), gain should be 0.25 (step 0), got {}",
        gain_after_first_block
    );

    // Verify the audio was actually scaled by 0.25.
    assert!(
        output.iter().all(|&s| (s - 0.25).abs() < 1e-6),
        "Output samples should be input * 0.25"
    );

    // Second process(): automation position = num_frames, step=1 → gain=0.75.
    g.process(&input, &mut output).unwrap();

    let gain_after_second_block = g
        .get_plugin(0)
        .unwrap()
        .get_parameter(&param_id)
        .and_then(|v| v.as_float())
        .expect("gain parameter must be readable");

    assert!(
        (gain_after_second_block - 0.75).abs() < 1e-6,
        "After second process(), gain should be 0.75 (step 1), got {}",
        gain_after_second_block
    );

    // Verify audio was scaled by 0.75.
    assert!(
        output.iter().all(|&s| (s - 0.75).abs() < 1e-6),
        "Output samples should be input * 0.75"
    );
}
