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

/// A parameter ramp for sample-accurate automation within a processing block.
///
/// When automation is active, the host evaluates the automation curve at the
/// start and end of each block and passes the ramp to the plugin via
/// [`ProcessContext::ramps`]. Plugins that support sample-accurate automation
/// can interpolate per-sample using [`ProcessContext::ramp_value_at`].
/// Plugins that don't read ramps are unaffected — the host also calls
/// `set_parameter(end_value)` for backward compatibility.
#[derive(Debug, Clone, Copy)]
pub struct ParameterRamp {
    /// Parameter index (position in the plugin's `parameters()` vec)
    pub param_index: u16,
    /// Value at the start of this processing block
    pub start_value: f32,
    /// Value at the end of this processing block
    pub end_value: f32,
}

/// Processing context passed to plugins
#[derive(Clone)]
pub struct ProcessContext {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of frames in this processing block
    pub num_frames: usize,
    /// Parameter ramps active for this block (empty if no automation).
    /// Plugins that support sample-accurate automation should interpolate
    /// between start_value and end_value across num_frames samples.
    pub ramps: Vec<ParameterRamp>,
}

impl ProcessContext {
    /// Create a new ProcessContext with no active parameter ramps.
    pub fn new(sample_rate: u32, num_frames: usize) -> Self {
        Self {
            sample_rate,
            num_frames,
            ramps: Vec::new(),
        }
    }

    /// Get the linearly interpolated value of a ramped parameter at a given
    /// sample offset within the current block.
    ///
    /// Returns `None` if no ramp exists for the given `param_index`.
    #[inline]
    pub fn ramp_value_at(&self, param_index: u16, sample_offset: usize) -> Option<f32> {
        self.ramps
            .iter()
            .find(|r| r.param_index == param_index)
            .map(|r| {
                let t = sample_offset as f32 / self.num_frames.max(1) as f32;
                r.start_value + (r.end_value - r.start_value) * t
            })
    }
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

    /// Returns the actual number of output frames from the last process() call.
    /// Default: returns None (unknown/not tracked).
    /// Plugins that produce variable output (like resamplers) should override this.
    fn last_output_frames(&self) -> Option<usize> {
        None
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

/// A MIDI message for instrument plugins.
/// Kept simple to avoid depending on the full sotf-midi crate.
#[derive(Debug, Clone)]
pub struct NoteEvent {
    /// Sample offset within the current processing block
    pub sample_offset: u32,
    /// MIDI channel (0-15)
    pub channel: u8,
    /// Event type
    pub kind: NoteEventKind,
}

/// The kind of note event.
#[derive(Debug, Clone)]
pub enum NoteEventKind {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    ControlChange { controller: u8, value: u8 },
    PitchBend { value: i16 },
}

/// Trait for instrument plugins that generate audio from MIDI/note events.
///
/// Instrument plugins are the first node in a MIDI track's processing chain.
/// They receive note events and produce audio output (no audio input).
pub trait InstrumentPlugin: Send {
    /// Plugin info
    fn info(&self) -> PluginInfo;

    /// Number of output channels (e.g., 2 for stereo synth)
    fn output_channels(&self) -> usize;

    /// Get the list of parameters
    fn parameters(&self) -> Vec<Parameter>;

    /// Set a parameter value
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()>;

    /// Get a parameter value
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue>;

    /// Process note events and generate audio output.
    ///
    /// `events` are sorted by sample_offset within the block.
    /// `output` is interleaved: [ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]
    /// Returns the number of frames written.
    fn process_events(
        &mut self,
        events: &[NoteEvent],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize>;

    /// Reset internal state (e.g., release all notes)
    fn reset(&mut self) {}

    /// Get the processing latency in samples
    fn latency_samples(&self) -> usize {
        0
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
            self.plugin.process_in_place(output, context)
        }
    }

    fn latency_samples(&self) -> usize {
        self.plugin.latency_samples()
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.plugin.get_data()
    }
}
