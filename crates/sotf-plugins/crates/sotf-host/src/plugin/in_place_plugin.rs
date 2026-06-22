use super::plugin_info::PluginInfo;
use super::process_context::ProcessContext;
use super::types::{PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginResult};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use std::any::Any;
use std::sync::Arc;

/// Helper trait for plugins that process audio in-place (input channels == output channels)
pub trait InPlacePlugin: Send {
    /// Get plugin information
    fn info(&self) -> PluginInfo;

    /// Get the number of output channels (same as input by default)
    fn channels(&self) -> usize;

    /// Get the number of input channels this plugin expects.
    /// Override this when the plugin needs more input channels than output channels,
    /// e.g. for external sidechain support where extra channels carry the sidechain signal.
    /// Default: same as `channels()`.
    fn input_channels(&self) -> usize {
        self.channels()
    }

    /// Get the list of parameters this plugin supports
    fn parameters(&self) -> Vec<Parameter>;

    /// Set a parameter value
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()>;

    /// Helper to validate a parameter value against its definition
    fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        let params = self.parameters();
        if let Some(param) = params.iter().find(|p| p.id == *id) {
            param.validate(value).map_err(|e| format!("{}: {}", id, e))
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    /// Get a parameter value
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue>;

    /// Initialize the plugin with the given sample rate
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        let _ = sample_rate;
        Ok(())
    }

    /// Reset the plugin state
    fn reset(&mut self) {
        // Default: no-op
    }

    /// Process audio samples in-place
    ///
    /// # Arguments
    /// * `buffer` - Interleaved audio samples [C0_F0, C1_F0, ..., C0_F1, C1_F1, ...]
    ///   Length is num_frames * channels()
    /// * `context` - Processing context
    ///
    /// # Returns
    /// Actual number of frames processed, or error message
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

    /// Process f64 audio samples in-place. Override together with
    /// `supports_f64()` for true double-precision DSP.
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

    /// Get the processing latency in samples (if any)
    fn latency_samples(&self) -> usize {
        0
    }

    /// Coarse cost category for host scheduling.
    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Scalar
    }

    /// Get data from the plugin (if it's an analyzer or exposes internal state)
    /// Returns `None` by default for plugins that don't expose data.
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }

    /// Returns the plugin's preferred oversampling factor, if any.
    /// When `Some(n)`, the host may insert oversampling before/after this plugin.
    /// `n` must be 2 or 4.
    fn preferred_oversampling(&self) -> Option<u32> {
        None
    }

    /// Whether this plugin can process in f64 precision.
    /// When true, the host may provide f64 buffers via a future `process_f64()` method.
    fn supports_f64(&self) -> bool {
        false
    }
}
