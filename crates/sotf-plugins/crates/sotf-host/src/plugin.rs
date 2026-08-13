use crate::parameters::{Parameter, ParameterId, ParameterValue};
use std::any::Any;
use std::sync::Arc;

mod in_place_plugin;
mod in_place_plugin_adapter;
mod loop_range;
mod midi_event;
mod midi_message;
mod misc;
mod note_expression_event;
mod parameter_event;
mod plugin_info;
mod process_context;
#[cfg(test)]
mod tests;
mod time_signature;
mod transport_info;
mod types;

pub use in_place_plugin::*;
pub use in_place_plugin_adapter::*;
pub use loop_range::*;
pub use midi_event::*;
pub use midi_message::*;
pub use note_expression_event::*;
pub use parameter_event::*;
pub use plugin_info::*;
pub use process_context::*;
pub use time_signature::*;
pub use transport_info::*;
pub use types::*;

/// Validate the common realtime block contract before any plugin state advances.
///
/// The host contract is deliberately strict: buffers contain exactly the
/// interleaved samples described by `context` and channel metadata, and input
/// samples must be finite. Rejected blocks leave output and plugin state
/// untouched. Individual plugins therefore do not need to rediscover malformed
/// buffers after partially updating filters, delay lines, or stochastic state.
pub fn validate_process_block_f32(
    input: &[f32],
    output: &[f32],
    context: &ProcessContext<'_>,
    input_channels: usize,
    output_channels: usize,
) -> PluginResult<()> {
    validate_process_block_lengths(
        input.len(),
        output.len(),
        context,
        input_channels,
        output_channels,
    )?;
    if let Some(index) = input.iter().position(|sample| !sample.is_finite()) {
        return Err(format!(
            "plugin input contains non-finite sample at interleaved index {index}"
        ));
    }
    Ok(())
}

/// f64 counterpart of [`validate_process_block_f32`].
pub fn validate_process_block_f64(
    input: &[f64],
    output: &[f64],
    context: &ProcessContext<'_>,
    input_channels: usize,
    output_channels: usize,
) -> PluginResult<()> {
    validate_process_block_lengths(
        input.len(),
        output.len(),
        context,
        input_channels,
        output_channels,
    )?;
    if let Some(index) = input.iter().position(|sample| !sample.is_finite()) {
        return Err(format!(
            "plugin input contains non-finite sample at interleaved index {index}"
        ));
    }
    Ok(())
}

fn validate_process_block_lengths(
    input_len: usize,
    output_len: usize,
    context: &ProcessContext<'_>,
    input_channels: usize,
    output_channels: usize,
) -> PluginResult<()> {
    if input_channels == 0 || output_channels == 0 {
        return Err(format!(
            "plugin channel counts must be non-zero, got input={input_channels} output={output_channels}"
        ));
    }
    let expected_input = context
        .num_frames
        .checked_mul(input_channels)
        .ok_or_else(|| "plugin input buffer length overflow".to_string())?;
    let expected_output = context
        .num_frames
        .checked_mul(output_channels)
        .ok_or_else(|| "plugin output buffer length overflow".to_string())?;
    if input_len != expected_input || output_len != expected_output {
        return Err(format!(
            "plugin expected input={expected_input} output={expected_output} samples for {} frames x {input_channels}/{output_channels} channels, got input={input_len} output={output_len}",
            context.num_frames
        ));
    }
    Ok(())
}

/// Result of one allocation-free end-of-stream drain step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginDrainResult {
    /// Output-rate frames written by this step.
    pub frames: usize,
    /// `true` when no buffered input or filter tail remains.
    pub complete: bool,
}

impl PluginDrainResult {
    pub const COMPLETE: Self = Self {
        frames: 0,
        complete: true,
    };
}

/// Core plugin trait
///
/// Plugins process audio samples in an interleaved format where samples are
/// organized as [L0, R0, L1, R1, ...] for stereo, or more generally
/// [C0_F0, C1_F0, C2_F0, ..., C0_F1, C1_F1, C2_F1, ...] for multi-channel.
///
/// Each plugin can process N input channels and produce P output channels,
/// allowing for flexible channel configuration (e.g., stereo to mono,
/// mono to stereo, surround processing, etc.).
pub trait Plugin: Send {
    /// Optional typed access for control-thread integrations that need to
    /// discover a concrete plugin wrapper behind `Box<dyn Plugin>`.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    /// Optional mutable typed access for control-thread integrations.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }

    /// Get plugin information
    fn info(&self) -> PluginInfo;

    /// Get the number of input channels this plugin expects
    fn input_channels(&self) -> usize;

    /// Get the number of output channels this plugin produces
    fn output_channels(&self) -> usize;

    /// Get the list of parameters this plugin supports
    fn parameters(&self) -> Vec<Parameter>;

    /// Set a parameter value
    /// Returns an error if the parameter doesn't exist or the value is invalid
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

    /// Optional opaque native state for out-of-process persistence.
    fn save_opaque_state(&self) -> PluginResult<Vec<u8>> {
        Err("plugin does not expose opaque state".to_string())
    }

    /// Restore opaque native state on the control thread.
    fn load_opaque_state(&mut self, _state: &[u8]) -> PluginResult<()> {
        Err("plugin does not expose opaque state".to_string())
    }

    /// Initialize the plugin with the given sample rate
    /// This is called before any audio processing begins
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        let _ = sample_rate;
        Ok(())
    }

    /// Reset the plugin state (e.g., clear buffers, reset filters)
    fn reset(&mut self) {
        // Default: no-op
    }

    /// Process audio samples
    ///
    /// # Arguments
    /// * `input` - Interleaved input samples [C0_F0, C1_F0, ..., C0_F1, C1_F1, ...]
    ///   Length must be num_frames * input_channels()
    /// * `output` - Interleaved output samples (will be filled by plugin)
    ///   Length must be num_frames * output_channels()
    /// * `context` - Processing context (sample rate, frame count, etc.)
    ///
    /// Input samples must be finite. Implementations must reject malformed or
    /// non-finite blocks before changing output or internal DSP state. The
    /// standard adapters enforce this contract for their wrapped plugins.
    ///
    /// # Returns
    /// Ok(()) on success, Err(message) on failure
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String>;

    /// Maximum output-rate frames written by one [`Plugin::drain`] call.
    ///
    /// Stateful streaming plugins override this together with `drain`. The
    /// value is a capacity bound, not a promise that frames are immediately
    /// available.
    fn drain_output_frames_max(&self) -> usize {
        0
    }

    /// Advance end-of-stream state without accepting new programme samples.
    ///
    /// Implementations must be object-safe, allocation-free, transactional on
    /// destination-capacity errors, and eventually return `complete = true`.
    /// Hosts call this repeatedly and pass any returned frames through every
    /// downstream plugin before draining the downstream plugin's own tail.
    fn drain(
        &mut self,
        _output: &mut [f32],
        _context: &ProcessContext,
    ) -> PluginResult<PluginDrainResult> {
        Ok(PluginDrainResult::COMPLETE)
    }

    /// Process a host-selected compiled operation.
    ///
    /// Returning `None` asks the host to use the regular `process` path. This
    /// keeps compiled plans opportunistic: the host can tag likely-specialized
    /// nodes while the concrete plugin decides whether its current state is
    /// eligible for the optimized operation.
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
    ///
    /// Return `Some(gain)` only when skipping `process()` for this block would
    /// preserve DSP state, for example when gain smoothing is already settled.
    fn compiled_static_gain(&self) -> Option<f32> {
        None
    }

    /// Parameter-sensitive compile/fusion metadata for this plugin state.
    fn compile_metadata(&self) -> types::PluginCompileMetadata {
        let mut metadata =
            types::PluginCompileMetadata::boundary(self.cost_class(), self.latency_samples());
        metadata.static_gain = self.compiled_static_gain();
        metadata
    }

    /// Process f64 audio samples.
    ///
    /// Plugins that need true double-precision processing should override this
    /// and return `supports_f64() == true`. The default bridges through f32 so
    /// callers have a stable API even for existing plugins.
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

    /// Get the processing latency in samples (if any)
    /// This is used to compensate for algorithmic delays
    fn latency_samples(&self) -> usize {
        0
    }

    /// Coarse cost category for host scheduling. Override for FFT,
    /// convolution, dynamics, and other non-scalar DSP.
    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Scalar
    }

    /// Check if the plugin supports a specific channel configuration
    /// By default, this checks that input/output match expected values
    fn supports_channel_config(&self, input_channels: usize, output_channels: usize) -> bool {
        input_channels == self.input_channels() && output_channels == self.output_channels()
    }

    /// Get data from the plugin (if it's an analyzer or exposes internal state)
    /// Returns `None` by default for plugins that don't expose data.
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }

    /// RT diagnostics: returns (contention_count, update_count) from internal
    /// RealTimeCache, then resets counters. Default returns (0, 0).
    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        (0, 0)
    }

    /// Returns the number of output frames for given input frames.
    /// Default: returns input unchanged (no frame count change).
    /// Plugins that change frame count (like resamplers) should override this.
    fn output_frames_for_input(&self, input_frames: usize) -> usize {
        input_frames
    }

    /// Returns the output sample rate given an input rate.
    /// Default: returns input unchanged (no rate change).
    /// Plugins that change sample rate (like resamplers) should override this.
    fn output_sample_rate(&self, input_rate: u32) -> u32 {
        input_rate
    }

    /// Returns the actual number of output frames from the last process() call.
    /// Default: returns None (unknown/not tracked).
    /// Plugins that produce variable output (like resamplers) should override this.
    fn last_output_frames(&self) -> Option<usize> {
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
