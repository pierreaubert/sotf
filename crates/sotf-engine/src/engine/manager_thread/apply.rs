use super::super::{AudioEngineState, PlaybackThread, PreparedHostUpdate, ProcessingThread};
use super::config_update_queue::ConfigUpdateQueue;
use super::error::ConfigError;
use super::estimate::estimate_graph_update_timeout;
use super::estimate::estimate_update_timeout;
use super::wait::wait_for_plugin_chain_update;
use crate::engine::processing_thread::{
    build_plugin_graph_host_with_policy, build_plugin_host_with_policy,
};
use crate::{EngineOversamplingPolicy, PluginBuildDiagnostic};
use arc_swap::ArcSwap;
use sotf_plugins::PluginHost;
use std::sync::Arc;

const HOST_BUILD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// The processing thread validates the host snapshot at the block boundary.
// A host update from another manager-side source can win between the state
// snapshot and that commit; rebuild against the new snapshot a small number
// of times instead of surfacing this recoverable race to the caller.
const MAX_STALE_HOST_UPDATE_RETRIES: usize = 2;

fn is_stale_host_update(error: &ConfigError) -> bool {
    matches!(
        error,
        ConfigError::ProcessingError { reason }
            if reason == "stale prepared host update: active host changed before commit"
    )
}

fn build_plugin_update_host_on_worker(
    plugins: Vec<super::super::PluginConfig>,
    sample_rate: u32,
    input_channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("sotf-plugin-host-builder".to_string())
        .spawn(move || {
            let result = build_plugin_host_with_policy(
                &plugins,
                sample_rate,
                input_channels,
                oversampling_policy,
            );
            result_tx.send(result).ok();
        })
        .map_err(|e| {
            PluginBuildDiagnostic::host(format!("failed to spawn plugin host builder: {e}"))
        })?;
    result_rx
        .recv_timeout(HOST_BUILD_TIMEOUT)
        .map_err(|error| {
            PluginBuildDiagnostic::host(match error {
                std::sync::mpsc::RecvTimeoutError::Timeout => {
                    format!("plugin host build timed out after {:?}", HOST_BUILD_TIMEOUT)
                }
                std::sync::mpsc::RecvTimeoutError::Disconnected => {
                    "plugin host builder panicked".to_string()
                }
            })
        })?
}

fn build_plugin_graph_host_on_worker(
    graph_config: super::super::types::PluginGraphConfig,
    sample_rate: u32,
    input_channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("sotf-plugin-graph-builder".to_string())
        .spawn(move || {
            let result = build_plugin_graph_host_with_policy(
                &graph_config,
                sample_rate,
                input_channels,
                oversampling_policy,
            );
            result_tx.send(result).ok();
        })
        .map_err(|e| {
            PluginBuildDiagnostic::host(format!("failed to spawn plugin graph builder: {e}"))
        })?;
    result_rx
        .recv_timeout(HOST_BUILD_TIMEOUT)
        .map_err(|error| {
            PluginBuildDiagnostic::host(match error {
                std::sync::mpsc::RecvTimeoutError::Timeout => format!(
                    "plugin graph build timed out after {:?}",
                    HOST_BUILD_TIMEOUT
                ),
                std::sync::mpsc::RecvTimeoutError::Disconnected => {
                    "plugin graph builder panicked".to_string()
                }
            })
        })?
}

pub(in crate::engine::manager_thread) fn store_plugin_build_diagnostics(
    state: &Arc<ArcSwap<AudioEngineState>>,
    diagnostics: Vec<PluginBuildDiagnostic>,
) {
    let mut new_state = (**state.load()).clone();
    new_state.plugin_build_diagnostics = diagnostics;
    state.store(Arc::new(new_state));
}

/// Apply a plugin update with proper synchronization.
/// A failed candidate is never committed, so the active host remains unchanged.
/// Waits for confirmation from processing thread and updates playback thread if needed.
#[allow(
    clippy::too_many_arguments,
    reason = "manager command handler: one argument per engine subsystem involved"
)]
fn apply_plugin_update_once(
    processing: &mut ProcessingThread,

    playback: &mut PlaybackThread,

    state: &Arc<ArcSwap<AudioEngineState>>,

    config_queue: &mut ConfigUpdateQueue,

    plugins: Vec<super::super::PluginConfig>,

    sample_rate: u32,

    input_channels: usize,

    playback_channel_limit: usize,

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

    let current = state.load();
    let prepared = PreparedHostUpdate::prepare(
        host,
        sample_rate,
        current.num_channels,
        current.plugin_latency_samples,
    )
    .map_err(|message| ConfigError::PluginBuild {
        diagnostic: PluginBuildDiagnostic::host(message),
    })?;
    drop(current);
    let (generation, request_id, ticket) = processing.send_host_update(prepared).map_err(|_| {
        log::error!("[Manager Thread] Failed to send CommitHostUpdate command: disconnected");
        ConfigError::ChannelDisconnected
    })?;

    log::debug!(
        "[Manager Thread] apply_plugin_update: Sent CommitHostUpdate to processing thread, waiting for ACK..."
    );

    // Calculate adaptive timeout based on plugin complexity

    let timeout = estimate_update_timeout(&plugins);

    log::debug!("[Manager Thread] Using adaptive timeout: {:?}", timeout);

    let start = std::time::Instant::now();
    let (output_channels, output_sample_rate, latency_samples) =
        match wait_for_plugin_chain_update(processing, request_id, generation, &ticket, timeout) {
            Ok(metadata) => metadata,
            Err(error) => {
                config_queue.metrics.record_failure();
                return Err(error);
            }
        };

    let current = state.load();
    let old_playback_channels = current.playback_channels;
    let old_sample_rate = current.sample_rate;
    let playback_channels = if playback_channel_limit > 0 {
        output_channels.min(playback_channel_limit)
    } else {
        output_channels
    };
    let mut new_state = (**current).clone();
    drop(current);
    let (actual_playback_channels, actual_playback_sample_rate) =
        if playback_channels != old_playback_channels || output_sample_rate != old_sample_rate {
            match playback.reconfigure(output_sample_rate, playback_channels) {
                Ok(actual) => (actual.channels, actual.sample_rate),
                Err(error) => {
                    let reason = format!(
                        "Plugin host committed, but playback output reconfiguration failed: {error}"
                    );
                    new_state.num_channels = output_channels;
                    new_state.sample_rate = output_sample_rate;
                    new_state.plugin_latency_samples = latency_samples;
                    new_state.last_error = Some(reason.clone());
                    new_state.playback_state = crate::PlaybackState::Stopped;
                    state.store(Arc::new(new_state));
                    config_queue.metrics.record_failure();
                    return Err(ConfigError::ProcessingError { reason });
                }
            }
        } else {
            (old_playback_channels, old_sample_rate)
        };
    new_state.num_channels = output_channels;
    new_state.playback_channels = actual_playback_channels;
    new_state.sample_rate = actual_playback_sample_rate;
    new_state.plugin_latency_samples = latency_samples;
    state.store(Arc::new(new_state));

    config_queue.metrics.record_success(start.elapsed());
    Ok(())
}

/// Retry a host replacement when its control-thread snapshot becomes stale
/// before the processing thread reaches its commit boundary.
pub(in crate::engine::manager_thread) fn apply_plugin_update(
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<ArcSwap<AudioEngineState>>,
    config_queue: &mut ConfigUpdateQueue,
    plugins: Vec<super::super::PluginConfig>,
    sample_rate: u32,
    input_channels: usize,
    playback_channel_limit: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(), ConfigError> {
    for attempt in 0..=MAX_STALE_HOST_UPDATE_RETRIES {
        match apply_plugin_update_once(
            processing,
            playback,
            state,
            config_queue,
            plugins.clone(),
            sample_rate,
            input_channels,
            playback_channel_limit,
            oversampling_policy,
        ) {
            Err(error)
                if is_stale_host_update(&error) && attempt < MAX_STALE_HOST_UPDATE_RETRIES =>
            {
                log::debug!(
                    "[Manager Thread] Prepared host became stale; retrying against the active host snapshot"
                );
            }
            result => return result,
        }
    }

    unreachable!("stale host update retry loop must return from the match");
}

/// Build a DawHost from a graph config and convert to a linear `Vec<PluginConfig>` isn't
/// possible for graph topologies. Instead, build the host directly and send it to the
/// processing thread. This reuses the same host-swap mechanism as `apply_plugin_update`.
#[allow(
    clippy::too_many_arguments,
    reason = "graph update coordinates the manager-owned processing and playback subsystems"
)]
pub(in crate::engine::manager_thread) fn apply_plugin_graph_update(
    processing: &mut ProcessingThread,
    playback: &mut PlaybackThread,
    state: &Arc<ArcSwap<AudioEngineState>>,
    graph_config: super::super::types::PluginGraphConfig,
    sample_rate: u32,
    input_channels: usize,
    playback_channel_limit: usize,
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

    let current = state.load();
    let prepared = PreparedHostUpdate::prepare(
        host,
        sample_rate,
        current.num_channels,
        current.plugin_latency_samples,
    )
    .map_err(|message| ConfigError::PluginBuild {
        diagnostic: PluginBuildDiagnostic::host(message),
    })?;
    drop(current);
    let (generation, request_id, ticket) = processing
        .send_host_update(prepared)
        .map_err(|_| ConfigError::ChannelDisconnected)?;

    let current = state.load();
    let old_playback_channels = current.playback_channels;
    let old_sample_rate = current.sample_rate;
    drop(current);
    let timeout = estimate_graph_update_timeout(&graph_config);
    let (output_channels, output_sample_rate, latency_samples) =
        wait_for_plugin_chain_update(processing, request_id, generation, &ticket, timeout)?;
    let playback_channels = if playback_channel_limit > 0 {
        output_channels.min(playback_channel_limit)
    } else {
        output_channels
    };

    let mut new_state = (**state.load()).clone();
    let (actual_playback_channels, actual_playback_sample_rate) = if playback_channels
        != old_playback_channels
        || output_sample_rate != old_sample_rate
    {
        match playback.reconfigure(output_sample_rate, playback_channels) {
            Ok(actual) => (actual.channels, actual.sample_rate),
            Err(error) => {
                let reason = format!(
                    "Plugin graph committed, but playback output reconfiguration failed: {error}"
                );
                new_state.num_channels = output_channels;
                new_state.sample_rate = output_sample_rate;
                new_state.plugin_latency_samples = latency_samples;
                new_state.last_error = Some(reason.clone());
                new_state.playback_state = crate::PlaybackState::Stopped;
                state.store(Arc::new(new_state));
                return Err(ConfigError::ProcessingError { reason });
            }
        }
    } else {
        (old_playback_channels, old_sample_rate)
    };
    new_state.num_channels = output_channels;
    new_state.playback_channels = actual_playback_channels;
    new_state.sample_rate = actual_playback_sample_rate;
    new_state.plugin_latency_samples = latency_samples;
    state.store(Arc::new(new_state));

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
            crate::PluginBuildTarget::GraphNode { node_id: 42 }
        ));
    }
}
