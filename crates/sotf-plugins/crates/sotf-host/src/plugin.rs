// ============================================================================
// Plugin Trait Definition
// ============================================================================

use crate::parameters::{Parameter, ParameterId, ParameterValue};
use std::any::Any;
use std::sync::Arc;

/// Information about a plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin author
    pub author: String,
    /// Plugin description
    pub description: String,
}

impl PluginInfo {
    /// Create a new PluginInfo with empty description
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            author: author.into(),
            description: String::new(),
        }
    }

    /// Add a description to the PluginInfo (builder pattern)
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// Processing context passed to plugins
#[derive(Clone)]
pub struct ProcessContext {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of frames in this processing block
    pub num_frames: usize,
}

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, String>;

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
    /// # Returns
    /// Ok(()) on success, Err(message) on failure
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String>;

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

/// Adapter to convert InPlacePlugin to Plugin
pub struct InPlacePluginAdapter<T: InPlacePlugin> {
    plugin: T,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal in-place plugin that uses all defaults.
    struct DummyInPlacePlugin;

    impl InPlacePlugin for DummyInPlacePlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("Dummy", "0.0.1", "Test")
        }
        fn channels(&self) -> usize {
            2
        }
        fn parameters(&self) -> Vec<Parameter> {
            vec![]
        }
        fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
            Ok(())
        }
        fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
            None
        }
        fn process_in_place(
            &mut self,
            _buffer: &mut [f32],
            context: &ProcessContext,
        ) -> PluginResult<usize> {
            Ok(context.num_frames)
        }
    }

    /// In-place plugin that declares oversampling and f64 support.
    struct OversampledPlugin;

    impl InPlacePlugin for OversampledPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("Oversampled", "0.0.1", "Test")
        }
        fn channels(&self) -> usize {
            2
        }
        fn parameters(&self) -> Vec<Parameter> {
            vec![]
        }
        fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
            Ok(())
        }
        fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
            None
        }
        fn process_in_place(
            &mut self,
            _buffer: &mut [f32],
            context: &ProcessContext,
        ) -> PluginResult<usize> {
            Ok(context.num_frames)
        }
        fn preferred_oversampling(&self) -> Option<u32> {
            Some(4)
        }
        fn supports_f64(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_default_oversampling_is_none() {
        let plugin = DummyInPlacePlugin;
        assert_eq!(plugin.preferred_oversampling(), None);
    }

    #[test]
    fn test_default_f64_is_false() {
        let plugin = DummyInPlacePlugin;
        assert!(!plugin.supports_f64());
    }

    #[test]
    fn test_adapter_forwards_oversampling() {
        let adapted = InPlacePluginAdapter::new(OversampledPlugin);
        assert_eq!(adapted.preferred_oversampling(), Some(4));
        assert!(adapted.supports_f64());
    }

    #[test]
    fn test_adapter_forwards_defaults() {
        let adapted = InPlacePluginAdapter::new(DummyInPlacePlugin);
        assert_eq!(adapted.preferred_oversampling(), None);
        assert!(!adapted.supports_f64());
    }
}
