// ============================================================================
// Audio Streaming Manager
// ============================================================================

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::engine::{AudioEngine, AudioEngineState, EngineConfig, PlaybackState, PluginConfig};
use crate::plugins::{LoudnessInfo, SpectrumInfo};
use crate::{AudioDecoderError, AudioDecoderResult, AudioFormat, AudioSpec, probe_file};

/// High-level audio streaming manager using native AudioEngine
pub struct AudioStreamingManager {
    /// Native audio engine
    engine: Arc<Mutex<Option<AudioEngine>>>,
    /// Current audio file information
    current_audio_info: Arc<Mutex<Option<AudioFileInfo>>>,
    /// Current streaming state
    state: Arc<Mutex<StreamingState>>,
    /// Enable signal watching (Ctrl-C, SIGTERM)
    watch_signals: bool,
    /// Pending request to enable spectrum monitoring (before engine starts)
    pending_spectrum_monitoring: Arc<Mutex<bool>>,
    /// Pending request to enable loudness monitoring (before engine starts)
    pending_loudness_monitoring: Arc<Mutex<bool>>,
}

/// Commands for controlling the streaming (kept for API compatibility)
#[derive(Debug, Clone)]
pub enum StreamingCommand {
    Start,
    Pause,
    Resume,
    Stop,
    SeekSeconds(f64),
}

/// Events emitted by the streaming manager (kept for API compatibility)
#[derive(Debug, Clone)]
pub enum StreamingEvent {
    StateChanged(StreamingState),
    EndOfStream,
    Error(String),
}

/// Current state of the streaming manager
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamingState {
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Seeking,
    Error,
}

/// Information about the currently loaded audio file
#[derive(Debug, Clone)]
pub struct AudioFileInfo {
    pub path: PathBuf,
    pub format: AudioFormat,
    pub spec: AudioSpec,
    pub duration_seconds: Option<f64>,
}

impl AudioStreamingManager {
    /// Create a new streaming manager
    pub fn new() -> Self {
        Self::with_signal_watching(false)
    }

    /// Create a new streaming manager with signal watching enabled
    ///
    /// When signal watching is enabled, the engine will handle Ctrl-C, SIGTERM, and SIGINT
    /// to cleanly shut down. This is useful for CLI applications but should be disabled
    /// for GUI/Tauri applications that manage their own lifecycle.
    pub fn with_signal_watching(watch_signals: bool) -> Self {
        Self {
            engine: Arc::new(Mutex::new(None)),
            current_audio_info: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(StreamingState::Idle)),
            watch_signals,
            pending_spectrum_monitoring: Arc::new(Mutex::new(false)),
            pending_loudness_monitoring: Arc::new(Mutex::new(false)),
        }
    }

    /// Load an audio file and prepare for streaming
    pub fn load_file<P: AsRef<Path>>(&mut self, file_path: P) -> AudioDecoderResult<AudioFileInfo> {
        let path = file_path.as_ref().to_path_buf();

        self.set_state(StreamingState::Loading);

        // Stop any current playback
        self.stop()?;

        log::debug!("[AudioStreamingManager] Loading file: {:?}", path);

        // Probe the file to get format and spec information
        let (format, spec) = probe_file(&path)?;

        let duration_seconds = spec.duration().map(|d| d.as_secs_f64());

        let audio_info = AudioFileInfo {
            path: path.clone(),
            format,
            spec,
            duration_seconds,
        };

        log::info!(
            "[AudioStreamingManager] Loaded {} file: {}Hz, {}ch, {:?}s duration",
            audio_info.format,
            audio_info.spec.sample_rate,
            audio_info.spec.channels,
            audio_info.duration_seconds
        );

        *self.current_audio_info.lock() = Some(audio_info.clone());
        self.set_state(StreamingState::Ready);

        Ok(audio_info)
    }

    /// Start streaming playback with the given plugin chain
    ///
    /// # Arguments
    /// * `_output_device` - Output device
    /// * `plugins` - Plugin chain to apply (upmixer, EQ, effects, etc.)
    /// * `output_channels` - Expected output channel count after all plugins
    pub fn start_playback(
        &mut self,
        output_device: Option<String>,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
    ) -> AudioDecoderResult<()> {
        let audio_info = self
            .current_audio_info
            .lock()
            .clone()
            .ok_or_else(|| AudioDecoderError::ConfigError("No file loaded".to_string()))?;

        log::debug!("[AudioStreamingManager] Starting playback");

        // Create engine config
        let config = EngineConfig {
            frame_size: 1024,
            buffer_ms: 200, // 200ms latency
            output_sample_rate: audio_info.spec.sample_rate,
            input_channels: audio_info.spec.channels as usize, // Input from audio file
            output_channels,                                   // Output after plugins
            output_device, // User-specified device or None for default
            plugins,
            volume: 1.0,
            muted: false,
            config_path: None,
            watch_config: self.watch_signals, // Enable signal watching if requested
        };

        log::info!(
            "[AudioStreamingManager] Creating engine: {}Hz, {}ch",
            config.output_sample_rate,
            config.output_channels
        );

        // Create and start engine
        let mut engine = AudioEngine::new(config).map_err(|e| {
            AudioDecoderError::ConfigError(format!("Failed to create engine: {}", e))
        })?;

        engine
            .play(&audio_info.path)
            .map_err(AudioDecoderError::IoError)?;

        // Store engine
        *self.engine.lock() = Some(engine);
        self.set_state(StreamingState::Playing);

        log::debug!("[AudioStreamingManager] Playback started");

        // Enable any pending analyzers that were requested before engine was running
        if *self.pending_spectrum_monitoring.lock() {
            log::debug!("[AudioStreamingManager] Enabling pending spectrum monitoring");
            if let Err(e) = self.enable_spectrum_monitoring() {
                log::error!(
                    "[AudioStreamingManager] Failed to enable spectrum monitoring: {}",
                    e
                );
            }
        }
        if *self.pending_loudness_monitoring.lock() {
            log::debug!("[AudioStreamingManager] Enabling pending loudness monitoring");
            if let Err(e) = self.enable_loudness_monitoring() {
                log::error!(
                    "[AudioStreamingManager] Failed to enable loudness monitoring: {}",
                    e
                );
            }
        }

        Ok(())
    }

    /// Start HAL playback without a file source
    ///
    /// This method is specifically for HAL input plugins that act as audio sources.
    /// Unlike `start_playback()`, this doesn't require a file to be loaded first.
    ///
    /// # Arguments
    /// * `output_device` - Output device (None for default)
    /// * `plugins` - Plugin chain (must include hal_input as first plugin)
    /// * `output_channels` - Expected output channel count after all plugins
    ///
    /// # Notes
    /// - Uses HAL default sample rate: 48000 Hz
    /// - Input channels set to 0 (HAL input plugin is the source)
    /// - No decoder thread is started since HAL input generates audio
    pub fn start_hal_playback(
        &mut self,
        output_device: Option<String>,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
    ) -> AudioDecoderResult<()> {
        log::debug!("[AudioStreamingManager] Starting HAL playback");

        // Validate that we have a HAL input plugin
        if !plugins.iter().any(|p| p.plugin_type == "hal_input") {
            return Err(AudioDecoderError::ConfigError(
                "HAL playback requires hal_input plugin in chain".to_string(),
            ));
        }

        // Create engine config for HAL (no file source)
        let config = EngineConfig {
            frame_size: 1024,
            buffer_ms: 200, // 200ms latency
            output_sample_rate: 48000, // HAL default sample rate
            input_channels: 0, // No file input - HAL input plugin is the source
            output_channels,
            output_device,
            plugins,
            volume: 1.0,
            muted: false,
            config_path: None,
            watch_config: self.watch_signals,
        };

        log::info!(
            "[AudioStreamingManager] Creating HAL engine: {}Hz, {}ch output",
            config.output_sample_rate,
            config.output_channels
        );

        // Create engine (but don't call play() since there's no file)
        let engine = AudioEngine::new(config).map_err(|e| {
            AudioDecoderError::ConfigError(format!("Failed to create HAL engine: {}", e))
        })?;

        // Store engine
        *self.engine.lock() = Some(engine);
        self.set_state(StreamingState::Playing);

        log::debug!("[AudioStreamingManager] HAL playback started");

        // Enable any pending analyzers
        if *self.pending_spectrum_monitoring.lock() {
            log::debug!("[AudioStreamingManager] Enabling pending spectrum monitoring");
            if let Err(e) = self.enable_spectrum_monitoring() {
                log::error!(
                    "[AudioStreamingManager] Failed to enable spectrum monitoring: {}",
                    e
                );
            }
        }
        if *self.pending_loudness_monitoring.lock() {
            log::debug!("[AudioStreamingManager] Enabling pending loudness monitoring");
            if let Err(e) = self.enable_loudness_monitoring() {
                log::error!(
                    "[AudioStreamingManager] Failed to enable loudness monitoring: {}",
                    e
                );
            }
        }

        Ok(())
    }

    /// Pause streaming
    pub fn pause(&self) -> AudioDecoderResult<()> {
        log::debug!("[AudioStreamingManager] Pausing");

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.pause().map_err(AudioDecoderError::IoError)?;
            self.set_state(StreamingState::Paused);
        }

        Ok(())
    }

    /// Resume streaming
    pub fn resume(&self) -> AudioDecoderResult<()> {
        log::debug!("[AudioStreamingManager] Resuming");

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.resume().map_err(AudioDecoderError::IoError)?;
            self.set_state(StreamingState::Playing);
        }

        Ok(())
    }

    /// Stop streaming and cleanup
    pub fn stop(&mut self) -> AudioDecoderResult<()> {
        log::debug!("[AudioStreamingManager] Stopping");

        if let Some(mut engine) = self.engine.lock().take() {
            engine.stop().map_err(AudioDecoderError::IoError)?;
            engine.shutdown().map_err(AudioDecoderError::IoError)?;
        }

        // Don't clear pending analyzer flags - they should persist until the analyzer is added
        // The flags are cleared when enable_loudness_monitoring/enable_spectrum_monitoring
        // successfully adds the analyzer to the engine

        self.set_state(StreamingState::Idle);

        Ok(())
    }

    /// Seek to position in seconds
    pub fn seek(&self, seconds: f64) -> AudioDecoderResult<()> {
        log::debug!("[AudioStreamingManager] Seeking to {:.2}s", seconds);

        self.set_state(StreamingState::Seeking);

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.seek(seconds).map_err(AudioDecoderError::IoError)?;
        }

        // Restore previous state (playing or paused)
        let engine_state = self.get_engine_state();
        let new_state = match engine_state.playback_state {
            PlaybackState::Playing => StreamingState::Playing,
            PlaybackState::Paused => StreamingState::Paused,
            _ => StreamingState::Idle,
        };
        self.set_state(new_state);

        Ok(())
    }

    /// Get current state
    pub fn get_state(&self) -> StreamingState {
        *self.state.lock()
    }

    /// Get current audio file info
    pub fn get_audio_info(&self) -> Option<AudioFileInfo> {
        self.current_audio_info.lock().clone()
    }

    /// Get current position in seconds
    pub fn get_position(&self) -> f64 {
        self.get_engine_state().position
    }

    /// Get current volume (0.0 - 1.0)
    pub fn get_volume(&self) -> f32 {
        self.get_engine_state().volume
    }

    /// Set volume (0.0 = silence, 1.0 = unity gain)
    pub fn set_volume(&self, volume: f32) -> AudioDecoderResult<()> {
        if let Some(ref mut engine) = *self.engine.lock() {
            engine
                .set_volume(volume)
                .map_err(AudioDecoderError::IoError)?;
        }
        Ok(())
    }

    /// Get mute state
    pub fn is_muted(&self) -> bool {
        self.get_engine_state().muted
    }

    /// Set mute state
    pub fn set_mute(&self, muted: bool) -> AudioDecoderResult<()> {
        if let Some(ref mut engine) = *self.engine.lock() {
            engine.set_mute(muted).map_err(AudioDecoderError::IoError)?;
        }
        Ok(())
    }

    /// Get underrun count
    pub fn get_underruns(&self) -> u64 {
        self.get_engine_state().underruns
    }

    // ========================================================================
    // Monitoring Support (Phase 2 - Stubs for now)
    // ========================================================================

    /// Enable loudness monitoring
    pub fn enable_loudness_monitoring(&mut self) -> Result<(), String> {
        log::debug!("[AudioStreamingManager] Enabling loudness monitoring");

        if let Some(ref mut engine) = *self.engine.lock() {
            // Use the engine's output channel count (after processing/plugins)
            let channels = engine.get_state().num_channels;
            log::info!(
                "[AudioStreamingManager] Adding loudness analyzer for {} channels",
                channels
            );
            engine.add_loudness_analyzer("loudness".to_string(), channels)?;
            log::debug!("[AudioStreamingManager] Loudness monitoring enabled");
            *self.pending_loudness_monitoring.lock() = false;
            Ok(())
        } else {
            // Engine not running yet - mark as pending and will be enabled when playback starts
            log::info!(
                "[AudioStreamingManager] Engine not running yet - loudness monitoring will be enabled when playback starts"
            );
            *self.pending_loudness_monitoring.lock() = true;
            Ok(())
        }
    }

    /// Disable loudness monitoring
    pub fn disable_loudness_monitoring(&mut self) {
        log::debug!("[AudioStreamingManager] Disabling loudness monitoring");
        *self.pending_loudness_monitoring.lock() = false;
        if let Some(ref mut engine) = *self.engine.lock() {
            engine.remove_analyzer("loudness".to_string()).ok();
        }
    }

    /// Get current loudness info
    pub fn get_loudness(&self) -> Option<LoudnessInfo> {
        use crate::plugins::LoudnessData;

        if let Some(ref mut engine) = *self.engine.lock() {
            match engine.get_analyzer_data("loudness".to_string()) {
                Ok(data) => {
                    // Downcast Arc<dyn Any + Send + Sync> to LoudnessData
                    // Convert LoudnessData to LoudnessInfo
                    data.downcast_ref::<LoudnessData>()
                        .map(|loudness_data| LoudnessInfo {
                            momentary_lufs: loudness_data.momentary_lufs,
                            shortterm_lufs: loudness_data.shortterm_lufs,
                            peak: loudness_data.peak,
                            channel_peaks: loudness_data.channel_peaks.clone(),
                        })
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// Check if loudness monitoring is enabled
    pub fn is_loudness_monitoring_enabled(&self) -> bool {
        if let Some(ref mut engine) = *self.engine.lock() {
            // Try to get data - if it succeeds, analyzer is enabled
            engine.get_analyzer_data("loudness".to_string()).is_ok()
        } else {
            false
        }
    }

    /// Enable spectrum monitoring
    pub fn enable_spectrum_monitoring(&mut self) -> Result<(), String> {
        log::debug!("[AudioStreamingManager] Enabling spectrum monitoring");

        if let Some(ref mut engine) = *self.engine.lock() {
            // Use the engine's output channel count (after processing/plugins)
            let channels = engine.get_state().num_channels;
            log::info!(
                "[AudioStreamingManager] Adding spectrum analyzer for {} channels",
                channels
            );
            engine.add_spectrum_analyzer("spectrum".to_string(), channels)?;
            log::debug!("[AudioStreamingManager] Spectrum monitoring enabled");
            *self.pending_spectrum_monitoring.lock() = false;
            Ok(())
        } else {
            // Engine not running yet - mark as pending and will be enabled when playback starts
            log::info!(
                "[AudioStreamingManager] Engine not running yet - spectrum monitoring will be enabled when playback starts"
            );
            *self.pending_spectrum_monitoring.lock() = true;
            Ok(())
        }
    }

    /// Disable spectrum monitoring
    pub fn disable_spectrum_monitoring(&mut self) {
        log::debug!("[AudioStreamingManager] Disabling spectrum monitoring");
        *self.pending_spectrum_monitoring.lock() = false;
        if let Some(ref mut engine) = *self.engine.lock() {
            engine.remove_analyzer("spectrum".to_string()).ok();
        }
    }

    /// Get current spectrum info
    pub fn get_spectrum(&self) -> Option<SpectrumInfo> {
        use crate::plugins::SpectrumData;

        if let Some(ref mut engine) = *self.engine.lock() {
            match engine.get_analyzer_data("spectrum".to_string()) {
                Ok(data) => {
                    // Downcast Arc<dyn Any + Send + Sync> to SpectrumData
                    // Convert SpectrumData to SpectrumInfo
                    data.downcast_ref::<SpectrumData>()
                        .map(|spectrum_data| SpectrumInfo {
                            frequencies: spectrum_data.frequencies.clone(),
                            magnitudes: spectrum_data.magnitudes.clone(),
                            peak_magnitude: spectrum_data.peak_magnitude,
                        })
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// Check if spectrum monitoring is enabled
    pub fn is_spectrum_monitoring_enabled(&self) -> bool {
        if let Some(ref mut engine) = *self.engine.lock() {
            // Try to get data - if it succeeds, analyzer is enabled
            engine.get_analyzer_data("spectrum".to_string()).is_ok()
        } else {
            false
        }
    }

    /// Enable plugin host
    pub fn enable_plugin_host(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Disable plugin host
    pub fn disable_plugin_host(&mut self) {
        // No-op - always enabled
    }

    /// Check if plugin host is enabled
    pub fn is_plugin_host_enabled(&self) -> bool {
        true // Always enabled in native engine
    }

    /// Update plugin chain
    /// TODO: Phase 3 - Implement plugin hot-reload
    pub fn update_plugin_chain(&self, plugins: Vec<PluginConfig>) -> Result<(), String> {
        log::info!(
            "[AudioStreamingManager] Updating plugin chain with {} plugins",
            plugins.len()
        );

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.update_plugin_chain(plugins)?;
            log::debug!("[AudioStreamingManager] Plugin chain updated successfully");
            Ok(())
        } else {
            Err("No engine running".to_string())
        }
    }

    // ========================================================================
    // Event Support
    // ========================================================================

    /// Try to receive an event (non-blocking)
    pub fn try_recv_event(&self) -> Option<StreamingEvent> {
        // Check engine state for end-of-stream
        let engine_state = self.get_engine_state();

        if engine_state.playback_state == PlaybackState::Stopped {
            let current_state = self.get_state();
            if current_state == StreamingState::Playing {
                // Was playing, now stopped = end of stream
                self.set_state(StreamingState::Idle);
                return Some(StreamingEvent::EndOfStream);
            }
        }

        None
    }

    /// Drain all pending events
    pub fn drain_events(&self) -> Vec<StreamingEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.try_recv_event() {
            events.push(event);
        }
        events
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn set_state(&self, state: StreamingState) {
        *self.state.lock() = state;
    }

    fn get_engine_state(&self) -> AudioEngineState {
        if let Some(ref engine) = *self.engine.lock() {
            engine.get_state()
        } else {
            AudioEngineState::default()
        }
    }
}

impl Default for AudioStreamingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioStreamingManager {
    fn drop(&mut self) {
        // Synchronous stop for drop
        if let Some(mut engine) = self.engine.lock().take() {
            engine.shutdown().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = AudioStreamingManager::new();
        assert_eq!(manager.get_state(), StreamingState::Idle);
        assert!(manager.get_audio_info().is_none());
    }

    #[test]
    fn test_state_transitions() {
        let manager = AudioStreamingManager::new();

        assert_eq!(manager.get_state(), StreamingState::Idle);

        // Loading state would be set by load_file
        manager.set_state(StreamingState::Loading);
        assert_eq!(manager.get_state(), StreamingState::Loading);

        manager.set_state(StreamingState::Ready);
        assert_eq!(manager.get_state(), StreamingState::Ready);
    }
}
