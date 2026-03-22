// ============================================================================
// Audio Engine - Main Coordinator
// ============================================================================
//
// Coordinates all threads and provides the main API.

use super::*;
use std::time::Duration;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// Main audio engine
pub struct AudioEngine {
    manager: ManagerThread,
}

impl AudioEngine {
    fn expect_ok_response(&self) -> Result<(), String> {
        match self.manager.recv_response_timeout(RESPONSE_TIMEOUT)? {
            ManagerResponse::Ok | ManagerResponse::Shutdown => Ok(()),
            ManagerResponse::Error(e) => Err(e),
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// Drain any pending manager response without blocking.
    /// Used by fire-and-forget commands to clear the response channel.
    fn drain_response(&self) {
        // Try to receive any response that arrived (non-blocking).
        // If the manager sent one for a previous command, consume it
        // so it doesn't pollute future synchronous calls.
        while self.manager.try_recv_response().is_some() {}
    }

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
    pub fn play<P: Into<std::path::PathBuf>>(&self, path: P) -> Result<(), String> {
        self.drain_response();
        self.manager
            .send_command(ManagerCommand::Play(path.into()))?;
        self.expect_ok_response()
    }

    /// Play an audio file at a specific position
    pub fn play_at<P: Into<std::path::PathBuf>>(
        &self,
        path: P,
        position: f64,
    ) -> Result<(), String> {
        self.drain_response();
        self.manager
            .send_command(ManagerCommand::PlayAt(path.into(), position))?;
        self.expect_ok_response()
    }

    /// Pause playback
    pub fn pause(&self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Pause)?;
        self.expect_ok_response()
    }

    /// Resume playback
    pub fn resume(&self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Resume)?;
        self.expect_ok_response()
    }

    /// Stop playback
    pub fn stop(&self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Stop)?;
        self.expect_ok_response()
    }

    /// Seek to position in seconds
    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Seek(position))?;
        self.expect_ok_response()
    }

    /// Set volume (0.0 = silence, 1.0 = unity gain)
    pub fn set_volume(&self, volume: f32) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::SetVolume(volume))?;
        self.expect_ok_response()
    }

    /// Mute/unmute
    pub fn set_mute(&self, muted: bool) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Mute(muted))?;
        self.expect_ok_response()
    }

    /// Update the plugin chain (hot-reload with crossfade)
    pub fn update_plugin_chain(&self, plugins: Vec<PluginConfig>) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::UpdatePluginChain(plugins))?;
        self.expect_ok_response()
    }

    /// Set a plugin parameter
    ///
    /// The value should be a string representation:
    /// - For primitives: "1.5", "42", "true"
    /// - For complex types: JSON string
    pub fn set_plugin_parameter(
        &self,
        plugin_index: usize,
        param_id: String,
        value: String,
    ) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::SetPluginParameter {
                plugin_index,
                param_id,
                value,
            })?;
        self.expect_ok_response()
    }

    /// Bypass all processing
    pub fn set_bypass(&self, bypass: bool) -> Result<(), String> {
        self.manager
            .send_command(ManagerCommand::BypassProcessing(bypass))?;
        self.expect_ok_response()
    }

    /// Get current engine state
    pub fn get_state(&self) -> AudioEngineState {
        self.manager.get_state()
    }

    /// Get current position in seconds
    pub fn get_position(&self) -> Result<f64, String> {
        self.manager.send_command(ManagerCommand::GetPosition)?;
        match self.manager.recv_response_timeout(RESPONSE_TIMEOUT)? {
            ManagerResponse::Position(pos) => Ok(pos),
            ManagerResponse::Error(e) => Err(e),
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// Get plugin data (e.g. analyzer results) via synchronous command round-trip.
    /// Prefer `get_cached_plugin_data` for UI polling to avoid blocking the audio pipeline.
    pub fn get_plugin_data(
        &self,
        index: usize,
    ) -> Result<std::sync::Arc<dyn std::any::Any + Send + Sync>, String> {
        self.manager
            .send_command(ManagerCommand::GetPluginData(index))?;
        match self.manager.recv_response_timeout(RESPONSE_TIMEOUT)? {
            ManagerResponse::PluginData(data) => Ok(data),
            ManagerResponse::Error(e) => Err(e),
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// Get cached plugin data directly without blocking the audio pipeline.
    /// The processing thread updates this cache after every frame.
    pub fn get_cached_plugin_data(
        &self,
        index: usize,
    ) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        self.manager.get_cached_plugin_data(index)
    }

    /// Reload configuration from file
    pub fn reload_config(&self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::ReloadConfig)?;
        self.expect_ok_response()
    }

    /// Shutdown the engine
    pub fn shutdown(&self) -> Result<(), String> {
        self.manager.send_command(ManagerCommand::Shutdown)?;
        // Manager may close channel before we receive response, which is fine
        self.manager.recv_response().ok();
        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        // We use &self shutdown if possible, but drop has &mut self anyway
        let _ = self.shutdown();
    }
}
