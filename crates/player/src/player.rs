use sotf_audio::engine::PluginConfig;
use sotf_audio::manager::{AudioEngineManager, StreamingEvent, StreamingState};
use std::path::PathBuf;
use std::sync::Arc;

/// Batched playback state to reduce mutex locking
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub is_playing: bool,
    pub sample_rate: Option<u32>,
    pub last_error: Option<String>,
}

pub struct Player {
    manager: AudioEngineManager,
}

impl Player {
    pub fn new() -> Self {
        Self {
            manager: AudioEngineManager::with_signal_watching(true),
        }
    }

    pub fn load_and_play(
        &mut self,
        path: PathBuf,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.load_and_play_at(path, plugins, output_channels, output_device, None)
    }

    pub fn load_and_play_at(
        &mut self,
        path: PathBuf,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        position: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Stop current playback if any
        self.manager.stop()?;

        // Load the new file
        self.manager.load_file(&path)?;

        // Start playback with plugins and specified output device
        self.manager
            .start_playback_at(output_device, plugins, output_channels, position)?;

        Ok(())
    }

    pub fn update_plugins(
        &mut self,
        plugins: Vec<PluginConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.manager.update_plugin_chain(plugins) {
            Ok(()) => Ok(()),
            Err(e) if e == "No engine running" => {
                // Engine not running yet - plugins will be applied on next playback
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Set a plugin parameter directly (zero-dropout update)
    ///
    /// This updates a single parameter without rebuilding the plugin chain.
    /// The value should be a string representation (JSON for complex types).
    pub fn set_plugin_parameter(
        &self,
        plugin_index: usize,
        param_id: String,
        value: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self
            .manager
            .set_plugin_parameter(plugin_index, param_id, value)
        {
            Ok(()) => Ok(()),
            Err(e) if e == "No engine running" => {
                // Engine not running yet - parameter will be applied on next playback
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.pause()?;
        Ok(())
    }

    pub fn resume(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.resume()?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.stop()?;
        Ok(())
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.set_volume(volume)?;
        Ok(())
    }

    pub fn get_position(&self) -> f64 {
        self.manager.get_position()
    }

    /// Seek to a specific position in seconds
    pub fn seek(&self, position_secs: f64) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.seek(position_secs)?;
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        let state = self.manager.get_state();
        matches!(state, StreamingState::Playing)
    }

    /// Get cached plugin data by engine index without blocking the audio pipeline.
    pub fn get_cached_plugin_data(
        &self,
        index: usize,
    ) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        self.manager.get_cached_plugin_data(index)
    }

    pub fn set_output_device(
        &mut self,
        device_name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Stop current playback before switching device
        self.manager.stop()?;

        // Store the device name for next playback
        // Note: The device will be applied when playback starts next time
        log::info!("Output device set to: {}", device_name);

        // The actual device switching happens in the AudioEngine when it starts
        // We need to restart playback with the new device setting
        Ok(())
    }

    /// Start HAL input playback (macOS only)
    ///
    /// This starts audio processing from the HAL virtual device instead of a file.
    /// The HAL device captures system-wide audio which is then processed through
    /// the plugin chain.
    ///
    /// # Arguments
    /// * `plugins` - Plugin chain to apply (should include hal_input as first plugin)
    /// * `output_channels` - Expected output channel count after all plugins
    /// * `output_device` - Output device name (None for default)
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub fn start_hal_playback(
        &mut self,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Use default sample rate (48kHz)
        self.start_hal_playback_with_config(plugins, output_channels, output_device, 48000)
    }

    /// Start HAL input playback with custom sample rate (macOS only)
    ///
    /// This starts audio processing from the HAL virtual device instead of a file.
    /// The HAL device captures system-wide audio which is then processed through
    /// the plugin chain.
    ///
    /// # Arguments
    /// * `plugins` - Plugin chain to apply (should include hal_input as first plugin)
    /// * `output_channels` - Expected output channel count after all plugins
    /// * `output_device` - Output device name (None for default)
    /// * `sample_rate` - Sample rate in Hz (e.g., 44100, 48000, 96000)
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub fn start_hal_playback_with_config(
        &mut self,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        sample_rate: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Stop current playback if any
        self.manager.stop()?;

        // Start HAL playback with custom sample rate
        self.manager.start_hal_playback_with_config(
            output_device,
            plugins,
            output_channels,
            sample_rate,
        )?;

        Ok(())
    }

    /// Get basic playback state in a single call.
    /// Plugin data should be queried separately via `get_cached_plugin_data`.
    pub fn get_playback_state(&self) -> PlaybackState {
        // Drain any pending streaming events and capture the last error (if any)
        let mut last_error: Option<String> = None;
        for event in self.manager.drain_events() {
            if let StreamingEvent::Error(msg) = event {
                last_error = Some(msg);
            }
        }

        let state = self.manager.get_state();
        let position_secs = self.manager.get_position();
        let is_playing = matches!(state, StreamingState::Playing);
        let sample_rate = self.manager.get_audio_info().map(|info| info.spec.sample_rate);

        PlaybackState {
            position_secs,
            is_playing,
            sample_rate,
            last_error,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_state_defaults_are_sane() {
        let player = Player::new();
        let state = player.get_playback_state();

        assert_eq!(state.position_secs, 0.0);
        assert!(!state.is_playing);
        assert!(state.sample_rate.is_none());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn update_plugins_is_ok_when_no_engine_running() {
        let mut player = Player::new();
        let result = player.update_plugins(Vec::new());
        assert!(result.is_ok());
    }
}
