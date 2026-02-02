// ============================================================================
// Plugin Trait Definition
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
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
    ) -> PluginResult<()>;

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
}

/// Helper trait for plugins that process audio in-place (input channels == output channels)
pub trait InPlacePlugin: Send {
    /// Get plugin information
    fn info(&self) -> PluginInfo;

    /// Get the number of channels (same for input and output)
    fn channels(&self) -> usize;

    /// Get the list of parameters this plugin supports
    fn parameters(&self) -> Vec<Parameter>;

    /// Set a parameter value
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()>;

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
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()>;

    /// Get the processing latency in samples (if any)
    fn latency_samples(&self) -> usize {
        0
    }

    /// Get data from the plugin (if it's an analyzer or exposes internal state)
    /// Returns `None` by default for plugins that don't expose data.
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
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
        self.plugin.channels()
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
    ) -> PluginResult<()> {
        // Copy input to output, then process in-place
        output.copy_from_slice(input);
        self.plugin.process_in_place(output, context)
    }

    fn latency_samples(&self) -> usize {
        self.plugin.latency_samples()
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.plugin.get_data()
    }
}
