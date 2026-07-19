use super::super::{
    AudioEngineState, PlaybackCommand, PlaybackThread, ProcessingCommand, ProcessingThread,
};
use super::config_update_queue::ConfigUpdateQueue;
use super::consts::SPIN_MS_SLEEP_MANAGER;
use super::error::ConfigError;
use super::estimate::estimate_graph_update_timeout;
use super::estimate::estimate_update_timeout;
use super::wait::wait_for_plugin_chain_update;
use crate::engine::processing_thread::{
    build_plugin_graph_host_with_policy, build_plugin_host_with_policy,
};
use arc_swap::ArcSwap;
use sotf_plugins::PluginHost;
use sotf_types::{EngineOversamplingPolicy, PluginBuildDiagnostic};
use std::sync::Arc;

fn build_plugin_update_host_on_worker(
    plugins: Vec<super::super::PluginConfig>,
    sample_rate: u32,
    input_channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    std::thread::Builder::new()
        .name("sotf-plugin-host-builder".to_string())
        .spawn(move || {
            build_plugin_host_with_policy(
                &plugins,
                sample_rate,
                input_channels,
                oversampling_policy,
            )
        })
        .map_err(|e| {
            PluginBuildDiagnostic::host(format!("failed to spawn plugin host builder: {e}"))
        })?
        .join()
        .map_err(|_| PluginBuildDiagnostic::host("plugin host builder panicked"))?
}

fn build_plugin_graph_host_on_worker(
    graph_config: super::super::types::PluginGraphConfig,
    sample_rate: u32,
    input_channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    std::thread::Builder::new()
        .name("sotf-plugin-graph-builder".to_string())
        .spawn(move || {
            build_plugin_graph_host_with_policy(
                &graph_config,
                sample_rate,
                input_channels,
                oversampling_policy,
            )
        })
        .map_err(|e| {
            PluginBuildDiagnostic::host(format!("failed to spawn plugin graph builder: {e}"))
        })?
        .join()
        .map_err(|_| PluginBuildDiagnostic::host("plugin graph builder panicked"))?
}

pub(in crate::engine::manager_thread) fn store_plugin_build_diagnostics(
    state: &Arc<ArcSwap<AudioEngineState>>,
    diagnostics: Vec<PluginBuildDiagnostic>,
) {
    let mut new_state = (**state.load()).clone();
    new_state.plugin_build_diagnostics = diagnostics;
    state.store(Arc::new(new_state));
}

/// Apply a plugin update with proper synchronization and rollback on failure.
/// Waits for confirmation from processing thread and updates playback thread if needed.
#[allow(
    clippy::too_many_arguments,
    reason = "manager command handler: one argument per engine subsystem involved"
)]
pub(in crate::engine::manager_thread) fn apply_plugin_update(
    processing: &mut ProcessingThread,

    playback: &mut PlaybackThread,

    state: &Arc<ArcSwap<AudioEngineState>>,

    config_queue: &mut ConfigUpdateQueue,

    plugins: Vec<super::super::PluginConfig>,

    sample_rate: u32,

    input_channels: usize,

    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(), ConfigError> {
    log::debug!(
        "[Manager Thread] apply_plugin_update: Starting update with {} plugins at {}Hz",
        plugins.len(),
        sample_rate
    );

    let start_build = std::time::Instant::now();

    log::debug!("[Manager Thread] Building plugin host on worker thread...");

    let (host, build_diagnostics) = match build_plugin_update_host_on_worker(
        plugins.clone(),
        sample_rate,
        input_channels,
        oversampling_policy,
    ) {
        Ok(result) => result,
        Err(diagnostic) => {
            log::error!("[Manager Thread] Worker build failed: {}", diagnostic);
            store_plugin_build_diagnostics(state, vec![diagnostic.clone()]);
            return Err(ConfigError::PluginBuild { diagnostic });
        }
    };

    for diagnostic in &build_diagnostics {
        log::warn!("[Manager Thread] {}", diagnostic);
    }
    // Surface build diagnostics in their dedicated state field. A clean build
    // deliberately clears diagnostics from the previous attempt without
    // disturbing the independently-owned general engine error.
    store_plugin_build_diagnostics(state, build_diagnostics);

    log::debug!(
        "[Manager Thread] Worker build successful in {:?}, output channels: {}",
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
                super::super::ProcessingResponse::PluginData(_)
                | super::super::ProcessingResponse::Ok => {
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
                super::super::ProcessingResponse::PluginChainUpdated {
                    output_channels,
                    latency_samples,
                } => {
                    log::debug!(
                        "[Manager Thread] ACK received: Plugin chain updated, output_channels={}, latency={}",
                        output_channels,
                        latency_samples
                    );

                    let old_channels = state.load().num_channels;

                    {
                        let mut new_state = (**state.load()).clone();
                        new_state.num_channels = output_channels;
                        new_state.plugin_latency_samples = latency_samples;
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

                super::super::ProcessingResponse::Error(e) => {
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

/// Build a DawHost from a graph config and convert to a linear `Vec<PluginConfig>` isn't
/// possible for graph topologies. Instead, build the host directly and send it to the
/// processing thread. This reuses the same host-swap mechanism as `apply_plugin_update`.
pub(in crate::engine::manager_thread) fn apply_plugin_graph_update(
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<ArcSwap<AudioEngineState>>,
    graph_config: super::super::types::PluginGraphConfig,
    sample_rate: u32,
    input_channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(), ConfigError> {
    log::debug!(
        "[Manager Thread] apply_plugin_graph_update: {} nodes, {} edges at {}Hz",
        graph_config.nodes.len(),
        graph_config.edges.len(),
        sample_rate
    );

    let (host, build_diagnostics) = match build_plugin_graph_host_on_worker(
        graph_config.clone(),
        sample_rate,
        input_channels,
        oversampling_policy,
    ) {
        Ok(result) => result,
        Err(diagnostic) => {
            log::error!("[Manager Thread] Graph build failed: {}", diagnostic);
            store_plugin_build_diagnostics(state, vec![diagnostic.clone()]);
            return Err(ConfigError::PluginBuild { diagnostic });
        }
    };

    for diagnostic in &build_diagnostics {
        log::warn!("[Manager Thread] {}", diagnostic);
    }
    store_plugin_build_diagnostics(state, build_diagnostics);

    processing
        .send_command(ProcessingCommand::UpdateHost(Box::new(host)))
        .map_err(|_| ConfigError::ChannelDisconnected)?;

    let old_channels = state.load().num_channels;
    let timeout = estimate_graph_update_timeout(&graph_config);
    let (output_channels, latency_samples) = wait_for_plugin_chain_update(processing, timeout)?;

    let mut new_state = (**state.load()).clone();
    new_state.num_channels = output_channels;
    new_state.plugin_latency_samples = latency_samples;
    state.store(Arc::new(new_state));

    if output_channels != old_channels {
        playback
            .send_command(PlaybackCommand::UpdateChannels(output_channels))
            .map_err(|_| ConfigError::ChannelDisconnected)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    #[test]
    fn plugin_build_diagnostics_persist_warnings_and_clear_stale_values() {
        let stale = PluginBuildDiagnostic::host("stale build warning");
        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState {
            last_error: Some("output device disconnected".to_string()),
            plugin_build_diagnostics: vec![stale],
            ..AudioEngineState::default()
        }));

        store_plugin_build_diagnostics(&state, Vec::new());
        assert!(state.load().plugin_build_diagnostics.is_empty());
        assert_eq!(
            state.load().last_error.as_deref(),
            Some("output device disconnected")
        );

        let diagnostic = PluginBuildDiagnostic::graph_node(
            42,
            Some(91),
            "external",
            "External plugin `/missing/example.vst3` could not be loaded",
        );
        store_plugin_build_diagnostics(&state, vec![diagnostic.clone()]);
        assert_eq!(state.load().plugin_build_diagnostics, vec![diagnostic]);
        assert_eq!(
            state.load().last_error.as_deref(),
            Some("output device disconnected")
        );
    }

    #[test]
    fn failed_graph_candidate_preserves_working_host_and_engine_snapshot() {
        let (mut processing, processing_commands) = ProcessingThread::command_probe();
        let (mut playback, playback_commands) = PlaybackThread::command_probe();
        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState {
            num_channels: 6,
            plugin_latency_samples: 321,
            last_error: Some("existing device diagnostic".to_string()),
            ..AudioEngineState::default()
        }));
        let graph = PluginGraphConfig::try_new(
            vec![
                PluginGraphNodeConfig::try_new(7, "gain", serde_json::json!({ "gain_db": 0.0 }), 2)
                    .unwrap(),
                PluginGraphNodeConfig::try_new(
                    42,
                    "definitely-not-a-real-plugin",
                    serde_json::json!({}),
                    2,
                )
                .unwrap(),
                PluginGraphNodeConfig::try_new(
                    99,
                    "gain",
                    serde_json::json!({ "gain_db": -6.0 }),
                    2,
                )
                .unwrap(),
            ],
            vec![
                PluginGraphEdgeConfig::new(7, 42),
                PluginGraphEdgeConfig::new(42, 99),
            ],
        )
        .unwrap();

        let error = apply_plugin_graph_update(
            &mut processing,
            &mut playback,
            &state,
            graph,
            48_000,
            2,
            EngineOversamplingPolicy::PluginPreferred,
        )
        .expect_err("a requested graph node failure must abort the candidate");

        assert!(error.to_string().contains("node 42"), "{error}");
        assert!(
            matches!(
                processing_commands.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ),
            "a failed candidate must not replace the processing host"
        );
        assert!(
            matches!(
                playback_commands.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ),
            "a failed candidate must not reconfigure playback"
        );
        let current = state.load();
        assert_eq!(current.num_channels, 6);
        assert_eq!(current.plugin_latency_samples, 321);
        assert_eq!(
            current.last_error.as_deref(),
            Some("existing device diagnostic")
        );
        assert_eq!(current.plugin_build_diagnostics.len(), 1);
        assert!(matches!(
            current.plugin_build_diagnostics[0].target,
            sotf_types::PluginBuildTarget::GraphNode { node_id: 42 }
        ));
    }
}
