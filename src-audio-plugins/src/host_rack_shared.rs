// ============================================================================
// Shared Plugin Host - Thread-safe wrapper for PluginHost
// ============================================================================

use super::host::Host;
use super::host_rack::PluginHost;
use super::plugin::Plugin;
use std::sync::{Arc, Mutex};

/// Thread-safe wrapper for PluginHost
pub struct SharedPluginHost {
    inner: Arc<Mutex<PluginHost>>,
}

impl SharedPluginHost {
    /// Create a new shared plugin host
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PluginHost::new(channels, sample_rate))),
        }
    }

    /// Add a plugin to the chain
    pub fn add_plugin(&self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        self.inner
            .lock()
            .map_err(|e| format!("Failed to lock host: {}", e))?
            .add_plugin(plugin)
    }

    /// Process audio through the plugin chain
    pub fn process(&self, input: &[f32], output: &mut [f32]) -> Result<usize, String> {
        self.inner
            .lock()
            .map_err(|e| format!("Failed to lock host: {}", e))?
            .process(input, output)
    }

    /// Reset all plugins
    pub fn reset(&self) -> Result<(), String> {
        self.inner
            .lock()
            .map_err(|e| format!("Failed to lock host: {}", e))?
            .reset();
        Ok(())
    }

    /// Get input channel count
    pub fn input_channels(&self) -> Result<usize, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|e| format!("Failed to lock host: {}", e))?
            .input_channels())
    }

    /// Get output channel count
    pub fn output_channels(&self) -> Result<usize, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|e| format!("Failed to lock host: {}", e))?
            .output_channels())
    }
}

impl Clone for SharedPluginHost {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
