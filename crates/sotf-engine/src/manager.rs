// ============================================================================
// Audio Streaming Manager
// ============================================================================

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

use crate::devices::verify_working_sample_rate;
use crate::engine::{AudioEngine, AudioEngineState, EngineConfig, PlaybackState, PluginConfig};
use crate::{AudioDecoderError, AudioDecoderResult, AudioFormat, AudioSpec, probe_file};

/// Cache for verified working sample rates per device.
/// The verification probe (creating test streams) is expensive (~300ms per rate)
/// and can cause ALSA device locking issues if called repeatedly. Cache the result
/// so subsequent calls for the same device return immediately.
static VERIFIED_RATE_CACHE: Mutex<Option<((Option<String>, usize), u32)>> = Mutex::new(None);

/// Clear the verified rate cache (e.g., when the output device changes)
pub fn clear_verified_rate_cache() {
    if let Ok(mut cache) = VERIFIED_RATE_CACHE.lock() {
        *cache = None;
    }
}

/// Select the output sample rate for playback
///
/// Uses the device's actual current sample rate, verified by creating a brief test stream.
/// On some ALSA systems, the device reports a default rate that doesn't produce working
/// audio callbacks (e.g., dmix configured for 44100Hz but hardware only works at 48000Hz).
/// The decoder will resample when the file rate differs from the device rate.
///
/// Results are cached per device to avoid repeated expensive probes.
pub fn select_output_sample_rate(file_sample_rate: u32, output_device: Option<&str>) -> u32 {
    select_output_sample_rate_for_channels(file_sample_rate, output_device, 2)
}

fn verified_rate_cache_key(
    output_device: Option<&str>,
    output_channels: usize,
) -> (Option<String>, usize) {
    (output_device.map(|s| s.to_string()), output_channels)
}

pub fn select_output_sample_rate_for_channels(
    file_sample_rate: u32,
    output_device: Option<&str>,
    output_channels: usize,
) -> u32 {
    // Check cache first — avoids repeated 300ms+ probes that can block the UI
    // and cause ALSA device locking issues
    let cache_key = verified_rate_cache_key(output_device, output_channels);
    if let Ok(cache) = VERIFIED_RATE_CACHE.lock()
        && let Some((ref cached_key, cached_rate)) = *cache
    {
        if cached_key == &cache_key {
            if cached_rate == file_sample_rate {
                log::debug!(
                    "[AudioEngineManager] Using cached device rate: {}Hz (matches file, no resampling)",
                    cached_rate
                );
            } else {
                log::debug!(
                    "[AudioEngineManager] Using cached device rate: {}Hz, file is {}Hz (will resample)",
                    cached_rate,
                    file_sample_rate
                );
            }
            return cached_rate;
        }
    }

    // Prefer the file's sample rate to avoid resampling when the device supports it.
    // The verification function tries: candidate first, then common rates (48k, 44.1k,
    // 96k, 192k) as fallback, plus the device's own default rate.
    let candidate_rate = file_sample_rate;

    // Verify the candidate rate actually produces working audio callbacks.
    // This catches ALSA systems where the reported default rate doesn't work.
    if let Some(verified_rate) =
        verify_working_sample_rate(output_device, candidate_rate, output_channels)
    {
        if verified_rate == file_sample_rate {
            log::info!(
                "[AudioEngineManager] Verified device rate matches file: {}Hz (no resampling)",
                verified_rate
            );
        } else {
            log::info!(
                "[AudioEngineManager] Verified device rate: {}Hz, file is {}Hz (will resample)",
                verified_rate,
                file_sample_rate
            );
        }

        // Cache the verified rate
        if let Ok(mut cache) = VERIFIED_RATE_CACHE.lock() {
            *cache = Some((cache_key, verified_rate));
        }

        return verified_rate;
    }

    // Verification failed for all rates — fall back to device reported rate or file rate
    log::warn!(
        "[AudioEngineManager] Could not verify any working sample rate, using candidate: {}Hz",
        candidate_rate
    );
    candidate_rate
}

/// High-level audio streaming manager using native AudioEngine
pub struct AudioEngineManager {
    /// Native audio engine (wrapped in ArcSwap for lock-free status/analyzer access)
    engine: Arc<arc_swap::ArcSwapOption<AudioEngine>>,
    /// Mutex for serializing engine control commands (play, stop, update_plugins)
    /// This prevents multiple threads from fighting for the single response channel.
    cmd_mutex: Mutex<()>,
    /// Current audio file information
    current_audio_info: Arc<Mutex<Option<AudioFileInfo>>>,
    /// Current streaming state (StreamingState as u8)
    state: AtomicU8,
    /// Enable signal watching (Ctrl-C, SIGTERM)
    watch_signals: bool,
    /// Index of loudness analyzer plugin (ATOMIC_NONE = None)
    loudness_plugin_index: AtomicU64,
    /// Index of spectrum analyzer plugin (ATOMIC_NONE = None)
    spectrum_plugin_index: AtomicU64,
    /// Current volume level (preserved across song changes), stored as f32 bits
    current_volume: AtomicU32,
    /// Current mute state (preserved across song changes)
    current_muted: AtomicBool,
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
#[repr(u8)]
pub enum StreamingState {
    Idle = 0,
    Loading = 1,
    Ready = 2,
    Playing = 3,
    Paused = 4,
    Seeking = 5,
    Error = 6,
}

impl StreamingState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Loading,
            2 => Self::Ready,
            3 => Self::Playing,
            4 => Self::Paused,
            5 => Self::Seeking,
            6 => Self::Error,
            other => panic!("invalid StreamingState discriminant: {}", other),
        }
    }
}

/// Sentinel value representing `None` for atomic Option<usize> fields
const ATOMIC_NONE: u64 = u64::MAX;

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
    #[allow(clippy::arc_with_non_send_sync)] // ArcSwapOption requires Arc; single-thread access is enforced by cmd_mutex
    pub fn with_signal_watching(watch_signals: bool) -> Self {
        Self {
            engine: Arc::new(arc_swap::ArcSwapOption::new(None)),
            cmd_mutex: Mutex::new(()),
            current_audio_info: Arc::new(Mutex::new(None)),
            state: AtomicU8::new(StreamingState::Idle as u8),
            watch_signals,
            loudness_plugin_index: AtomicU64::new(ATOMIC_NONE),
            spectrum_plugin_index: AtomicU64::new(ATOMIC_NONE),
            current_volume: AtomicU32::new(1.0f32.to_bits()),
            current_muted: AtomicBool::new(false),
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

        *self.current_audio_info.lock().unwrap() = Some(audio_info.clone());
        self.set_state(StreamingState::Ready);

        Ok(audio_info)
    }

    /// Start streaming playback with the given plugin chain
    pub fn start_playback(
        &mut self,
        output_device: Option<String>,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
    ) -> AudioDecoderResult<()> {
        self.start_playback_at(output_device, plugins, output_channels, None)
    }

    /// Start streaming playback at a specific position
    pub fn start_playback_at(
        &mut self,
        output_device: Option<String>,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        position: Option<f64>,
    ) -> AudioDecoderResult<()> {
        let _guard = self.cmd_mutex.lock().unwrap();

        let audio_info = self
            .current_audio_info
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AudioDecoderError::ConfigError("No file loaded".to_string()))?;

        log::debug!(
            "[AudioEngineManager] start_playback_at called: {} plugins, requested output_channels={}, position={:?}",
            plugins.len(),
            output_channels,
            position
        );

        // Select optimal output sample rate based on device capabilities
        let file_sample_rate = audio_info.spec.sample_rate;
        let output_sample_rate = select_output_sample_rate_for_channels(
            file_sample_rate,
            output_device.as_deref(),
            output_channels,
        );

        // Create engine config with preserved volume
        let volume = f32::from_bits(self.current_volume.load(Ordering::Relaxed));
        let muted = self.current_muted.load(Ordering::Relaxed);
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
            hal_mode: false,
        };

        log::warn!(
            "[AudioEngineManager] Creating engine: file_sr={}Hz, device_sr={}Hz, output_sr={}Hz, input_ch={}, output_ch={}, plugins={}{}",
            file_sample_rate,
            output_sample_rate,
            config.output_sample_rate,
            config.input_channels,
            config.output_channels,
            config.plugins.len(),
            if file_sample_rate != config.output_sample_rate {
                " (RESAMPLING)"
            } else {
                ""
            }
        );

        // Create and start engine
        let engine = AudioEngine::new(config).map_err(|e| {
            AudioDecoderError::ConfigError(format!("Failed to create engine: {}", e))
        })?;

        if let Some(pos) = position {
            engine
                .play_at(&audio_info.path, pos)
                .map_err(AudioDecoderError::IoError)?;
        } else {
            engine
                .play(&audio_info.path)
                .map_err(AudioDecoderError::IoError)?;
        }

        // Store engine handle lock-free
        #[allow(clippy::arc_with_non_send_sync)]
        // AudioEngine is !Sync but access is serialized by cmd_mutex
        self.engine.store(Some(Arc::new(engine)));
        self.set_state(StreamingState::Playing);

        log::debug!("[AudioEngineManager] Playback started");

        Ok(())
    }

    /// Start HAL playback without a file source
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
    pub fn start_hal_playback_with_config(
        &mut self,
        output_device: Option<String>,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        sample_rate: u32,
    ) -> AudioDecoderResult<()> {
        let _guard = self.cmd_mutex.lock().unwrap();

        log::debug!(
            "[AudioEngineManager] Starting HAL playback at {}Hz",
            sample_rate
        );

        // Create engine config for HAL (no file source) with preserved volume
        let volume = f32::from_bits(self.current_volume.load(Ordering::Relaxed));
        let muted = self.current_muted.load(Ordering::Relaxed);
        let config = EngineConfig {
            version: 1,
            frame_size: 1024,
            buffer_ms: 200, // 200ms latency
            output_sample_rate: sample_rate,
            input_channels: 2, // HAL always provides stereo
            output_channels,
            output_device,
            plugins,
            volume,
            muted,
            config_path: None,
            watch_config: self.watch_signals,
            hal_mode: true,
        };

        log::info!(
            "[AudioEngineManager] Creating HAL engine: {}Hz, {}ch output",
            config.output_sample_rate,
            config.output_channels
        );

        // Create engine
        let engine = AudioEngine::new(config).map_err(|e| {
            AudioDecoderError::ConfigError(format!("Failed to create HAL engine: {}", e))
        })?;

        // Store engine handle lock-free
        #[allow(clippy::arc_with_non_send_sync)]
        // AudioEngine is !Sync but access is serialized by cmd_mutex
        self.engine.store(Some(Arc::new(engine)));
        self.set_state(StreamingState::Playing);

        log::debug!("[AudioEngineManager] HAL playback started");

        Ok(())
    }

    /// Pause streaming
    pub fn pause(&self) -> AudioDecoderResult<()> {
        let _guard = self.cmd_mutex.lock().unwrap();
        log::debug!("[AudioEngineManager] Pausing");

        if let Some(engine) = &*self.engine.load() {
            engine.pause().map_err(AudioDecoderError::IoError)?;
            self.set_state(StreamingState::Paused);
        }

        Ok(())
    }

    /// Resume streaming
    pub fn resume(&self) -> AudioDecoderResult<()> {
        let _guard = self.cmd_mutex.lock().unwrap();
        log::debug!("[AudioEngineManager] Resuming");

        if let Some(engine) = &*self.engine.load() {
            engine.resume().map_err(AudioDecoderError::IoError)?;
            self.set_state(StreamingState::Playing);
        }

        Ok(())
    }

    /// Stop streaming and cleanup
    pub fn stop(&mut self) -> AudioDecoderResult<()> {
        let _guard = self.cmd_mutex.lock().unwrap();
        log::debug!("[AudioEngineManager] Stopping");

        if let Some(mut engine_arc) = self.engine.swap(None) {
            // shutdown() is internally thread-safe now as it just sends a command.
            // stop() is best-effort: if it fails (e.g., decoder ACK timeout after
            // end-of-stream), we still proceed to shutdown which joins all threads.
            if let Some(engine) = Arc::get_mut(&mut engine_arc) {
                if let Err(e) = engine.stop() {
                    log::warn!("[AudioEngineManager] stop() failed (proceeding to shutdown): {}", e);
                }
                engine.shutdown().map_err(AudioDecoderError::IoError)?;
            } else {
                // Another thread is holding a reference (e.g. status polling).
                // Send commands via the shared Arc.
                if let Err(e) = engine_arc.stop() {
                    log::warn!("[AudioEngineManager] stop() failed (proceeding to shutdown): {}", e);
                }
                engine_arc.shutdown().map_err(AudioDecoderError::IoError)?;
            }
        }

        self.set_state(StreamingState::Idle);

        Ok(())
    }

    /// Seek to position in seconds
    pub fn seek(&self, seconds: f64) -> AudioDecoderResult<()> {
        let _guard = self.cmd_mutex.lock().unwrap();
        log::debug!("[AudioEngineManager] Seeking to {:.2}s", seconds);

        self.set_state(StreamingState::Seeking);

        if let Some(engine) = &*self.engine.load() {
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

    /// Get current state (lock-free)
    pub fn get_state(&self) -> StreamingState {
        StreamingState::from_u8(self.state.load(Ordering::Relaxed))
    }

    /// Get current audio file info
    pub fn get_audio_info(&self) -> Option<AudioFileInfo> {
        self.current_audio_info.lock().unwrap().clone()
    }

    /// Get current position in seconds (lock-free)
    pub fn get_position(&self) -> f64 {
        self.get_engine_state().position
    }

    /// Get current volume (0.0 - 1.0) (lock-free)
    pub fn get_volume(&self) -> f32 {
        if self.engine.load().is_some() {
            self.get_engine_state().volume
        } else {
            f32::from_bits(self.current_volume.load(Ordering::Relaxed))
        }
    }

    /// Set volume (0.0 = silence, 1.0 = unity gain)
    pub fn set_volume(&self, volume: f32) -> AudioDecoderResult<()> {
        // Store volume so it's preserved across song changes
        self.current_volume
            .store(volume.to_bits(), Ordering::Relaxed);

        if let Some(engine) = &*self.engine.load() {
            engine
                .set_volume(volume)
                .map_err(AudioDecoderError::IoError)?;
        }
        Ok(())
    }

    /// Get mute state (lock-free)
    pub fn is_muted(&self) -> bool {
        if self.engine.load().is_some() {
            self.get_engine_state().muted
        } else {
            self.current_muted.load(Ordering::Relaxed)
        }
    }

    /// Set mute state
    pub fn set_mute(&self, muted: bool) -> AudioDecoderResult<()> {
        // Store mute state so it's preserved
        self.current_muted.store(muted, Ordering::Relaxed);

        if let Some(engine) = &*self.engine.load() {
            engine.set_mute(muted).map_err(AudioDecoderError::IoError)?;
        }
        Ok(())
    }

    /// Get underrun count (lock-free)
    pub fn get_underruns(&self) -> u64 {
        self.get_engine_state().underruns
    }

    /// Update plugin chain
    pub fn update_plugin_chain(&self, plugins: Vec<PluginConfig>) -> Result<(), String> {
        let _guard = self.cmd_mutex.lock().unwrap();
        log::info!(
            "[AudioEngineManager] Updating plugin chain with {} plugins",
            plugins.len()
        );

        if let Some(engine) = &*self.engine.load() {
            engine.update_plugin_chain(plugins)?;
            log::debug!("[AudioEngineManager] Plugin chain updated successfully");
            Ok(())
        } else {
            Err("No engine running".to_string())
        }
    }

    /// Set a plugin parameter (zero-dropout update)
    pub fn set_plugin_parameter(
        &self,
        plugin_index: usize,
        param_id: String,
        value: String,
    ) -> Result<(), String> {
        let _guard = self.cmd_mutex.lock().unwrap();
        log::debug!(
            "[AudioEngineManager] Setting plugin {} parameter {} = {}",
            plugin_index,
            param_id,
            value
        );

        if let Some(engine) = &*self.engine.load() {
            engine.set_plugin_parameter(plugin_index, param_id, value)?;
            log::debug!("[AudioEngineManager] Parameter set successfully");
            Ok(())
        } else {
            Err("No engine running".to_string())
        }
    }

    /// Get plugin data via synchronous command round-trip (serialized via cmd_mutex)
    pub fn get_plugin_data(
        &self,
        index: usize,
    ) -> AudioDecoderResult<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        let _guard = self.cmd_mutex.lock().unwrap();
        if let Some(engine) = &*self.engine.load() {
            engine
                .get_plugin_data(index)
                .map_err(AudioDecoderError::ConfigError)
        } else {
            Err(AudioDecoderError::ConfigError(
                "No engine running".to_string(),
            ))
        }
    }

    /// Get cached plugin data directly without blocking (lock-free)
    pub fn get_cached_plugin_data(
        &self,
        index: usize,
    ) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        if let Some(engine) = &*self.engine.load() {
            engine.get_cached_plugin_data(index)
        } else {
            None
        }
    }

    /// Get current loudness measurements (lock-free)
    pub fn get_loudness(&self) -> Option<crate::LoudnessInfo> {
        let raw = self.loudness_plugin_index.load(Ordering::Relaxed);
        if raw == ATOMIC_NONE {
            return None;
        }
        let plugin_index = raw as usize;

        self.get_cached_plugin_data(plugin_index)
            .and_then(|data| data.downcast_ref::<crate::LoudnessInfo>().cloned())
    }

    /// Set the loudness plugin index
    pub fn set_loudness_plugin_index(&mut self, index: usize) {
        self.loudness_plugin_index
            .store(index as u64, Ordering::Relaxed);
        log::debug!(
            "[AudioEngineManager] Loudness plugin index set to {}",
            index
        );
    }

    /// Get current spectrum measurements (lock-free)
    pub fn get_spectrum(&self) -> Option<crate::SpectrumInfo> {
        let raw = self.spectrum_plugin_index.load(Ordering::Relaxed);
        if raw == ATOMIC_NONE {
            return None;
        }
        let plugin_index = raw as usize;

        self.get_cached_plugin_data(plugin_index)
            .and_then(|data| data.downcast_ref::<crate::SpectrumInfo>().cloned())
    }

    /// Try to receive an event (non-blocking, lock-free status check)
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

    /// Drain all pending events (lock-free status check)
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
        self.state.store(state as u8, Ordering::Relaxed);
    }

    /// Get current engine state (snapshot from ArcSwap, lock-free)
    fn get_engine_state(&self) -> AudioEngineState {
        if let Some(engine) = &*self.engine.load() {
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
        // Stop and shutdown properly
        let _ = self.stop();
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

    #[test]
    fn verified_rate_cache_key_includes_output_channels() {
        assert_ne!(
            verified_rate_cache_key(Some("Built-in Output"), 2),
            verified_rate_cache_key(Some("Built-in Output"), 6)
        );
    }
}
