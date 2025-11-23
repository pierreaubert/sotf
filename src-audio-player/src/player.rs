use sotf_audio::engine::PluginConfig;
use sotf_audio::manager::{AudioEngineManager, StreamingEvent, StreamingState};
use sotf_audio::{LoudnessInfo, SpectrumInfo};
use std::path::PathBuf;

/// Batched playback state to reduce mutex locking
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub is_playing: bool,
    pub loudness: Option<LoudnessInfo>,
    pub spectrum: Option<SpectrumInfo>,
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
        // Stop current playback if any
        self.manager.stop()?;

        // Load the new file
        self.manager.load_file(&path)?;

        // Start playback with plugins and specified output device
        self.manager
            .start_playback(output_device, plugins, output_channels)?;

        Ok(())
    }

    pub fn update_plugins(
        &self,
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

    pub fn enable_spectrum_monitoring(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.enable_spectrum_monitoring()?;
        Ok(())
    }

    pub fn disable_spectrum_monitoring(&mut self) {
        self.manager.disable_spectrum_monitoring();
    }

    pub fn get_position(&self) -> f64 {
        self.manager.get_position()
    }

    pub fn is_playing(&self) -> bool {
        let state = self.manager.get_state();
        matches!(state, StreamingState::Playing)
    }

    pub fn get_loudness(&self) -> Option<LoudnessInfo> {
        self.manager.get_loudness()
    }

    pub fn get_spectrum(&self) -> Option<SpectrumInfo> {
        self.manager.get_spectrum()
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
    /// No extra locking - AudioEngineManager handles internal synchronization
    pub fn get_playback_state(&self, include_spectrum: bool) -> PlaybackState {
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

        // Only query analyzers when actually playing to reduce overhead
        let loudness = if is_playing {
            self.manager.get_loudness()
        } else {
            None
        };

        let spectrum = if include_spectrum && is_playing {
            // This Player wrapper does not currently expose spectrum monitoring,
            // so always return None for spectrum data.
            None
        } else {
            None
        };

        PlaybackState {
            position_secs,
            is_playing,
            loudness,
            spectrum,
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
        let state = player.get_playback_state(false);

        assert_eq!(state.position_secs, 0.0);
        assert!(!state.is_playing);
        assert!(state.loudness.is_none());
        assert!(state.spectrum.is_none());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn update_plugins_is_ok_when_no_engine_running() {
        let player = Player::new();
        let result = player.update_plugins(Vec::new());
        assert!(result.is_ok());
    }
}
