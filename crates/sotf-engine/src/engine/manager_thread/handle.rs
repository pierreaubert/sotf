use super::super::{
    AudioEngineState, ConfigEvent, DecoderThread, EngineConfig, ManagerCommand, ManagerResponse,
    PlaybackState, PlaybackThread, ProcessingThread, ThreadEvent,
};
use super::commands;
use super::commands::ManagerCommandHandler;
use super::config_error::load_config_file;
use super::config_update_queue::ConfigUpdateQueue;
use super::types::ConfigUpdatePriority;
use super::validate::validate_plugin_configs;
use arc_swap::ArcSwap;
use std::sync::Arc;

/// Handle a thread event by dispatching it through the thread-event visitor.
pub(super) fn handle_thread_event(event: ThreadEvent, state: &Arc<ArcSwap<AudioEngineState>>) {
    super::thread_event_visitor::update_state_with_event(event, state);
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
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
            ThreadEvent::PlaybackOutputAccessChanged(
                sotf_types::OutputAccessStatus::ExclusiveActive,
            ),
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
    let mut ctx = commands::ManagerContext {
        decoder,
        processing,
        playback,
        state,
        config,
        config_queue,
    };

    match command {
        ManagerCommand::Play(source) => commands::PlayCommand(source).execute(&mut ctx),
        ManagerCommand::PlayAt(source, position) => {
            commands::PlayAtCommand(source, position).execute(&mut ctx)
        }
        ManagerCommand::Pause => commands::PauseCommand.execute(&mut ctx),
        ManagerCommand::Resume => commands::ResumeCommand.execute(&mut ctx),
        ManagerCommand::Stop => commands::StopCommand.execute(&mut ctx),
        ManagerCommand::Seek(position) => commands::SeekCommand(position).execute(&mut ctx),
        ManagerCommand::QueueNext(source) => commands::QueueNextCommand(source).execute(&mut ctx),
        ManagerCommand::CancelNext => commands::CancelNextCommand.execute(&mut ctx),
        ManagerCommand::SetVolume(volume) => commands::SetVolumeCommand(volume).execute(&mut ctx),
        ManagerCommand::Mute(muted) => commands::MuteCommand(muted).execute(&mut ctx),
        ManagerCommand::UpdatePluginChain(plugins) => {
            commands::UpdatePluginChainCommand(plugins).execute(&mut ctx)
        }
        ManagerCommand::UpdatePluginGraph(graph_config) => {
            commands::UpdatePluginGraphCommand(graph_config).execute(&mut ctx)
        }
        ManagerCommand::SetPluginParameter {
            plugin_index,
            param_id,
            value,
        } => commands::SetPluginParameterCommand {
            plugin_index,
            param_id,
            value,
        }
        .execute(&mut ctx),
        ManagerCommand::BypassProcessing(bypass) => {
            commands::BypassProcessingCommand(bypass).execute(&mut ctx)
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        ManagerCommand::MaintainIsolatedExternalPluginWorkers => {
            commands::MaintainIsolatedExternalPluginWorkersCommand.execute(&mut ctx)
        }
        ManagerCommand::GetState => commands::GetStateCommand.execute(&mut ctx),
        ManagerCommand::GetPosition => commands::GetPositionCommand.execute(&mut ctx),
        ManagerCommand::GetPluginData(index) => {
            commands::GetPluginDataCommand(index).execute(&mut ctx)
        }
        ManagerCommand::ReloadConfig => commands::ReloadConfigCommand.execute(&mut ctx),
        ManagerCommand::Shutdown => commands::ShutdownCommand.execute(&mut ctx),
    }
}
