use super::Plugin;
use super::in_place_plugin::InPlacePlugin;
use super::plugin_info::PluginInfo;
use super::process_context::ProcessContext;
use super::types::{PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginResult};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use std::any::Any;
use std::sync::Arc;

/// Adapter to convert InPlacePlugin to Plugin
pub struct InPlacePluginAdapter<T: InPlacePlugin> {
    pub(super) plugin: T,
    /// Reusable f32 scratch for the f64 processing path. Pre-sized and grown on
    /// demand so the audio thread does not allocate per block.
    scratch: Vec<f32>,
}

impl<T: InPlacePlugin> InPlacePluginAdapter<T> {
    pub fn new(plugin: T) -> Self {
        Self {
            plugin,
            scratch: Vec::new(),
        }
    }

    fn ensure_scratch(&mut self, len: usize) {
        if self.scratch.len() < len {
            self.scratch.resize(len, 0.0);
        }
    }

    fn process_in_place_f64_with_scratch(
        &mut self,
        buffer: &mut [f64],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        self.ensure_scratch(buffer.len());
        let scratch = &mut self.scratch[..buffer.len()];
        for (dst, &src) in scratch.iter_mut().zip(buffer.iter()) {
            *dst = src as f32;
        }
        let frames = self.plugin.process_in_place(scratch, context)?;
        for (dst, &src) in buffer.iter_mut().zip(scratch.iter()) {
            *dst = src as f64;
        }
        Ok(frames)
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

    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        self.plugin.process_compiled_f32(op, input, output, context)
    }

    fn compiled_static_gain(&self) -> Option<f32> {
        self.plugin.compiled_static_gain()
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        self.plugin.compile_metadata()
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
            self.process_in_place_f64_with_scratch(output, context)
        } else {
            output[..input.len()].copy_from_slice(input);
            let frames = self
                .process_in_place_f64_with_scratch(&mut output[..input.len()], context)?;
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

    fn cost_class(&self) -> PluginCostClass {
        self.plugin.cost_class()
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
