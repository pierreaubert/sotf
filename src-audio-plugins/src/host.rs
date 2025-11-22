// ============================================================================
// Plugin Host Trait - Common interface for plugin hosts
// ============================================================================

use super::plugin::Plugin;

/// Common trait for plugin hosts
///
/// Plugin hosts manage a collection of audio plugins and route audio through them.
/// Different implementations provide different topologies:
/// - PluginHost: Simple linear chain (rack-style)
/// - GraphHost: Directed acyclic graph with parallel stages
/// - DawHost: Thread-safe DAG with parallel processing (DAW-style)
///
/// # Example
/// ```
/// use sotf_plugins::Host;
/// use sotf_plugins::{PluginHost, GainPlugin, InPlacePluginAdapter};
///
/// let mut host = PluginHost::new(2, 44100); // Start with 2 channels
/// let gain = GainPlugin::new(2, -6.0);
/// host.add_plugin(Box::new(InPlacePluginAdapter::new(gain))).unwrap();
///
/// // Process audio
/// let input = vec![1.0; 8]; // 4 frames, 2 channels
/// let mut output = vec![0.0; 8];
/// host.process(&input, &mut output).unwrap();
/// ```
pub trait Host {
    /// Add a plugin to the host
    ///
    /// Returns an error if the plugin's input channels don't match
    /// the current output channels.
    fn add_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String>;

    /// Remove a plugin at the given index
    ///
    /// Returns the removed plugin, or an error if the index is out of bounds.
    fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String>;

    /// Get the number of plugins in the host
    fn plugin_count(&self) -> usize;

    /// Get plugin at index (immutable)
    fn get_plugin(&self, index: usize) -> Option<&dyn Plugin>;

    /// Get input channel count
    fn input_channels(&self) -> usize;

    /// Get output channel count (after all plugins)
    fn output_channels(&self) -> usize;

    /// Process audio through the plugin chain/graph
    ///
    /// # Arguments
    /// * `input` - Interleaved input samples (length = num_frames * input_channels)
    /// * `output` - Interleaved output samples (length = num_frames * output_channels)
    ///
    /// # Returns
    /// Number of frames processed, or error message
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String>;

    /// Reset all plugins in the host
    fn reset(&mut self);

    /// Get total latency in samples
    fn total_latency_samples(&self) -> usize;
}
