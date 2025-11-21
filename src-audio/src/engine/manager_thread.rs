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
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel, sync_channel};
use std::sync::{Arc, Mutex};

const SPIN_MS_SLEEP_MANAGER: u64 = 10;
const SPIN_MS_CHECK_MANAGER: u64 = 50;
const MAX_CONFIG_QUEUE_SIZE: usize = 5; // Maximum pending config updates

/// Helper function to safely lock a mutex, handling poisoned mutexes
/// by recovering the data instead of panicking
fn safe_lock<T>(mutex: &Arc<Mutex<T>>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    match mutex.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            log::warn!("[Manager] Mutex was poisoned, recovering data");
            // Recover the data from the poisoned mutex
            Ok(poisoned.into_inner())
        }
    }
}

/// Pending config update
#[derive(Debug)]
struct PendingConfigUpdate {
    plugins: Vec<super::PluginConfig>,
    timestamp: std::time::Instant,
}

/// Config update queue manager
struct ConfigUpdateQueue {
    queue: VecDeque<PendingConfigUpdate>,
    update_in_progress: bool,
}

impl ConfigUpdateQueue {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            update_in_progress: false,
        }
    }

    /// Add a config update to the queue
    /// Returns true if added, false if queue is full (drops oldest)
    fn enqueue(&mut self, plugins: Vec<super::PluginConfig>) -> bool {
        let update = PendingConfigUpdate {
            plugins,
            timestamp: std::time::Instant::now(),
        };

        if self.queue.len() >= MAX_CONFIG_QUEUE_SIZE {
            log::warn!(
                "[Manager] Config update queue full ({} items), dropping oldest update",
                self.queue.len()
            );
            self.queue.pop_front(); // Drop oldest update
        }

        self.queue.push_back(update);
        log::debug!(
            "[Manager] Config update queued (queue size: {})",
            self.queue.len()
        );
        true
    }

    /// Check if we can start processing the next update
    fn can_process_next(&self) -> bool {
        !self.update_in_progress && !self.queue.is_empty()
    }

    /// Start processing the next update in queue
    fn start_processing(&mut self) -> Option<Vec<super::PluginConfig>> {
        if self.update_in_progress {
            return None;
        }

        if let Some(update) = self.queue.pop_front() {
            self.update_in_progress = true;
            log::debug!(
                "[Manager] Starting config update (queued for {:?}, {} remaining in queue)",
                update.timestamp.elapsed(),
                self.queue.len()
            );
            Some(update.plugins)
        } else {
            None
        }
    }

    /// Mark current update as completed
    fn complete_processing(&mut self) {
        if self.update_in_progress {
            self.update_in_progress = false;
            log::debug!("[Manager] Config update completed");
        }
    }

    /// Check if currently processing an update
    fn is_processing(&self) -> bool {
        self.update_in_progress
    }
}

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
        safe_lock(&self.state)
            .map(|guard| guard.clone())
            .unwrap_or_else(|e| {
                log::error!("[Manager] Failed to lock state: {}", e);
                AudioEngineState::default()
            })
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

    // Create config update queue for serializing plugin updates
    let mut config_update_queue = ConfigUpdateQueue::new();

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

        while start.elapsed() < std::time::Duration::from_millis(SPIN_MS_CHECK_MANAGER) {
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
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
        }

        // Update state with actual channel count
        if let Ok(mut state_lock) = safe_lock(&state) {
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
            match handle_config_event(config_event, &config, &mut config_update_queue, &state) {
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

        // Process pending config updates (one at a time)
        if config_update_queue.can_process_next() {
            if let Some(plugins) = config_update_queue.start_processing() {
                log::debug!("[Manager Thread] Processing queued config update");
                if let Err(e) = apply_plugin_update(
                    &mut processing_thread,
                    &mut playback_thread,
                    &state,
                    plugins,
                ) {
                    log::error!("[Manager Thread] Failed to apply config update: {}", e);
                }
                config_update_queue.complete_processing();
            }
        }

        // Check for commands (blocking with timeout)
        match command_rx.recv_timeout(std::time::Duration::from_millis(SPIN_MS_CHECK_MANAGER)) {
            Ok(command) => {
                let response = handle_command(
                    command,
                    &mut decoder_thread,
                    &mut processing_thread,
                    &mut playback_thread,
                    &state,
                    &mut config_update_queue,
                );

                if let ManagerResponse::Ok = response {
                    // Check if shutdown
                    let should_exit = if let Ok(state_guard) = safe_lock(&state) {
                        state_guard.playback_state == PlaybackState::Stopped
                            && matches!(response, ManagerResponse::Ok)
                    } else {
                        false
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
            if let Ok(mut state) = safe_lock(state) {
                state.playback_state = PlaybackState::Stopped;
            }
        }
        ThreadEvent::DecoderError(err) => {
            log::debug!("[Manager] Decoder error: {}", err);
            if let Ok(mut state) = safe_lock(state) {
                state.playback_state = PlaybackState::Stopped;
            }
        }
        ThreadEvent::PlaybackUnderrun => {
            if let Ok(mut state) = safe_lock(state) {
                state.underruns += 1;
                // Log summary every 50 underruns to track overall pattern
                if state.underruns % 50 == 1 {
                    log::warn!("[Manager] Playback underrun count: {}", state.underruns);
                }
            }
        }
        ThreadEvent::ProcessingError(err) => {
            log::debug!("[Manager] Processing error: {}", err);
        }
        ThreadEvent::ThreadPanic(thread_name) => {
            log::debug!("[Manager] Thread panicked: {}", thread_name);
        }
        ThreadEvent::PositionUpdate(position) => {
            if let Ok(mut state) = safe_lock(state) {
                state.position = position;
            }
        }
    }
}

/// Handle a config watcher event
/// Returns Ok(true) if shutdown requested, Ok(false) otherwise
fn handle_config_event(
    event: ConfigEvent,
    config: &EngineConfig,
    config_queue: &mut ConfigUpdateQueue,
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
                        log::debug!("[Manager] Config loaded, enqueuing plugin update");

                        // Enqueue the update instead of applying immediately
                        config_queue.enqueue(new_config.plugins);
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
            if let Ok(mut state_lock) = safe_lock(state) {
                state_lock.playback_state = PlaybackState::Stopped;
            }

            Ok(true)
        }
    }
}

/// Apply a plugin update with proper synchronization
/// Waits for confirmation from processing thread and updates playback thread if needed
fn apply_plugin_update(
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<Mutex<AudioEngineState>>,
    plugins: Vec<super::PluginConfig>,
) -> Result<(), String> {
    // Send update command to processing thread
    processing.send_command(ProcessingCommand::UpdatePlugins(plugins))?;

    // Wait for response with longer timeout to allow crossfade to complete
    let timeout = std::time::Duration::from_millis(500); // 500ms should be enough for crossfade
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = processing.try_recv_response() {
            match response {
                super::ProcessingResponse::PluginChainUpdated { output_channels } => {
                    log::info!(
                        "[Manager] Plugin chain updated, output channels: {}",
                        output_channels
                    );

                    // Get old channel count before updating
                    let old_channels = if let Ok(state_guard) = safe_lock(state) {
                        state_guard.num_channels
                    } else {
                        return Err("Failed to lock state".to_string());
                    };

                    // Update state with new channel count
                    if let Ok(mut state_guard) = safe_lock(state) {
                        state_guard.num_channels = output_channels;
                    }

                    // If channel count changed, update playback thread
                    if output_channels != old_channels {
                        log::info!(
                            "[Manager] Channel count changed {}→{}, updating playback thread",
                            old_channels,
                            output_channels
                        );

                        // Clear ring buffer first to flush any pending frames with old channel count
                        // This prevents audio corruption from channel count mismatches during hot-reload
                        playback.send_command(PlaybackCommand::Stop)?;

                        // Send update command (fire-and-forget)
                        // Playback thread will handle channel update asynchronously
                        playback.send_command(PlaybackCommand::UpdateChannels(output_channels))?;
                    }

                    return Ok(());
                }
                super::ProcessingResponse::Error(e) => {
                    return Err(format!("Plugin update error: {}", e));
                }
                _ => {
                    return Err("Unexpected response from processing thread".to_string());
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err("Timeout waiting for plugin update response".to_string())
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
    config_queue: &mut ConfigUpdateQueue,
) -> ManagerResponse {
    match command {
        ManagerCommand::Play(path) => {
            log::debug!("[Manager] Play: {:?}", path);

            // Update state
            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.current_file = Some(path.clone());
                state_guard.playback_state = PlaybackState::Playing;
                state_guard.position = 0.0;
            }

            // Send to decoder
            if let Err(e) = decoder.send_command(DecoderCommand::Play(path)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Pause => {
            log::debug!("[Manager] Pause");

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Paused;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Pause) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Resume => {
            log::debug!("[Manager] Resume");

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Playing;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Resume) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Stop => {
            log::debug!("[Manager] Stop");

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Stopped;
                state_guard.current_file = None;
                state_guard.position = 0.0;
            }

            decoder.send_command(DecoderCommand::Stop).ok();
            playback.send_command(PlaybackCommand::Stop).ok();

            ManagerResponse::Ok
        }
        ManagerCommand::Seek(position) => {
            log::debug!("[Manager] Seek to {:.2}s", position);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.position = position;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Seek(position)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::SetVolume(volume) => {
            log::debug!("[Manager] Set volume: {:.2}", volume);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.volume = volume;
            }

            if let Err(e) = playback.send_command(PlaybackCommand::SetVolume(volume)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Mute(muted) => {
            log::debug!("[Manager] Mute: {}", muted);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.muted = muted;
            }

            if let Err(e) = playback.send_command(PlaybackCommand::Mute(muted)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::UpdatePluginChain(plugins) => {
            log::debug!("[Manager] Update plugin chain ({} plugins)", plugins.len());

            // If a config update is already in progress, enqueue this one
            if config_queue.is_processing() {
                log::debug!("[Manager] Config update in progress, enqueuing new update");
                config_queue.enqueue(plugins);
                return ManagerResponse::Ok;
            }

            // Otherwise, apply immediately using the synchronized apply function
            match apply_plugin_update(processing, playback, state, plugins) {
                Ok(()) => ManagerResponse::Ok,
                Err(e) => ManagerResponse::Error(e),
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

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.processing_bypassed = bypass;
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
            if let Ok(state_guard) = safe_lock(state) {
                ManagerResponse::State(state_guard.clone())
            } else {
                ManagerResponse::Error("Failed to lock state".to_string())
            }
        }
        ManagerCommand::GetPosition => {
            if let Ok(state_guard) = safe_lock(state) {
                ManagerResponse::Position(state_guard.position)
            } else {
                ManagerResponse::Error("Failed to lock state".to_string())
            }
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

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Stopped;
            }

            // Signal threads to shutdown
            decoder.send_command(DecoderCommand::Shutdown).ok();
            processing.send_command(ProcessingCommand::Shutdown).ok();
            playback.send_command(PlaybackCommand::Shutdown).ok();

            ManagerResponse::Ok
        }
    }
}
