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
const PLUGIN_INIT_TIMEOUT_MS: u64 = 10000; // 10 seconds for plugin initialization (SOFA loading can be slow)
const MAX_CONFIG_QUEUE_SIZE: usize = 5; // Maximum pending config updates

/// Priority for config updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ConfigUpdatePriority {
    FileWatcher = 1,  // Lowest priority - automatic file watching
    SignalReload = 2, // Medium priority - SIGHUP signal
    UserDirect = 3,   // Highest priority - direct API/command
}

/// Structured config error types
#[derive(Debug, Clone)]
enum ConfigError {
    /// Failed to parse config file
    ParseError {
        path: std::path::PathBuf,
        reason: String,
    },
    /// Config validation failed
    ValidationError { plugin_index: usize, reason: String },
    /// Plugin update timed out
    TimeoutError { waited_ms: u64 },
    /// Plugin update failed in processing thread
    ProcessingError { reason: String },
    /// Unexpected response from processing thread
    UnexpectedResponse,
    /// Failed to lock state mutex
    StateLockError,
    /// Communication channel disconnected
    ChannelDisconnected,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ParseError { path, reason } => {
                write!(f, "Failed to parse config {:?}: {}", path, reason)
            }
            Self::ValidationError {
                plugin_index,
                reason,
            } => {
                write!(f, "Plugin {} validation failed: {}", plugin_index, reason)
            }
            Self::TimeoutError { waited_ms } => {
                write!(f, "Plugin update timed out after {}ms", waited_ms)
            }
            Self::ProcessingError { reason } => {
                write!(f, "Plugin processing error: {}", reason)
            }
            Self::UnexpectedResponse => {
                write!(f, "Unexpected response from processing thread")
            }
            Self::StateLockError => {
                write!(f, "Failed to acquire state lock")
            }
            Self::ChannelDisconnected => {
                write!(f, "Communication channel disconnected")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Metrics for config update operations
#[derive(Default, Debug, Clone)]
struct ConfigUpdateMetrics {
    /// Total number of update attempts
    total_updates: u64,
    /// Number of successful updates
    successful_updates: u64,
    /// Number of failed updates
    failed_updates: u64,
    /// Number of updates rejected (validation or queue full)
    rejected_updates: u64,
    /// Number of rollbacks attempted
    rollback_attempts: u64,
    /// Number of successful rollbacks
    successful_rollbacks: u64,
    /// Total time spent on updates (milliseconds)
    total_update_time_ms: u64,
    /// Maximum queue depth observed
    max_queue_depth: usize,
    /// Last update timestamp
    last_update_time: Option<std::time::Instant>,
}

impl ConfigUpdateMetrics {
    fn new() -> Self {
        Self::default()
    }

    fn record_success(&mut self, duration: std::time::Duration) {
        self.total_updates += 1;
        self.successful_updates += 1;
        self.total_update_time_ms += duration.as_millis() as u64;
        self.last_update_time = Some(std::time::Instant::now());
    }

    fn record_failure(&mut self) {
        self.total_updates += 1;
        self.failed_updates += 1;
    }

    fn record_rejection(&mut self) {
        self.rejected_updates += 1;
    }

    fn record_rollback(&mut self, success: bool) {
        self.rollback_attempts += 1;
        if success {
            self.successful_rollbacks += 1;
        }
    }

    fn update_queue_depth(&mut self, depth: usize) {
        self.max_queue_depth = self.max_queue_depth.max(depth);
    }

    fn success_rate(&self) -> f64 {
        if self.total_updates == 0 {
            return 1.0;
        }
        self.successful_updates as f64 / self.total_updates as f64
    }

    fn avg_update_time_ms(&self) -> f64 {
        if self.successful_updates == 0 {
            return 0.0;
        }
        self.total_update_time_ms as f64 / self.successful_updates as f64
    }
}

/// Helper function to safely lock a mutex, handling poisoned mutexes
/// by recovering the data instead of panicking
fn safe_lock<T>(mutex: &Arc<Mutex<T>>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    match mutex.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            log::warn!("[Manager Thread] Mutex was poisoned, recovering data");
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
    priority: ConfigUpdatePriority,
}

/// Config update queue manager
struct ConfigUpdateQueue {
    queue: VecDeque<PendingConfigUpdate>,
    update_in_progress: bool,
    last_working_config: Option<Vec<super::PluginConfig>>,
    metrics: ConfigUpdateMetrics,
}

impl ConfigUpdateQueue {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            update_in_progress: false,
            last_working_config: None,
            metrics: ConfigUpdateMetrics::new(),
        }
    }

    /// Save a working config for rollback
    fn save_working_config(&mut self, plugins: Vec<super::PluginConfig>) {
        self.last_working_config = Some(plugins);
    }

    /// Get the last working config for rollback
    fn get_rollback_config(&self) -> Option<&Vec<super::PluginConfig>> {
        self.last_working_config.as_ref()
    }

    /// Add a config update to the queue with priority-based management
    /// Returns true if added, false if rejected
    fn enqueue(
        &mut self,
        plugins: Vec<super::PluginConfig>,
        priority: ConfigUpdatePriority,
    ) -> bool {
        let update = PendingConfigUpdate {
            plugins,
            timestamp: std::time::Instant::now(),
            priority,
        };

        if self.queue.len() >= MAX_CONFIG_QUEUE_SIZE {
            // Find lowest priority item in queue
            let min_priority_idx = self
                .queue
                .iter()
                .enumerate()
                .min_by_key(|(_, u)| u.priority)
                .map(|(i, _)| i);

            if let Some(idx) = min_priority_idx {
                let min_priority = self.queue[idx].priority;

                if priority > min_priority {
                    // New update has higher priority - drop lower priority item
                    let dropped = self.queue.remove(idx).unwrap();
                    log::warn!(
                        "[Manager Thread] Config queue full, dropping {:?} update to make room for {:?} update",
                        dropped.priority,
                        priority
                    );
                } else {
                    // New update has equal or lower priority - reject it
                    log::warn!(
                        "[Manager Thread] Config queue full with higher priority items, rejecting {:?} update",
                        priority
                    );
                    self.metrics.record_rejection();
                    return false;
                }
            }
        }

        self.queue.push_back(update);
        self.metrics.update_queue_depth(self.queue.len());
        log::debug!(
            "[Manager Thread] Config update queued with priority {:?} (queue size: {}, max: {})",
            priority,
            self.queue.len(),
            self.metrics.max_queue_depth
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
                "[Manager Thread] Starting config update (queued for {:?}, {} remaining in queue)",
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
            log::debug!("[Manager Thread] Config update completed");
        }
    }

    /// Check if currently processing an update
    fn is_processing(&self) -> bool {
        self.update_in_progress
    }

    /// Get current metrics
    #[allow(dead_code)]
    fn get_metrics(&self) -> &ConfigUpdateMetrics {
        &self.metrics
    }

    /// Log metrics summary
    fn log_metrics_summary(&self) {
        log::info!(
            "[Manager Thread] Config Update Metrics: {} total, {} success ({:.1}%), {} failed, {} rejected, {} rollbacks ({} successful), avg {:.0}ms, max queue depth: {}",
            self.metrics.total_updates,
            self.metrics.successful_updates,
            self.metrics.success_rate() * 100.0,
            self.metrics.failed_updates,
            self.metrics.rejected_updates,
            self.metrics.rollback_attempts,
            self.metrics.successful_rollbacks,
            self.metrics.avg_update_time_ms(),
            self.metrics.max_queue_depth
        );
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
                log::error!("[Manager Thread] Failed to lock state: {}", e);
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
        log::info!(
            "[Manager Thread] Loading initial plugin chain ({} plugins)...",
            config.plugins.len()
        );
        processing_thread.send_command(ProcessingCommand::UpdatePlugins(config.plugins.clone()))?;

        // Wait for response to get output channel count (with timeout)
        // Use longer timeout for initial plugin loading since SOFA files can take time
        let start = std::time::Instant::now();
        let mut output_channels: Option<usize> = None;
        let mut plugin_error: Option<String> = None;

        while start.elapsed() < std::time::Duration::from_millis(PLUGIN_INIT_TIMEOUT_MS) {
            if let Some(response) = processing_thread.try_recv_response() {
                match response {
                    super::ProcessingResponse::PluginChainUpdated {
                        output_channels: ch,
                    } => {
                        log::info!(
                            "[Manager Thread] Initial plugin chain loaded in {:?}, output channels: {}",
                            start.elapsed(),
                            ch
                        );
                        output_channels = Some(ch);
                        break;
                    }
                    super::ProcessingResponse::Error(e) => {
                        log::error!(
                            "[Manager Thread] FATAL: Failed to initialize plugin chain: {}",
                            e
                        );
                        plugin_error = Some(e);
                        break;
                    }
                    _ => {
                        log::error!(
                            "[Manager Thread] Unexpected response during plugin initialization"
                        );
                        plugin_error =
                            Some("Unexpected response during plugin initialization".to_string());
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
        }

        // Check for errors - fail hard instead of continuing with mismatched channels
        if let Some(e) = plugin_error {
            return Err(format!(
                "Plugin chain initialization failed: {}. \
                 Cannot proceed with mismatched channel configuration.",
                e
            ));
        }

        // Check if we timed out
        if output_channels.is_none() {
            let elapsed = start.elapsed();
            log::error!(
                "[Manager Thread] Plugin initialization timed out after {:?}",
                elapsed
            );
            return Err(format!(
                "Plugin chain initialization timed out after {:?}. \
                 Cannot proceed without knowing output channel count.",
                elapsed
            ));
        }

        let output_channels = output_channels.unwrap();

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
        if config_update_queue.can_process_next()
            && let Some(plugins) = config_update_queue.start_processing()
        {
            log::debug!("[Manager Thread] Processing queued config update");
            if let Err(e) = apply_plugin_update(
                &mut processing_thread,
                &mut playback_thread,
                &state,
                &mut config_update_queue,
                plugins,
            ) {
                log::error!("[Manager Thread] Failed to apply config update: {}", e);
            }
            config_update_queue.complete_processing();
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
                    &config,
                    &mut config_update_queue,
                );

                let should_exit = matches!(response, ManagerResponse::Shutdown);
                response_tx.send(response).ok();

                if should_exit {
                    log::debug!("[Manager Thread] Shutdown response sent, exiting loop");
                    break;
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

    // Log final metrics before shutdown
    config_update_queue.log_metrics_summary();

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
            log::debug!("[Manager Thread] Decoder end of stream");
            if let Ok(mut state) = safe_lock(state) {
                state.playback_state = PlaybackState::Stopped;
                state.last_error = None;
            }
        }
        ThreadEvent::DecoderError(err) => {
            log::debug!("[Manager Thread] Decoder error: {}", err);
            if let Ok(mut state) = safe_lock(state) {
                state.playback_state = PlaybackState::Stopped;
                state.last_error = Some(err);
            }
        }
        ThreadEvent::PlaybackUnderrun => {
            if let Ok(mut state) = safe_lock(state) {
                state.underruns += 1;
                // Log summary every 50 underruns to track overall pattern
                if state.underruns % 50 == 1 {
                    log::warn!(
                        "[Manager Thread] Playback underrun count: {}",
                        state.underruns
                    );
                }
            }
        }
        ThreadEvent::ProcessingError(err) => {
            log::debug!("[Manager Thread] Processing error: {}", err);
            if let Ok(mut state) = safe_lock(state) {
                state.playback_state = PlaybackState::Stopped;
                state.last_error = Some(err);
            }
        }
        ThreadEvent::ThreadPanic(thread_name) => {
            log::debug!("[Manager Thread] Thread panicked: {}", thread_name);
            if let Ok(mut state) = safe_lock(state) {
                state.playback_state = PlaybackState::Stopped;
                state.last_error = Some(format!("Thread panicked: {}", thread_name));
            }
        }
        ThreadEvent::PositionUpdate(position) => {
            if let Ok(mut state) = safe_lock(state)
                && state.playback_state != PlaybackState::Stopped
                && !state.seeking
            {
                state.position = position;
            }
        }
        ThreadEvent::SeekComplete => {
            log::debug!("[Manager Thread] Seek complete");
            if let Ok(mut state) = safe_lock(state) {
                state.seeking = false;
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
            log::debug!("[Manager Thread] Config reload requested");

            // If we have a config path, reload from file
            if let Some(config_path) = config.config_path.as_ref() {
                log::debug!("[Manager Thread] Reloading config from: {:?}", config_path);

                // Load and parse config file
                match load_config_file(config_path) {
                    Ok(new_config) => {
                        // Validate config before queuing
                        match validate_plugin_configs(&new_config.plugins) {
                            Ok(_) => {
                                log::debug!(
                                    "[Manager Thread] Config validated, enqueuing plugin update"
                                );
                                // Use SignalReload priority for explicit reloads, FileWatcher for file changes
                                let priority = match event {
                                    ConfigEvent::Reload => ConfigUpdatePriority::SignalReload,
                                    _ => ConfigUpdatePriority::FileWatcher,
                                };
                                config_queue.enqueue(new_config.plugins, priority);
                            }
                            Err(e) => {
                                log::warn!("[Manager Thread] Config validation failed: {}", e);
                                config_queue.metrics.record_rejection();
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[Manager Thread] Config parse failed: {}", e);
                    }
                }
            } else {
                log::debug!("[Manager Thread] No config path set, ignoring reload request");
            }

            Ok(false)
        }
        ConfigEvent::Shutdown => {
            log::debug!("[Manager Thread] Shutdown signal received");

            // Update state to Stopped so applications can detect shutdown
            if let Ok(mut state_lock) = safe_lock(state) {
                state_lock.playback_state = PlaybackState::Stopped;
            }

            Ok(true)
        }
    }
}

/// Estimate timeout duration based on plugin complexity
/// Complex plugins (SOFA loading, large convolutions) need more time
fn estimate_update_timeout(plugins: &[super::PluginConfig]) -> std::time::Duration {
    let mut timeout_ms: u64 = 200; // Base timeout for crossfade

    for plugin in plugins {
        timeout_ms += match plugin.plugin_type.as_str() {
            "convolution" => {
                // SOFA/IR loading can be very slow
                2000
            }
            "upmixer" => {
                // FFT setup and buffer allocation
                300
            }
            "crossover" => {
                // Multiple filter banks
                200
            }
            "EQ" => {
                // Count number of filters if available
                if let Some(filters) = plugin.parameters.get("filters") {
                    if let Some(array) = filters.as_array() {
                        array.len() as u64 * 10 // ~10ms per filter
                    } else {
                        50
                    }
                } else {
                    50
                }
            }
            "resampler" => 150,
            "limiter" | "compressor" | "gate" => 100,
            "gain" | "matrix" => 20,
            _ => 50,
        };
    }

    // Cap at 10 seconds (for very complex chains)
    std::time::Duration::from_millis(timeout_ms.min(10000))
}

/// Apply a plugin update with proper synchronization and rollback on failure
/// Waits for confirmation from processing thread and updates playback thread if needed
fn apply_plugin_update(
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<Mutex<AudioEngineState>>,
    config_queue: &mut ConfigUpdateQueue,
    plugins: Vec<super::PluginConfig>,
) -> Result<(), ConfigError> {
    log::trace!(
        "[Manager Thread] apply_plugin_update: Starting update with {} plugins",
        plugins.len()
    );

    // Send update command to processing thread
    processing
        .send_command(ProcessingCommand::UpdatePlugins(plugins.clone()))
        .map_err(|_| ConfigError::ChannelDisconnected)?;
    log::trace!(
        "[Manager Thread] apply_plugin_update: Sent UpdatePlugins command to processing thread"
    );

    // Calculate adaptive timeout based on plugin complexity
    let timeout = estimate_update_timeout(&plugins);
    log::debug!("[Manager Thread] Using adaptive timeout: {:?}", timeout);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = processing.try_recv_response() {
            log::trace!(
                "[Manager Thread] apply_plugin_update: Received response after {:?}",
                start.elapsed()
            );
            match response {
                super::ProcessingResponse::PluginChainUpdated { output_channels } => {
                    log::info!(
                        "[Manager Thread] Plugin chain updated in {:?}, output channels: {}",
                        start.elapsed(),
                        output_channels
                    );

                    // Get old channel count before updating
                    let old_channels = if let Ok(state_guard) = safe_lock(state) {
                        state_guard.num_channels
                    } else {
                        return Err(ConfigError::StateLockError);
                    };

                    // Update state with new channel count
                    if let Ok(mut state_guard) = safe_lock(state) {
                        state_guard.num_channels = output_channels;
                    }

                    // Log whether channel count actually changed
                    if output_channels != old_channels {
                        log::info!(
                            "[Manager Thread] Channel count changed {}→{}, no crossfade (clearing queues)",
                            old_channels,
                            output_channels
                        );
                    } else {
                        log::info!(
                            "[Manager Thread] Channel count unchanged ({}→{}), forcing playback reconfiguration",
                            old_channels,
                            output_channels
                        );
                    }

                    // Save current playback state to potentially resume after hot-reload
                    let was_playing = if let Ok(state_guard) = safe_lock(state) {
                        state_guard.playback_state == PlaybackState::Playing
                    } else {
                        false
                    };

                    // CRITICAL: When channel count changes, we CANNOT crossfade
                    // The processing thread will do immediate swap (no crossfade)
                    // We need to clear all queues to prevent channel mismatches:

                    // 1. Clear playback ring buffer (removes old-channel-count frames)
                    playback
                        .send_command(PlaybackCommand::Stop)
                        .map_err(|_| ConfigError::ChannelDisconnected)?;
                    log::debug!("[Manager Thread] Cleared playback ring buffer");

                    // 2. Update playback thread channel configuration
                    //    The UpdateChannels handler will:
                    //    - Drain all pending frames from processing→playback queue
                    //    - Update channel count
                    //    - Rebuild audio stream
                    playback
                        .send_command(PlaybackCommand::UpdateChannels(output_channels))
                        .map_err(|_| ConfigError::ChannelDisconnected)?;
                    log::debug!(
                        "[Manager Thread] Sent UpdateChannels({}) to playback thread",
                        output_channels
                    );

                    // If playback was active, update state to reflect that we're ready to resume
                    // The decoder thread should automatically continue feeding data
                    if was_playing {
                        log::debug!(
                            "[Manager Thread] Playback was active during hot-reload, will auto-resume"
                        );
                        // Note: We don't explicitly resume here - the processing pipeline will
                        // automatically continue once the new channel configuration is applied
                    }

                    // Save this as the last working config for future rollback
                    config_queue.save_working_config(plugins);

                    // Record success metrics
                    config_queue.metrics.record_success(start.elapsed());

                    return Ok(());
                }
                super::ProcessingResponse::Error(e) => {
                    log::error!("[Manager Thread] Plugin update error: {}", e);

                    // Record failure metrics
                    config_queue.metrics.record_failure();

                    // Attempt rollback to last working config
                    if let Some(rollback_config) = config_queue.get_rollback_config() {
                        log::warn!(
                            "[Manager Thread] Attempting rollback to last working config ({} plugins)",
                            rollback_config.len()
                        );

                        // Try to restore the last working config
                        if let Err(_) = processing
                            .send_command(ProcessingCommand::UpdatePlugins(rollback_config.clone()))
                        {
                            log::error!("[Manager Thread] Failed to send rollback command");
                            return Err(ConfigError::ChannelDisconnected);
                        }

                        // Wait for rollback confirmation (shorter timeout)
                        let rollback_start = std::time::Instant::now();
                        let rollback_timeout = std::time::Duration::from_millis(250);
                        let mut rollback_success = false;
                        let mut rollback_channels: Option<usize> = None;

                        while rollback_start.elapsed() < rollback_timeout {
                            if let Some(rollback_response) = processing.try_recv_response() {
                                match rollback_response {
                                    super::ProcessingResponse::PluginChainUpdated {
                                        output_channels: ch,
                                    } => {
                                        log::info!(
                                            "[Manager Thread] Rollback successful, output channels: {}",
                                            ch
                                        );
                                        rollback_success = true;
                                        rollback_channels = Some(ch);
                                        break;
                                    }
                                    super::ProcessingResponse::Error(e) => {
                                        log::error!("[Manager Thread] Rollback failed: {}", e);
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(
                                SPIN_MS_SLEEP_MANAGER,
                            ));
                        }

                        // If rollback succeeded, update playback thread channel count
                        if rollback_success {
                            if let Some(ch) = rollback_channels {
                                // Update state
                                if let Ok(mut state_guard) = safe_lock(state) {
                                    state_guard.num_channels = ch;
                                }

                                // Clear playback buffer and update channel count
                                let _ = playback.send_command(PlaybackCommand::Stop);
                                let _ = playback.send_command(PlaybackCommand::UpdateChannels(ch));
                                log::info!(
                                    "[Manager Thread] Playback reconfigured to {} channels after rollback",
                                    ch
                                );
                            }
                        }

                        // Record rollback metrics
                        config_queue.metrics.record_rollback(rollback_success);
                    } else {
                        log::warn!("[Manager Thread] No rollback config available");
                    }

                    return Err(ConfigError::ProcessingError { reason: e });
                }
                _ => {
                    return Err(ConfigError::UnexpectedResponse);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err(ConfigError::TimeoutError {
        waited_ms: timeout.as_millis() as u64,
    })
}

/// Validate plugin configurations before applying
fn validate_plugin_configs(configs: &[super::PluginConfig]) -> Result<(), ConfigError> {
    for (i, config) in configs.iter().enumerate() {
        // Check if plugin type is recognized (case-insensitive)
        let valid_types = [
            "eq",
            "gain",
            "upmixer",
            "compressor",
            "gate",
            "limiter",
            "loudness_compensation",
            "loudness_monitor",
            "spectrum_analyzer",
            "channel_mute_solo",
            "binaural_decoder",
            "matrix",
            "convolution",
            "crossover",
            "delay",
            "resampler",
        ];

        let plugin_type_lower = config.plugin_type.to_lowercase();
        if !valid_types.contains(&plugin_type_lower.as_str()) {
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason: format!("Unknown plugin type '{}'", config.plugin_type),
            });
        }

        // Validate that parameters exist
        if config.parameters.is_null() {
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason: format!("Plugin '{}' missing parameters", config.plugin_type),
            });
        }

        // Type-specific validation (case-insensitive)
        match plugin_type_lower.as_str() {
            "eq" => {
                // Validate EQ filter structure
                if let Some(filters) = config.parameters.get("filters")
                    && !filters.is_array()
                {
                    return Err(ConfigError::ValidationError {
                        plugin_index: i,
                        reason: "Invalid 'filters' parameter (must be array)".to_string(),
                    });
                }
            }
            "gain" => {
                // Validate gain_db exists
                if config.parameters.get("gain_db").is_none() {
                    return Err(ConfigError::ValidationError {
                        plugin_index: i,
                        reason: "Missing 'gain_db' parameter".to_string(),
                    });
                }
            }
            "upmixer" => {
                // Validate upmixer mode
                if let Some(mode) = config.parameters.get("mode")
                    && !mode.is_string()
                {
                    return Err(ConfigError::ValidationError {
                        plugin_index: i,
                        reason: "Invalid 'mode' parameter (must be string)".to_string(),
                    });
                }
            }
            _ => {
                // Basic validation for other types
            }
        }
    }

    Ok(())
}

/// Load config from YAML file
fn load_config_file(path: &std::path::Path) -> Result<EngineConfig, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(|e| ConfigError::ParseError {
        path: path.to_path_buf(),
        reason: format!("Failed to read file: {}", e),
    })?;

    let config: EngineConfig =
        serde_yaml::from_str(&contents).map_err(|e| ConfigError::ParseError {
            path: path.to_path_buf(),
            reason: format!("YAML parse error: {}", e),
        })?;

    Ok(config)
}

/// Handle a manager command
fn handle_command(
    command: ManagerCommand,
    decoder: &mut DecoderThread,
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<Mutex<AudioEngineState>>,
    config: &EngineConfig,
    config_queue: &mut ConfigUpdateQueue,
) -> ManagerResponse {
    match command {
        ManagerCommand::Play(path) => {
            log::debug!("[Manager Thread] Play: {:?}", path);

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
            log::debug!("[Manager Thread] Pause");

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Paused;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Pause) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Resume => {
            log::debug!("[Manager Thread] Resume");

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Playing;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Resume) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Stop => {
            log::debug!("[Manager Thread] Stop");

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
            log::debug!("[Manager Thread] Seek to {:.2}s", position);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.position = position;
                state_guard.seeking = true;
            }

            if let Err(e) = decoder.send_command(DecoderCommand::Seek(position)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::SetVolume(volume) => {
            log::debug!("[Manager Thread] Set volume: {:.2}", volume);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.volume = volume;
            }

            if let Err(e) = playback.send_command(PlaybackCommand::SetVolume(volume)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Mute(muted) => {
            log::debug!("[Manager Thread] Mute: {}", muted);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.muted = muted;
            }

            if let Err(e) = playback.send_command(PlaybackCommand::Mute(muted)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::UpdatePluginChain(plugins) => {
            log::debug!(
                "[Manager Thread] Update plugin chain ({} plugins)",
                plugins.len()
            );
            log::trace!(
                "[Manager Thread] UpdatePluginChain: Validating configuration with {} plugins",
                plugins.len()
            );

            // Validate config before processing
            if let Err(e) = validate_plugin_configs(&plugins) {
                log::warn!(
                    "[Manager Thread] Plugin configuration validation failed: {}",
                    e
                );
                config_queue.metrics.record_rejection();
                return ManagerResponse::Error(e.to_string());
            }

            log::trace!("[Manager Thread] UpdatePluginChain: Configuration validated successfully");

            // If a config update is already in progress, enqueue this one
            if config_queue.is_processing() {
                log::debug!("[Manager Thread] Config update in progress, enqueuing new update");
                log::trace!(
                    "[Manager Thread] UpdatePluginChain: Queueing update (queue size before: {})",
                    config_queue.queue.len()
                );
                let queued = config_queue.enqueue(plugins, ConfigUpdatePriority::UserDirect);
                if queued {
                    log::trace!("[Manager Thread] UpdatePluginChain: Update queued successfully");
                    return ManagerResponse::Ok;
                } else {
                    log::warn!(
                        "[Manager Thread] UpdatePluginChain: Failed to queue update (queue full)"
                    );
                    return ManagerResponse::Error("Plugin update queue is full".to_string());
                }
            }

            log::trace!("[Manager Thread] UpdatePluginChain: Applying update immediately");

            // Otherwise, apply immediately using the synchronized apply function
            match apply_plugin_update(processing, playback, state, config_queue, plugins) {
                Ok(()) => {
                    log::trace!("[Manager Thread] UpdatePluginChain: Update applied successfully");
                    ManagerResponse::Ok
                }
                Err(e) => {
                    log::trace!("[Manager Thread] UpdatePluginChain: Update failed: {}", e);
                    ManagerResponse::Error(e.to_string())
                }
            }
        }
        ManagerCommand::SetPluginParameter {
            plugin_index,
            param_id,
            value,
        } => {
            log::info!(
                "[Manager Thread] Set plugin {} parameter {} = {}",
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
            log::debug!("[Manager Thread] Bypass processing: {}", bypass);

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.processing_bypassed = bypass;
            }

            if let Err(e) = processing.send_command(ProcessingCommand::Bypass(bypass)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
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
        ManagerCommand::GetPluginData(index) => {
            if let Err(e) = processing.send_command(ProcessingCommand::GetPluginData(index)) {
                return ManagerResponse::Error(e);
            }

            // Wait for response from processing thread
            if let Some(response) = processing.try_recv_response() {
                match response {
                    super::ProcessingResponse::PluginData(data) => {
                        ManagerResponse::PluginData(data)
                    }
                    super::ProcessingResponse::Error(e) => ManagerResponse::Error(e),
                    _ => ManagerResponse::Error("Unexpected response".to_string()),
                }
            } else {
                ManagerResponse::Error("No response from processing thread".to_string())
            }
        }
        ManagerCommand::ReloadConfig => {
            log::debug!("[Manager Thread] Reload config requested");

            // If we have a config path, reload from file
            if let Some(config_path) = config.config_path.as_ref() {
                log::debug!("[Manager Thread] Reloading config from: {:?}", config_path);

                // Load and parse config file
                match load_config_file(config_path) {
                    Ok(new_config) => {
                        // Validate config before queuing
                        match validate_plugin_configs(&new_config.plugins) {
                            Ok(_) => {
                                log::debug!(
                                    "[Manager Thread] Config validated, enqueuing plugin update"
                                );
                                // Use SignalReload priority for explicit reloads
                                config_queue
                                    .enqueue(new_config.plugins, ConfigUpdatePriority::UserDirect);
                                ManagerResponse::Ok
                            }
                            Err(e) => {
                                log::warn!("[Manager Thread] Config validation failed: {}", e);
                                config_queue.metrics.record_rejection();
                                ManagerResponse::Error(format!("Config validation failed: {}", e))
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[Manager Thread] Config parse failed: {}", e);
                        ManagerResponse::Error(format!("Config parse failed: {}", e))
                    }
                }
            } else {
                log::debug!("[Manager Thread] No config path set, cannot reload config");
                ManagerResponse::Error("No config path configured".to_string())
            }
        }
        ManagerCommand::Shutdown => {
            log::debug!("[Manager Thread] Shutdown requested");

            if let Ok(mut state_guard) = safe_lock(state) {
                state_guard.playback_state = PlaybackState::Stopped;
            }

            // Signal threads to shutdown
            decoder.send_command(DecoderCommand::Shutdown).ok();
            processing.send_command(ProcessingCommand::Shutdown).ok();
            playback.send_command(PlaybackCommand::Shutdown).ok();

            ManagerResponse::Shutdown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        // Test ParseError display
        let err = ConfigError::ParseError {
            path: std::path::PathBuf::from("/test/config.yaml"),
            reason: "invalid syntax".to_string(),
        };
        assert!(err.to_string().contains("Failed to parse config"));
        assert!(err.to_string().contains("invalid syntax"));

        // Test ValidationError display
        let err = ConfigError::ValidationError {
            plugin_index: 2,
            reason: "unknown plugin type".to_string(),
        };
        assert!(err.to_string().contains("Plugin 2"));
        assert!(err.to_string().contains("unknown plugin type"));

        // Test TimeoutError display
        let err = ConfigError::TimeoutError { waited_ms: 5000 };
        assert!(err.to_string().contains("5000ms"));

        // Test ProcessingError display
        let err = ConfigError::ProcessingError {
            reason: "plugin init failed".to_string(),
        };
        assert!(err.to_string().contains("plugin init failed"));

        // Test UnexpectedResponse display
        let err = ConfigError::UnexpectedResponse;
        assert!(err.to_string().contains("Unexpected response"));

        // Test StateLockError display
        let err = ConfigError::StateLockError;
        assert!(err.to_string().contains("state lock"));

        // Test ChannelDisconnected display
        let err = ConfigError::ChannelDisconnected;
        assert!(err.to_string().contains("disconnected"));
    }

    #[test]
    fn test_config_error_is_error_trait() {
        let err: Box<dyn std::error::Error> =
            Box::new(ConfigError::TimeoutError { waited_ms: 100 });
        assert!(err.to_string().contains("100ms"));
    }

    #[test]
    fn test_validate_plugin_configs_valid() {
        let configs = vec![
            super::super::PluginConfig {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({"gain_db": -3.0}),
            },
            super::super::PluginConfig {
                plugin_type: "EQ".to_string(),
                parameters: serde_json::json!({"filters": []}),
            },
        ];
        assert!(validate_plugin_configs(&configs).is_ok());
    }

    #[test]
    fn test_validate_plugin_configs_unknown_type() {
        let configs = vec![super::super::PluginConfig {
            plugin_type: "unknown_plugin".to_string(),
            parameters: serde_json::json!({}),
        }];
        let result = validate_plugin_configs(&configs);
        assert!(result.is_err());
        if let Err(ConfigError::ValidationError {
            plugin_index,
            reason,
        }) = result
        {
            assert_eq!(plugin_index, 0);
            assert!(reason.contains("Unknown plugin type"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_validate_plugin_configs_missing_gain_db() {
        let configs = vec![super::super::PluginConfig {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"other": 1.0}),
        }];
        let result = validate_plugin_configs(&configs);
        assert!(result.is_err());
        if let Err(ConfigError::ValidationError { reason, .. }) = result {
            assert!(reason.contains("gain_db"));
        } else {
            panic!("Expected ValidationError");
        }
    }
}
