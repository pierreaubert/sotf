//! Parametric in-place plugin trait and adapter.
//!
//! `ParametricInPlacePlugin` is the in-place analogue of [`ParametricPlugin`].
//! A plugin implements `parameter_schema`, `current_values`, `apply_values` and
//! a small handful of lifecycle/processing hooks; the
//! [`ParametricInPlacePluginAdapter`] then turns it into a regular
//! [`InPlacePlugin`] whose `parameters`, `set_parameter` and `get_parameter`
//! methods are derived automatically from the schema.

use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::parametric_plugin::{ParameterSchema, ParameterSet};
use crate::plugin::{
    InPlacePlugin, Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo,
    PluginResult, ProcessContext, validate_process_block_f32, validate_process_block_f64,
};
use std::any::Any;
use std::sync::Arc;

/// Trait for in-place plugins whose parameters can be described by a declarative schema.
///
/// Implementors only need to provide:
/// - static/dynamic metadata via [`parameter_schema`](Self::parameter_schema)
/// - current values via [`current_values`](Self::current_values)
/// - value application via [`apply_values`](Self::apply_values)
/// - audio processing via [`process_in_place`](Self::process_in_place)
///
/// The repetitive `parameters` / `set_parameter` / `get_parameter` wiring is
/// handled by [`ParametricInPlacePluginAdapter`].
pub trait ParametricInPlacePlugin: Send {
    /// Plugin metadata.
    fn info(&self) -> PluginInfo;

    /// Number of channels.
    fn channels(&self) -> usize;

    /// Number of input channels this plugin expects.
    /// Override this when the plugin needs more input channels than output channels,
    /// e.g. for external sidechain support where extra channels carry the sidechain signal.
    /// Default: same as `channels()`.
    fn input_channels(&self) -> usize {
        self.channels()
    }

    /// Parameter metadata. May be dynamic (e.g. per-channel gains).
    fn parameter_schema(&self) -> ParameterSchema;

    /// Current values for every parameter declared in the schema.
    fn current_values(&self) -> ParameterSet;

    /// Apply a new set of values to the plugin state.
    ///
    /// Values have already been validated against the schema. Implementations
    /// should ignore missing keys and treat unknown keys as an error.
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()>;

    /// Initialize the plugin with the given sample rate.
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        let _ = sample_rate;
        Ok(())
    }

    /// Reset plugin state.
    fn reset(&mut self) {}

    /// Process audio samples in-place.
    ///
    /// The public adapter validates exact block shape and finite input before
    /// entering this method. Direct lower-level callers must uphold that same
    /// contract themselves.
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize>;

    /// Optional specialized operation used by host compiled render plans.
    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        let _ = (op, input, output, context);
        None
    }

    /// Stable scalar gain that a host compiled plan may fuse with adjacent ops.
    fn compiled_static_gain(&self) -> Option<f32> {
        None
    }

    /// Parameter-sensitive compile/fusion metadata for this plugin state.
    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata =
            PluginCompileMetadata::boundary(self.cost_class(), self.latency_samples());
        metadata.static_gain = self.compiled_static_gain();
        metadata
    }

    /// Process f64 audio samples in-place.
    fn process_in_place_f64(
        &mut self,
        buffer: &mut [f64],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let mut buffer_f32 = vec![0.0; buffer.len()];
        for (dst, &src) in buffer_f32.iter_mut().zip(buffer.iter()) {
            *dst = src as f32;
        }
        let frames = self.process_in_place(&mut buffer_f32, context)?;
        for (dst, &src) in buffer.iter_mut().zip(buffer_f32.iter()) {
            *dst = src as f64;
        }
        Ok(frames)
    }

    /// Processing latency in samples.
    fn latency_samples(&self) -> usize {
        0
    }

    /// Coarse cost category for host scheduling.
    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Scalar
    }

    /// Get data from the plugin (if it's an analyzer or exposes internal state).
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }

    /// Preferred oversampling factor, if any.
    fn preferred_oversampling(&self) -> Option<u32> {
        None
    }

    /// Whether this plugin can process in f64 precision.
    fn supports_f64(&self) -> bool {
        false
    }

    /// Validate a value against the parameter schema.
    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        if let Some(param) = self.parameter_schema().iter().find(|p| &p.id == id) {
            param.validate(value).map_err(|e| format!("{}: {}", id, e))
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    /// Build the parameter list from the schema and current values.
    fn parametric_parameters(&self) -> Vec<Parameter> {
        let values = self.current_values();
        self.parameter_schema()
            .iter()
            .map(|param| {
                let mut param = param.clone();
                if let Some(value) = values.get(&param.id) {
                    param.default_value = value.clone();
                }
                param
            })
            .collect()
    }

    /// Set a single parameter value after validating it against the schema.
    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        self.parametric_validate_parameter(&id, &value)?;
        let mut values = ParameterSet::new();
        values.insert(id, value);
        self.apply_values(values)
    }

    /// Read a single parameter value.
    fn parametric_get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.current_values().get(id).cloned()
    }

    /// Convenience alias for [`parametric_validate_parameter`](Self::parametric_validate_parameter).
    fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        self.parametric_validate_parameter(id, value)
    }

    /// Convenience alias for [`parametric_parameters`](Self::parametric_parameters).
    fn parameters(&self) -> Vec<Parameter> {
        self.parametric_parameters()
    }

    /// Convenience alias for [`parametric_set_parameter`](Self::parametric_set_parameter).
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.parametric_set_parameter(id, value)
    }

    /// Convenience alias for [`parametric_get_parameter`](Self::parametric_get_parameter).
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.parametric_get_parameter(id)
    }
}

/// Adapter that turns a [`ParametricInPlacePlugin`] into a standard [`InPlacePlugin`].
#[derive(Debug)]
pub struct ParametricInPlacePluginAdapter<T: ParametricInPlacePlugin> {
    plugin: T,
    /// Reusable f32 scratch for the f64 in-place processing path. Pre-sized and
    /// grown on demand so the audio thread does not allocate per block.
    scratch: Vec<f32>,
}

impl<T: ParametricInPlacePlugin> ParametricInPlacePluginAdapter<T> {
    /// Wrap a parametric in-place plugin for use in the host graph.
    pub fn new(plugin: T) -> Self {
        Self {
            plugin,
            scratch: Vec::new(),
        }
    }

    /// Consume the adapter and return the inner plugin.
    pub fn into_inner(self) -> T {
        self.plugin
    }

    fn ensure_scratch(&mut self, len: usize) {
        if self.scratch.len() < len {
            self.scratch.resize(len, 0.0);
        }
    }
}

impl<T: ParametricInPlacePlugin> InPlacePlugin for ParametricInPlacePluginAdapter<T> {
    fn info(&self) -> PluginInfo {
        self.plugin.info()
    }

    fn channels(&self) -> usize {
        self.plugin.channels()
    }

    fn input_channels(&self) -> usize {
        self.plugin.input_channels()
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.plugin.parametric_parameters()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.plugin.parametric_set_parameter(id, value)
    }

    fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        self.plugin.parametric_validate_parameter(id, value)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.plugin.parametric_get_parameter(id)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.plugin.initialize(sample_rate)
    }

    fn reset(&mut self) {
        self.plugin.reset()
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        self.plugin.process_in_place(buffer, context)
    }

    fn process_in_place_f64(
        &mut self,
        buffer: &mut [f64],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
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

impl<T: ParametricInPlacePlugin> Plugin for ParametricInPlacePluginAdapter<T> {
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
        self.plugin.parametric_parameters()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.plugin.parametric_set_parameter(id, value)
    }

    fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        self.plugin.parametric_validate_parameter(id, value)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.plugin.parametric_get_parameter(id)
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
            validate_process_block_f32(input, output, context, in_ch, out_ch)?;
            output.copy_from_slice(input);
            self.plugin.process_in_place(output, context)
        } else {
            validate_process_block_f32(input, output, context, in_ch, in_ch)?;
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
        let validation = validate_process_block_f32(
            input,
            output,
            context,
            self.plugin.input_channels(),
            self.plugin.channels(),
        );
        if let Err(error) = validation {
            return Some(Err(error));
        }
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
            validate_process_block_f64(input, output, context, in_ch, out_ch)?;
            output.copy_from_slice(input);
            self.plugin.process_in_place_f64(output, context)
        } else {
            validate_process_block_f64(input, output, context, in_ch, in_ch)?;
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
