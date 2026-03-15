// ============================================================================
// Manager Thread - Coordination and Signal Handling
// ============================================================================
//
// Coordinates all worker threads, handles commands, and manages signals.

use super::{
    AudioEngineState, ConfigEvent, ConfigWatcher, DecoderCommand, DecoderThread, EngineConfig,
    GcThread, ManagerCommand, ManagerResponse, PlaybackCommand, PlaybackState, PlaybackThread,
    PluginDataCache, ProcessingCommand, ProcessingThread, ThreadEvent,
};
use crate::engine::processing_thread::build_plugin_host;
use arc_swap::ArcSwap;
use std::any::Any;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender, channel, sync_channel};

const SPIN_MS_SLEEP_MANAGER: u64 = 10;
const SPIN_MS_CHECK_MANAGER: u64 = 50;
const PLUGIN_INIT_TIMEOUT_MS: u64 = 10000; // 10 seconds for plugin initialization (SOFA loading can be slow)
const MAX_CONFIG_QUEUE_SIZE: usize = 5; // Maximum pending config updates
const PROCESSING_COMMAND_TIMEOUT_MS: u64 = 100;
const DECODER_COMMAND_TIMEOUT_MS: u64 = 1000;

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
    response_rx: Mutex<Receiver<ManagerResponse>>,
    state: Arc<ArcSwap<AudioEngineState>>,
    plugin_data_cache: PluginDataCache,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl ManagerThread {
    /// Create and start the manager thread
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = channel();
        let (response_tx, response_rx) = channel();

        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState::default()));
        let state_clone = Arc::clone(&state);

        let plugin_data_cache: PluginDataCache =
            Arc::new(arc_swap::ArcSwap::from_pointee(Vec::new()));
        let cache_clone = Arc::clone(&plugin_data_cache);

        let thread_handle = std::thread::Builder::new()
            .name("manager".to_string())
            .spawn(move || {
                if let Err(e) =
                    run_manager_thread(config, command_rx, response_tx, state_clone, cache_clone)
                {
                    log::debug!("[Manager Thread] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn manager thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_rx: Mutex::new(response_rx),
            state,
            plugin_data_cache,
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
            .lock()
            .map_err(|e| format!("Failed to lock response_rx: {}", e))?
            .recv()
            .map_err(|e| format!("Failed to receive response: {}", e))
    }

    /// Receive a response with timeout
    pub fn recv_response_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<ManagerResponse, String> {
        self.response_rx
            .lock()
            .map_err(|e| format!("Failed to lock response_rx: {}", e))?
            .recv_timeout(timeout)
            .map_err(|e| format!("Failed to receive response: {}", e))
    }

    /// Try to receive a response (non-blocking)
    pub fn try_recv_response(&self) -> Option<ManagerResponse> {
        self.response_rx.lock().ok()?.try_recv().ok()
    }

    /// Get current state (lock-free)
    pub fn get_state(&self) -> AudioEngineState {
        (**self.state.load()).clone()
    }

    /// Get cached plugin data directly (no command round-trip).
    /// The processing thread updates this cache after every frame.
    pub fn get_cached_plugin_data(&self, index: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        let cache = self.plugin_data_cache.load();
        cache.get(index).and_then(|slot| slot.clone())
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
    state: Arc<ArcSwap<AudioEngineState>>,
    plugin_data_cache: PluginDataCache,
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

    // Use bounded channels for recycling to avoid growth allocations.
    // Double capacity to ensure plenty of buffers are available for the entire pipeline.
    let (recycle_tx, recycle_rx) = sync_channel(queue_capacity * 2);
    let (decoder_recycle_tx, decoder_recycle_rx) = sync_channel(queue_capacity * 2);

    // Pre-fill recycle queues to avoid initial allocations in the hot path.
    // We use a safe upper bound for sample count: frame_size * max_channels.
    // Most plugins use 2-8 channels; 16 is a safe ceiling for most use cases.
    let prefill_samples = config.frame_size * 16;
    for _ in 0..queue_capacity * 2 {
        let _ = recycle_tx.send(vec![0.0; prefill_samples]);
        let _ = decoder_recycle_tx.send(vec![0.0; prefill_samples]);
    }

    // Create GC thread for off-audio-thread deallocation
    let mut gc_thread = GcThread::new();
    let gc_tx = gc_thread.sender();

    // Create threads
    let mut decoder_thread = DecoderThread::new(
        decoder_tx,
        event_tx.clone(),
        config.output_sample_rate,
        config.frame_size,
        decoder_recycle_rx,
    )?;

    let mut processing_thread = ProcessingThread::new(
        decoder_rx,
        processing_tx,
        event_tx.clone(),
        config.output_sample_rate,
        config.input_channels, // Use input channels, not output
        plugin_data_cache,
        gc_tx,
        recycle_rx,
        decoder_recycle_tx,
    )?;

    // Determine actual output channel count by loading plugin chain first
    let mut actual_output_channels = if !config.plugins.is_empty() {
        log::info!(
            "[Manager Thread] Loading initial plugin chain ({} plugins)...",
            config.plugins.len()
        );

        let start = std::time::Instant::now();
        // Build host locally to avoid blocking audio thread later
        let host_result = build_plugin_host(
            &config.plugins,
            config.output_sample_rate,
            config.input_channels,
        );

        match host_result {
            Ok(host) => {
                ensure_output_channel_capacity(
                    host.output_channels(),
                    config.output_channels,
                    config.output_device.as_deref(),
                )
                .map_err(|e| e.to_string())?;
                // Send host to processing thread
                if let Err(e) =
                    processing_thread.send_command(ProcessingCommand::UpdateHost(Box::new(host)))
                {
                    return Err(format!("Failed to send initial plugin host: {}", e));
                }

                // Wait for confirmation (should be fast as host is already built)
                // We still need to wait for ProcessingThread to acknowledge and update its state
                let mut output_channels_confirmed: Option<usize> = None;
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
                                output_channels_confirmed = Some(ch);
                                break;
                            }
                            super::ProcessingResponse::Error(e) => {
                                plugin_error = Some(e);
                                break;
                            }
                            _ => {
                                // Ignore other messages
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
                }

                if let Some(e) = plugin_error {
                    return Err(format!("Plugin chain initialization failed: {}", e));
                }

                match output_channels_confirmed {
                    Some(ch) => {
                        // Update state with actual channel count
                        {
                            let mut new_state = (**state.load()).clone();
                            new_state.num_channels = ch;
                            state.store(Arc::new(new_state));
                        }
                        ch
                    }
                    None => {
                        return Err("Plugin chain initialization timed out".to_string());
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to build initial plugin chain: {}", e));
            }
        }
    } else {
        config.output_channels
    };

    // Cap playback channels to config.output_channels (validated against device max by the app).
    // The playback thread has robust N→2 downmix code that handles the mismatch when the
    // processing chain outputs more channels than the hardware stream supports.
    if actual_output_channels > config.output_channels && config.output_channels > 0 {
        log::info!(
            "[Manager Thread] Clamping playback from {} to {} channels (config limit)",
            actual_output_channels,
            config.output_channels
        );
        actual_output_channels = config.output_channels;
    }

    log::warn!(
        "[Manager Thread] CREATING playback thread with {} channels",
        actual_output_channels
    );

    // Now create playback thread with the correct channel count
    let mut playback_thread = PlaybackThread::new(
        processing_rx,
        event_tx.clone(),
        config.output_sample_rate,
        config.buffer_ms,
        actual_output_channels,
        config.output_device.clone(),
        recycle_tx,
        config.allow_virtual_output,
    )?;

    // Set initial volume and mute
    playback_thread.send_command(PlaybackCommand::SetVolume(config.volume))?;
    playback_thread.send_command(PlaybackCommand::Mute(config.muted))?;

    // Start silent source mode (HAL playback: audio from shared memory, not file)
    if config.hal_mode {
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
                config.output_sample_rate,
                config.input_channels,
            ) {
                log::error!("[Manager Thread] Failed to apply config update: {}", e);
                let mut new_state = (**state.load()).clone();
                new_state.last_error = Some(e.to_string());
                state.store(Arc::new(new_state));
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
    gc_thread.shutdown(); // Last — other threads may still send garbage during shutdown

    log::debug!("[Manager Thread] Stopped");
    Ok(())
}

/// Handle a thread event
fn handle_thread_event(event: ThreadEvent, state: &Arc<ArcSwap<AudioEngineState>>) {
    match event {
        ThreadEvent::DecoderEndOfStream => {
            log::debug!("[Manager Thread] Decoder end of stream (waiting for playback drain)");
            // Don't set Stopped here - wait for PlaybackDrained so remaining
            // audio in the ring buffer gets played to hardware first.
        }
        ThreadEvent::PlaybackChannelsChanged(channels) => {
            let mut new_state = (**state.load()).clone();
            new_state.num_channels = channels;
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackDrained => {
            log::debug!("[Manager Thread] Playback drained - all audio played");
            let mut new_state = (**state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            new_state.last_error = None;
            state.store(Arc::new(new_state));
        }
        ThreadEvent::DecoderError(err) => {
            log::debug!("[Manager Thread] Decoder error: {}", err);
            let mut new_state = (**state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            new_state.last_error = Some(err);
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackUnderrun(underruns) => {
            let mut new_state = (**state.load()).clone();
            new_state.underruns = underruns;
            if underruns == 1 || underruns.is_multiple_of(100) {
                log::warn!(
                    "[Manager Thread] Playback underrun count: {}",
                    underruns
                );
            }
            state.store(Arc::new(new_state));
        }
        ThreadEvent::ProcessingError(err) => {
            log::debug!("[Manager Thread] Processing error: {}", err);
            let mut new_state = (**state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            new_state.last_error = Some(err);
            state.store(Arc::new(new_state));
        }
        ThreadEvent::ThreadPanic(thread_name) => {
            log::debug!("[Manager Thread] Thread panicked: {}", thread_name);
            let mut new_state = (**state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            new_state.last_error = Some(format!("Thread panicked: {}", thread_name));
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PositionUpdate(position) => {
            let current = state.load();
            if current.playback_state != PlaybackState::Stopped && !current.seeking {
                let mut new_state = (**current).clone();
                new_state.position = position;
                state.store(Arc::new(new_state));
            }
        }
        ThreadEvent::SeekComplete => {
            log::debug!("[Manager Thread] Seek complete");
            let mut new_state = (**state.load()).clone();
            new_state.seeking = false;
            state.store(Arc::new(new_state));
        }
    }
}

/// Handle a config watcher event
/// Returns Ok(true) if shutdown requested, Ok(false) otherwise
fn handle_config_event(
    event: ConfigEvent,
    config: &EngineConfig,
    config_queue: &mut ConfigUpdateQueue,
    state: &Arc<ArcSwap<AudioEngineState>>,
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
            let mut new_state = (**state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            state.store(Arc::new(new_state));

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

fn wait_for_processing_ack(
    processing: &ProcessingThread,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = processing.try_recv_response() {
            match response {
                super::ProcessingResponse::Ok => return Ok(()),
                super::ProcessingResponse::Error(e) => return Err(e),
                _ => continue,
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err(format!(
        "Timed out waiting for processing thread acknowledgment after {}ms",
        timeout.as_millis()
    ))
}

fn wait_for_decoder_ack(
    decoder: &DecoderThread,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = decoder.try_recv_response() {
            return match response {
                super::DecoderResponse::Ok => Ok(()),
                super::DecoderResponse::Error(e) => Err(e),
            };
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err(format!(
        "Timed out waiting for decoder thread acknowledgment after {}ms",
        timeout.as_millis()
    ))
}

fn ensure_output_channel_capacity(
    required_output_channels: usize,
    configured_output_channels: usize,
    output_device: Option<&str>,
) -> Result<(), ConfigError> {
    if configured_output_channels > 0 && required_output_channels > configured_output_channels {
        let reason = match output_device {
            Some(device) => format!(
                "Plugin chain requires {} output channels, but output device '{}' is configured for {} channels. Disable the upmixer or choose a compatible output device.",
                required_output_channels, device, configured_output_channels
            ),
            None => format!(
                "Plugin chain requires {} output channels, but the current output is configured for {} channels. Disable the upmixer or choose a compatible output device.",
                required_output_channels, configured_output_channels
            ),
        };
        return Err(ConfigError::ProcessingError { reason });
    }

    Ok(())
}

/// Apply a plugin update with proper synchronization and rollback on failure.
/// Waits for confirmation from processing thread and updates playback thread if needed.
fn apply_plugin_update(
    processing: &mut ProcessingThread,

    playback: &mut PlaybackThread,

    state: &Arc<ArcSwap<AudioEngineState>>,

    config_queue: &mut ConfigUpdateQueue,

    plugins: Vec<super::PluginConfig>,

    sample_rate: u32,

    input_channels: usize,
) -> Result<(), ConfigError> {
    log::debug!(
        "[Manager Thread] apply_plugin_update: Starting update with {} plugins at {}Hz",
        plugins.len(),
        sample_rate
    );

    // Build the plugin host locally (this blocks ManagerThread, preventing UI updates but saving audio thread)

    // In a future improvement, this could be done in a spawned thread, but we need to handle the result asynchronously.

    let start_build = std::time::Instant::now();

    log::debug!("[Manager Thread] Building plugin host locally...");

    let host = build_plugin_host(&plugins, sample_rate, input_channels).map_err(|e| {
        log::error!("[Manager Thread] Local build failed: {}", e);

        ConfigError::ProcessingError { reason: e }
    })?;

    log::debug!(
        "[Manager Thread] Local build successful in {:?}, output channels: {}",
        start_build.elapsed(),
        host.output_channels()
    );

    // Send update command to processing thread

    processing
        .send_command(ProcessingCommand::UpdateHost(Box::new(host)))
        .map_err(|_| {
            log::error!("[Manager Thread] Failed to send UpdateHost command: disconnected");
            ConfigError::ChannelDisconnected
        })?;

    log::debug!(
        "[Manager Thread] apply_plugin_update: Sent UpdateHost to processing thread, waiting for ACK..."
    );

    // Calculate adaptive timeout based on plugin complexity

    let timeout = estimate_update_timeout(&plugins);

    log::debug!("[Manager Thread] Using adaptive timeout: {:?}", timeout);

    let start = std::time::Instant::now();

    let mut loop_count = 0;

    let mut _skipped_responses = 0;

    while start.elapsed() < timeout {
        loop_count += 1;

        if let Some(response) = processing.try_recv_response() {
            log::debug!(
                "[Manager Thread] Received response from processing thread after {:?} (loop {})",
                start.elapsed(),
                loop_count
            );

            match &response {
                super::ProcessingResponse::PluginData(_) | super::ProcessingResponse::Ok => {
                    _skipped_responses += 1;
                    log::trace!(
                        "[Manager Thread] Skipping unrelated response: {:?}",
                        response
                    );
                    continue;
                }

                _ => {}
            }

            match response {
                super::ProcessingResponse::PluginChainUpdated { output_channels } => {
                    log::debug!(
                        "[Manager Thread] ACK received: Plugin chain updated, output_channels={}",
                        output_channels
                    );

                    let old_channels = state.load().num_channels;

                    {
                        let mut new_state = (**state.load()).clone();
                        new_state.num_channels = output_channels;
                        state.store(Arc::new(new_state));
                    }

                    if output_channels != old_channels {
                        log::info!(
                            "[Manager Thread] Channel count changed ({} -> {}), updating playback thread",
                            old_channels,
                            output_channels
                        );
                        playback
                            .send_command(PlaybackCommand::UpdateChannels(output_channels))
                            .map_err(|_| ConfigError::ChannelDisconnected)?;
                    }

                    config_queue.save_working_config(plugins);

                    config_queue.metrics.record_success(start.elapsed());

                    log::debug!("[Manager Thread] apply_plugin_update completed successfully");
                    return Ok(());
                }

                super::ProcessingResponse::Error(e) => {
                    log::error!("[Manager Thread] Processing thread reported error: {}", e);

                    config_queue.metrics.record_failure();

                    return Err(ConfigError::ProcessingError { reason: e });
                }

                _ => {
                    log::error!("[Manager Thread] Unexpected response type from processing thread");
                    return Err(ConfigError::UnexpectedResponse);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    log::error!(
        "[Manager Thread] apply_plugin_update: TIMEOUT after {} loops, {:?}",
        loop_count,
        start.elapsed()
    );

    Err(ConfigError::TimeoutError {
        waited_ms: timeout.as_millis() as u64,
    })
}

// Actually, I'll do it in one go if possible.

// `run_manager_thread` has `config`.

// `apply_plugin_update` is called in `run_manager_thread`.

// Let's replace `apply_plugin_update` first.

/// Validate plugin configurations before applying
fn validate_plugin_configs(configs: &[super::PluginConfig]) -> Result<(), ConfigError> {
    log::debug!(
        "[Manager Thread] Starting validation of {} plugins",
        configs.len()
    );

    for (i, config) in configs.iter().enumerate() {
        log::debug!(
            "[Manager Thread] Validating plugin {}: type='{}', params={}",
            i,
            config.plugin_type,
            config.parameters
        );

        // Check if plugin type is recognized (case-insensitive)
        let valid_types = [
            "eq",
            "gain",
            "upmixer",
            "compressor",
            "gate",
            "limiter",
            "expander",
            "multiband_compressor",
            "multiband_expander",
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
            "xtc",
            "fletcher_munson",
            "denoiser",
            "pnd",
            "ab_compare",
            "band_split",
            "band_merge",
            "downmix",
            "mono_to_stereo",
            "crossfeed",
        ];

        let plugin_type_lower = config.plugin_type.to_lowercase();
        if !valid_types.contains(&plugin_type_lower.as_str()) {
            log::error!(
                "[Manager Thread] Validation failed: Unknown plugin type '{}'",
                config.plugin_type
            );
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason: format!("Unknown plugin type '{}'", config.plugin_type),
            });
        }

        // Validate that parameters exist
        if config.parameters.is_null() {
            log::error!(
                "[Manager Thread] Validation failed: Plugin '{}' missing parameters",
                config.plugin_type
            );
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason: format!("Plugin '{}' missing parameters", config.plugin_type),
            });
        }

        // Type-specific validation (case-insensitive)
        match plugin_type_lower.as_str() {
            "eq" => {
                // Validate EQ filter structure
                if let Some(filters) = config.parameters.get("filters") {
                    if !filters.is_array() {
                        log::error!(
                            "[Manager Thread] EQ validation failed: 'filters' must be an array"
                        );
                        return Err(ConfigError::ValidationError {
                            plugin_index: i,
                            reason: "Invalid 'filters' parameter (must be array)".to_string(),
                        });
                    }
                    log::debug!(
                        "[Manager Thread] EQ validated with {} filters",
                        filters.as_array().unwrap().len()
                    );
                }
            }
            "gain" => {
                // Validate gain_db exists
                if let Some(gain) = config.parameters.get("gain_db") {
                    if !gain.is_number() {
                        log::error!(
                            "[Manager Thread] Gain validation failed: 'gain_db' must be a number"
                        );
                        return Err(ConfigError::ValidationError {
                            plugin_index: i,
                            reason: "'gain_db' must be a number".to_string(),
                        });
                    }
                } else {
                    log::error!("[Manager Thread] Gain validation failed: Missing 'gain_db'");
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
                    log::error!(
                        "[Manager Thread] Upmixer validation failed: 'mode' must be a string"
                    );
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

    log::debug!(
        "[Manager Thread] All {} plugins validated successfully",
        configs.len()
    );
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
    state: &Arc<ArcSwap<AudioEngineState>>,
    config: &EngineConfig,
    config_queue: &mut ConfigUpdateQueue,
) -> ManagerResponse {
    match command {
        ManagerCommand::Play(path) => {
            log::debug!("[Manager Thread] Play: {:?}", path);

            // Send to decoder
            if let Err(e) = decoder.send_command(DecoderCommand::Play(path.clone())) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.current_file = Some(path);
                    new_state.playback_state = PlaybackState::Playing;
                    new_state.position = 0.0;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::PlayAt(path, position) => {
            log::debug!("[Manager Thread] PlayAt: {:?} at {:.2}s", path, position);

            // Send to decoder
            if let Err(e) = decoder.send_command(DecoderCommand::PlayAt(path.clone(), position)) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.current_file = Some(path);
                    new_state.playback_state = PlaybackState::Playing;
                    new_state.position = position;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::Pause => {
            log::debug!("[Manager Thread] Pause");

            if let Err(e) = decoder.send_command(DecoderCommand::Pause) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.playback_state = PlaybackState::Paused;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::Resume => {
            log::debug!("[Manager Thread] Resume");

            if let Err(e) = decoder.send_command(DecoderCommand::Resume) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.playback_state = PlaybackState::Playing;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::Stop => {
            log::debug!("[Manager Thread] Stop");
            if let Err(e) = decoder.send_command(DecoderCommand::Stop) {
                return ManagerResponse::Error(e);
            }
            if let Err(e) = wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                return ManagerResponse::Error(e);
            }
            if let Err(e) = playback.send_command(PlaybackCommand::Stop) {
                return ManagerResponse::Error(e);
            }

            let mut new_state = (**state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            new_state.current_file = None;
            new_state.position = 0.0;
            new_state.seeking = false;
            state.store(Arc::new(new_state));

            ManagerResponse::Ok
        }
        ManagerCommand::Seek(position) => {
            log::debug!("[Manager Thread] Seek to {:.2}s", position);

            if let Err(e) = decoder.send_command(DecoderCommand::Seek(position)) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.position = position;
                    new_state.seeking = true;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::SetVolume(volume) => {
            log::debug!("[Manager Thread] Set volume: {:.2}", volume);

            {
                let mut new_state = (**state.load()).clone();
                new_state.volume = volume;
                state.store(Arc::new(new_state));
            }

            if let Err(e) = playback.send_command(PlaybackCommand::SetVolume(volume)) {
                return ManagerResponse::Error(e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::Mute(muted) => {
            log::debug!("[Manager Thread] Mute: {}", muted);

            {
                let mut new_state = (**state.load()).clone();
                new_state.muted = muted;
                state.store(Arc::new(new_state));
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
                log::warn!(
                    "[Manager Thread] Config update ALREADY IN PROGRESS - queuing (this may cause channel mismatch!)"
                );
                log::trace!(
                    "[Manager Thread] UpdatePluginChain: Queueing update (queue size before: {})",
                    config_queue.queue.len()
                );
                let queued = config_queue.enqueue(plugins, ConfigUpdatePriority::UserDirect);
                if queued {
                    log::warn!(
                        "[Manager Thread] UpdatePluginChain: Update QUEUED (not applied immediately)"
                    );
                    return ManagerResponse::Ok;
                } else {
                    log::warn!(
                        "[Manager Thread] UpdatePluginChain: Failed to queue update (queue full)"
                    );
                    return ManagerResponse::Error("Plugin update queue is full".to_string());
                }
            }

            log::warn!(
                "[Manager Thread] UpdatePluginChain: Applying update immediately (not queued)"
            );

            // Otherwise, apply immediately using the synchronized apply function
            match apply_plugin_update(
                processing,
                playback,
                state,
                config_queue,
                plugins,
                config.output_sample_rate,
                config.input_channels,
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.last_error = None;
                    state.store(Arc::new(new_state));
                    log::trace!("[Manager Thread] UpdatePluginChain: Update applied successfully");
                    ManagerResponse::Ok
                }
                Err(e) => {
                    let message = e.to_string();
                    let mut new_state = (**state.load()).clone();
                    new_state.last_error = Some(message.clone());
                    state.store(Arc::new(new_state));
                    log::trace!("[Manager Thread] UpdatePluginChain: Update failed: {}", e);
                    ManagerResponse::Error(message)
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

            match wait_for_processing_ack(
                processing,
                std::time::Duration::from_millis(PROCESSING_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => ManagerResponse::Ok,
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::BypassProcessing(bypass) => {
            log::debug!("[Manager Thread] Bypass processing: {}", bypass);

            if let Err(e) = processing.send_command(ProcessingCommand::Bypass(bypass)) {
                return ManagerResponse::Error(e);
            }

            match wait_for_processing_ack(
                processing,
                std::time::Duration::from_millis(PROCESSING_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.processing_bypassed = bypass;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::GetState => ManagerResponse::State((**state.load()).clone()),
        ManagerCommand::GetPosition => ManagerResponse::Position(state.load().position),
        ManagerCommand::GetPluginData(index) => {
            if let Err(e) = processing.send_command(ProcessingCommand::GetPluginData(index)) {
                return ManagerResponse::Error(e);
            }

            // Wait for response from processing thread with timeout
            // GetPluginData is time-sensitive for UI, so we wait briefly
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_millis(100);

            loop {
                if let Some(response) = processing.try_recv_response() {
                    match response {
                        super::ProcessingResponse::PluginData(data) => {
                            return ManagerResponse::PluginData(data);
                        }
                        super::ProcessingResponse::Error(e) => {
                            return ManagerResponse::Error(e);
                        }
                        _ => {
                            // Ignore unexpected responses (e.g. from previous timed out requests)
                            continue;
                        }
                    }
                }

                if start.elapsed() > timeout {
                    return ManagerResponse::Error("Timeout waiting for plugin data".to_string());
                }

                std::thread::yield_now();
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

            {
                let mut new_state = (**state.load()).clone();
                new_state.playback_state = PlaybackState::Stopped;
                state.store(Arc::new(new_state));
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

    #[test]
    fn test_validate_plugin_configs_accepts_all_types() {
        use crate::plugins::{PluginSettings, PluginType};

        let sample_rate = 48000.0;

        for plugin_type in PluginType::all() {
            let settings = PluginSettings::default_for(&plugin_type);
            let config = settings.to_plugin_config(sample_rate);

            let result = validate_plugin_configs(std::slice::from_ref(&config));
            assert!(
                result.is_ok(),
                "validate_plugin_configs rejected '{}': {:?}",
                config.plugin_type,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn test_ensure_output_channel_capacity_rejects_incompatible_chain() {
        let result = ensure_output_channel_capacity(6, 2, Some("Built-in Output"));

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ProcessingError { reason } => {
                assert!(reason.contains("requires 6 output channels"));
                assert!(reason.contains("Built-in Output"));
                assert!(reason.contains("configured for 2 channels"));
            }
            other => panic!("Expected ProcessingError, got {:?}", other),
        }
    }

    #[test]
    fn test_ensure_output_channel_capacity_accepts_supported_chain() {
        let result = ensure_output_channel_capacity(2, 2, Some("Built-in Output"));

        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_thread_event_uses_actual_underrun_count() {
        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState::default()));

        handle_thread_event(ThreadEvent::PlaybackUnderrun(101), &state);

        assert_eq!(state.load().underruns, 101);

        handle_thread_event(ThreadEvent::PlaybackUnderrun(205), &state);

        assert_eq!(state.load().underruns, 205);
    }

    #[test]
    fn test_handle_thread_event_updates_playback_channels() {
        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState::default()));

        handle_thread_event(ThreadEvent::PlaybackChannelsChanged(6), &state);
        assert_eq!(state.load().num_channels, 6);

        handle_thread_event(ThreadEvent::PlaybackChannelsChanged(2), &state);
        assert_eq!(state.load().num_channels, 2);
    }
}
