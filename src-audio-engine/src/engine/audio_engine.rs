// ============================================================================
// Audio Engine - Main Coordinator
// ============================================================================
//
// Coordinates all threads and provides the main API.

use super::*;

/// Main audio engine
pub struct AudioEngine {
    manager: ManagerThread,
}

impl AudioEngine {
    /// Create and start a new audio engine
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let manager = ManagerThread::new(config)?;
        Ok(Self { manager })
    }

    /// Create with default configuration
    pub fn new_default() -> Result<Self, String> {
        Self::new(EngineConfig::default())
    }

    /// Play an audio file
    pub fn play<P: Into<std::path::PathBuf>>(&mut self, path: P) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::Play(path.into()))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Pause playback
    pub fn pause(&mut self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Pause)?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Resume playback
    pub fn resume(&mut self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Resume)?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Stop playback
    pub fn stop(&mut self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Stop)?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Seek to position in seconds
    pub fn seek(&mut self, position: f64) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Seek(position))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Set volume (0.0 = silence, 1.0 = unity gain)
    pub fn set_volume(&mut self, volume: f32) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::SetVolume(volume))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Mute/unmute
    pub fn set_mute(&mut self, muted: bool) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Mute(muted))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Update the plugin chain (hot-reload with crossfade)
    pub fn update_plugin_chain(&mut self, plugins: Vec<PluginConfig>) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::UpdatePluginChain(plugins))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Set a plugin parameter
    pub fn set_plugin_parameter(
        &mut self,
        plugin_index: usize,
        param_id: String,
        value: f32,
    ) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::SetPluginParameter {
                plugin_index,
                param_id,
                value,
            })?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Bypass all processing
    pub fn set_bypass(&mut self, bypass: bool) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::BypassProcessing(bypass))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Add a loudness analyzer
    pub fn add_loudness_analyzer(&mut self, id: String, channels: usize) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::AddLoudnessAnalyzer { id, channels })?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Add a spectrum analyzer
    pub fn add_spectrum_analyzer(&mut self, id: String, channels: usize) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::AddSpectrumAnalyzer { id, channels })?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Remove an analyzer
    pub fn remove_analyzer(&mut self, id: String) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::RemoveAnalyzer(id))?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Get current engine state
    pub fn get_state(&self) -> AudioEngineState {
        self.manager.get_state()
    }

    /// Get current position in seconds
    pub fn get_position(&mut self) -> Result<f64, String> {
        self.manager.send_command(ManagerCommand::GetPosition)?;
        match self.manager.recv_response()? {
            ManagerResponse::Position(pos) => Ok(pos),
            ManagerResponse::Error(e) => Err(e),
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// Get analyzer data
    pub fn get_analyzer_data(
        &mut self,
        analyzer_id: String,
    ) -> Result<std::sync::Arc<dyn std::any::Any + Send + Sync>, String> {
        self.manager
            .send_command(ManagerCommand::GetAnalyzerData(analyzer_id))?;
        match self.manager.recv_response()? {
            ManagerResponse::AnalyzerData(data) => Ok(data),
            ManagerResponse::Error(e) => Err(e),
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// Reload configuration from file
    pub fn reload_config(&mut self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::ReloadConfig)?;
        self.manager.recv_response()?;
        Ok(())
    }

    /// Shutdown the engine
    pub fn shutdown(&mut self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Shutdown)?;
        // Manager may close channel before we receive response, which is fine
        self.manager.recv_response().ok();
        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.shutdown().ok();
    }
}
