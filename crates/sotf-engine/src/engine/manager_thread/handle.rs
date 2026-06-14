use super::super::{
    AudioEngineState, ConfigEvent, DecoderCommand, DecoderThread, EngineConfig, ManagerCommand,
    ManagerResponse, PlaybackCommand, PlaybackState, PlaybackThread, ProcessingCommand,
    ProcessingThread, ThreadEvent,
};
use super::apply::apply_plugin_graph_update;
use super::commands;
use super::commands::ManagerCommandHandler;
use super::config_error::load_config_file;
use super::config_update_queue::ConfigUpdateQueue;
use super::consts::DECODER_COMMAND_TIMEOUT_MS;
use super::consts::PROCESSING_COMMAND_TIMEOUT_MS;
use super::types::ConfigUpdatePriority;
use super::validate::validate_gapless_source_compatible;
use super::validate::validate_plugin_configs;
use super::wait::wait_for_decoder_ack;
use super::wait::wait_for_processing_ack;
use arc_swap::ArcSwap;
use std::sync::Arc;

/// Handle a thread event
pub(super) fn handle_thread_event(event: ThreadEvent, state: &Arc<ArcSwap<AudioEngineState>>) {
    match event {
        ThreadEvent::DecoderEndOfStream => {
            log::debug!("[Manager Thread] Decoder end of stream (waiting for playback drain)");
            // Don't set Stopped here - wait for PlaybackDrained so remaining
            // audio in the ring buffer gets played to hardware first.
        }
        ThreadEvent::DecoderGaplessTransition(source) => {
            log::info!(
                "[Manager Thread] Gapless transition to: {}",
                source.display_name()
            );
            let mut new_state = (**state.load()).clone();
            new_state.current_file = source.as_path().map(|p| p.to_path_buf());
            new_state.current_source = Some(source);
            new_state.position = 0.0;
            // playback_state stays Playing — no interruption
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackChannelsChanged(channels) => {
            let mut new_state = (**state.load()).clone();
            new_state.num_channels = channels;
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackOutputDeviceChanged(device_name) => {
            let mut new_state = (**state.load()).clone();
            new_state.playback_output_device = Some(device_name);
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackOutputAccessChanged(status) => {
            let mut new_state = (**state.load()).clone();
            new_state.output_access_status = status;
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackStats {
            callback_count,
            buffer_fill_percent,
            stream_error_count,
            frames_received,
            frames_written,
            frames_dropped,
            effective_sample_rate,
        } => {
            let mut new_state = (**state.load()).clone();
            new_state.playback_callback_count = callback_count;
            new_state.playback_buffer_fill_percent = buffer_fill_percent;
            new_state.playback_stream_error_count = stream_error_count;
            new_state.playback_frames_received = frames_received;
            new_state.playback_frames_written = frames_written;
            new_state.playback_frames_dropped = frames_dropped;
            new_state.playback_effective_sample_rate = effective_sample_rate;
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
        ThreadEvent::StreamMetadataChanged(stream_metadata) => {
            let mut new_state = (**state.load()).clone();
            new_state.stream_metadata = stream_metadata;
            state.store(Arc::new(new_state));
        }
        ThreadEvent::PlaybackUnderrun(underruns) => {
            let mut new_state = (**state.load()).clone();
            new_state.underruns = underruns;
            if underruns == 1 || (underruns <= 1000 && underruns.is_multiple_of(100)) {
                log::warn!("[Manager Thread] Playback underrun count: {}", underruns);
            } else if underruns.is_multiple_of(10000) {
                log::debug!("[Manager Thread] Playback underrun count: {}", underruns);
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
        ThreadEvent::ProcessingWarning(warning) => {
            log::warn!("[Manager Thread] Processing warning: {}", warning);
            let mut new_state = (**state.load()).clone();
            new_state.last_error = Some(warning);
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
                // Compensate for plugin chain latency: the decoder position
                // is ahead of actual playback by the total pipeline latency.
                // When processing is bypassed, audio passes through without
                // plugin processing, so effective latency is 0.
                let latency_sec = if new_state.sample_rate > 0
                    && new_state.latency_compensation_enabled
                    && !new_state.processing_bypassed
                {
                    new_state.plugin_latency_samples as f64 / new_state.sample_rate as f64
                } else {
                    0.0
                };
                new_state.position = (position - latency_sec).max(0.0);
                state.store(Arc::new(new_state));
            }
        }
        ThreadEvent::PluginLatencyUpdate(latency_samples) => {
            let mut new_state = (**state.load()).clone();
            let old_latency = new_state.plugin_latency_samples;
            new_state.plugin_latency_samples = latency_samples;
            // Adjust displayed position to compensate for the latency delta,
            // preventing a visible position jump when plugins change mid-stream.
            if new_state.sample_rate > 0
                && new_state.latency_compensation_enabled
                && old_latency != latency_samples
            {
                let delta_sec =
                    (latency_samples as f64 - old_latency as f64) / new_state.sample_rate as f64;
                new_state.position = (new_state.position - delta_sec).max(0.0);
            }
            state.store(Arc::new(new_state));
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        ThreadEvent::IsolatedExternalPluginWorkerStatuses(reports) => {
            let mut new_state = (**state.load()).clone();
            new_state.isolated_external_plugin_worker_statuses = reports;
            state.store(Arc::new(new_state));
        }
        ThreadEvent::SeekComplete => {
            log::debug!("[Manager Thread] Seek complete");
            let mut new_state = (**state.load()).clone();
            new_state.seeking = false;
            state.store(Arc::new(new_state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{AudioEngineState, PlaybackState, ThreadEvent};
    use super::handle_thread_event;
    use arc_swap::ArcSwap;
    use std::sync::Arc;

    #[test]
    fn handle_thread_event_all_variants_update_state() {
        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState {
            playback_state: PlaybackState::Playing,
            sample_rate: 48_000,
            plugin_latency_samples: 0,
            position: 10.0,
            ..AudioEngineState::default()
        }));

        // DecoderEndOfStream: no state change
        handle_thread_event(ThreadEvent::DecoderEndOfStream, &state);
        assert_eq!(state.load().playback_state, PlaybackState::Playing);

        // DecoderGaplessTransition: updates current file/source and resets position
        let source = crate::decoder::AudioSource::File(std::path::PathBuf::from("/gapless.wav"));
        handle_thread_event(ThreadEvent::DecoderGaplessTransition(source), &state);
        assert_eq!(
            state.load().current_file,
            Some(std::path::PathBuf::from("/gapless.wav"))
        );
        assert_eq!(state.load().position, 0.0);

        // PlaybackChannelsChanged
        handle_thread_event(ThreadEvent::PlaybackChannelsChanged(6), &state);
        assert_eq!(state.load().num_channels, 6);

        // PlaybackOutputDeviceChanged
        handle_thread_event(
            ThreadEvent::PlaybackOutputDeviceChanged("Test Device".to_string()),
            &state,
        );
        assert_eq!(
            state.load().playback_output_device.as_deref(),
            Some("Test Device")
        );

        // PlaybackOutputAccessChanged
        handle_thread_event(
            ThreadEvent::PlaybackOutputAccessChanged(sotf_types::OutputAccessStatus::ExclusiveActive),
            &state,
        );
        assert_eq!(
            state.load().output_access_status,
            sotf_types::OutputAccessStatus::ExclusiveActive
        );

        // PlaybackStats
        handle_thread_event(
            ThreadEvent::PlaybackStats {
                callback_count: 1,
                buffer_fill_percent: 50,
                stream_error_count: 2,
                frames_received: 100,
                frames_written: 99,
                frames_dropped: 1,
                effective_sample_rate: 48_000,
            },
            &state,
        );
        let s = state.load();
        assert_eq!(s.playback_callback_count, 1);
        assert_eq!(s.playback_buffer_fill_percent, 50);
        assert_eq!(s.playback_stream_error_count, 2);
        assert_eq!(s.playback_frames_received, 100);
        assert_eq!(s.playback_frames_written, 99);
        assert_eq!(s.playback_frames_dropped, 1);
        assert_eq!(s.playback_effective_sample_rate, 48_000);
        drop(s);

        // PlaybackDrained
        handle_thread_event(ThreadEvent::PlaybackDrained, &state);
        assert_eq!(state.load().playback_state, PlaybackState::Stopped);

        // Reset to Playing for error events
        state.store(Arc::new(AudioEngineState {
            playback_state: PlaybackState::Playing,
            ..AudioEngineState::default()
        }));

        // DecoderError
        handle_thread_event(ThreadEvent::DecoderError("decode fail".to_string()), &state);
        let s = state.load();
        assert_eq!(s.playback_state, PlaybackState::Stopped);
        assert_eq!(s.last_error.as_deref(), Some("decode fail"));
        drop(s);

        // StreamMetadataChanged
        let metadata = sotf_types::StreamMetadata {
            stream_title: Some("Title".to_string()),
            stream_url: None,
            content_type: Some("audio/mpeg".to_string()),
            bitrate_kbps: Some(320),
        };
        handle_thread_event(
            ThreadEvent::StreamMetadataChanged(Some(metadata.clone())),
            &state,
        );
        assert_eq!(state.load().stream_metadata, Some(metadata));

        // PlaybackUnderrun
        handle_thread_event(ThreadEvent::PlaybackUnderrun(42), &state);
        assert_eq!(state.load().underruns, 42);

        // ProcessingError
        state.store(Arc::new(AudioEngineState {
            playback_state: PlaybackState::Playing,
            ..AudioEngineState::default()
        }));
        handle_thread_event(ThreadEvent::ProcessingError("fatal".to_string()), &state);
        let s = state.load();
        assert_eq!(s.playback_state, PlaybackState::Stopped);
        assert_eq!(s.last_error.as_deref(), Some("fatal"));
        drop(s);

        // ProcessingWarning
        state.store(Arc::new(AudioEngineState {
            playback_state: PlaybackState::Playing,
            ..AudioEngineState::default()
        }));
        handle_thread_event(ThreadEvent::ProcessingWarning("warn".to_string()), &state);
        let s = state.load();
        assert_eq!(s.playback_state, PlaybackState::Playing);
        assert_eq!(s.last_error.as_deref(), Some("warn"));
        drop(s);

        // ThreadPanic
        state.store(Arc::new(AudioEngineState {
            playback_state: PlaybackState::Playing,
            ..AudioEngineState::default()
        }));
        handle_thread_event(ThreadEvent::ThreadPanic("worker".to_string()), &state);
        let s = state.load();
        assert_eq!(s.playback_state, PlaybackState::Stopped);
        assert_eq!(s.last_error.as_deref(), Some("Thread panicked: worker"));
        drop(s);

        // PositionUpdate
        state.store(Arc::new(AudioEngineState {
            playback_state: PlaybackState::Playing,
            sample_rate: 48_000,
            plugin_latency_samples: 4_800,
            latency_compensation_enabled: true,
            position: 0.0,
            ..AudioEngineState::default()
        }));
        handle_thread_event(ThreadEvent::PositionUpdate(5.0), &state);
        assert!((state.load().position - 4.9).abs() < 1e-9);

        // PluginLatencyUpdate
        state.store(Arc::new(AudioEngineState {
            playback_state: PlaybackState::Playing,
            sample_rate: 48_000,
            plugin_latency_samples: 0,
            latency_compensation_enabled: true,
            position: 10.0,
            ..AudioEngineState::default()
        }));
        handle_thread_event(ThreadEvent::PluginLatencyUpdate(4_800), &state);
        let s = state.load();
        assert_eq!(s.plugin_latency_samples, 4_800);
        assert!((s.position - 9.9).abs() < 1e-9);
        drop(s);

        // SeekComplete
        state.store(Arc::new(AudioEngineState {
            seeking: true,
            ..AudioEngineState::default()
        }));
        handle_thread_event(ThreadEvent::SeekComplete, &state);
        assert!(!state.load().seeking);

        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let status = sotf_types::IsolatedExternalPluginWorkerStatus {
                plugin_index: 0,
                node_id: 1,
                event: Some(sotf_types::IsolatedExternalPluginWorkerEvent::Started { pid: 123 }),
                error: None,
                worker_start_count: 1,
                worker_exit_count: 0,
                worker_launch_failure_count: 0,
                block_timeout_count: 0,
                block_worker_failure_count: 0,
                block_wrong_sequence_count: 0,
                sandbox_status: sotf_types::IsolatedExternalPluginSandboxStatus::Enforced,
                sandbox_backend:
                    sotf_types::IsolatedExternalPluginSandboxBackend::MacosProcessIsolation,
                sandbox_reason: None,
            };
            handle_thread_event(
                ThreadEvent::IsolatedExternalPluginWorkerStatuses(vec![status.clone()]),
                &state,
            );
            assert_eq!(
                state.load().isolated_external_plugin_worker_statuses,
                vec![status]
            );
        }
    }
}

/// Handle a config watcher event
/// Returns Ok(true) if shutdown requested, Ok(false) otherwise
pub(super) fn handle_config_event(
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

/// Handle a manager command
pub(super) fn handle_command(
    command: ManagerCommand,
    decoder: &mut DecoderThread,
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<ArcSwap<AudioEngineState>>,
    config: &EngineConfig,
    config_queue: &mut ConfigUpdateQueue,
) -> ManagerResponse {
    match command {
        ManagerCommand::Play(source) => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::PlayCommand(source).execute(&mut ctx)
        }
        ManagerCommand::PlayAt(source, position) => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::PlayAtCommand(source, position).execute(&mut ctx)
        }
        ManagerCommand::Pause => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::PauseCommand.execute(&mut ctx)
        }
        ManagerCommand::Resume => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::ResumeCommand.execute(&mut ctx)
        }
        ManagerCommand::Stop => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::StopCommand.execute(&mut ctx)
        }
        ManagerCommand::Seek(position) => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::SeekCommand(position).execute(&mut ctx)
        }
        ManagerCommand::QueueNext(source) => {
            log::debug!("[Manager Thread] QueueNext: {}", source.display_name());

            if let Err(e) = validate_gapless_source_compatible(&source, config.input_channels) {
                return ManagerResponse::Error(e);
            }

            if let Err(e) = decoder.send_command(DecoderCommand::QueueNext(source)) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => ManagerResponse::Ok,
                Err(e) => ManagerResponse::Error(e),
            }
        }
        ManagerCommand::CancelNext => {
            log::debug!("[Manager Thread] CancelNext");

            if let Err(e) = decoder.send_command(DecoderCommand::CancelNext) {
                return ManagerResponse::Error(e);
            }

            match wait_for_decoder_ack(
                decoder,
                std::time::Duration::from_millis(DECODER_COMMAND_TIMEOUT_MS),
            ) {
                Ok(()) => ManagerResponse::Ok,
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

            // Best-effort: the playback thread may have already exited after
            // end-of-stream drain. The volume is stored in state and will be
            // applied when the next engine starts.
            if let Err(e) = playback.send_command(PlaybackCommand::SetVolume(volume)) {
                log::debug!(
                    "[Manager Thread] SetVolume send failed (playback ended): {}",
                    e
                );
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
                log::debug!("[Manager Thread] Mute send failed (playback ended): {}", e);
            }

            ManagerResponse::Ok
        }
        ManagerCommand::UpdatePluginChain(plugins) => {
            let mut ctx = commands::ManagerContext {
                decoder,
                processing,
                playback,
                state,
                config,
                config_queue,
            };
            commands::UpdatePluginChainCommand(plugins).execute(&mut ctx)
        }
        ManagerCommand::UpdatePluginGraph(graph_config) => {
            log::debug!(
                "[Manager Thread] Update plugin graph ({} nodes, {} edges)",
                graph_config.nodes.len(),
                graph_config.edges.len()
            );

            match apply_plugin_graph_update(
                processing,
                playback,
                state,
                graph_config,
                config.output_sample_rate,
                config.input_channels,
                config.oversampling_policy,
            ) {
                Ok(()) => {
                    let mut new_state = (**state.load()).clone();
                    new_state.last_error = None;
                    state.store(Arc::new(new_state));
                    ManagerResponse::Ok
                }
                Err(e) => {
                    let message = e.to_string();
                    let mut new_state = (**state.load()).clone();
                    new_state.last_error = Some(message.clone());
                    state.store(Arc::new(new_state));
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
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        ManagerCommand::MaintainIsolatedExternalPluginWorkers => {
            log::trace!("[Manager Thread] Manual external plugin worker status poll requested");

            if let Err(e) =
                processing.send_command(ProcessingCommand::PollIsolatedExternalPluginWorkers)
            {
                return ManagerResponse::Error(e);
            }
            ManagerResponse::Ok
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
                        super::super::ProcessingResponse::PluginData(data) => {
                            return ManagerResponse::PluginData(data);
                        }
                        super::super::ProcessingResponse::Error(e) => {
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
            if let Err(e) = decoder.send_command(DecoderCommand::Shutdown) {
                log::trace!("[Manager Thread] Decoder shutdown command dropped: {}", e);
            }
            if let Err(e) = processing.send_command(ProcessingCommand::Shutdown) {
                log::trace!(
                    "[Manager Thread] Processing shutdown command dropped: {}",
                    e
                );
            }
            if let Err(e) = playback.send_command(PlaybackCommand::Shutdown) {
                log::trace!("[Manager Thread] Playback shutdown command dropped: {}", e);
            }

            ManagerResponse::Shutdown
        }
    }
}
