use super::Plugin;
use super::in_place_plugin::InPlacePlugin;
use super::plugin_info::PluginInfo;
use super::process_context::ProcessContext;
use super::types::PluginResult;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use std::any::Any;
use std::sync::Arc;

/// Adapter to convert InPlacePlugin to Plugin
pub struct InPlacePluginAdapter<T: InPlacePlugin> {
    pub(super) plugin: T,
}

impl<T: InPlacePlugin> InPlacePluginAdapter<T> {
    pub fn new(plugin: T) -> Self {
        Self { plugin }
    }
}

impl<T: InPlacePlugin> Plugin for InPlacePluginAdapter<T> {
    fn info(&self) -> PluginInfo {
        self.plugin.info()
    }

    fn input_channels(&self) -> usize {
        self.plugin.input_channels()
    }

    fn output_channels(&self) -> usize {
        self.plugin.channels()
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.plugin.parameters()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.plugin.set_parameter(id, value)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.plugin.get_parameter(id)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.plugin.initialize(sample_rate)
    }

    fn reset(&mut self) {
        self.plugin.reset()
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let in_ch = self.plugin.input_channels();
        let out_ch = self.plugin.channels();
        if in_ch == out_ch {
            // Standard in-place: copy input to output, then process
            output.copy_from_slice(input);
            self.plugin.process_in_place(output, context)
        } else {
            // Extended input (e.g. external sidechain): copy full input to output buffer
            // which is sized for input_channels, then process in-place.
            // The output buffer must be sized for input_channels * num_frames.
            // After processing, only the first out_ch channels per frame are meaningful.
            output[..input.len()].copy_from_slice(input);
            let frames = self
                .plugin
                .process_in_place(&mut output[..input.len()], context)?;
            for frame in 0..frames {
                let src = frame * in_ch;
                let dst = frame * out_ch;
                output.copy_within(src..src + out_ch, dst);
            }
            Ok(frames)
        }
    }

    fn process_f64(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let in_ch = self.plugin.input_channels();
        let out_ch = self.plugin.channels();
        if in_ch == out_ch {
            output.copy_from_slice(input);
            self.plugin.process_in_place_f64(output, context)
        } else {
            output[..input.len()].copy_from_slice(input);
            let frames = self
                .plugin
                .process_in_place_f64(&mut output[..input.len()], context)?;
            for frame in 0..frames {
                let src = frame * in_ch;
                let dst = frame * out_ch;
                output.copy_within(src..src + out_ch, dst);
            }
            Ok(frames)
        }
    }

    fn latency_samples(&self) -> usize {
        self.plugin.latency_samples()
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.plugin.get_data()
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        self.plugin.preferred_oversampling()
    }

    fn supports_f64(&self) -> bool {
        self.plugin.supports_f64()
    }
}
