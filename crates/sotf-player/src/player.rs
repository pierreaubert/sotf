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
    /// Set once after the engine auto-restarted from a crash. Cleared after read.
    pub engine_restarted: bool,
    /// Set when the engine crashed twice — no further auto-restart will be attempted.
    pub engine_fatal: bool,
}

/// Saved configuration for restarting after a crash.
#[derive(Clone)]
struct SavedPlaybackConfig {
    path: PathBuf,
    plugins: Vec<PluginConfig>,
    output_channels: usize,
    output_device: Option<String>,
    last_position_secs: f64,
}

pub struct Player {
    manager: AudioEngineManager,
    /// Config saved at each `load_and_play_at` for crash recovery.
    saved_config: Option<SavedPlaybackConfig>,
    /// How many times we've restarted for the current track (max 1).
    restart_count: u32,
    /// One-shot flag: engine was restarted, cleared after `get_playback_state` returns it.
    engine_restarted_flag: bool,
    /// Sticky flag: engine crashed fatally (second crash), cleared on next `load_and_play_at`.
    engine_fatal_flag: bool,
}

impl Player {
    pub fn new() -> Self {
        Self {
            manager: AudioEngineManager::with_signal_watching(true),
            saved_config: None,
            restart_count: 0,
            engine_restarted_flag: false,
            engine_fatal_flag: false,
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
        // Save config for potential crash recovery
        self.saved_config = Some(SavedPlaybackConfig {
            path: path.clone(),
            plugins: plugins.clone(),
            output_channels,
            output_device: output_device.clone(),
            last_position_secs: position.unwrap_or(0.0),
        });
        self.restart_count = 0;
        self.engine_restarted_flag = false;
        self.engine_fatal_flag = false;

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
        match self.manager.update_plugin_chain(plugins.clone()) {
            Ok(()) => {
                // Update saved config so crash recovery uses latest plugins
                if let Some(ref mut config) = self.saved_config {
                    config.plugins = plugins;
                }
                Ok(())
            }
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
        self.saved_config = None;
        self.restart_count = 0;
        self.engine_restarted_flag = false;
        self.engine_fatal_flag = false;
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

        // Clear the verified sample rate cache since the device changed
        sotf_audio::clear_verified_rate_cache();

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
    ///
    /// This also drives crash detection and auto-restart: when a streaming error
    /// is detected and we have a saved config, the engine is restarted once.
    /// A second crash sets `engine_fatal` and gives up.
    pub fn get_playback_state(&mut self) -> PlaybackState {
        // Drain any pending streaming events and capture the last error (if any)
        let mut last_error: Option<String> = None;
        for event in self.manager.drain_events() {
            if let StreamingEvent::Error(msg) = event {
                last_error = Some(msg);
            }
        }

        // Crash detection & auto-restart
        if let Some(ref err) = last_error {
            if self.saved_config.is_some() {
                if self.restart_count == 0 {
                    log::error!(
                        "[Player] Engine crashed: {}. Attempting auto-restart...",
                        err
                    );
                    self.restart_count = 1;
                    match self.attempt_restart() {
                        Ok(()) => {
                            log::info!("[Player] Engine restarted successfully");
                            self.engine_restarted_flag = true;
                            // Clear the error — the restart succeeded
                            last_error = None;
                        }
                        Err(e) => {
                            log::error!("[Player] Restart failed: {}", e);
                            self.engine_fatal_flag = true;
                            last_error = Some(format!("Engine crashed and restart failed: {}", e));
                        }
                    }
                } else {
                    log::error!("[Player] Engine crashed again: {}. Giving up.", err);
                    self.engine_fatal_flag = true;
                }
            }
        }

        let state = self.manager.get_state();
        let position_secs = self.manager.get_position();
        let is_playing = matches!(state, StreamingState::Playing);
        let sample_rate = self
            .manager
            .get_audio_info()
            .map(|info| info.spec.sample_rate);

        // Read and clear one-shot flags
        let engine_restarted = self.engine_restarted_flag;
        self.engine_restarted_flag = false;
        let engine_fatal = self.engine_fatal_flag;

        PlaybackState {
            position_secs,
            is_playing,
            sample_rate,
            last_error,
            engine_restarted,
            engine_fatal,
        }
    }

    /// Attempt to restart the engine from saved config.
    fn attempt_restart(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = self
            .saved_config
            .clone()
            .ok_or("attempt_restart called without saved config")?;

        // Use the last known position so the user resumes near where they were
        let position = self.manager.get_position();
        let resume_pos = if position > 0.0 {
            position
        } else {
            config.last_position_secs
        };

        // Stop the dead engine
        let _ = self.manager.stop();

        // Re-load and play
        self.manager.load_file(&config.path)?;
        self.manager.start_playback_at(
            config.output_device,
            config.plugins,
            config.output_channels,
            Some(resume_pos),
        )?;

        // Update saved position for any future restart
        if let Some(ref mut cfg) = self.saved_config {
            cfg.last_position_secs = resume_pos;
        }

        Ok(())
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
        let mut player = Player::new();
        let state = player.get_playback_state();

        assert_eq!(state.position_secs, 0.0);
        assert!(!state.is_playing);
        assert!(state.sample_rate.is_none());
        assert!(state.last_error.is_none());
        assert!(!state.engine_restarted);
        assert!(!state.engine_fatal);
    }

    #[test]
    fn update_plugins_is_ok_when_no_engine_running() {
        let mut player = Player::new();
        let result = player.update_plugins(Vec::new());
        assert!(result.is_ok());
    }
}
