// ============================================================================
// Audio Streaming Manager
// ============================================================================

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::devices::get_device_supported_sample_rates;
use crate::engine::{AudioEngine, AudioEngineState, EngineConfig, PlaybackState, PluginConfig};
use crate::{AudioDecoderError, AudioDecoderResult, AudioFormat, AudioSpec, probe_file};

/// Select the optimal output sample rate for playback
///
/// Strategy:
/// 1. If device supports file's native rate -> use it (no resampling)
/// 2. Otherwise, find best supported rate:
///    - Prefer rates in the same "family" (44100-based or 48000-based)
///    - Prefer higher rates (less quality loss during resampling)
///    - Fallback to device default
fn select_output_sample_rate(file_sample_rate: u32, output_device: Option<&str>) -> u32 {
    // Get device supported rates
    let supported_rates = match get_device_supported_sample_rates(output_device) {
        Some(rates) if !rates.is_empty() => rates,
        _ => {
            // No device info available, assume file rate is supported
            log::debug!(
                "[AudioEngineManager] Could not query device rates, using file rate: {}Hz",
                file_sample_rate
            );
            return file_sample_rate;
        }
    };

    log::debug!(
        "[AudioEngineManager] File: {}Hz, Device supports: {:?}",
        file_sample_rate,
        supported_rates
    );

    // If device supports file's native rate, use it (no resampling needed)
    if supported_rates.contains(&file_sample_rate) {
        log::info!(
            "[AudioEngineManager] Using native rate: {}Hz",
            file_sample_rate
        );
        return file_sample_rate;
    }

    // Device doesn't support file rate - find best alternative
    // Determine the sample rate "family" (44100-based vs 48000-based)
    let is_44100_family = file_sample_rate % 44100 == 0 || file_sample_rate == 22050;
    let is_48000_family = file_sample_rate % 48000 == 0 || file_sample_rate == 24000;

    // Preferred rates in order of preference for each family
    let preferred_rates: &[u32] = if is_44100_family {
        // 44100 family: prefer multiples/divisors within family, then cross-family
        &[88200, 176400, 44100, 96000, 192000, 48000]
    } else if is_48000_family {
        // 48000 family: prefer multiples/divisors within family, then cross-family
        &[96000, 192000, 48000, 88200, 176400, 44100]
    } else {
        // Unknown family (e.g., 32000Hz): prefer highest quality rates
        &[192000, 176400, 96000, 88200, 48000, 44100]
    };

    // Find the best available rate
    for &preferred in preferred_rates {
        if supported_rates.contains(&preferred) {
            log::info!(
                "[AudioEngineManager] Resampling {}Hz -> {}Hz (native not supported)",
                file_sample_rate,
                preferred
            );
            return preferred;
        }
    }

    // Fallback: use highest supported rate
    let highest = *supported_rates.last().unwrap_or(&48000);
    log::info!(
        "[AudioEngineManager] Resampling {}Hz -> {}Hz (fallback to highest)",
        file_sample_rate,
        highest
    );
    highest
}

/// High-level audio streaming manager using native AudioEngine
pub struct AudioEngineManager {
    /// Native audio engine
    engine: Arc<Mutex<Option<AudioEngine>>>,
    /// Current audio file information
    current_audio_info: Arc<Mutex<Option<AudioFileInfo>>>,
    /// Current streaming state
    state: Arc<Mutex<StreamingState>>,
    /// Enable signal watching (Ctrl-C, SIGTERM)
    watch_signals: bool,
    /// Index of loudness analyzer plugin (if enabled)
    loudness_plugin_index: Arc<Mutex<Option<usize>>>,
    /// Index of spectrum analyzer plugin (if enabled)
    spectrum_plugin_index: Arc<Mutex<Option<usize>>>,
    /// Current volume level (preserved across song changes)
    current_volume: Arc<Mutex<f32>>,
    /// Current mute state (preserved across song changes)
    current_muted: Arc<Mutex<bool>>,
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

impl AudioEngineManager {
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
            loudness_plugin_index: Arc::new(Mutex::new(None)),
            spectrum_plugin_index: Arc::new(Mutex::new(None)),
            current_volume: Arc::new(Mutex::new(1.0)),
            current_muted: Arc::new(Mutex::new(false)),
        }
    }

    /// Load an audio file and prepare for streaming
    pub fn load_file<P: AsRef<Path>>(&mut self, file_path: P) -> AudioDecoderResult<AudioFileInfo> {
        let path = file_path.as_ref().to_path_buf();

        self.set_state(StreamingState::Loading);

        // Stop any current playback
        self.stop()?;

        log::debug!("[AudioEngineManager] Loading file: {:?}", path);

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
            "[AudioEngineManager] Loaded {} file: {}Hz, {}ch, {:?}s duration",
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

        log::debug!(
            "[AudioEngineManager] start_playback called: {} plugins, requested output_channels={}",
            plugins.len(),
            output_channels
        );

        // Select optimal output sample rate based on device capabilities
        let file_sample_rate = audio_info.spec.sample_rate;
        let output_sample_rate =
            select_output_sample_rate(file_sample_rate, output_device.as_deref());

        // Create engine config with preserved volume
        let volume = *self.current_volume.lock();
        let muted = *self.current_muted.lock();
        let config = EngineConfig {
            version: 1,
            frame_size: 1024,
            buffer_ms: 200, // 200ms latency
            output_sample_rate,
            input_channels: audio_info.spec.channels as usize, // Input from audio file
            output_channels,                                   // Output after plugins
            output_device, // User-specified device or None for default
            plugins,
            volume,
            muted,
            config_path: None,
            watch_config: self.watch_signals, // Enable signal watching if requested
        };

        log::info!(
            "[AudioEngineManager] Creating engine: source={}Hz, output={}Hz, {}ch{}",
            file_sample_rate,
            config.output_sample_rate,
            config.output_channels,
            if file_sample_rate != config.output_sample_rate {
                " (resampling)"
            } else {
                ""
            }
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

        log::debug!("[AudioEngineManager] Playback started");

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
        // Use default sample rate
        self.start_hal_playback_with_config(output_device, plugins, output_channels, 48000)
    }

    /// Start HAL playback with custom sample rate
    ///
    /// This method is specifically for HAL input plugins that act as audio sources.
    /// Unlike `start_playback()`, this doesn't require a file to be loaded first.
    ///
    /// # Arguments
    /// * `output_device` - Output device (None for default)
    /// * `plugins` - Plugin chain (must include hal_input as first plugin)
    /// * `output_channels` - Expected output channel count after all plugins
    /// * `sample_rate` - Output sample rate in Hz (e.g., 44100, 48000, 96000)
    ///
    /// # Notes
    /// - Input channels set to 0 (HAL input plugin is the source)
    /// - No decoder thread is started since HAL input generates audio
    pub fn start_hal_playback_with_config(
        &mut self,
        output_device: Option<String>,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        sample_rate: u32,
    ) -> AudioDecoderResult<()> {
        log::debug!(
            "[AudioEngineManager] Starting HAL playback at {}Hz",
            sample_rate
        );

        // No plugin validation required - decoder thread's HalInputReader is the audio source,
        // not the hal_input plugin. Empty plugin chains are valid.

        // Create engine config for HAL (no file source) with preserved volume
        let volume = *self.current_volume.lock();
        let muted = *self.current_muted.lock();
        let config = EngineConfig {
            version: 1,
            frame_size: 1024,
            buffer_ms: 200,               // 200ms latency
            output_sample_rate: sample_rate,
            input_channels: 0,            // No file input - HAL input plugin is the source
            output_channels,
            output_device,
            plugins,
            volume,
            muted,
            config_path: None,
            watch_config: self.watch_signals,
        };

        log::info!(
            "[AudioEngineManager] Creating HAL engine: {}Hz, {}ch output",
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

        log::debug!("[AudioEngineManager] HAL playback started");

        Ok(())
    }

    /// Pause streaming
    pub fn pause(&self) -> AudioDecoderResult<()> {
        log::debug!("[AudioEngineManager] Pausing");

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.pause().map_err(AudioDecoderError::IoError)?;
            self.set_state(StreamingState::Paused);
        }

        Ok(())
    }

    /// Resume streaming
    pub fn resume(&self) -> AudioDecoderResult<()> {
        log::debug!("[AudioEngineManager] Resuming");

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.resume().map_err(AudioDecoderError::IoError)?;
            self.set_state(StreamingState::Playing);
        }

        Ok(())
    }

    /// Stop streaming and cleanup
    pub fn stop(&mut self) -> AudioDecoderResult<()> {
        log::debug!("[AudioEngineManager] Stopping");

        if let Some(mut engine) = self.engine.lock().take() {
            engine.stop().map_err(AudioDecoderError::IoError)?;
            engine.shutdown().map_err(AudioDecoderError::IoError)?;
        }

        self.set_state(StreamingState::Idle);

        Ok(())
    }

    /// Seek to position in seconds
    pub fn seek(&self, seconds: f64) -> AudioDecoderResult<()> {
        log::debug!("[AudioEngineManager] Seeking to {:.2}s", seconds);

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
        if self.engine.lock().is_some() {
            self.get_engine_state().volume
        } else {
            *self.current_volume.lock()
        }
    }

    /// Set volume (0.0 = silence, 1.0 = unity gain)
    pub fn set_volume(&self, volume: f32) -> AudioDecoderResult<()> {
        // Store volume so it's preserved across song changes
        *self.current_volume.lock() = volume;

        if let Some(ref mut engine) = *self.engine.lock() {
            engine
                .set_volume(volume)
                .map_err(AudioDecoderError::IoError)?;
        }
        Ok(())
    }

    /// Get mute state
    pub fn is_muted(&self) -> bool {
        if self.engine.lock().is_some() {
            self.get_engine_state().muted
        } else {
            *self.current_muted.lock()
        }
    }

    /// Set mute state
    pub fn set_mute(&self, muted: bool) -> AudioDecoderResult<()> {
        // Store mute state so it's preserved
        *self.current_muted.lock() = muted;

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
    // Monitoring Support - REMOVED
    // Analyzers are now treated as normal plugins managed via update_plugin_chain
    // ========================================================================

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
    pub fn update_plugin_chain(&self, plugins: Vec<PluginConfig>) -> Result<(), String> {
        log::info!(
            "[AudioEngineManager] Updating plugin chain with {} plugins",
            plugins.len()
        );

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.update_plugin_chain(plugins)?;
            log::debug!("[AudioEngineManager] Plugin chain updated successfully");
            Ok(())
        } else {
            Err("No engine running".to_string())
        }
    }

    /// Set a plugin parameter (zero-dropout update)
    ///
    /// This updates a single parameter without rebuilding the plugin chain.
    /// The value should be a string representation (JSON for complex types).
    pub fn set_plugin_parameter(
        &self,
        plugin_index: usize,
        param_id: String,
        value: String,
    ) -> Result<(), String> {
        log::debug!(
            "[AudioEngineManager] Setting plugin {} parameter {} = {}",
            plugin_index,
            param_id,
            value
        );

        if let Some(ref mut engine) = *self.engine.lock() {
            engine.set_plugin_parameter(plugin_index, param_id, value)?;
            log::debug!("[AudioEngineManager] Parameter set successfully");
            Ok(())
        } else {
            Err("No engine running".to_string())
        }
    }

    /// Get plugin data (e.g. analyzer results)
    pub fn get_plugin_data(
        &self,
        index: usize,
    ) -> AudioDecoderResult<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        if let Some(ref mut engine) = *self.engine.lock() {
            engine
                .get_plugin_data(index)
                .map_err(AudioDecoderError::ConfigError)
        } else {
            Err(AudioDecoderError::ConfigError(
                "No engine running".to_string(),
            ))
        }
    }

    /// Get current loudness measurements
    ///
    /// Returns None if loudness monitoring is not enabled or no data is available yet.
    pub fn get_loudness(&self) -> Option<crate::LoudnessInfo> {
        let plugin_index = (*self.loudness_plugin_index.lock())?;

        match self.get_plugin_data(plugin_index) {
            Ok(data) => {
                // Try to downcast to LoudnessInfo
                data.downcast_ref::<crate::LoudnessInfo>().cloned()
            }
            Err(_) => None,
        }
    }

    /// Set the loudness plugin index (call this after adding loudness_monitor to plugin chain)
    pub fn set_loudness_plugin_index(&mut self, index: usize) {
        *self.loudness_plugin_index.lock() = Some(index);
        log::debug!(
            "[AudioEngineManager] Loudness plugin index set to {}",
            index
        );
    }

    /// Get current spectrum measurements
    ///
    /// Returns None if spectrum monitoring is not enabled or no data is available yet.
    pub fn get_spectrum(&self) -> Option<crate::SpectrumInfo> {
        let plugin_index = (*self.spectrum_plugin_index.lock())?;

        match self.get_plugin_data(plugin_index) {
            Ok(data) => {
                // Try to downcast to SpectrumInfo
                data.downcast_ref::<crate::SpectrumInfo>().cloned()
            }
            Err(_) => None,
        }
    }

    // ========================================================================
    // Event Support
    // ========================================================================

    /// Try to receive an event (non-blocking)
    pub fn try_recv_event(&self) -> Option<StreamingEvent> {
        // Check engine state for end-of-stream or error
        let engine_state = self.get_engine_state();
        let current_state = self.get_state();

        if engine_state.playback_state == PlaybackState::Stopped {
            if let Some(err) = engine_state.last_error.clone()
                && !err.is_empty()
                && current_state != StreamingState::Idle
            {
                self.set_state(StreamingState::Error);
                return Some(StreamingEvent::Error(err));
            }

            if current_state == StreamingState::Playing {
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

impl Default for AudioEngineManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioEngineManager {
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
        let manager = AudioEngineManager::new();
        assert_eq!(manager.get_state(), StreamingState::Idle);
        assert!(manager.get_audio_info().is_none());
    }

    #[test]
    fn test_state_transitions() {
        let manager = AudioEngineManager::new();

        assert_eq!(manager.get_state(), StreamingState::Idle);

        // Loading state would be set by load_file
        manager.set_state(StreamingState::Loading);
        assert_eq!(manager.get_state(), StreamingState::Loading);

        manager.set_state(StreamingState::Ready);
        assert_eq!(manager.get_state(), StreamingState::Ready);
    }

    #[test]
    fn try_recv_event_emits_end_of_stream_when_stopped_from_playing() {
        let manager = AudioEngineManager::new();

        // Simulate that we were previously playing; engine state defaults to Stopped
        manager.set_state(StreamingState::Playing);

        let event = manager.try_recv_event();
        assert!(matches!(event, Some(StreamingEvent::EndOfStream)));
        assert_eq!(manager.get_state(), StreamingState::Idle);
    }
}
