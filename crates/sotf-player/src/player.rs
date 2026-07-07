use sotf_audio::decoder::AudioSource;
use sotf_audio::engine::{PluginConfig, StreamMetadata};
use sotf_audio::manager::{AudioEngineManager, StreamingEvent, StreamingState};
use std::path::PathBuf;
use std::sync::Arc;

mod types;

pub use types::*;

use types::SavedPlaybackConfig;

pub struct Player {
    manager: AudioEngineManager,
    /// Config saved at each `load_and_play_at` for crash recovery.
    saved_config: Option<SavedPlaybackConfig>,
    /// How many times we've restarted for the current track (max 1).
    restart_count: u32,
    /// One-shot flag: engine was restarted, cleared after `get_playback_state` returns it.
    engine_restarted_flag: bool,
    /// One-shot flag: engine crashed fatally (second crash). Cleared after
    /// `get_playback_state` returns it, mirroring `engine_restarted_flag`.
    /// Also reset on `stop` / `load_and_play_at` / `load_and_play_source_at` /
    /// `switch_to_source_at` to keep the flag clean across track loads.
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
        self.load_and_play_source_at(
            AudioSource::File(path),
            plugins,
            output_channels,
            output_device,
            position,
        )
    }

    /// Load and play any audio source (file, URL, or service stream).
    pub fn load_and_play_source(
        &mut self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.load_and_play_source_at(source, plugins, output_channels, output_device, None)
    }

    /// Load and play any audio source at a specific position.
    pub fn load_and_play_source_at(
        &mut self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        position: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Save config for potential crash recovery
        self.saved_config = Some(SavedPlaybackConfig {
            source: Arc::new(source.clone()),
            plugins: Arc::from(plugins.clone().into_boxed_slice()),
            output_channels,
            output_device: output_device.as_deref().map(Arc::from),
            last_position_secs: position.unwrap_or(0.0),
        });
        self.restart_count = 0;
        self.engine_restarted_flag = false;
        self.engine_fatal_flag = false;

        // Stop current playback if any
        self.manager.stop()?;

        // Load the source (file, URL, or service stream)
        self.manager.load_source(source)?;

        // Start playback with plugins and specified output device
        self.manager
            .start_playback_at(output_device, plugins, output_channels, position)?;

        Ok(())
    }

    /// Switch the current engine to another compatible source without tearing
    /// down the output stream. This keeps manual queue jumps from taking the
    /// harsher stop/load/start path used when the engine format must change.
    pub fn switch_to_source_at(
        &mut self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        position: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(config) = &self.saved_config
            && (config.output_channels != output_channels
                || config.output_device.as_deref() != output_device.as_deref())
        {
            let mut mismatches = Vec::new();
            if config.output_channels != output_channels {
                mismatches.push(format!(
                    "output_channels: saved={} requested={}",
                    config.output_channels, output_channels
                ));
            }
            if config.output_device.as_deref() != output_device.as_deref() {
                mismatches.push(format!(
                    "output_device: saved={:?} requested={:?}",
                    config.output_device.as_deref(),
                    output_device
                ));
            }
            return Err(format!(
                "Smooth source switch requires the same output configuration ({})",
                mismatches.join(", ")
            )
            .into());
        }

        self.saved_config = Some(SavedPlaybackConfig {
            source: Arc::new(source.clone()),
            plugins: Arc::from(plugins.clone().into_boxed_slice()),
            output_channels,
            output_device: output_device.as_deref().map(Arc::from),
            last_position_secs: position.unwrap_or(0.0),
        });
        self.restart_count = 0;
        self.engine_restarted_flag = false;
        self.engine_fatal_flag = false;

        self.manager.update_plugin_chain(&plugins)?;
        self.manager.switch_source_at(source, position)?;

        Ok(())
    }

    pub fn update_plugins(
        &mut self,
        plugins: Vec<PluginConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("[Player] update_plugins: {} plugins", plugins.len(),);
        match self.manager.update_plugin_chain(&plugins) {
            Ok(()) => {
                // Update saved config so crash recovery uses latest plugins.
                if let Some(ref mut config) = self.saved_config {
                    config.plugins = Arc::from(plugins.into_boxed_slice());
                }
                log::info!("[Player] update_plugins: success");
                Ok(())
            }
            Err(e) if e == "No engine running" => {
                // Engine not running yet - plugins will be applied on next playback
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Update the plugin graph (DAG topology for multi-driver crossovers)
    pub fn update_plugin_graph(
        &mut self,
        config: sotf_audio::engine::PluginGraphConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!(
            "[Player] update_plugin_graph: {} nodes, {} edges",
            config.nodes.len(),
            config.edges.len()
        );
        match self.manager.update_plugin_graph(config) {
            Ok(()) => {
                log::info!("[Player] update_plugin_graph: success");
                Ok(())
            }
            Err(e) if e == "No engine running" => Ok(()),
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

    pub fn get_volume(&self) -> f32 {
        self.manager.get_volume()
    }

    pub fn set_mute(&self, muted: bool) -> Result<(), Box<dyn std::error::Error>> {
        self.manager.set_mute(muted)?;
        Ok(())
    }

    pub fn is_muted(&self) -> bool {
        self.manager.is_muted()
    }

    /// Queue the next file for gapless playback.
    /// When the current track finishes, the decoder seamlessly transitions
    /// to the queued file without any gap. Only one file can be queued at a time.
    pub fn queue_next(&self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        self.queue_next_source(AudioSource::File(path))
    }

    /// Queue any audio source for gapless playback.
    pub fn queue_next_source(&self, source: AudioSource) -> Result<(), Box<dyn std::error::Error>> {
        match self.manager.queue_next(source) {
            Ok(()) => Ok(()),
            Err(e) if e == "No engine running" => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Cancel a previously queued next file for gapless playback.
    pub fn cancel_next(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self.manager.cancel_next() {
            Ok(()) => Ok(()),
            Err(e) if e == "No engine running" => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_underruns(&self) -> u64 {
        self.manager.get_underruns()
    }

    pub fn get_position(&self) -> f64 {
        self.manager.get_position()
    }

    pub fn get_engine_state(&self) -> sotf_audio::engine::AudioEngineState {
        self.manager.get_engine_state()
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
        // Drain any pending streaming events and capture errors / end-of-stream
        let mut last_error: Option<String> = None;
        let mut track_ended = false;
        let mut gapless_transition: Option<AudioSource> = None;
        let mut stream_metadata_event: Option<Option<StreamMetadata>> = None;
        for event in self.manager.drain_events() {
            match event {
                StreamingEvent::Error(msg) => last_error = Some(msg),
                StreamingEvent::EndOfStream => track_ended = true,
                StreamingEvent::GaplessTransition(source) => {
                    // Update saved config for crash recovery
                    if let Some(ref mut config) = self.saved_config {
                        config.source = Arc::new(source.clone());
                        config.last_position_secs = 0.0;
                    }
                    gapless_transition = Some(source);
                }
                StreamingEvent::StreamMetadataChanged(metadata) => {
                    stream_metadata_event = Some(metadata);
                }
                _ => {}
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
        let engine_state = self.manager.get_engine_state();
        let stream_metadata = stream_metadata_event.unwrap_or(engine_state.stream_metadata);

        // Read and clear one-shot flags
        let engine_restarted = self.engine_restarted_flag;
        self.engine_restarted_flag = false;
        let engine_fatal = self.engine_fatal_flag;
        self.engine_fatal_flag = false;

        PlaybackState {
            position_secs,
            is_playing,
            sample_rate,
            last_error,
            engine_restarted,
            engine_fatal,
            track_ended,
            gapless_transition,
            stream_metadata,
        }
    }

    /// Attempt to restart the engine from saved config.
    fn attempt_restart(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (source, plugins, output_channels, output_device, last_position_secs) = {
            let config = self
                .saved_config
                .as_ref()
                .ok_or("attempt_restart called without saved config")?;
            (
                (*config.source).clone(),
                config.plugins.iter().cloned().collect::<Vec<_>>(),
                config.output_channels,
                config.output_device.as_deref().map(str::to_owned),
                config.last_position_secs,
            )
        };

        // Use the last known position so the user resumes near where they were
        let position = self.manager.get_position();
        let resume_pos = if position > 0.0 {
            position
        } else {
            last_position_secs
        };

        // Stop the dead engine
        let _ = self.manager.stop();

        // Re-load and play
        self.manager.load_source(source)?;
        self.manager.start_playback_at(
            output_device,
            plugins,
            output_channels,
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
        assert!(state.stream_metadata.is_none());
    }

    #[test]
    fn update_plugins_is_ok_when_no_engine_running() {
        let mut player = Player::new();
        let result = player.update_plugins(Vec::new());
        assert!(result.is_ok());
    }

    #[test]
    fn engine_fatal_flag_is_one_shot() {
        let mut player = Player::new();
        // Simulate a fatal flag being set by the crash path.
        player.engine_fatal_flag = true;

        // First poll returns true.
        let first = player.get_playback_state();
        assert!(
            first.engine_fatal,
            "engine_fatal should be reported on first poll after the flag is set"
        );

        // Subsequent polls return false — the flag is one-shot.
        let second = player.get_playback_state();
        assert!(
            !second.engine_fatal,
            "engine_fatal should be cleared after being read once"
        );
    }
}
