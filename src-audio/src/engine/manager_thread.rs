// ============================================================================
// Manager Thread - Coordination and Signal Handling
// ============================================================================
//
// Coordinates all worker threads, handles commands, and manages signals.

use super::{
    AudioEngineState, ConfigEvent, ConfigWatcher, DecoderCommand, DecoderThread, EngineConfig,
    ManagerCommand, ManagerResponse, PlaybackCommand, PlaybackState, PlaybackThread,
    ProcessingCommand, ProcessingThread, ThreadEvent,
};
use std::sync::mpsc::{Receiver, Sender, channel, sync_channel};
use std::sync::{Arc, Mutex};

/// Manager thread handle
pub struct ManagerThread {
    command_tx: Sender<ManagerCommand>,
    response_rx: Receiver<ManagerResponse>,
    state: Arc<Mutex<AudioEngineState>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl ManagerThread {
    /// Create and start the manager thread
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = channel();
        let (response_tx, response_rx) = channel();

        let state = Arc::new(Mutex::new(AudioEngineState::default()));
        let state_clone = Arc::clone(&state);

        let thread_handle = std::thread::Builder::new()
            .name("manager".to_string())
            .spawn(move || {
                if let Err(e) = run_manager_thread(config, command_rx, response_tx, state_clone) {
                    log::debug!("[Manager Thread] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn manager thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_rx,
            state,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the manager
    pub fn send_command(&self, command: ManagerCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Receive a response (blocking)
    pub fn recv_response(&self) -> Result<ManagerResponse, String> {
        self.response_rx
            .recv()
            .map_err(|e| format!("Failed to receive response: {}", e))
    }

    /// Try to receive a response (non-blocking)
    pub fn try_recv_response(&self) -> Option<ManagerResponse> {
        self.response_rx.try_recv().ok()
    }

    /// Get current state
    pub fn get_state(&self) -> AudioEngineState {
        self.state.lock().unwrap().clone()
    }

    /// Shutdown the manager thread
    pub fn shutdown(&mut self) {
        self.send_command(ManagerCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for ManagerThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Main manager thread function
fn run_manager_thread(
    config: EngineConfig,
    command_rx: Receiver<ManagerCommand>,
    response_tx: Sender<ManagerResponse>,
    state: Arc<Mutex<AudioEngineState>>,
) -> Result<(), String> {
    log::debug!("[Manager Thread] Starting with config: {:?}", config);

    // Create bounded queues for backpressure
    // Queue capacity based on buffer_ms to provide proper flow control
    let queue_capacity = config.queue_capacity_frames();
    log::debug!("[Manager Thread] Queue capacity: {} frames", queue_capacity);

    let (decoder_tx, decoder_rx) = sync_channel(queue_capacity);
    let (processing_tx, processing_rx) = sync_channel(queue_capacity);
    let (event_tx, event_rx) = channel(); // Events can be unbounded

    // Create threads
    let mut decoder_thread = DecoderThread::new(
        decoder_tx,
        event_tx.clone(),
        config.output_sample_rate,
        config.frame_size,
    )?;

    let mut processing_thread = ProcessingThread::new(
        decoder_rx,
        processing_tx,
        event_tx.clone(),
        config.output_sample_rate,
        config.input_channels, // Use input channels, not output
    )?;

    // Determine actual output channel count by loading plugin chain first
    let actual_output_channels = if !config.plugins.is_empty() {
        processing_thread.send_command(ProcessingCommand::UpdatePlugins(config.plugins.clone()))?;

        // Wait for response to get output channel count (with timeout)
        let start = std::time::Instant::now();
        let mut output_channels = config.output_channels;

        while start.elapsed() < std::time::Duration::from_millis(500) {
            if let Some(response) = processing_thread.try_recv_response() {
                match response {
                    super::ProcessingResponse::PluginChainUpdated {
                        output_channels: ch,
                    } => {
                        log::info!(
                            "[Manager Thread] Initial plugin chain loaded, output channels: {}",
                            ch
                        );
                        output_channels = ch;
                        break;
                    }
                    super::ProcessingResponse::Error(e) => {
                        log::debug!("[Manager Thread] Failed to initialize plugin chain: {}", e);
                        break;
                    }
                    _ => {
                        log::info!(
                            "[Manager Thread] Unexpected response during plugin initialization"
                        );
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Update state with actual channel count
        {
            let mut state_lock = state.lock().unwrap();
            state_lock.num_channels = output_channels;
        }

        output_channels
    } else {
        config.output_channels
    };

    log::info!(
        "[Manager Thread] Creating playback thread with {} channels",
        actual_output_channels
    );

    // Now create playback thread with the correct channel count
    let mut playback_thread = PlaybackThread::new(
        processing_rx,
        event_tx.clone(),
        config.output_sample_rate,
        actual_output_channels,
        config.output_device.clone(),
    )?;

    // Set initial volume and mute
    playback_thread.send_command(PlaybackCommand::SetVolume(config.volume))?;
    playback_thread.send_command(PlaybackCommand::Mute(config.muted))?;

    // Start silent source mode if no input channels (HAL playback)
    if config.input_channels == 0 {
        log::info!("[Manager Thread] Starting silent source mode for HAL input");
        decoder_thread.send_command(super::DecoderCommand::StartSilentSource)?;
    }

    // Setup config watcher if enabled
    let config_watcher = if config.watch_config {
        match ConfigWatcher::new(config.config_path.clone(), true) {
            Ok(watcher) => {
                log::debug!("[Manager Thread] Config watcher enabled");
                Some(watcher)
            }
            Err(e) => {
                log::debug!("[Manager Thread] Failed to create config watcher: {}", e);
                None
            }
        }
    } else {
        None
    };

    log::debug!("[Manager Thread] All threads started");

    // Main loop
    loop {
        // Check for thread events (non-blocking)
        if let Ok(event) = event_rx.try_recv() {
            handle_thread_event(event, &state);
        }

        // Check for config watcher events (non-blocking)
        if let Some(ref watcher) = config_watcher
            && let Some(config_event) = watcher.try_recv()
        {
            match handle_config_event(config_event, &config, &mut processing_thread, &state) {
                Ok(should_exit) => {
                    if should_exit {
                        log::debug!("[Manager Thread] Shutdown requested via signal");
                        break;
                    }
                }
                Err(e) => {
                    log::debug!("[Manager Thread] Config event error: {}", e);
                }
            }
        }

        // Check for commands (blocking with timeout)
        match command_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(command) => {
                let response = handle_command(
                    command,
                    &mut decoder_thread,
                    &mut processing_thread,
                    &mut playback_thread,
                    &state,
                );

                if let ManagerResponse::Ok = response {
                    // Check if shutdown
                    let should_exit = {
                        let state = state.lock().unwrap();
                        state.playback_state == PlaybackState::Stopped
                            && matches!(response, ManagerResponse::Ok)
                    };

                    response_tx.send(response).ok();

                    if should_exit {
                        // This was a shutdown command
                        break;
                    }
                } else {
                    response_tx.send(response).ok();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No command, continue
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::debug!("[Manager Thread] Command channel disconnected");
                break;
            }
        }
    }

    // Cleanup
    log::debug!("[Manager Thread] Shutting down threads");
    decoder_thread.shutdown();
    processing_thread.shutdown();
    playback_thread.shutdown();

    log::debug!("[Manager Thread] Stopped");
    Ok(())
}

/// Handle a thread event
fn handle_thread_event(event: ThreadEvent, state: &Arc<Mutex<AudioEngineState>>) {
    match event {
        ThreadEvent::DecoderEndOfStream => {
            log::debug!("[Manager] Decoder end of stream");
            let mut state = state.lock().unwrap();
            state.playback_state = PlaybackState::Stopped;
        }
        ThreadEvent::DecoderError(err) => {
            log::debug!("[Manager] Decoder error: {}", err);
            let mut state = state.lock().unwrap();
            state.playback_state = PlaybackState::Stopped;
        }
        ThreadEvent::PlaybackUnderrun => {
            let mut state = state.lock().unwrap();
            state.underruns += 1;
            if state.underruns % 100 == 1 {
                log::debug!("[Manager] Playback underrun (total: {})", state.underruns);
            }
        }
        ThreadEvent::ProcessingError(err) => {
            log::debug!("[Manager] Processing error: {}", err);
        }
        ThreadEvent::ThreadPanic(thread_name) => {
            log::debug!("[Manager] Thread panicked: {}", thread_name);
        }
        ThreadEvent::PositionUpdate(position) => {
            let mut state = state.lock().unwrap();
            state.position = position;
        }
    }
}

/// Handle a config watcher event
/// Returns Ok(true) if shutdown requested, Ok(false) otherwise
fn handle_config_event(
    event: ConfigEvent,
    config: &EngineConfig,
    processing: &mut ProcessingThread,
    state: &Arc<Mutex<AudioEngineState>>,
) -> Result<bool, String> {
    match event {
        ConfigEvent::ConfigChanged(_) | ConfigEvent::Reload => {
            log::debug!("[Manager] Config reload requested");

            // If we have a config path, reload from file
            if let Some(config_path) = config.config_path.as_ref() {
                log::debug!("[Manager] Reloading config from: {:?}", config_path);

                // Load and parse config file
                match load_config_file(config_path) {
                    Ok(new_config) => {
                        log::debug!("[Manager] Config loaded, updating plugin chain");

                        // Update plugin chain with seamless crossfade
                        if let Err(e) = processing
                            .send_command(ProcessingCommand::UpdatePlugins(new_config.plugins))
                        {
                            log::debug!("[Manager] Failed to update plugins: {}", e);
                        } else {
                            // Wait for response to update channel count
                            if let Some(response) = processing.try_recv_response() {
                                match response {
                                    super::ProcessingResponse::PluginChainUpdated {
                                        output_channels,
                                    } => {
                                        log::info!(
                                            "[Manager] Plugin chain updated, output channels: {}",
                                            output_channels
                                        );

                                        // Get old channel count before updating
                                        let old_channels = {
                                            let state = state.lock().unwrap();
                                            state.num_channels
                                        };

                                        // Update state with new channel count
                                        let mut state = state.lock().unwrap();
                                        state.num_channels = output_channels;
                                        drop(state);

                                        // TODO: Update playback thread channel count
                                        // (requires playback thread reference in this function)
                                        if output_channels != old_channels {
                                            log::info!(
                                                "[Manager] Channel count changed {}→{} (playback will need restart)",
                                                old_channels,
                                                output_channels
                                            );
                                        }
                                    }
                                    super::ProcessingResponse::Error(e) => {
                                        log::debug!("[Manager] Plugin update error: {}", e);
                                    }
                                    _ => {
                                        log::info!(
                                            "[Manager] Unexpected response from processing thread"
                                        );
                                    }
                                }
                            }
                            log::debug!("[Manager] Plugin chain updated successfully");
                        }
                    }
                    Err(e) => {
                        log::debug!("[Manager] Failed to load config: {}", e);
                    }
                }
            } else {
                log::debug!("[Manager] No config path set, ignoring reload request");
            }

            Ok(false)
        }
        ConfigEvent::Shutdown => {
            log::debug!("[Manager] Shutdown signal received");

            // Update state to Stopped so applications can detect shutdown
            {
                let mut state_lock = state.lock().unwrap();
                state_lock.playback_state = PlaybackState::Stopped;
            }

            Ok(true)
        }
    }
}

/// Load config from YAML file
fn load_config_file(path: &std::path::Path) -> Result<EngineConfig, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read config: {}", e))?;

    let config: EngineConfig =
        serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse config: {}", e))?;

    Ok(config)
}

/// Handle a manager command
fn handle_command(
    command: ManagerCommand,
    decoder: &mut DecoderThread,
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<Mutex<AudioEngineState>>,
) -> ManagerResponse {
    match command {
        ManagerCommand::Play(path) => {
            log::debug!("[Manager] Play: {:?}", path);

            // Update state
            {
                let mut state = state.lock().unwrap();
                state.current_file = Some(path.clone());
                state.playback_state = PlaybackState::Playing;
                state.position = 0.0;
            }

            // Send to decoder
            if let Err(e) = decoder.send_command(DecoderCommand::Play(path)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Pause => {
            log::debug!("[Manager] Pause");

            {
                let mut state = state.lock().unwrap();
                state.playback_state = PlaybackState::Paused;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Pause) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Resume => {
            log::debug!("[Manager] Resume");

            {
                let mut state = state.lock().unwrap();
                state.playback_state = PlaybackState::Playing;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Resume) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Stop => {
            log::debug!("[Manager] Stop");

            {
                let mut state = state.lock().unwrap();
                state.playback_state = PlaybackState::Stopped;
                state.current_file = None;
                state.position = 0.0;
            }

            decoder.send_command(DecoderCommand::Stop).ok();
            playback.send_command(PlaybackCommand::Stop).ok();

            ManagerResponse::Ok
        }
        ManagerCommand::Seek(position) => {
            log::debug!("[Manager] Seek to {:.2}s", position);

            {
                let mut state = state.lock().unwrap();
                state.position = position;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Seek(position)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::SetVolume(volume) => {
            log::debug!("[Manager] Set volume: {:.2}", volume);

            {
                let mut state = state.lock().unwrap();
                state.volume = volume;
            }

            if let Err(e) = playback.send_command(PlaybackCommand::SetVolume(volume)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Mute(muted) => {
            log::debug!("[Manager] Mute: {}", muted);

            {
                let mut state = state.lock().unwrap();
                state.muted = muted;
            }

            if let Err(e) = playback.send_command(PlaybackCommand::Mute(muted)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::UpdatePluginChain(plugins) => {
            log::debug!("[Manager] Update plugin chain ({} plugins)", plugins.len());

            if let Err(e) = processing.send_command(ProcessingCommand::UpdatePlugins(plugins)) {
                return ManagerResponse::Error(e);
            }

            // Wait for response to get the output channel count
            if let Some(response) = processing.try_recv_response() {
                match response {
                    super::ProcessingResponse::PluginChainUpdated { output_channels } => {
                        log::info!(
                            "[Manager] Plugin chain updated, output channels: {}",
                            output_channels
                        );

                        // Get old channel count before updating
                        let old_channels = {
                            let state = state.lock().unwrap();
                            state.num_channels
                        };

                        // Update state with new channel count
                        {
                            let mut state = state.lock().unwrap();
                            state.num_channels = output_channels;
                        }

                        // If channel count changed, update playback thread
                        if output_channels != old_channels {
                            log::info!(
                                "[Manager] Channel count changed {}→{}, updating playback thread",
                                old_channels,
                                output_channels
                            );
                            playback
                                .send_command(PlaybackCommand::UpdateChannels(output_channels))
                                .ok();
                        }

                        ManagerResponse::Ok
                    }
                    super::ProcessingResponse::Error(e) => ManagerResponse::Error(e),
                    _ => ManagerResponse::Error(
                        "Unexpected response from processing thread".to_string(),
                    ),
                }
            } else {
                ManagerResponse::Error("No response from processing thread".to_string())
            }
        }
        ManagerCommand::SetPluginParameter {
            plugin_index,
            param_id,
            value,
        } => {
            log::info!(
                "[Manager] Set plugin {} parameter {} = {}",
                plugin_index,
                param_id,
                value
            );

            if let Err(e) = processing.send_command(ProcessingCommand::SetParameter {
                plugin_index,
                param_id,
                value,
            }) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::BypassProcessing(bypass) => {
            log::debug!("[Manager] Bypass processing: {}", bypass);

            {
                let mut state = state.lock().unwrap();
                state.processing_bypassed = bypass;
            }

            if let Err(e) = processing.send_command(ProcessingCommand::Bypass(bypass)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::AddLoudnessAnalyzer { id, channels } => {
            log::info!(
                "[Manager] Add loudness analyzer: {} ({} channels)",
                id,
                channels
            );

            if let Err(e) =
                processing.send_command(ProcessingCommand::AddLoudnessAnalyzer { id, channels })
            {
                return ManagerResponse::Error(e);
            }

            // Wait for response
            if let Some(response) = processing.try_recv_response() {
                match response {
                    super::ProcessingResponse::Ok => ManagerResponse::Ok,
                    super::ProcessingResponse::Error(e) => ManagerResponse::Error(e),
                    _ => ManagerResponse::Error("Unexpected response".to_string()),
                }
            } else {
                ManagerResponse::Error("No response from processing thread".to_string())
            }
        }
        ManagerCommand::AddSpectrumAnalyzer { id, channels } => {
            log::info!(
                "[Manager] Add spectrum analyzer: {} ({} channels)",
                id,
                channels
            );

            if let Err(e) =
                processing.send_command(ProcessingCommand::AddSpectrumAnalyzer { id, channels })
            {
                return ManagerResponse::Error(e);
            }

            // Wait for response
            if let Some(response) = processing.try_recv_response() {
                match response {
                    super::ProcessingResponse::Ok => ManagerResponse::Ok,
                    super::ProcessingResponse::Error(e) => ManagerResponse::Error(e),
                    _ => ManagerResponse::Error("Unexpected response".to_string()),
                }
            } else {
                ManagerResponse::Error("No response from processing thread".to_string())
            }
        }
        ManagerCommand::RemoveAnalyzer(id) => {
            log::debug!("[Manager] Remove analyzer: {}", id);

            if let Err(e) = processing.send_command(ProcessingCommand::RemoveAnalyzer(id)) {
                return ManagerResponse::Error(e);
            }

            // Wait for response
            if let Some(response) = processing.try_recv_response() {
                match response {
                    super::ProcessingResponse::Ok => ManagerResponse::Ok,
                    super::ProcessingResponse::Error(e) => ManagerResponse::Error(e),
                    _ => ManagerResponse::Error("Unexpected response".to_string()),
                }
            } else {
                ManagerResponse::Error("No response from processing thread".to_string())
            }
        }
        ManagerCommand::GetState => {
            let state = state.lock().unwrap().clone();
            ManagerResponse::State(state)
        }
        ManagerCommand::GetPosition => {
            let position = state.lock().unwrap().position;
            ManagerResponse::Position(position)
        }
        ManagerCommand::GetAnalyzerData(analyzer_id) => {
            // log::debug!("[Manager] Get analyzer data: {}", analyzer_id);

            if let Err(e) = processing.send_command(ProcessingCommand::GetAnalyzerData(analyzer_id))
            {
                return ManagerResponse::Error(e);
            }

            // Wait for response from processing thread
            if let Some(response) = processing.try_recv_response() {
                match response {
                    super::ProcessingResponse::AnalyzerData(data) => {
                        ManagerResponse::AnalyzerData(data)
                    }
                    super::ProcessingResponse::Error(e) => ManagerResponse::Error(e),
                    _ => ManagerResponse::Error("Unexpected response".to_string()),
                }
            } else {
                ManagerResponse::Error("No response from processing thread".to_string())
            }
        }
        ManagerCommand::ReloadConfig => {
            log::debug!("[Manager] Reload config (not implemented)");
            // TODO: Reload config from file
            ManagerResponse::Ok
        }
        ManagerCommand::Shutdown => {
            log::debug!("[Manager] Shutdown requested");

            {
                let mut state = state.lock().unwrap();
                state.playback_state = PlaybackState::Stopped;
            }

            // Signal threads to shutdown
            decoder.send_command(DecoderCommand::Shutdown).ok();
            processing.send_command(ProcessingCommand::Shutdown).ok();
            playback.send_command(PlaybackCommand::Shutdown).ok();

            ManagerResponse::Ok
        }
    }
}
