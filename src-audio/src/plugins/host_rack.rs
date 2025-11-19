// ============================================================================
// Plugin Host Rack - Simple linear chain of plugins
// ============================================================================

use super::host::Host;
use super::plugin::{Plugin, ProcessContext};

/// Plugin host that chains multiple audio plugins
///
/// The host manages a chain of plugins, routing audio through them sequentially.
/// Channel count can change between plugins (e.g., stereo -> mono -> stereo).
///
/// # Example
/// ```
/// use sotf_audio::plugins::Host;
/// use sotf_audio::{PluginHost, GainPlugin, InPlacePluginAdapter};
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
pub struct PluginHost {
    /// Chain of plugins
    plugins: Vec<Box<dyn Plugin>>,
    /// Current input channel count
    input_channels: usize,
    /// Current output channel count (after all plugins)
    output_channels: usize,
    /// Sample rate
    sample_rate: u32,
    /// Intermediate buffers for plugin chain
    buffers: Vec<Vec<f32>>,
    /// Maximum buffer size (in frames)
    max_buffer_frames: usize,
}

impl PluginHost {
    /// Create a new plugin host
    ///
    /// # Arguments
    /// * `channels` - Initial number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            plugins: Vec::new(),
            input_channels: channels,
            output_channels: channels,
            sample_rate,
            buffers: Vec::new(),
            max_buffer_frames: 0,
        }
    }

    /// Allocate intermediate buffers for the plugin chain
    fn allocate_buffers(&mut self, num_frames: usize) {
        self.max_buffer_frames = num_frames;
        self.buffers.clear();

        // Allocate a buffer for each plugin (except the last one, which writes to output)
        if self.plugins.len() > 1 {
            for i in 0..self.plugins.len() - 1 {
                let output_channels = self.plugins[i].output_channels();
                let buffer_size = num_frames * output_channels;
                self.buffers.push(vec![0.0; buffer_size]);
            }
        }
    }
}

impl Host for PluginHost {
    /// Add a plugin to the end of the chain
    ///
    /// Returns an error if the plugin's input channels don't match
    /// the current output channels.
    fn add_plugin(&mut self, mut plugin: Box<dyn Plugin>) -> Result<(), String> {
        // Verify channel compatibility
        let expected_input = if self.plugins.is_empty() {
            self.input_channels
        } else {
            self.output_channels
        };

        if plugin.input_channels() != expected_input {
            return Err(format!(
                "Plugin '{}' expects {} input channels, but chain provides {}",
                plugin.info().name,
                plugin.input_channels(),
                expected_input
            ));
        }

        // Initialize the plugin
        plugin.initialize(self.sample_rate)?;

        // Update output channel count
        self.output_channels = plugin.output_channels();

        // Add to chain
        self.plugins.push(plugin);

        Ok(())
    }

    /// Remove a plugin at the given index
    fn remove_plugin(&mut self, index: usize) -> Result<Box<dyn Plugin>, String> {
        if index >= self.plugins.len() {
            return Err(format!("Plugin index {} out of bounds", index));
        }

        let plugin = self.plugins.remove(index);

        // Recalculate output channels
        self.output_channels = if self.plugins.is_empty() {
            self.input_channels
        } else {
            self.plugins.last().unwrap().output_channels()
        };

        Ok(plugin)
    }

    /// Get the number of plugins in the chain
    fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get plugin at index (immutable)
    fn get_plugin(&self, index: usize) -> Option<&dyn Plugin> {
        self.plugins.get(index).map(|p| p.as_ref())
    }

    /// Get input channel count
    fn input_channels(&self) -> usize {
        self.input_channels
    }

    /// Get output channel count (after all plugins)
    fn output_channels(&self) -> usize {
        self.output_channels
    }

    /// Process audio through the plugin chain
    ///
    /// # Arguments
    /// * `input` - Interleaved input samples (length = num_frames * input_channels)
    /// * `output` - Interleaved output samples (length = num_frames * output_channels)
    ///
    /// # Returns
    /// Number of frames processed, or error message
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        if self.plugins.is_empty() {
            // No plugins - just copy input to output
            if input.len() != output.len() {
                return Err(format!(
                    "Input and output sizes must match when no plugins are present (got {} vs {})",
                    input.len(),
                    output.len()
                ));
            }
            output.copy_from_slice(input);
            return Ok(input.len() / self.input_channels);
        }

        // Verify input size
        if !input.len().is_multiple_of(self.input_channels) {
            return Err(format!(
                "Input size {} is not a multiple of input channels {}",
                input.len(),
                self.input_channels
            ));
        }

        let num_frames = input.len() / self.input_channels;

        // Verify output size
        let expected_output_size = num_frames * self.output_channels;
        if output.len() != expected_output_size {
            return Err(format!(
                "Output buffer size {} doesn't match expected {} (frames={}, channels={})",
                output.len(),
                expected_output_size,
                num_frames,
                self.output_channels
            ));
        }

        // Resize buffers if needed
        if num_frames > self.max_buffer_frames {
            self.allocate_buffers(num_frames);
        }

        // Create processing context
        let context = ProcessContext {
            sample_rate: self.sample_rate,
            num_frames,
        };

        // Process through plugin chain
        // We need to handle the borrow checker carefully here
        let plugin_count = self.plugins.len();
        let mut current_input_vec = input.to_vec();

        for i in 0..plugin_count {
            let plugin = &mut self.plugins[i];
            let output_channels = plugin.output_channels();
            let output_size = num_frames * output_channels;

            // Determine output buffer
            if i == plugin_count - 1 {
                // Last plugin writes directly to output
                plugin.process(&current_input_vec, &mut output[..output_size], &context)?;
            } else {
                // Intermediate plugin writes to buffer, then we copy to input_vec for next iteration
                plugin.process(
                    &current_input_vec,
                    &mut self.buffers[i][..output_size],
                    &context,
                )?;
                // Copy buffer to input_vec for next plugin
                current_input_vec.clear();
                current_input_vec.extend_from_slice(&self.buffers[i][..output_size]);
            }
        }

        Ok(num_frames)
    }

    /// Reset all plugins in the chain
    fn reset(&mut self) {
        for plugin in &mut self.plugins {
            plugin.reset();
        }
    }

    /// Get total latency in samples (sum of all plugin latencies)
    fn total_latency_samples(&self) -> usize {
        self.plugins.iter().map(|p| p.latency_samples()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::super::plugin::InPlacePluginAdapter;
    use super::*;
    use crate::plugins::GainPlugin;

    #[test]
    fn test_empty_host() {
        let mut host = PluginHost::new(2, 44100);
        let input = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let mut output = vec![0.0; 4];

        let frames = host.process(&input, &mut output).unwrap();
        assert_eq!(frames, 2);
        assert_eq!(output, input); // Should pass through unchanged
    }

    #[test]
    fn test_single_plugin() {
        let mut host = PluginHost::new(2, 44100);
        let gain_plugin = GainPlugin::new(2, -6.0);
        let adapter = InPlacePluginAdapter::new(gain_plugin);
        host.add_plugin(Box::new(adapter)).unwrap();

        let input = vec![1.0, 1.0, 1.0, 1.0]; // 2 frames, 2 channels
        let mut output = vec![0.0; 4];

        let frames = host.process(&input, &mut output).unwrap();
        assert_eq!(frames, 2);

        // -6dB is approximately 0.5x amplitude
        for &sample in &output {
            assert!((sample - 0.5).abs() < 0.01);
        }
    }
}
