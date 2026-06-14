use super::super::daw_host::DawHost;
use crate::plugin::Plugin;
use std::sync::Arc;

/// Mock plugin that records playback position metadata from process contexts.
struct PlaybackContextRecorderPlugin {
    pub(super) channels: usize,
    pub(super) positions: std::cell::RefCell<Vec<(u64, f64)>>,
}

impl PlaybackContextRecorderPlugin {
    pub(super) fn new(channels: usize) -> Self {
        Self {
            channels,
            positions: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl Plugin for PlaybackContextRecorderPlugin {
    fn info(&self) -> crate::plugin::PluginInfo {
        crate::plugin::PluginInfo::new("PlaybackContextRecorder", "0.1", "test")
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
        self.positions
            .borrow_mut()
            .push((ctx.transport.sample_position, ctx.transport.ppq_position));
        output[..input.len()].copy_from_slice(input);
        Ok(ctx.num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        Some(Arc::new(self.positions.borrow().clone()))
    }
}

#[test]
fn test_process_context_receives_playback_position() {
    let mut g = DawHost::new(2, 48000);
    let recorder = g
        .add_node(
            "recorder".into(),
            Box::new(PlaybackContextRecorderPlugin::new(2)),
        )
        .unwrap();
    g.build().unwrap();

    let input = vec![0.1_f32; 128 * 2];
    let mut output = vec![0.0_f32; input.len()];

    g.process(&input, &mut output).unwrap();
    g.process(&input, &mut output).unwrap();

    let captured = g.plugins[recorder]
        .as_ref()
        .unwrap()
        .get_data()
        .unwrap()
        .downcast_ref::<Vec<(u64, f64)>>()
        .unwrap()
        .clone();

    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0].0, 0);
    assert_eq!(captured[1].0, 128);
    assert!((captured[0].1 - 0.0).abs() < 1e-12);
    assert!((captured[1].1 - (128.0 / 48_000.0 * 2.0)).abs() < 1e-12);
}
