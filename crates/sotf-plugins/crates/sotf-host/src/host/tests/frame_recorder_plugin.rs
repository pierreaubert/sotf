use crate::plugin::Plugin;
use std::sync::Arc;

/// Mock plugin that records the ProcessContext.num_frames it receives.
pub(super) struct FrameRecorderPlugin {
    pub(super) channels: usize,
    pub(super) last_num_frames: std::cell::Cell<usize>,
}

impl FrameRecorderPlugin {
    pub(super) fn new(channels: usize) -> Self {
        Self {
            channels,
            last_num_frames: std::cell::Cell::new(0),
        }
    }
}

impl Plugin for FrameRecorderPlugin {
    fn info(&self) -> crate::plugin::PluginInfo {
        crate::plugin::PluginInfo::new("FrameRecorder", "0.1", "test")
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
        self.last_num_frames.set(ctx.num_frames);
        let len = input.len().min(output.len());
        output[..len].copy_from_slice(&input[..len]);
        Ok(ctx.num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        Some(Arc::new(self.last_num_frames.get()))
    }
}
