//! Parametric plugin trait and adapter.
//!
//! `ParametricPlugin` is the first step in reducing per-plugin parameter
//! boilerplate. A plugin implements `parameter_schema`, `current_values`,
//! `apply_values` and a small handful of lifecycle/processing hooks; the
//! `ParametricPluginAdapter` then turns it into a regular `Plugin` whose
//! `parameters`, `set_parameter` and `get_parameter` methods are derived
//! automatically from the schema.
//!
//! Method names intentionally do **not** collide with `InPlacePlugin` so that
//! existing plugins can implement both traits during a gradual migration.

use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Schema returned by a [`ParametricPlugin`].
///
/// Each entry carries the metadata (name, range, unit, ...) for one parameter.
/// The current value is supplied separately by [`ParametricPlugin::current_values`].
pub type ParameterSchema = Vec<Parameter>;

/// Snapshot of current parameter values.
pub type ParameterSet = BTreeMap<ParameterId, ParameterValue>;

/// Trait for plugins whose parameters can be described by a declarative schema.
///
/// Implementors only need to provide:
/// - static/dynamic metadata via [`parameter_schema`](Self::parameter_schema)
/// - current values via [`current_values`](Self::current_values)
/// - value application via [`apply_values`](Self::apply_values)
/// - audio processing via [`process`](Self::process)
///
/// The repetitive `parameters` / `set_parameter` / `get_parameter` wiring is
/// handled by [`ParametricPluginAdapter`].
pub trait ParametricPlugin: Send {
    /// Plugin metadata.
    fn plugin_info(&self) -> PluginInfo;

    /// Number of input channels.
    fn input_channels(&self) -> usize;

    /// Number of output channels.
    fn output_channels(&self) -> usize;

    /// Parameter metadata. May be dynamic (e.g. per-channel gains).
    fn parameter_schema(&self) -> ParameterSchema;

    /// Current values for every parameter declared in the schema.
    fn current_values(&self) -> ParameterSet;

    /// Apply a new set of values to the plugin state.
    ///
    /// Values have already been validated against the schema. Implementations
    /// should ignore missing keys and treat unknown keys as an error.
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()>;

    /// Optional typed access for control-thread integrations.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    /// Optional mutable typed access.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }

    /// Initialize the plugin with the host sample rate.
    fn plugin_initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        let _ = sample_rate;
        Ok(())
    }

    /// Reset plugin state.
    fn plugin_reset(&mut self) {}

    /// Process one block of interleaved audio.
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String>;

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

    /// Process one block of interleaved f64 audio.
    fn process_f64(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let mut input_f32 = vec![0.0; input.len()];
        let mut output_f32 = vec![0.0; output.len()];
        for (dst, &src) in input_f32.iter_mut().zip(input.iter()) {
            *dst = src as f32;
        }
        let frames = self.process(&input_f32, &mut output_f32, context)?;
        for (dst, &src) in output.iter_mut().zip(output_f32.iter()) {
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

    /// Expose internal data for analyzers.
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }

    /// Preferred oversampling factor, if any.
    fn preferred_oversampling(&self) -> Option<u32> {
        None
    }

    /// Whether the plugin supports f64 processing.
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
}

/// Adapter that turns a [`ParametricPlugin`] into a standard [`Plugin`].
#[derive(Debug)]
pub struct ParametricPluginAdapter<T: ParametricPlugin> {
    plugin: T,
    /// Reusable f32 input scratch for the f64 processing path. Pre-sized and
    /// grown on demand so the audio thread does not allocate per block.
    input_scratch: Vec<f32>,
    /// Reusable f32 output scratch for the f64 processing path.
    output_scratch: Vec<f32>,
}

impl<T: ParametricPlugin> ParametricPluginAdapter<T> {
    /// Wrap a parametric plugin for use in the host graph.
    pub fn new(plugin: T) -> Self {
        Self {
            plugin,
            input_scratch: Vec::new(),
            output_scratch: Vec::new(),
        }
    }

    /// Consume the adapter and return the inner plugin.
    pub fn into_inner(self) -> T {
        self.plugin
    }

    fn ensure_scratch(&mut self, input_len: usize, output_len: usize) {
        if self.input_scratch.len() < input_len {
            self.input_scratch.resize(input_len, 0.0);
        }
        if self.output_scratch.len() < output_len {
            self.output_scratch.resize(output_len, 0.0);
        }
    }
}

impl<T: ParametricPlugin> Plugin for ParametricPluginAdapter<T> {
    fn as_any(&self) -> Option<&dyn Any> {
        self.plugin.as_any()
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        self.plugin.as_any_mut()
    }

    fn info(&self) -> PluginInfo {
        self.plugin.plugin_info()
    }

    fn input_channels(&self) -> usize {
        self.plugin.input_channels()
    }

    fn output_channels(&self) -> usize {
        self.plugin.output_channels()
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
        self.plugin.plugin_initialize(sample_rate)
    }

    fn reset(&mut self) {
        self.plugin.plugin_reset()
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        self.plugin.process(input, output, context)
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
        self.ensure_scratch(input.len(), output.len());
        let input_f32 = &mut self.input_scratch[..input.len()];
        let output_f32 = &mut self.output_scratch[..output.len()];
        for (dst, &src) in input_f32.iter_mut().zip(input.iter()) {
            *dst = src as f32;
        }
        let frames = self.plugin.process(input_f32, output_f32, context)?;
        for (dst, &src) in output.iter_mut().zip(output_f32.iter()) {
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

    fn output_frames_for_input(&self, input_frames: usize) -> usize {
        input_frames
    }

    fn output_sample_rate(&self, input_rate: u32) -> u32 {
        input_rate
    }

    fn last_output_frames(&self) -> Option<usize> {
        None
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        self.plugin.preferred_oversampling()
    }

    fn supports_f64(&self) -> bool {
        self.plugin.supports_f64()
    }
}
