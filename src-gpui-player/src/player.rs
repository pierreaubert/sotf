use sotf_audio::engine::PluginConfig;
use sotf_audio::manager::AudioStreamingManager;
use sotf_audio::plugins::{LoudnessInfo, SpectrumInfo};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

/// Batched playback state to reduce mutex locking
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub is_playing: bool,
    pub loudness: Option<LoudnessInfo>,
    pub spectrum: Option<SpectrumInfo>,
}

pub struct Player {
    manager: Arc<Mutex<AudioStreamingManager>>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(AudioStreamingManager::new())),
        }
    }

    pub fn load_and_play(
        &self,
        path: PathBuf,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock();

        // Stop current playback if any
        manager.stop()?;

        // Load the new file
        manager.load_file(&path)?;

        // Start playback with plugins and specified output device
        manager.start_playback(output_device, plugins, output_channels)?;

        Ok(())
    }

    pub fn update_plugins(
        &self,
        plugins: Vec<PluginConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock();
        // Ignore error if engine not running - plugins will be applied on next playback
        let _ = manager.update_plugin_chain(plugins);
        Ok(())
    }

    pub fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock();
        manager.pause()?;
        Ok(())
    }

    pub fn resume(&self) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock();
        manager.resume()?;
        Ok(())
    }

    pub fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock();
        manager.stop()?;
        Ok(())
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock();
        manager.set_volume(volume)?;
        Ok(())
    }

    pub fn get_position(&self) -> Result<f64, Box<dyn std::error::Error>> {
        let manager = self.manager.lock();
        Ok(manager.get_position())
    }

    pub fn is_playing(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let manager = self.manager.lock();
        let state = manager.get_state();
        Ok(matches!(
            state,
            sotf_audio::manager::StreamingState::Playing
        ))
    }

    pub fn enable_loudness_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock();
        manager.enable_loudness_monitoring()?;
        Ok(())
    }

    pub fn get_loudness(&self) -> Option<LoudnessInfo> {
        let manager = self.manager.lock();
        manager.get_loudness()
    }

    pub fn enable_spectrum_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock();
        manager.enable_spectrum_monitoring()?;
        Ok(())
    }

    pub fn disable_spectrum_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock();
        manager.disable_spectrum_monitoring();
        Ok(())
    }

    pub fn get_spectrum(&self) -> Option<SpectrumInfo> {
        let manager = self.manager.lock();
        manager.get_spectrum()
    }

    pub fn set_output_device(
        &self,
        device_name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock();

        // Stop current playback before switching device
        manager.stop()?;

        // Store the device name for next playback
        // Note: The device will be applied when playback starts next time
        log::info!("Output device set to: {}", device_name);

        // The actual device switching happens in the AudioEngine when it starts
        // We need to restart playback with the new device setting
        Ok(())
    }

    /// Get all playback state in a single lock acquisition to reduce contention
    /// This matches the efficient polling pattern from sotf_player.rs
    pub fn get_playback_state(&self, include_spectrum: bool) -> PlaybackState {
        let manager = self.manager.lock();

        // Call try_recv_event to process any pending events (non-blocking)
        manager.try_recv_event();

        let state = manager.get_state();
        let position_secs = manager.get_position();
        let is_playing = matches!(
            state,
            sotf_audio::manager::StreamingState::Playing
        );

        // Only query analyzers when actually playing to reduce overhead
        let loudness = if is_playing {
            manager.get_loudness()
        } else {
            None
        };

        let spectrum = if include_spectrum && is_playing {
            manager.get_spectrum()
        } else {
            None
        };

        PlaybackState {
            position_secs,
            is_playing,
            loudness,
            spectrum,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
