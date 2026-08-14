use super::super::{
    AudioEngineState, ConfigWatcher, DecoderThread, EngineConfig, GcThread, ManagerResponse,
    PlaybackCommand, PlaybackThread, PluginDataCache, ProcessingCommand, ProcessingThread,
    ThreadEvent, plan_engine_features,
};
use super::apply::{apply_plugin_update, store_plugin_build_diagnostics};
use super::config_error::ensure_output_channel_capacity;
use super::config_update_metrics::ConfigUpdateMetrics;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::consts::EXTERNAL_PLUGIN_MAINTENANCE_INTERVAL_MS;
use super::consts::MAX_CONFIG_QUEUE_SIZE;
use super::consts::MAX_THREAD_EVENTS_PER_TICK;
use super::consts::PLUGIN_INIT_TIMEOUT_MS;
use super::consts::SPIN_MS_CHECK_MANAGER;
use super::handle::handle_command;
use super::handle::handle_config_event;
use super::handle::handle_thread_event;
use super::misc::initial_engine_state_from_config;
#[cfg(feature = "streaming")]
use super::misc::start_network_stream_server;
use super::types::ConfigUpdatePriority;
use super::types::PendingConfigUpdate;
use super::{ManagerReply, ManagerRequest};
use crate::engine::processing_thread::build_plugin_host_with_policy;
use crate::{DsdOutputStatus, OutputAccessStatus};
use arc_swap::ArcSwap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, sync_channel};

fn drain_thread_events(
    event_rx: &crossbeam::channel::Receiver<ThreadEvent>,
    state: &Arc<ArcSwap<AudioEngineState>>,
) -> usize {
    let mut handled = 0;
    while handled < MAX_THREAD_EVENTS_PER_TICK {
        let Ok(event) = event_rx.try_recv() else {
            break;
        };
        handle_thread_event(event, state);
        handled += 1;
    }
    handled
}

/// Config update queue manager
pub(in crate::engine::manager_thread) struct ConfigUpdateQueue {
    pub(in crate::engine::manager_thread) queue: VecDeque<PendingConfigUpdate>,
    pub(in crate::engine::manager_thread) update_in_progress: bool,
    pub(in crate::engine::manager_thread) metrics: ConfigUpdateMetrics,
}

impl ConfigUpdateQueue {
    pub(in crate::engine::manager_thread) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            update_in_progress: false,
            metrics: ConfigUpdateMetrics::new(),
        }
    }

    /// Add a config update to the queue with priority-based management
    /// Returns true if added, false if rejected
    pub(in crate::engine::manager_thread) fn enqueue(
        &mut self,
        plugins: Vec<super::super::PluginConfig>,
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
    pub(super) fn can_process_next(&self) -> bool {
        !self.update_in_progress && !self.queue.is_empty()
    }

    /// Start processing the next update in queue
    pub(super) fn start_processing(&mut self) -> Option<Vec<super::super::PluginConfig>> {
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
    pub(super) fn complete_processing(&mut self) {
        if self.update_in_progress {
            self.update_in_progress = false;
            log::debug!("[Manager Thread] Config update completed");
        }
    }

    /// Get current metrics
    #[allow(dead_code)]
    pub(super) fn get_metrics(&self) -> &ConfigUpdateMetrics {
        &self.metrics
    }

    /// Log metrics summary
    pub(super) fn log_metrics_summary(&self) {
        log::info!(
            "[Manager Thread] Config Update Metrics: {} total, {} success ({:.1}%), {} failed, {} rejected, avg {:.0}ms, max queue depth: {}",
            self.metrics.total_updates,
            self.metrics.successful_updates,
            self.metrics.success_rate() * 100.0,
            self.metrics.failed_updates,
            self.metrics.rejected_updates,
            self.metrics.avg_update_time_ms(),
            self.metrics.max_queue_depth
        );
    }
}

/// Main manager thread function
pub(super) fn run_manager_thread(
    config: EngineConfig,
    command_rx: Receiver<ManagerRequest>,
    response_tx: Sender<ManagerReply>,
    state: Arc<ArcSwap<AudioEngineState>>,
    plugin_data_cache: PluginDataCache,
) -> Result<(), String> {
    log::debug!("[Manager Thread] Starting with config: {:?}", config);
    state.store(Arc::new(initial_engine_state_from_config(&config)));

    let feature_plan = plan_engine_features(&config);
    let output_access_plan = &feature_plan.output_access;
    if config.output_access.requires_exclusive()
        && output_access_plan.status == OutputAccessStatus::Unsupported
    {
        return Err(output_access_plan.reason.clone().unwrap_or_else(|| {
            "Exclusive output is required, but no exclusive backend is available".to_string()
        }));
    }
    let dsd_output_plan = &feature_plan.dsd_output;
    if config.dsd_output.requires_bitstream_output()
        && matches!(
            dsd_output_plan.status,
            DsdOutputStatus::DopUnavailable | DsdOutputStatus::NativeUnavailable
        )
    {
        return Err(dsd_output_plan.reason.clone().unwrap_or_else(|| {
            "DSD bitstream output is required, but no DSD bitstream backend is available"
                .to_string()
        }));
    }

    // Create config update queue for serializing plugin updates
    let mut config_update_queue = ConfigUpdateQueue::new();

    // Create bounded queues for backpressure
    // Queue capacity based on buffer_ms to provide proper flow control
    let queue_capacity = config.queue_capacity_frames();
    log::debug!("[Manager Thread] Queue capacity: {} frames", queue_capacity);

    let (decoder_tx, decoder_rx) = sync_channel(queue_capacity);
    let (processing_tx, processing_rx) = sync_channel(queue_capacity);
    // Event delivery is bounded so a stalled manager cannot force allocations
    // on decoder/processing/playback threads. Producers use nonblocking sends.
    let (event_tx, event_rx) = crossbeam::channel::bounded(256);

    // Use bounded channels for recycling to avoid growth allocations.
    // Double capacity to ensure plenty of buffers are available for the entire pipeline.
    let (recycle_tx, recycle_rx) = sync_channel(queue_capacity * 2);
    let (decoder_recycle_tx, decoder_recycle_rx) = sync_channel(queue_capacity * 2);

    // Create GC thread for off-audio-thread deallocation
    let mut gc_thread = GcThread::new()?;
    let gc_tx = gc_thread.sender();

    #[cfg(feature = "streaming")]
    let mut network_stream_server = start_network_stream_server(&config, &state);
    #[cfg(feature = "streaming")]
    let network_stream_tap = network_stream_server
        .as_ref()
        .map(|(_, handle)| handle.clone());

    // Create threads
    let mut decoder_thread = DecoderThread::new(
        decoder_tx,
        event_tx.clone(),
        config.output_sample_rate,
        config.frame_size,
        decoder_recycle_rx,
        config.dsd_output,
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
        decoder_recycle_tx.clone(),
        #[cfg(feature = "streaming")]
        network_stream_tap,
    )?;

    // Pre-fill recycle queues to avoid initial allocations in the hot path.
    // This must happen after the decoder/processing threads are started so the
    // receivers exist, and it must be non-blocking because the threads may not
    // be consuming the queues yet.
    let prefill_samples = config.frame_size * 64;
    for _ in 0..queue_capacity * 4 {
        let _ = recycle_tx.try_send(vec![0.0; prefill_samples]);
        let _ = decoder_recycle_tx.try_send(vec![0.0; prefill_samples]);
    }

    // Determine actual output channel count by loading plugin chain first
    let mut actual_output_sample_rate = config.output_sample_rate;
    let mut actual_output_channels = if !config.plugins.is_empty() {
        log::info!(
            "[Manager Thread] Loading initial plugin chain ({} plugins)...",
            config.plugins.len()
        );

        // Build host locally to avoid blocking audio thread later
        let host_result = build_plugin_host_with_policy(
            &config.plugins,
            config.output_sample_rate,
            config.input_channels,
            config.oversampling_policy,
        );

        match host_result {
            Ok((host, build_diagnostics)) => {
                for diagnostic in &build_diagnostics {
                    log::warn!("[Manager Thread] Initial plugin load: {}", diagnostic);
                }
                store_plugin_build_diagnostics(&state, build_diagnostics);
                ensure_output_channel_capacity(
                    host.output_channels(),
                    config.output_channels,
                    config.output_device.as_deref(),
                )
                .map_err(|e| e.to_string())?;
                // Send host to processing thread
                let prepared = super::super::PreparedHostUpdate::prepare(
                    host,
                    config.output_sample_rate,
                    config.input_channels,
                    0,
                )?;
                let (generation, request_id, ticket) =
                    match processing_thread.send_host_update(prepared) {
                        Ok(ticket) => ticket,
                        Err(e) => {
                            return Err(format!("Failed to send initial plugin host: {}", e));
                        }
                    };

                // Start the commit deadline only after the potentially slow build.
                match super::wait::wait_for_plugin_chain_update(
                    &processing_thread,
                    request_id,
                    generation,
                    &ticket,
                    std::time::Duration::from_millis(PLUGIN_INIT_TIMEOUT_MS),
                ) {
                    Ok((ch, output_sample_rate, latency_samples)) => {
                        actual_output_sample_rate = output_sample_rate;
                        log::info!(
                            "[Manager Thread] Initial plugin chain loaded, output channels: {}, latency: {} samples",
                            ch,
                            latency_samples
                        );
                        // Update state with actual channel count
                        {
                            let mut new_state = (**state.load()).clone();
                            new_state.num_channels = ch;
                            new_state.sample_rate = output_sample_rate;
                            new_state.plugin_latency_samples = latency_samples;
                            state.store(Arc::new(new_state));
                        }
                        ch
                    }
                    Err(error) => {
                        return Err(format!("Plugin chain initialization failed: {error}"));
                    }
                }
            }
            Err(diagnostic) => {
                store_plugin_build_diagnostics(&state, vec![diagnostic.clone()]);
                return Err(format!(
                    "Failed to build initial plugin chain: {}",
                    diagnostic
                ));
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

    {
        let mut new_state = (**state.load()).clone();
        new_state.playback_channels = actual_output_channels;
        state.store(Arc::new(new_state));
    }

    log::info!(
        "[Manager Thread] CREATING playback thread with {} channels",
        actual_output_channels
    );

    // Now create playback thread with the correct channel count
    let mut playback_thread = PlaybackThread::new(
        processing_rx,
        event_tx.clone(),
        actual_output_sample_rate,
        config.buffer_ms,
        actual_output_channels,
        config.frame_size,
        config.output_device.clone(),
        recycle_tx,
        config.allow_virtual_output,
        config.output_access,
    )?;

    // Set initial volume and mute
    playback_thread.send_command(PlaybackCommand::SetVolume(config.volume))?;
    playback_thread.send_command(PlaybackCommand::Mute(config.muted))?;

    // Start silent source mode (driver playback: audio from driver, not file)
    if config.driver_mode {
        log::info!("[Manager Thread] Starting silent source mode for driver input");
        decoder_thread.send_command(super::super::DecoderCommand::StartSilentSource(
            config.input_channels.max(1),
        ))?;
    }

    // Setup config watcher if enabled
    let config_watcher = if config_watcher_enabled(&config) {
        match ConfigWatcher::new(config.config_path.clone(), config.watch_signals) {
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

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    let mut next_external_worker_maintenance = std::time::Instant::now()
        + std::time::Duration::from_millis(EXTERNAL_PLUGIN_MAINTENANCE_INTERVAL_MS);

    log::debug!("[Manager Thread] All threads started");

    let mut decoder_exit_reported = false;
    let mut processing_exit_reported = false;
    let mut playback_exit_reported = false;

    let mut shutdown_reply_id = None;

    // Main loop
    loop {
        for (finished, reported, name) in [
            (
                decoder_thread.is_finished(),
                &mut decoder_exit_reported,
                "decoder",
            ),
            (
                processing_thread.is_finished(),
                &mut processing_exit_reported,
                "processing",
            ),
            (
                playback_thread.is_finished(),
                &mut playback_exit_reported,
                "playback",
            ),
        ] {
            if finished && !*reported {
                *reported = true;
                log::error!("[Manager Thread] {name} worker exited unexpectedly");
                handle_thread_event(
                    ThreadEvent::ProcessingError(format!("{name} worker exited unexpectedly")),
                    &state,
                );
            }
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let now = std::time::Instant::now();
            if now >= next_external_worker_maintenance {
                if let Err(e) = processing_thread
                    .send_command(ProcessingCommand::PollIsolatedExternalPluginWorkers)
                {
                    log::trace!(
                        "[Manager Thread] Failed to enqueue external plugin maintenance command: {}",
                        e
                    );
                }
                next_external_worker_maintenance =
                    now + std::time::Duration::from_millis(EXTERNAL_PLUGIN_MAINTENANCE_INTERVAL_MS);
            }
        }

        // Drain bounded bursts. Audio threads publish position/stats faster
        // than the manager's 50 ms command wait, so handling only one event
        // per loop can indefinitely delay lifecycle events such as SeekComplete.
        drain_thread_events(&event_rx, &state);

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
                config.output_channels,
                config.oversampling_policy,
            ) {
                log::error!("[Manager Thread] Failed to apply config update: {}", e);
                if !e.is_plugin_build_error() {
                    // Avoid cloning the whole state when the same error is reported
                    // repeatedly. Only perform the in-place update when the error
                    // actually changes.
                    let error_string = e.to_string();
                    let current = state.load();
                    if current.last_error.as_deref() != Some(error_string.as_str()) {
                        drop(current);
                        super::state_helpers::update_engine_state(&state, |new_state| {
                            new_state.last_error = Some(error_string);
                        });
                    }
                }
            }
            config_update_queue.complete_processing();
        }

        // Check for commands (blocking with timeout)
        match command_rx.recv_timeout(std::time::Duration::from_millis(SPIN_MS_CHECK_MANAGER)) {
            Ok(request) => {
                let response = handle_command(
                    request.command,
                    &mut decoder_thread,
                    &mut processing_thread,
                    &mut playback_thread,
                    &state,
                    &config,
                    &mut config_update_queue,
                );

                let should_exit = matches!(response, ManagerResponse::Shutdown);
                if should_exit {
                    shutdown_reply_id = Some(request.id);
                    log::debug!("[Manager Thread] Shutdown requested; cleaning up workers");
                    break;
                }
                if let Err(e) = response_tx.send(ManagerReply {
                    id: request.id,
                    response,
                }) {
                    log::trace!("[Manager Thread] Response receiver dropped: {}", e);
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
    #[cfg(feature = "streaming")]
    if let Some((server, _)) = network_stream_server.as_mut() {
        server.shutdown();
    }
    gc_thread.shutdown(); // Last — other threads may still send garbage during shutdown

    if let Some(id) = shutdown_reply_id {
        let _ = response_tx.send(ManagerReply {
            id,
            response: ManagerResponse::Shutdown,
        });
    }

    log::debug!("[Manager Thread] Stopped");
    Ok(())
}

fn config_watcher_enabled(config: &super::super::EngineConfig) -> bool {
    config.watch_config || config.watch_signals
}

#[cfg(test)]
mod event_drain_tests {
    use super::*;

    #[test]
    fn event_bursts_are_drained_in_bounded_batches() {
        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState {
            seeking: true,
            ..AudioEngineState::default()
        }));
        let (event_tx, event_rx) = crossbeam::channel::bounded(MAX_THREAD_EVENTS_PER_TICK + 16);

        for position in 0..(MAX_THREAD_EVENTS_PER_TICK + 10) {
            event_tx
                .try_send(ThreadEvent::PositionUpdate(position as f64))
                .unwrap();
        }
        event_tx.try_send(ThreadEvent::SeekComplete).unwrap();

        assert_eq!(
            drain_thread_events(&event_rx, &state),
            MAX_THREAD_EVENTS_PER_TICK
        );
        assert!(state.load().seeking);
        assert_eq!(drain_thread_events(&event_rx, &state), 11);
        assert!(!state.load().seeking);
        assert_eq!(drain_thread_events(&event_rx, &state), 0);
    }

    #[test]
    fn signal_only_configuration_starts_the_watcher() {
        let signal_only = super::super::super::EngineConfig {
            watch_signals: true,
            watch_config: false,
            ..super::super::super::EngineConfig::default()
        };
        let file_only = super::super::super::EngineConfig {
            watch_signals: false,
            watch_config: true,
            ..super::super::super::EngineConfig::default()
        };

        assert!(config_watcher_enabled(&signal_only));
        assert!(config_watcher_enabled(&file_only));
        assert!(!config_watcher_enabled(
            &super::super::super::EngineConfig::default()
        ));
    }
}
