// ============================================================================
// Audio Engine - Main Coordinator
// ============================================================================
//
// Coordinates all threads and provides the main API.

use super::*;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

// Host construction and the subsequent processing-thread commit are each
// bounded independently. Keep the public call budget longer than their sum so
// a caller cannot time out while an accepted update is still able to land.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_RESPONSE_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_RUNNING: u8 = 0;
const SHUTDOWN_STARTED: u8 = 1;
const SHUTDOWN_COMPLETE: u8 = 2;

/// Main audio engine
pub struct AudioEngine {
    manager: ManagerThread,
    command_lock: Mutex<()>,
    shutdown_state: AtomicU8,
}

impl AudioEngine {
    fn expect_ok_response(&self, request_id: u64) -> Result<(), String> {
        match self.manager.recv_response_for(request_id, RESPONSE_TIMEOUT) {
            Ok(ManagerResponse::Ok | ManagerResponse::Shutdown) => Ok(()),
            Ok(ManagerResponse::Error(e)) => Err(e),
            Ok(_) => Err("Unexpected response".to_string()),
            Err(_) => {
                // Channel disconnected — manager thread likely died during init.
                // Check the shared state for a more descriptive error.
                let engine_state = self.manager.get_state();
                if let Some(err) = engine_state.last_error {
                    Err(err)
                } else {
                    Err("Engine thread exited unexpectedly".to_string())
                }
            }
        }
    }

    fn send_expect_ok(&self, command: ManagerCommand) -> Result<(), String> {
        let _guard = self
            .command_lock
            .lock()
            .map_err(|e| format!("Failed to lock command channel: {}", e))?;
        let request_id = self.manager.send_command(command)?;
        self.expect_ok_response(request_id)
    }

    fn send_recv(&self, command: ManagerCommand) -> Result<ManagerResponse, String> {
        let _guard = self
            .command_lock
            .lock()
            .map_err(|e| format!("Failed to lock command channel: {}", e))?;
        let request_id = self.manager.send_command(command)?;
        self.manager.recv_response_for(request_id, RESPONSE_TIMEOUT)
    }

    /// Create and start a new audio engine
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let manager = ManagerThread::new(config)?;
        Ok(Self {
            manager,
            command_lock: Mutex::new(()),
            shutdown_state: AtomicU8::new(SHUTDOWN_RUNNING),
        })
    }

    /// Create with default configuration
    pub fn new_default() -> Result<Self, String> {
        Self::new(EngineConfig::default())
    }

    /// Play an audio source (file, URL, or service stream).
    pub fn play(&self, source: impl Into<crate::decoder::AudioSource>) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::Play(source.into()))
    }

    /// Play an audio source at a specific position.
    pub fn play_at(
        &self,
        source: impl Into<crate::decoder::AudioSource>,
        position: f64,
    ) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::PlayAt(source.into(), position))
    }

    /// Queue the next source for gapless playback.
    ///
    /// When the current track finishes decoding, the decoder seamlessly transitions
    /// to the queued source without any gap in audio output. Only one source can be
    /// queued at a time; calling this again replaces the previous queued source.
    pub fn queue_next(&self, source: impl Into<crate::decoder::AudioSource>) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::QueueNext(source.into()))
    }

    /// Cancel a previously queued next source.
    ///
    /// If no source is queued, this is a no-op (still returns Ok).
    pub fn cancel_next(&self) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::CancelNext)
    }

    /// Pause playback
    pub fn pause(&self) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::Pause)
    }

    /// Resume playback
    pub fn resume(&self) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::Resume)
    }

    /// Stop playback
    pub fn stop(&self) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::Stop)
    }

    /// Seek to position in seconds
    pub fn seek(&self, position: f64) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::Seek(position))
    }

    /// Set volume (0.0 = silence, 1.0 = unity gain)
    pub fn set_volume(&self, volume: f32) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::SetVolume(volume))
    }

    /// Mute/unmute
    pub fn set_mute(&self, muted: bool) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::Mute(muted))
    }

    /// Update the plugin chain (hot-reload with crossfade).
    ///
    /// Takes a slice so callers can keep their owned `Vec` for other uses
    /// (e.g. crash-recovery snapshots); the engine clones what it needs to
    /// send across the manager thread boundary.
    pub fn update_plugin_chain(&self, plugins: &[PluginConfig]) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::UpdatePluginChain(plugins.to_vec()))
    }

    /// Update the plugin graph (DAG topology for multi-driver crossovers)
    pub fn update_plugin_graph(
        &self,
        config: super::types::PluginGraphConfig,
    ) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::UpdatePluginGraph(config))
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
        self.send_expect_ok(ManagerCommand::SetPluginParameter {
            plugin_index,
            param_id,
            value,
        })
    }

    /// Bypass all processing
    pub fn set_bypass(&self, bypass: bool) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::BypassProcessing(bypass))
    }

    /// Poll isolated external plugin worker status.
    ///
    /// This does not start or restart worker processes from the realtime processing
    /// thread; crashed workers are reported and audio falls back to passthrough.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn maintain_isolated_external_plugin_workers(&self) -> Result<(), String> {
        self.send_expect_ok(ManagerCommand::MaintainIsolatedExternalPluginWorkers)
    }

    /// Get current engine state
    pub fn get_state(&self) -> AudioEngineState {
        self.manager.get_state()
    }

    /// Get just the current playback state without cloning the full engine state.
    pub fn get_playback_state(&self) -> PlaybackState {
        self.manager.get_playback_state()
    }

    /// Get the latest isolated external plugin worker statuses.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn get_isolated_external_plugin_worker_statuses(
        &self,
    ) -> Vec<crate::IsolatedExternalPluginWorkerStatus> {
        self.manager
            .get_state()
            .isolated_external_plugin_worker_statuses
    }

    /// Get current position in seconds
    pub fn get_position(&self) -> Result<f64, String> {
        match self.send_recv(ManagerCommand::GetPosition)? {
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
        match self.send_recv(ManagerCommand::GetPluginData(index))? {
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
        self.send_expect_ok(ManagerCommand::ReloadConfig)
    }

    /// Shutdown the engine
    pub fn shutdown(&self) -> Result<(), String> {
        let _guard = self
            .command_lock
            .lock()
            .map_err(|e| format!("Failed to lock command channel: {}", e))?;
        if self.shutdown_state.load(Ordering::Acquire) == SHUTDOWN_COMPLETE {
            return Ok(());
        }
        let request_id = if self
            .shutdown_state
            .compare_exchange(
                SHUTDOWN_RUNNING,
                SHUTDOWN_STARTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.manager.send_command(ManagerCommand::Shutdown)?
        } else {
            // The command lock serializes callers, so STARTED here means an
            // earlier attempt timed out while cleanup was still in progress.
            // Wait for manager completion rather than returning false success.
            return self
                .manager
                .wait_for_exit(SHUTDOWN_RESPONSE_TIMEOUT)
                .map(|_| {
                    self.shutdown_state
                        .store(SHUTDOWN_COMPLETE, Ordering::Release);
                });
        };
        match self
            .manager
            .recv_response_for(request_id, SHUTDOWN_RESPONSE_TIMEOUT)
        {
            Ok(ManagerResponse::Shutdown | ManagerResponse::Ok) => {
                self.shutdown_state
                    .store(SHUTDOWN_COMPLETE, Ordering::Release);
                Ok(())
            }
            Ok(ManagerResponse::Error(error)) => Err(error),
            Ok(_) => Err("Unexpected shutdown response from engine manager".to_string()),
            Err(error) => Err(format!(
                "Engine shutdown response timed out after {:?}: {}",
                SHUTDOWN_RESPONSE_TIMEOUT, error
            )),
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        // We use &self shutdown if possible, but drop has &mut self anyway
        let _ = self.shutdown();
    }
}
