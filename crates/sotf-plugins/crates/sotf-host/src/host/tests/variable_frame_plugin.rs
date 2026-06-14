use crate::plugin::Plugin;

/// Mock variable-frame plugin that returns a configurable output frame count.
pub(super) struct VariableFramePlugin {
    pub(super) channels: usize,
    pub(super) output_frames: usize,
}

impl VariableFramePlugin {
    pub(super) fn new(channels: usize, output_frames: usize) -> Self {
        Self {
            channels,
            output_frames,
        }
    }
}

impl Plugin for VariableFramePlugin {
    fn info(&self) -> crate::plugin::PluginInfo {
        crate::plugin::PluginInfo::new("VariableFrame", "0.1", "test")
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
        _ctx: &crate::plugin::ProcessContext,
    ) -> Result<usize, String> {
        let out_len = self.output_frames * self.channels;
        for (o, &i) in output[..out_len].iter_mut().zip(input.iter().cycle()) {
            *o = i;
        }
        Ok(self.output_frames)
    }
    fn output_frames_for_input(&self, _: usize) -> usize {
        self.output_frames
    }
    fn latency_samples(&self) -> usize {
        1
    }
}
