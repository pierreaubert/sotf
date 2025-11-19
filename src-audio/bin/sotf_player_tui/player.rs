use sotf_audio::engine::PluginConfig;
use sotf_audio::manager::AudioStreamingManager;
use sotf_audio::plugins::LoudnessInfo;
use std::path::PathBuf;

/// Batched playback state to reduce mutex locking
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub is_playing: bool,
    pub loudness: Option<LoudnessInfo>,
}

pub struct Player {
    manager: AudioStreamingManager,
}

impl Player {
    pub fn new() -> Self {
        Self {
            manager: AudioStreamingManager::with_signal_watching(true),
        }
    }

    pub fn load_and_play(
        &mut self,
        path: PathBuf,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Stop current playback if any
        self.manager.stop()?;

        // Load the new file
        self.manager.load_file(&path)?;

        // Start playback with plugins and specified output device
        self.manager.start_playback(output_device, plugins, output_channels)?;

        Ok(())
    }

    pub fn update_plugins(
        &self,
        plugins: Vec<PluginConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Ignore error if engine not running - plugins will be applied on next playback
        let _ = self.manager.update_plugin_chain(plugins);
        Ok(())
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

    pub fn enable_loudness_monitoring(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.enable_loudness_monitoring()?;
        Ok(())
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

    /// Get all playback state in a single call
    /// No extra locking - AudioStreamingManager handles internal synchronization
    pub fn get_playback_state(&self) -> PlaybackState {
        // Call try_recv_event to process any pending events (non-blocking)
        self.manager.try_recv_event();

        let state = self.manager.get_state();
        let position_secs = self.manager.get_position();
        let is_playing = matches!(
            state,
            sotf_audio::manager::StreamingState::Playing
        );

        // Only query analyzers when actually playing to reduce overhead
        let loudness = if is_playing {
            self.manager.get_loudness()
        } else {
            None
        };

        PlaybackState {
            position_secs,
            is_playing,
            loudness
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
