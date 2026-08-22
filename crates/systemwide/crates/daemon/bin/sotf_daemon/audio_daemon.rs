use super::command::Command;
use super::configured::configured_output_device_from_env;
use super::consts::LEGACY_SOCKET_PATH;
use super::consts::MAX_HAL_CHANNELS;
use super::consts::empty_loudness_json;
use super::consts::get_socket_path;
use super::consts::metering_source_json;
use super::driver_manager::{DriverManager, get_driver_status};
use super::loudness::loudness_data_to_json;
use super::loudness::loudness_info_to_json;
use super::misc::bind_unix_socket;
use super::misc::build_driver_plugin_chain;
use super::misc::is_safe_output_device_name;
use super::misc::list_audio_devices;
use super::misc::push_metering_faults;
use super::misc::socket_is_unix_socket;
use super::misc::transport_snapshot_and_faults;
use super::pipeline_spec::pipeline_spec_to_json;
use super::plugin::plugin_parameter_descriptors;
use super::plugin::plugin_type_category;
use super::plugin::plugin_type_to_engine_str;
use super::plugin_artifact::{PluginArtifactPlan, plan_plugin_artifact};
use super::response::Response;
use super::response::serialize_response_safely;
use super::security::{
    KeyManager, PeerClass, classify_peer, current_uid as security_current_uid,
    ensure_secure_socket_dir, peer_allows_command, validate_user_load_path,
    verify_peer_credentials,
};
use super::systemwide_state::SystemwideState;
use super::systemwide_state::spawn_driver_config_watcher;
use super::types::IpcLine;
use super::types::PipelinePlan;
use super::types::read_ipc_line_bounded;
use driver_common::DriverConfig;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};
use sotf_audio::manager::AudioEngineManager;
use sotf_audio::plugins::PluginType;
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use super::consts::MAX_IPC_CLIENTS;

/// Stable wire shape returned by `get_driver_config`/`get_hal_config`.
///
/// The daemon historically exposed configuration through a hand-built JSON
/// object with aliases such as `active` and `actual_sample_rate`. Keep those
/// aliases stable, but make the shape explicit so additions to
/// `DriverStatus` cannot silently change this separate endpoint.
#[derive(Debug, Serialize)]
struct DriverConfigWire {
    sample_rate: u32,
    actual_sample_rate: u32,
    buffer_frames: u32,
    actual_buffer_frames: u32,
    channel_count: u32,
    active: bool,
    driver_name: &'static str,
    driver_installed: bool,
    driver_ready: bool,
    platform_supported: bool,
}

impl From<&driver_common::DriverStatus> for DriverConfigWire {
    fn from(status: &driver_common::DriverStatus) -> Self {
        Self {
            sample_rate: status.sample_rate,
            actual_sample_rate: status.sample_rate,
            buffer_frames: status.buffer_frames,
            actual_buffer_frames: status.buffer_frames,
            channel_count: status.channel_count,
            active: status.capture_active,
            driver_name: status.driver_name,
            driver_installed: status.driver_installed,
            driver_ready: status.driver_ready,
            platform_supported: status.platform_supported,
        }
    }
}

pub(super) fn pipeline_timing_after_config_request(
    result: &driver_common::ConfigResult,
    requested_sample_rate: u32,
    requested_buffer_frames: u32,
) -> (u32, u32) {
    match result {
        driver_common::ConfigResult::Negotiated {
            actual_rate,
            actual_frames,
            ..
        } => (*actual_rate, *actual_frames),
        driver_common::ConfigResult::Accepted | driver_common::ConfigResult::Error(_) => {
            (requested_sample_rate, requested_buffer_frames)
        }
        _ => (requested_sample_rate, requested_buffer_frames),
    }
}

/// Return the node IDs of a graph when it is exactly one linear chain.
fn linear_graph_node_ids(graph: &PluginGraphConfig) -> Option<Vec<usize>> {
    if graph.nodes.is_empty() {
        return graph.edges.is_empty().then_some(Vec::new());
    }
    if graph.edges.len() != graph.nodes.len().saturating_sub(1) {
        return None;
    }

    let node_ids: HashSet<usize> = graph.nodes.iter().map(|node| node.id).collect();
    if node_ids.len() != graph.nodes.len() {
        return None;
    }

    let mut incoming = HashMap::<usize, usize>::with_capacity(graph.nodes.len());
    let mut outgoing = HashMap::<usize, usize>::with_capacity(graph.nodes.len());
    for &id in &node_ids {
        incoming.insert(id, 0);
    }
    for edge in &graph.edges {
        if !node_ids.contains(&edge.from_node) || !node_ids.contains(&edge.to_node) {
            return None;
        }
        *incoming.get_mut(&edge.to_node)? += 1;
        if outgoing.insert(edge.from_node, edge.to_node).is_some() {
            return None;
        }
    }

    let mut roots = incoming
        .iter()
        .filter_map(|(&id, &count)| (count == 0).then_some(id));
    let root = roots.next()?;
    if roots.next().is_some() {
        return None;
    }

    let mut order = Vec::with_capacity(graph.nodes.len());
    let mut current = Some(root);
    while let Some(id) = current {
        order.push(id);
        current = outgoing.get(&id).copied();
    }
    (order.len() == graph.nodes.len()).then_some(order)
}

/// Reorder a linear graph without changing node IDs, parameters, channel
/// counts, or bypass state. The order is expressed as node IDs, not positions.
pub(super) fn reorder_linear_graph(
    graph: &PluginGraphConfig,
    order: &[usize],
) -> Result<PluginGraphConfig, String> {
    graph
        .validate()
        .map_err(|error| format!("Invalid plugin graph: {error}"))?;
    let current_order = linear_graph_node_ids(graph)
        .ok_or_else(|| "Graph reorder requires a single linear graph".to_string())?;
    if order.len() != current_order.len() {
        return Err(format!(
            "Order length {} doesn't match graph node count {}",
            order.len(),
            current_order.len()
        ));
    }

    let expected: HashSet<usize> = current_order.iter().copied().collect();
    let mut seen = HashSet::with_capacity(order.len());
    for &id in order {
        if !expected.contains(&id) || !seen.insert(id) {
            return Err(format!(
                "Invalid graph order: duplicate or unknown node ID {id}"
            ));
        }
    }

    let nodes_by_id: HashMap<usize, &PluginGraphNodeConfig> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let Some(nodes) = order
        .iter()
        .map(|id| nodes_by_id.get(id).map(|node| (*node).clone()))
        .collect::<Option<Vec<_>>>()
    else {
        return Err("Invalid graph order: node lookup failed".to_string());
    };
    let edges = order
        .windows(2)
        .map(|pair| PluginGraphEdgeConfig::new(pair[0], pair[1]))
        .collect::<Vec<_>>();

    PluginGraphConfig::try_new(nodes, edges)
        .map_err(|error| format!("Reordered graph is invalid: {error}"))
}

/// Convert the legacy rack representation into a linear graph so per-node
/// channel and bypass state has a durable owner. Existing rack order and
/// plugin parameters are preserved; unspecified state uses the pipeline's
/// current input geometry and enabled-by-default behavior.
pub(super) fn rack_plugins_to_linear_graph(
    plugins: &[PluginConfig],
    pipeline_input_channels: usize,
    selected_index: usize,
    input_channels: Option<usize>,
    bypassed: Option<bool>,
) -> Result<PluginGraphConfig, String> {
    if input_channels.is_none() && bypassed.is_none() {
        return Err("set_rack_plugin_state requires input_channels or bypassed".to_string());
    }
    if selected_index >= plugins.len() {
        return Err(format!(
            "Plugin index {} out of range (have {})",
            selected_index,
            plugins.len()
        ));
    }
    if let Some(channels) = input_channels
        && !(1..=MAX_HAL_CHANNELS).contains(&channels)
    {
        return Err(format!(
            "Invalid plugin input channel count {}. Must be between 1 and {}.",
            channels, MAX_HAL_CHANNELS
        ));
    }

    let default_channels = pipeline_input_channels.max(1);
    let nodes = plugins
        .iter()
        .enumerate()
        .map(|(index, plugin)| PluginGraphNodeConfig {
            id: index,
            plugin_type: plugin.plugin_type.clone(),
            parameters: plugin.parameters.clone(),
            input_channels: if index == selected_index {
                input_channels.unwrap_or(default_channels)
            } else {
                default_channels
            },
            bypassed: if index == selected_index {
                bypassed.unwrap_or(false)
            } else {
                false
            },
        })
        .collect::<Vec<_>>();
    let edges = (0..nodes.len().saturating_sub(1))
        .map(|index| PluginGraphEdgeConfig::new(index, index + 1))
        .collect::<Vec<_>>();

    PluginGraphConfig::try_new(nodes, edges)
        .map_err(|error| format!("Rack state cannot be represented as a graph: {error}"))
}

pub(super) const METERING_LATENCY_BUDGET_MICROS: u64 = 5_000;
const PIPELINE_LATENCY_BUDGET_MICROS: u64 = 1_000_000;
const METERING_RESPONSE_BUDGET_BYTES: usize = 64 * 1024;
pub(super) const PIPELINE_RESPONSE_BUDGET_BYTES: usize = 256 * 1024;

#[derive(Debug, Default)]
struct OperationTelemetry {
    requests: AtomicU64,
    total_micros: AtomicU64,
    max_micros: AtomicU64,
    max_response_bytes: AtomicU64,
    budget_exceeded: AtomicU64,
}

impl OperationTelemetry {
    fn record(
        &self,
        elapsed: std::time::Duration,
        response_bytes: usize,
        latency_budget_micros: u64,
        response_budget_bytes: usize,
    ) {
        let elapsed_micros = elapsed.as_micros().min(u64::MAX as u128) as u64;
        let response_bytes = response_bytes.min(u64::MAX as usize) as u64;
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.total_micros
            .fetch_add(elapsed_micros, Ordering::Relaxed);
        self.max_micros.fetch_max(elapsed_micros, Ordering::Relaxed);
        self.max_response_bytes
            .fetch_max(response_bytes, Ordering::Relaxed);
        if elapsed_micros > latency_budget_micros || response_bytes > response_budget_bytes as u64 {
            self.budget_exceeded.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> Value {
        let requests = self.requests.load(Ordering::Relaxed);
        let total_micros = self.total_micros.load(Ordering::Relaxed);
        serde_json::json!({
            "requests": requests,
            "average_micros": total_micros.checked_div(requests).unwrap_or(0),
            "max_micros": self.max_micros.load(Ordering::Relaxed),
            "max_response_bytes": self.max_response_bytes.load(Ordering::Relaxed),
            "budget_exceeded": self.budget_exceeded.load(Ordering::Relaxed),
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct RuntimeTelemetry {
    metering: OperationTelemetry,
    pipeline_reload: OperationTelemetry,
}

impl RuntimeTelemetry {
    pub(super) fn record_command(
        &self,
        command: &str,
        elapsed: std::time::Duration,
        response_bytes: usize,
    ) {
        match command {
            "get_metering" | "get_loudness" => self.metering.record(
                elapsed,
                response_bytes,
                METERING_LATENCY_BUDGET_MICROS,
                METERING_RESPONSE_BUDGET_BYTES,
            ),
            "load_plugins"
            | "load_plugin_artifact"
            | "add_plugin"
            | "remove_plugin"
            | "update_plugin"
            | "reorder_plugins"
            | "reorder_graph_nodes"
            | "set_input_channels"
            | "set_output_channels"
            | "set_pipeline_channels" => self.pipeline_reload.record(
                elapsed,
                response_bytes,
                PIPELINE_LATENCY_BUDGET_MICROS,
                PIPELINE_RESPONSE_BUDGET_BYTES,
            ),
            _ => {}
        }
    }

    pub(super) fn snapshot(&self) -> Value {
        serde_json::json!({
            "metering": self.metering.snapshot(),
            "pipeline_reload": self.pipeline_reload.snapshot(),
            "budgets": {
                "metering_latency_micros": METERING_LATENCY_BUDGET_MICROS,
                "metering_response_bytes": METERING_RESPONSE_BUDGET_BYTES,
                "pipeline_latency_micros": PIPELINE_LATENCY_BUDGET_MICROS,
                "pipeline_response_bytes": PIPELINE_RESPONSE_BUDGET_BYTES,
            }
        })
    }
}

#[derive(Clone)]
pub(super) struct AudioDaemon {
    pub(super) manager: Arc<Mutex<AudioEngineManager>>,
    pub(super) running: Arc<Mutex<bool>>,
    pub(super) driver_manager: Arc<Mutex<DriverManager>>,
    /// Desired and applied systemwide daemon state.
    pub(super) system_state: Arc<Mutex<SystemwideState>>,
    /// Encryption key manager
    pub(super) key_manager: Arc<Mutex<KeyManager>>,
    /// Serializes read-modify-apply pipeline mutations across IPC clients.
    pub(super) pipeline_mutation: Arc<Mutex<()>>,
    /// Low-overhead IPC latency and serialized-size regression telemetry.
    pub(super) runtime_telemetry: Arc<RuntimeTelemetry>,
}

/// Try to reserve one bounded client-handler slot without taking a mutex in
/// the accept loop. The matching permit releases it when the handler exits.
pub(super) fn try_acquire_client_slot(active: &AtomicUsize) -> bool {
    loop {
        let current = active.load(Ordering::Acquire);
        if current >= MAX_IPC_CLIENTS {
            return false;
        }
        if active
            .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return true;
        }
    }
}

struct ClientSlot(Arc<AtomicUsize>);

impl Drop for ClientSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl AudioDaemon {
    pub(super) fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::new())),
            system_state: Arc::new(Mutex::new(SystemwideState::default())),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
            pipeline_mutation: Arc::new(Mutex::new(())),
            runtime_telemetry: Arc::new(RuntimeTelemetry::default()),
        }
    }

    pub(super) fn spawn_initial_driver_playback(&self) {
        let daemon = self.clone();
        std::thread::spawn(move || {
            println!("Auto-starting driver playback (2ch)...");

            let output_device = configured_output_device_from_env();
            println!("   Output device: {:?}", output_device);

            if let Some(device) = output_device {
                if let Err(e) = daemon
                    .system_state
                    .lock()
                    .set_desired_output_device(Some(device.clone()))
                {
                    println!("   Ignoring configured output device {:?}: {}", device, e);
                }
            } else {
                println!("   No output device override; playback thread will choose a safe device");
            }

            let plugins: Vec<PluginConfig> = vec![];

            let result = daemon.handle_load_plugins_with_channels(plugins, 2, 2);
            if result.success {
                println!("   Driver playback started successfully");
            } else {
                println!("   Driver playback failed: {:?}", result.error);
            }
        });
    }

    pub(super) fn handle_command(&self, cmd: Command) -> Response {
        match cmd {
            Command::Status => self.handle_status(),
            Command::GetSnapshot => self.handle_get_snapshot(),
            Command::DumpState => self.handle_dump_state(),
            Command::Load { path } => self.handle_load(&path),
            Command::Play => self.handle_play(),
            Command::Pause => self.handle_pause(),
            Command::Stop => self.handle_stop(),
            Command::Seek { position } => self.handle_seek(position),
            Command::SetVolume { volume } => self.handle_set_volume(volume),
            Command::ListDevices => self.handle_list_devices(),
            Command::SetDevice { device } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_set_device(&device)
            }
            Command::LoadPlugins {
                plugins,
                input_channels,
                output_channels,
            } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
            }
            Command::LoadPluginArtifact {
                artifact,
                base_generation,
            } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_load_plugin_artifact(artifact, base_generation)
            }
            Command::ReorderGraph {
                order,
                base_generation,
            } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_reorder_graph(order, base_generation)
            }
            Command::SetInputChannels { channels } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_set_pipeline_channels(Some(channels), None)
            }
            Command::SetOutputChannels { channels } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_set_pipeline_channels(None, Some(channels))
            }
            Command::SetPipelineChannels {
                input_channels,
                output_channels,
            } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_set_pipeline_channels(input_channels, output_channels)
            }
            Command::GetLoudness => self.handle_get_loudness(),
            Command::GetMetering => self.handle_get_metering(),
            Command::GetPlugins => self.handle_get_plugins(),
            Command::GetAvailablePlugins => self.handle_get_available_plugins(),
            Command::AddPlugin { plugin, index } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_add_plugin(plugin, index)
            }
            Command::RemovePlugin { index } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_remove_plugin(index)
            }
            Command::UpdatePlugin { index, parameters } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_update_plugin(index, parameters)
            }
            Command::ReorderPlugins { order } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_reorder_plugins(order)
            }
            Command::SetRackPluginState {
                index,
                input_channels,
                bypassed,
                base_generation,
            } => {
                let _mutation = self.pipeline_mutation.lock();
                self.handle_set_rack_plugin_state(index, input_channels, bypassed, base_generation)
            }
            Command::DriverStatus => self.handle_driver_status(),
            Command::Shutdown => {
                *self.running.lock() = false;
                Response::ok_empty()
            }
            // Encryption commands
            Command::SetEncryption { enabled } => self.handle_set_encryption(enabled),
            Command::EncryptionStatus => self.handle_encryption_status(),
            Command::RotateEncryptionKey => self.handle_rotate_encryption_key(),
            // Driver config commands
            Command::SetSampleRate { rate } => self.handle_set_sample_rate(rate),
            Command::SetBufferFrames { frames } => self.handle_set_buffer_frames(frames),
            Command::GetDriverConfig => self.handle_get_driver_config(),
        }
    }

    pub(super) fn metering_snapshot(&self) -> Value {
        let (input_idx, output_idx, fallback_input_channels) = {
            let pipeline = self.system_state.lock();
            (
                pipeline.input_loudness_index(),
                pipeline.output_loudness_index(),
                pipeline.input_channels(),
            )
        };

        // Snapshot the manager-owned values only after releasing the daemon
        // state lock. This keeps the lock order one-way and prevents the UI's
        // polling path from extending a cross-component lock hold while it
        // clones analyzer payloads.
        let fallback_output_channels = {
            let manager = self.manager.lock();
            manager.get_engine_state().num_channels
        };

        // `get_cached_plugin_data` returns an Arc-backed snapshot from the
        // engine's lock-free cache. Clone that Arc while the manager mutex is
        // held, then downcast/clone the analyzer payload after releasing the
        // mutex. Meter polling must never hold the daemon's manager lock while
        // copying the (potentially large) loudness vectors.
        let snapshot_loudness = |index: Option<usize>| {
            let data = index.and_then(|idx| {
                let manager = self.manager.lock();
                manager.get_cached_plugin_data(idx)
            });
            data.and_then(|data| data.downcast_ref::<sotf_audio::LoudnessData>().cloned())
        };
        let input_data = snapshot_loudness(input_idx);
        let output_data = snapshot_loudness(output_idx);

        let input_json = input_data
            .as_ref()
            .map(loudness_data_to_json)
            .unwrap_or_else(|| empty_loudness_json(fallback_input_channels));
        let output_json = output_data
            .as_ref()
            .map(loudness_data_to_json)
            .unwrap_or_else(|| empty_loudness_json(fallback_output_channels));

        serde_json::json!({
            "input": input_json,
            "output": output_json,
            "sources": {
                "input": metering_source_json(input_data.is_some(), fallback_input_channels),
                "output": metering_source_json(output_data.is_some(), fallback_output_channels),
            },
        })
    }

    pub(super) fn snapshot_json(&self) -> Value {
        let driver_status = self.driver_manager.lock().status();
        let key_status = self.key_manager.lock().status();

        let manager = self.manager.lock();
        let state = manager.get_state();
        let state_name = format!("{:?}", state);
        let engine_state = manager.get_engine_state();
        let volume = manager.get_volume();
        let muted = manager.is_muted();
        drop(manager);

        let pipeline = self.system_state.lock();
        let desired = pipeline.desired_spec();
        let applied = pipeline.applied_spec();
        let applied_generation = pipeline.applied_generation();
        let applied_output_device = pipeline.applied_output_device();
        let pipeline_recovery = pipeline.pipeline_recovery();
        drop(pipeline);

        let (transport, mut faults) =
            transport_snapshot_and_faults(&state_name, &driver_status, &engine_state);

        if desired
            .output_device
            .as_ref()
            .is_some_and(|device| !is_safe_output_device_name(device))
        {
            faults.push(serde_json::json!({
                "code": "unsafe_desired_output_device",
                "severity": "error",
                "message": "Desired output device is virtual/loopback and would create a feedback risk.",
            }));
        }
        if engine_state
            .playback_output_device
            .as_ref()
            .is_some_and(|device| !is_safe_output_device_name(device))
        {
            faults.push(serde_json::json!({
                "code": "unsafe_observed_output_device",
                "severity": "error",
                "message": "Observed playback output device is virtual/loopback and risks feedback.",
            }));
        }
        if let Some(recovery) = &pipeline_recovery {
            faults.push(serde_json::json!({
                "code": "pipeline_recovery_required",
                "severity": "error",
                "message": recovery.error,
                "actions": recovery.actions,
            }));
        }

        let metering = self.metering_snapshot();
        push_metering_faults(&state_name, &metering, &mut faults);
        let health = if faults
            .iter()
            .any(|fault| fault["severity"].as_str() == Some("error"))
        {
            "fault"
        } else if faults.is_empty() {
            "ok"
        } else {
            "warning"
        };

        serde_json::json!({
            "schema_version": 1,
            "desired": pipeline_spec_to_json(&desired),
            "applied": {
                "generation": applied_generation,
                "output_device": applied_output_device,
                "spec": applied.as_ref().map(pipeline_spec_to_json),
            },
            "observed": {
                "engine": {
                    "state": state_name,
                    "volume": volume,
                    "muted": muted,
                    "sample_rate": engine_state.sample_rate,
                    "channels": engine_state.num_channels,
                    "underruns": engine_state.underruns,
                    "playback_output_device": engine_state.playback_output_device,
                    "playback_callback_count": engine_state.playback_callback_count,
                    "playback_buffer_fill_percent": engine_state.playback_buffer_fill_percent,
                    "playback_stream_error_count": engine_state.playback_stream_error_count,
                    "playback_frames_received": engine_state.playback_frames_received,
                    "playback_frames_written": engine_state.playback_frames_written,
                    "playback_frames_dropped": engine_state.playback_frames_dropped,
                    "playback_effective_sample_rate": engine_state.playback_effective_sample_rate,
                    "last_error": engine_state.last_error,
                },
                "driver": {
                    "platform_supported": driver_status.platform_supported,
                    "driver_installed": driver_status.driver_installed,
                    "driver_ready": driver_status.driver_ready,
                    "capture_active": driver_status.capture_active,
                    "sample_rate": driver_status.sample_rate,
                    "channel_count": driver_status.channel_count,
                    "buffer_frames": driver_status.buffer_frames,
                    "driver_name": driver_status.driver_name,
                },
                "encryption": {
                    "enabled": key_status.enabled,
                    "fingerprint": key_status.fingerprint,
                    "key_path": key_status.key_path,
                },
                "transport": transport,
                "metering": metering,
            },
            "diagnostics": {
                "health": health,
                "faults": faults,
                "pipeline_recovery": pipeline_recovery.as_ref().map(|recovery| {
                    serde_json::json!({
                        "error": recovery.error,
                        "actions": recovery.actions,
                    })
                }),
            },
        })
    }

    pub(super) fn handle_get_snapshot(&self) -> Response {
        Response::ok(self.snapshot_json())
    }

    pub(super) fn handle_dump_state(&self) -> Response {
        let state = self.system_state.lock();
        let user_graph = state.user_graph();
        let user_plugins = state.user_plugins();
        drop(state);
        Response::ok(serde_json::json!({
            "snapshot": self.snapshot_json(),
            "topology": if user_graph.is_some() { "graph" } else { "rack" },
            "plugins": user_plugins,
            "graph": user_graph,
            "runtime_telemetry": self.runtime_telemetry.snapshot(),
        }))
    }

    pub(super) fn handle_status(&self) -> Response {
        let (state, engine_state, volume, muted) = {
            let manager = self.manager.lock();
            (
                manager.get_state(),
                manager.get_engine_state(),
                manager.get_volume(),
                manager.is_muted(),
            )
        };
        let (
            selected_device,
            input_channels,
            output_channels,
            pipeline_generation,
            pipeline_applied_output_device,
            pipeline_recovery,
        ) = {
            let pipeline = self.system_state.lock();
            (
                pipeline.selected_output_device(),
                pipeline.input_channels(),
                pipeline.output_channels(),
                pipeline.applied_generation(),
                pipeline.applied_output_device(),
                pipeline.pipeline_recovery(),
            )
        };
        let driver_status = self.driver_manager.lock().status();
        let key_status = self.key_manager.lock().status();

        let mut recovery_actions = Vec::<String>::new();
        if !driver_status.platform_supported {
            recovery_actions.push("driver_not_supported".to_string());
        }
        if !driver_status.driver_installed {
            recovery_actions.push("reinstall_driver".to_string());
        }
        if driver_status.driver_installed && !driver_status.driver_ready {
            recovery_actions.push("restart_daemon".to_string());
        }
        if selected_device.is_none()
            && pipeline_applied_output_device.is_none()
            && engine_state.playback_output_device.is_none()
        {
            recovery_actions.push("select_output_device".to_string());
        }
        if key_status.enabled && key_status.fingerprint.len() < 16 {
            recovery_actions.push("rotate_encryption_key".to_string());
        }
        if engine_state.underruns > 0 {
            recovery_actions.push("reset_shared_memory".to_string());
        }
        if let Some(recovery) = &pipeline_recovery {
            for action in &recovery.actions {
                if !recovery_actions.iter().any(|existing| existing == action) {
                    recovery_actions.push(action.clone());
                }
            }
        }

        Response::ok(serde_json::json!({
            "state": format!("{:?}", state),
            "volume": volume,
            "muted": muted,
            "selected_device": selected_device,
            "pipeline_generation": pipeline_generation,
            "pipeline_applied_output_device": pipeline_applied_output_device,
            "sample_rate": engine_state.sample_rate,
            "input_channels": input_channels,
            "output_channels": output_channels,
            "channels": engine_state.num_channels,
            "underruns": engine_state.underruns,
            "playback_output_device": engine_state.playback_output_device,
            "playback_callback_count": engine_state.playback_callback_count,
            "playback_buffer_fill_percent": engine_state.playback_buffer_fill_percent,
            "playback_stream_error_count": engine_state.playback_stream_error_count,
            "playback_frames_received": engine_state.playback_frames_received,
            "playback_frames_written": engine_state.playback_frames_written,
            "playback_frames_dropped": engine_state.playback_frames_dropped,
            "playback_effective_sample_rate": engine_state.playback_effective_sample_rate,
            "last_error": engine_state.last_error,
            "driver": {
                "installed": driver_status.driver_installed,
                "ready": driver_status.driver_ready,
                "capture_active": driver_status.capture_active,
                "frame_size": driver_status.buffer_frames,
                "sample_rate": driver_status.sample_rate,
                "channel_count": driver_status.channel_count,
            },
            "encryption": {
                "enabled": key_status.enabled,
                "fingerprint": key_status.fingerprint,
            },
            "active_route": {
                "desired_output_device": selected_device,
                "applied_output_device": pipeline_applied_output_device,
                "playback_output_device": engine_state.playback_output_device,
                "capture_active": driver_status.capture_active,
            },
            "pipeline_recovery": pipeline_recovery.as_ref().map(|recovery| {
                serde_json::json!({
                    "error": recovery.error,
                    "actions": recovery.actions,
                })
            }),
            "recovery_actions": recovery_actions,
        }))
    }

    pub(super) fn handle_load(&self, path: &str) -> Response {
        let path = match validate_user_load_path(std::path::Path::new(path)) {
            Ok(path) => path,
            Err(error) => {
                return Response::err(format!("Refusing to load audio file: {error}"));
            }
        };

        let mut manager = self.manager.lock();
        match manager.load_file(&path) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to load file: {}", e)),
        }
    }

    pub(super) fn handle_play(&self) -> Response {
        let _mutation = self.pipeline_mutation.lock();
        let driver_status = self.driver_manager.lock().status();
        let driver_sample_rate = if driver_status.sample_rate > 0 {
            driver_status.sample_rate
        } else {
            48_000
        };
        let driver_buffer_frames = if driver_status.buffer_frames > 0 {
            driver_status.buffer_frames
        } else {
            512
        };
        let fallback_input_channels = if driver_status.channel_count > 0 {
            driver_status.channel_count as usize
        } else {
            2
        };

        let plan = {
            let state = self.system_state.lock();
            state.prepare_from_spec(state.desired_spec(), fallback_input_channels)
        };
        match plan {
            Ok(plan) => self.apply_pipeline_plan(
                plan,
                driver_status,
                driver_sample_rate,
                driver_buffer_frames,
            ),
            Err(error) => Response::err(format!("Failed to prepare playback pipeline: {error}")),
        }
    }

    pub(super) fn handle_pause(&self) -> Response {
        let manager = self.manager.lock();
        match manager.pause() {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to pause: {}", e)),
        }
    }

    pub(super) fn handle_stop(&self) -> Response {
        // Lock-order invariant: driver_manager -> manager. The config
        // watcher thread also acquires them in this order. Using the
        // `lock_order::lock_with_order_warning` helper turns silent
        // contention with the watcher into a logged warning so a future
        // contributor who introduces an inverse acquisition order has a
        // diagnostic to follow instead of an undetectable deadlock.
        super::lock_order::lock_with_order_warning(&self.driver_manager, "driver_manager")
            .set_engine_ready(false);
        log::debug!("Cleared engine_ready flag via driver");

        let mut manager = super::lock_order::lock_with_order_warning(&self.manager, "manager");
        match manager.stop() {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to stop: {}", e)),
        }
    }

    pub(super) fn handle_seek(&self, position: f64) -> Response {
        let manager = self.manager.lock();
        match manager.seek(position) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to seek: {}", e)),
        }
    }

    pub(super) fn handle_set_volume(&self, volume: f32) -> Response {
        let manager = self.manager.lock();
        match manager.set_volume(volume) {
            Ok(()) => Response::ok_empty(),
            Err(error) => Response::err(format!("Failed to set volume: {error}")),
        }
    }

    pub(super) fn handle_list_devices(&self) -> Response {
        match list_audio_devices() {
            Ok(devices) => Response::ok(serde_json::json!({ "devices": devices })),
            Err(e) => Response::err(format!("Failed to list devices: {}", e)),
        }
    }

    pub(super) fn handle_set_device(&self, device: &str) -> Response {
        use cpal::traits::DeviceTrait;
        let is_asio = sotf_audio::devices::is_asio_device(device);
        let host = sotf_audio::devices::get_host_for_device(Some(device));
        let device_name = sotf_audio::devices::strip_asio_prefix(device);

        match sotf_audio::devices::find_device(&host, device_name, false) {
            Ok(cpal_device) => {
                let resolved_name = cpal_device
                    .description()
                    .map(|d| d.name().to_string())
                    .unwrap_or_else(|_| "Unknown Device".to_string());

                if !is_safe_output_device_name(&resolved_name) {
                    log::warn!(
                        "Rejected virtual output device '{}' (requested '{}') to prevent feedback",
                        resolved_name,
                        device
                    );
                    return Response::err(format!(
                        "'{}' is a virtual/loopback device and cannot be used as Systemwide speaker output. Select hardware speakers/headphones here, and select SotF Virtual Audio in macOS Sound Output.",
                        resolved_name
                    ));
                }

                // Store with ASIO prefix preserved so playback thread selects the right host
                let stored_name = if is_asio {
                    format!(
                        "{}{}",
                        sotf_audio::devices::ASIO_DEVICE_PREFIX,
                        resolved_name
                    )
                } else {
                    resolved_name.clone()
                };
                log::info!(
                    "Output device set to: {} (matched from '{}')",
                    resolved_name,
                    device
                );

                let driver_status = self.driver_manager.lock().status();
                let driver_sample_rate = if driver_status.sample_rate > 0 {
                    driver_status.sample_rate
                } else {
                    48_000
                };
                let driver_buffer_frames = if driver_status.buffer_frames > 0 {
                    driver_status.buffer_frames
                } else {
                    512
                };
                let plan = match self
                    .system_state
                    .lock()
                    .prepare_with_selected_device(stored_name.clone())
                {
                    Ok(plan) => plan,
                    Err(e) => return Response::err(e),
                };

                log::info!(
                    "Starting/restarting driver playback with output device: {}",
                    resolved_name
                );
                let resp = self.apply_pipeline_plan(
                    plan,
                    driver_status,
                    driver_sample_rate,
                    driver_buffer_frames,
                );
                if !resp.success {
                    return resp;
                }

                Response::ok_empty()
            }
            Err(e) => {
                log::warn!("Failed to set device '{}': {}", device, e);
                Response::err(format!("Device '{}' not found. {}", device, e))
            }
        }
    }

    pub(super) fn apply_pipeline_plan(
        &self,
        plan: PipelinePlan,
        driver_status: driver_common::DriverStatus,
        driver_sample_rate: u32,
        driver_buffer_frames: u32,
    ) -> Response {
        let fallback_input_channels = if driver_status.channel_count > 0 {
            driver_status.channel_count as usize
        } else {
            2
        };
        let previous_plan = {
            let state = self.system_state.lock();
            state
                .applied_spec()
                .and_then(|spec| state.prepare_from_spec(spec, fallback_input_channels).ok())
        };

        let response = self.apply_pipeline_plan_once(
            plan,
            driver_status.clone(),
            driver_sample_rate,
            driver_buffer_frames,
        );
        if response.success {
            self.system_state.lock().clear_pipeline_recovery();
            return response;
        }

        if previous_plan.is_none() {
            let error = response
                .error
                .clone()
                .unwrap_or_else(|| "pipeline apply failed".to_string());
            self.system_state.lock().mark_pipeline_recovery(error);
            return response;
        }

        let Some(previous_plan) = previous_plan else {
            return response;
        };
        let restore = self.apply_pipeline_plan_once(
            previous_plan,
            driver_status,
            driver_sample_rate,
            driver_buffer_frames,
        );
        if restore.success {
            self.system_state.lock().clear_pipeline_recovery();
            Response::err(format!(
                "{}; restored the last working pipeline; retry the requested change",
                response
                    .error
                    .unwrap_or_else(|| "pipeline apply failed".to_string())
            ))
        } else {
            let error = format!(
                "{}; pipeline recovery also failed, restart the daemon",
                response
                    .error
                    .unwrap_or_else(|| "pipeline apply failed".to_string())
            );
            self.system_state
                .lock()
                .mark_pipeline_recovery(error.clone());
            Response::err(error)
        }
    }

    fn apply_pipeline_plan_once(
        &self,
        plan: PipelinePlan,
        driver_status: driver_common::DriverStatus,
        driver_sample_rate: u32,
        driver_buffer_frames: u32,
    ) -> Response {
        self.driver_manager.lock().set_engine_ready(false);

        {
            let mut manager = self.manager.lock();
            let _ = manager.stop();
        }

        let mut effective_driver_sample_rate = driver_sample_rate;
        let mut effective_driver_buffer_frames = driver_buffer_frames;

        if driver_status.driver_installed
            && driver_status.channel_count != plan.spec.input_channels as u32
        {
            let result = self.driver_manager.lock().request_config(DriverConfig::new(
                driver_sample_rate,
                driver_buffer_frames,
                plan.spec.input_channels as u32,
            ));

            match result {
                driver_common::ConfigResult::Accepted
                | driver_common::ConfigResult::Negotiated { .. } => {
                    (effective_driver_sample_rate, effective_driver_buffer_frames) =
                        pipeline_timing_after_config_request(
                            &result,
                            driver_sample_rate,
                            driver_buffer_frames,
                        );
                    log::info!(
                        "HAL input channel count set to {} via driver config",
                        plan.spec.input_channels
                    );
                }
                driver_common::ConfigResult::Error(e) => {
                    log::error!("Failed to set HAL input channels: {}", e);
                    return Response::err(format!("Failed to set HAL input channels: {}", e));
                }
                _ => return Response::err("Driver returned an unknown configuration result"),
            }
        }

        log::info!(
            "Loading driver plugin chain: {} user plugins + 2 monitors = {} total, {}Hz {}ch input, {} output channels, device: {:?}",
            plan.spec.user_plugins.len(),
            plan.runtime_plugins.len(),
            effective_driver_sample_rate,
            plan.spec.input_channels,
            plan.spec.output_channels,
            plan.spec.output_device
        );

        let result = {
            let mut manager = self.manager.lock();
            Self::start_pipeline_plan(
                &mut manager,
                &plan,
                effective_driver_sample_rate,
                effective_driver_buffer_frames,
            )
        };

        match result {
            Ok(_) => {
                self.system_state.lock().commit_applied(&plan);
                log::info!("Driver plugin chain loaded successfully");

                self.driver_manager.lock().set_engine_ready(true);
                log::info!("Set engine_ready=true via driver");
                if let Err(e) = self.sync_encryption_to_shared_memory(false) {
                    log::warn!("{}", e);
                }

                Response::ok_empty()
            }
            Err(e) => {
                log::error!("Failed to load driver plugins: {}", e);
                Response::err(format!("Failed to load plugin chain: {}", e))
            }
        }
    }

    /// Start one prepared pipeline, including the graph bootstrap/update
    /// sequence and loudness monitor selection. Both IPC mutations and the
    /// driver reconfiguration watcher use this helper so their recovery paths
    /// cannot drift apart.
    pub(super) fn start_pipeline_plan(
        manager: &mut AudioEngineManager,
        plan: &PipelinePlan,
        sample_rate: u32,
        buffer_frames: u32,
    ) -> Result<(), String> {
        let bootstrap_plugins = if plan.runtime_graph.is_some() {
            build_driver_plugin_chain(Vec::new()).0
        } else {
            plan.runtime_plugins.clone()
        };
        manager
            .start_hal_playback_with_driver_config(
                plan.spec.output_device.clone(),
                bootstrap_plugins,
                plan.spec.output_channels,
                sample_rate,
                buffer_frames,
                plan.spec.input_channels,
            )
            .map_err(|error| error.to_string())?;

        if let Some(graph) = plan.runtime_graph.clone()
            && let Err(error) = manager.update_plugin_graph(graph)
        {
            let _ = manager.stop();
            return Err(error);
        }

        manager.set_loudness_plugin_index(plan.output_loudness_index);
        Ok(())
    }

    pub(super) fn handle_load_plugins_with_channels(
        &self,
        plugins: Vec<PluginConfig>,
        input_channels: usize,
        output_channels: usize,
    ) -> Response {
        let driver_status = self.driver_manager.lock().status();
        let driver_sample_rate = if driver_status.sample_rate > 0 {
            driver_status.sample_rate
        } else {
            48_000
        };
        let driver_buffer_frames = if driver_status.buffer_frames > 0 {
            driver_status.buffer_frames
        } else {
            512
        };
        let stored_input_channels = self.system_state.lock().input_channels();
        let fallback_input_channels = if driver_status.channel_count > 0 {
            driver_status.channel_count as usize
        } else if stored_input_channels > 0 {
            stored_input_channels
        } else {
            2
        };

        let plan = match self.system_state.lock().prepare_plan(
            plugins,
            input_channels,
            output_channels,
            fallback_input_channels,
        ) {
            Ok(plan) => plan,
            Err(e) => return Response::err(e),
        };

        self.apply_pipeline_plan(
            plan,
            driver_status,
            driver_sample_rate,
            driver_buffer_frames,
        )
    }

    pub(super) fn handle_load_plugin_artifact(
        &self,
        artifact: Value,
        base_generation: Option<u64>,
    ) -> Response {
        if let Some(base_generation) = base_generation {
            let current_generation = self.system_state.lock().applied_generation().unwrap_or(0);
            if base_generation != current_generation {
                return Response::err(format!(
                    "Plugin artifact generation conflict: editor based on generation {base_generation}, current generation is {current_generation}. Refresh before applying."
                ));
            }
        }
        match plan_plugin_artifact(artifact) {
            Ok(PluginArtifactPlan::RackChain { plugins }) => {
                let (input_channels, output_channels) = {
                    let state = self.system_state.lock();
                    (state.input_channels(), state.output_channels())
                };
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
            }
            Ok(PluginArtifactPlan::Graph { graph }) => {
                let (input_channels, output_channels) = {
                    let state = self.system_state.lock();
                    (state.input_channels(), state.output_channels())
                };
                self.handle_load_plugin_graph_with_channels(graph, input_channels, output_channels)
            }
            Ok(PluginArtifactPlan::UnsupportedGraph { reason }) => Response::err(format!(
                "Unsupported graph plugin artifact: {}. Use a graph-aware loader instead of flattening it into the rack.",
                reason
            )),
            Err(e) => Response::err(format!("Invalid plugin artifact: {}", e)),
        }
    }

    pub(super) fn handle_reorder_graph(
        &self,
        order: Vec<usize>,
        base_generation: Option<u64>,
    ) -> Response {
        if let Some(base_generation) = base_generation {
            let current_generation = self.system_state.lock().applied_generation().unwrap_or(0);
            if base_generation != current_generation {
                return Response::err(format!(
                    "Graph generation conflict: editor based on generation {base_generation}, current generation is {current_generation}. Refresh before reordering."
                ));
            }
        }

        let (graph, input_channels, output_channels) = {
            let state = self.system_state.lock();
            let Some(graph) = state.user_graph() else {
                return Response::err(
                    "Graph reorder requires an active graph pipeline; use reorder_plugins for a rack.",
                );
            };
            (graph, state.input_channels(), state.output_channels())
        };

        let reordered = match reorder_linear_graph(&graph, &order) {
            Ok(graph) => graph,
            Err(error) => return Response::err(error),
        };
        self.handle_load_plugin_graph_with_channels(reordered, input_channels, output_channels)
    }

    fn handle_load_plugin_graph_with_channels(
        &self,
        graph: sotf_audio::engine::PluginGraphConfig,
        input_channels: usize,
        output_channels: usize,
    ) -> Response {
        let (current_input_channels, current_output_channels) = {
            let state = self.system_state.lock();
            (state.input_channels(), state.output_channels())
        };
        let channel_geometry_changed =
            input_channels != current_input_channels || output_channels != current_output_channels;
        let driver_status = self.driver_manager.lock().status();
        let driver_sample_rate = if driver_status.sample_rate > 0 {
            driver_status.sample_rate
        } else {
            48_000
        };
        let driver_buffer_frames = if driver_status.buffer_frames > 0 {
            driver_status.buffer_frames
        } else {
            512
        };
        let fallback_input_channels = if driver_status.channel_count > 0 {
            driver_status.channel_count as usize
        } else {
            self.system_state.lock().input_channels().max(2)
        };
        let plan = match self.system_state.lock().prepare_graph_plan(
            graph,
            input_channels,
            output_channels,
            fallback_input_channels,
        ) {
            Ok(plan) => plan,
            Err(error) => return Response::err(error),
        };

        if !channel_geometry_changed
            && self.manager.lock().get_state() != sotf_audio::manager::StreamingState::Idle
        {
            let Some(runtime_graph) = plan.runtime_graph.clone() else {
                return Response::err("Graph plan did not contain a runtime graph");
            };
            let mut manager = self.manager.lock();
            if let Err(error) = manager.update_plugin_graph(runtime_graph) {
                return Response::err(format!(
                    "Failed to apply plugin graph; previous pipeline remains active: {error}"
                ));
            }
            manager.set_loudness_plugin_index(plan.output_loudness_index);
            drop(manager);
            self.system_state.lock().commit_applied(&plan);
            return Response::ok(serde_json::json!({
                "topology": "graph",
                "nodes": plan.spec.user_graph.as_ref().map_or(0, |graph| graph.nodes.len()),
                "edges": plan.spec.user_graph.as_ref().map_or(0, |graph| graph.edges.len()),
                "generation": self.system_state.lock().applied_generation(),
            }));
        }

        self.apply_pipeline_plan(
            plan,
            driver_status,
            driver_sample_rate,
            driver_buffer_frames,
        )
    }

    pub(super) fn handle_set_pipeline_channels(
        &self,
        input_channels: Option<usize>,
        output_channels: Option<usize>,
    ) -> Response {
        if input_channels.is_none() && output_channels.is_none() {
            return Response::err(
                "set_pipeline_channels requires input_channels or output_channels",
            );
        }

        let (plugins, graph, current_input_channels, current_output_channels) = {
            let state = self.system_state.lock();
            (
                state.user_plugins(),
                state.user_graph(),
                state.input_channels(),
                state.output_channels(),
            )
        };

        let next_input_channels = input_channels.unwrap_or(current_input_channels);
        let next_output_channels = output_channels.unwrap_or(current_output_channels);

        if let Some(graph) = graph {
            self.handle_load_plugin_graph_with_channels(
                graph,
                next_input_channels,
                next_output_channels,
            )
        } else {
            self.handle_load_plugins_with_channels(
                plugins,
                next_input_channels,
                next_output_channels,
            )
        }
    }

    pub(super) fn handle_get_loudness(&self) -> Response {
        let manager = self.manager.lock();
        match manager.get_loudness() {
            Some(loudness) => Response::ok(loudness_info_to_json(&loudness)),
            None => Response::err("Loudness monitoring not enabled"),
        }
    }

    pub(super) fn handle_get_metering(&self) -> Response {
        Response::ok(self.metering_snapshot())
    }

    // =========================================================================
    // Plugin management handlers
    // =========================================================================

    pub(super) fn handle_get_plugins(&self) -> Response {
        let state = self.system_state.lock();
        if let Some(graph) = state.user_graph() {
            return Response::ok(serde_json::json!({
                "topology": "graph",
                "graph": graph,
                "plugins": [],
                "generation": state.applied_generation(),
            }));
        }
        let input_channels = state.input_channels().max(1);
        let plugins = state.user_plugins();
        drop(state);
        let result: Vec<Value> = plugins
            .iter()
            .enumerate()
            .map(|(i, p)| {
                serde_json::json!({
                    "index": i,
                    "plugin_type": p.plugin_type,
                    "parameters": p.parameters,
                    // Legacy rack entries have no per-node metadata. These
                    // defaults are made explicit so the Configbar can issue a
                    // state patch that promotes the rack to a graph.
                    "input_channels": input_channels,
                    "bypassed": false,
                })
            })
            .collect();
        Response::ok(serde_json::json!({
            "topology": "rack",
            "plugins": result,
            "generation": self.system_state.lock().applied_generation(),
        }))
    }

    pub(super) fn handle_get_available_plugins(&self) -> Response {
        static AVAILABLE_PLUGINS: OnceLock<Value> = OnceLock::new();

        let available = AVAILABLE_PLUGINS.get_or_init(|| {
            let excluded = [
                "loudness_monitor",
                "spectrum_analyzer",
                "resampler",
                "hal_input",
                "hal_output",
                "band_split",
                "band_merge",
                "ab_compare",
                "fletcher_munson",
            ];

            let plugins: Vec<Value> = PluginType::all()
                .into_iter()
                .filter(|pt| {
                    let engine_type = plugin_type_to_engine_str(pt);
                    !excluded.contains(&engine_type)
                })
                .filter_map(|pt| {
                    let engine_type = plugin_type_to_engine_str(&pt);
                    let category = plugin_type_category(&pt);
                    let default_settings =
                        match sotf_audio::PluginSettings::default_for(&pt) {
                            Ok(settings) => settings,
                            Err(error) => {
                                log::warn!(
                                    "Skipping plugin type {} because default settings are unavailable: {}",
                                    engine_type,
                                    error
                                );
                                return None;
                            }
                        };
                    let default_parameters = default_settings.to_plugin_config(48_000.0).parameters;
                    Some(serde_json::json!({
                        "type": engine_type,
                        "name": pt.name(),
                        "description": pt.description(),
                        "category": category,
                        "maturity": format!("{:?}", pt.maturity()),
                        "default_parameters": default_parameters,
                        "parameters": plugin_parameter_descriptors(&default_settings),
                    }))
                })
                .collect();

            serde_json::json!({ "plugins": plugins })
        });

        Response::ok(available.clone())
    }

    pub(super) fn handle_add_plugin(&self, plugin: PluginConfig, index: Option<usize>) -> Response {
        let mut plugins = {
            let state = self.system_state.lock();
            if state.user_graph().is_some() {
                return Response::err(
                    "A graph pipeline is active; edit and reload the graph artifact instead of using rack mutation commands.",
                );
            }
            state.user_plugins()
        };
        match index {
            Some(i) if i <= plugins.len() => plugins.insert(i, plugin),
            _ => plugins.push(plugin),
        }
        self.reload_plugins_with_user_plugins(plugins)
    }

    pub(super) fn handle_remove_plugin(&self, index: usize) -> Response {
        let mut plugins = {
            let state = self.system_state.lock();
            if state.user_graph().is_some() {
                return Response::err(
                    "A graph pipeline is active; edit and reload the graph artifact instead of using rack mutation commands.",
                );
            }
            state.user_plugins()
        };
        if index >= plugins.len() {
            return Response::err(format!(
                "Plugin index {} out of range (have {})",
                index,
                plugins.len()
            ));
        }
        plugins.remove(index);
        self.reload_plugins_with_user_plugins(plugins)
    }

    pub(super) fn handle_update_plugin(&self, index: usize, parameters: Value) -> Response {
        let mut plugins = {
            let state = self.system_state.lock();
            if state.user_graph().is_some() {
                return Response::err(
                    "A graph pipeline is active; edit and reload the graph artifact instead of using rack mutation commands.",
                );
            }
            state.user_plugins()
        };
        if index >= plugins.len() {
            return Response::err(format!(
                "Plugin index {} out of range (have {})",
                index,
                plugins.len()
            ));
        }
        plugins[index].parameters = parameters;
        self.reload_plugins_with_user_plugins(plugins)
    }

    pub(super) fn handle_reorder_plugins(&self, order: Vec<usize>) -> Response {
        let plugins = {
            let state = self.system_state.lock();
            if state.user_graph().is_some() {
                return Response::err(
                    "A graph pipeline is active; edit and reload the graph artifact instead of using rack mutation commands.",
                );
            }
            state.user_plugins()
        };
        let n = plugins.len();

        if order.len() != n {
            return Response::err(format!(
                "Order length {} doesn't match plugin count {}",
                order.len(),
                n
            ));
        }
        let mut seen = vec![false; n];
        for &idx in &order {
            if idx >= n || seen[idx] {
                return Response::err(format!(
                    "Invalid order: duplicate or out-of-range index {}",
                    idx
                ));
            }
            seen[idx] = true;
        }

        let old = plugins.clone();
        let mut reordered = plugins;
        for (new_pos, &old_pos) in order.iter().enumerate() {
            reordered[new_pos] = old[old_pos].clone();
        }
        self.reload_plugins_with_user_plugins(reordered)
    }

    pub(super) fn handle_set_rack_plugin_state(
        &self,
        index: usize,
        input_channels: Option<usize>,
        bypassed: Option<bool>,
        base_generation: Option<u64>,
    ) -> Response {
        if let Some(base_generation) = base_generation {
            let current_generation = self.system_state.lock().applied_generation().unwrap_or(0);
            if base_generation != current_generation {
                return Response::err(format!(
                    "Rack generation conflict: editor based on generation {base_generation}, current generation is {current_generation}. Refresh before changing plugin state."
                ));
            }
        }

        let (plugins, graph, input_geometry, output_channels) = {
            let state = self.system_state.lock();
            (
                state.user_plugins(),
                state.user_graph(),
                state.input_channels(),
                state.output_channels(),
            )
        };
        if graph.is_some() {
            return Response::err(
                "Rack plugin state requires a rack pipeline; use graph commands for graph nodes.",
            );
        }

        let graph = match rack_plugins_to_linear_graph(
            &plugins,
            input_geometry,
            index,
            input_channels,
            bypassed,
        ) {
            Ok(graph) => graph,
            Err(error) => return Response::err(error),
        };
        self.handle_load_plugin_graph_with_channels(graph, input_geometry, output_channels)
    }

    pub(super) fn reload_plugins_with_user_plugins(&self, plugins: Vec<PluginConfig>) -> Response {
        let prepared_plan = {
            let pipeline = self.system_state.lock();
            pipeline.prepare_plan(
                plugins,
                pipeline.input_channels(),
                pipeline.output_channels(),
                pipeline.input_channels(),
            )
        };
        let plan = match prepared_plan {
            Ok(plan) => plan,
            Err(e) => return Response::err(e),
        };

        if self.manager.lock().get_state() == sotf_audio::manager::StreamingState::Idle {
            log::info!("No running driver engine; starting driver playback");
            return self.handle_load_plugins_with_channels(
                plan.spec.user_plugins.clone(),
                plan.spec.input_channels,
                plan.spec.output_channels,
            );
        }

        let result = {
            let manager = self.manager.lock();
            manager.update_plugin_chain(&plan.runtime_plugins)
        };

        match result {
            Ok(()) => {
                self.manager
                    .lock()
                    .set_loudness_plugin_index(plan.output_loudness_index);
                self.system_state.lock().commit_applied(&plan);
                log::info!("Driver plugin chain hot-updated successfully");
                Response::ok_empty()
            }
            Err(e) => {
                log::error!("Failed to hot-update plugin chain: {}", e);
                Response::err(format!("Failed to update plugin chain: {}", e))
            }
        }
    }

    pub(super) fn handle_driver_status(&self) -> Response {
        let status = get_driver_status(&self.driver_manager.lock());
        let mut data = match serde_json::to_value(&status) {
            Ok(serde_json::Value::Object(data)) => data,
            Ok(_) => return Response::err("Driver status did not serialize as an object"),
            Err(error) => {
                return Response::err(format!("Failed to serialize driver status: {error}"));
            }
        };
        // Preserve the historical aliases while deriving the canonical fields
        // from DriverStatus itself. This keeps the JSON wire shape aligned with
        // the serde contract whenever a new status field is added.
        data.insert(
            "buffer_initialized".to_string(),
            serde_json::Value::Bool(status.capture_active || status.driver_installed),
        );
        data.insert(
            "ready".to_string(),
            serde_json::Value::Bool(
                status.platform_supported && status.driver_installed && status.driver_ready,
            ),
        );
        Response::ok(serde_json::Value::Object(data))
    }

    // =========================================================================
    // Encryption handlers
    // =========================================================================

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn sync_encryption_to_shared_memory(&self, flush_audio: bool) -> Result<(), String> {
        let key_manager = self.key_manager.lock();
        Self::apply_encryption_to_shared_memory(&key_manager, flush_audio)
    }

    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    pub(super) fn sync_encryption_to_shared_memory(
        &self,
        _flush_audio: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn apply_encryption_to_shared_memory(
        key_manager: &KeyManager,
        flush_audio: bool,
    ) -> Result<(), String> {
        match driver_hal::SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                if flush_audio {
                    buffer.flush_audio();
                }
                if key_manager.is_enabled() {
                    buffer.set_key_fingerprint(*key_manager.fingerprint());
                }
                buffer.set_encrypted(key_manager.is_enabled());
                buffer.set_config_changed();
                Ok(())
            }
            Err(e) => {
                let message = format!("Failed to sync encryption state to shared memory: {}", e);
                log::warn!("{}", message);
                Err(message)
            }
        }
    }

    pub(super) fn handle_set_encryption(&self, enabled: bool) -> Response {
        let mut key_manager = self.key_manager.lock();
        key_manager.set_enabled(enabled);

        if enabled && !key_manager.is_enabled() {
            return Response::err(
                "Encryption unavailable: the daemon has no session cipher; use the macOS HAL-enabled build and verify session-key runtime access",
            );
        }

        // On macOS with HAL, update shared memory encryption flag if the HAL
        // shared memory is available. Missing shared memory is normal when the
        // HAL driver is not currently running; the daemon-side encryption state
        // remains set and will be synced when the driver reconnects.
        #[cfg(all(target_os = "macos", feature = "hal"))]
        let (transport_state, transport_error) = match Self::apply_encryption_to_shared_memory(
            &key_manager,
            true,
        ) {
            Ok(()) => ("synced", None),
            Err(error) => {
                log::warn!(
                    "Encryption state is pending shared-memory sync (HAL may not be running): {}",
                    error
                );
                ("pending", Some(error))
            }
        };
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        let (transport_state, transport_error): (&str, Option<String>) = ("not_applicable", None);

        Response::ok(serde_json::json!({
            "enabled": key_manager.is_enabled(),
            "fingerprint": key_manager.fingerprint_hex(),
            "transport_state": transport_state,
            "transport_error": transport_error,
        }))
    }

    pub(super) fn handle_encryption_status(&self) -> Response {
        let key_manager = self.key_manager.lock();
        let status = key_manager.status();

        #[cfg(all(target_os = "macos", feature = "hal"))]
        let (transport_state, transport_error) = match driver_hal::SharedAudioBuffer::open_default()
        {
            Ok(buffer) => {
                let fingerprint_matches =
                    !status.enabled || buffer.key_fingerprint() == *key_manager.fingerprint();
                if buffer.is_encrypted() == status.enabled && fingerprint_matches {
                    ("synced", None)
                } else {
                    (
                        "mismatch",
                        Some(
                            "shared-memory encryption state differs from daemon state".to_string(),
                        ),
                    )
                }
            }
            Err(error) => ("unavailable", Some(error.to_string())),
        };
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        let (transport_state, transport_error): (&str, Option<String>) = ("not_applicable", None);

        Response::ok(serde_json::json!({
            "enabled": status.enabled,
            "fingerprint": status.fingerprint,
            "key_path": status.key_path,
            "transport_state": transport_state,
            "transport_error": transport_error,
        }))
    }

    pub(super) fn handle_rotate_encryption_key(&self) -> Response {
        let mut key_manager = self.key_manager.lock();

        match key_manager.force_rotate() {
            Ok(()) => {
                // On macOS with HAL, update shared memory fingerprint if the HAL
                // shared memory is available. Missing shared memory is normal when
                // the HAL driver is not currently running.
                #[cfg(all(target_os = "macos", feature = "hal"))]
                let (transport_state, transport_error) =
                    match Self::apply_encryption_to_shared_memory(&key_manager, true) {
                        Ok(()) => ("synced", None),
                        Err(error) => {
                            log::warn!(
                                "Rotated encryption key is pending shared-memory sync (HAL may not be running): {}",
                                error
                            );
                            ("pending", Some(error))
                        }
                    };
                #[cfg(not(all(target_os = "macos", feature = "hal")))]
                let (transport_state, transport_error): (&str, Option<String>) =
                    ("not_applicable", None);

                Response::ok(serde_json::json!({
                    "fingerprint": key_manager.fingerprint_hex(),
                    "transport_state": transport_state,
                    "transport_error": transport_error,
                }))
            }
            Err(e) => Response::err(format!("Failed to rotate key: {}", e)),
        }
    }

    // =========================================================================
    // Driver config handlers
    // =========================================================================

    pub(super) fn handle_set_sample_rate(&self, rate: u32) -> Response {
        const SUPPORTED: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

        if !SUPPORTED.contains(&rate) {
            return Response::err(format!(
                "Unsupported sample rate: {}. Supported: {:?}",
                rate, SUPPORTED
            ));
        }

        let manager = self.manager.lock();
        let state = manager.get_state();
        drop(manager);

        if state != sotf_audio::manager::StreamingState::Idle {
            return Response::err(
                "Cannot change sample rate while playback is active; stop playback and retry",
            );
        }

        let mut driver = self.driver_manager.lock();
        let result = driver.request_config(DriverConfig::with_sample_rate(rate));

        match result {
            driver_common::ConfigResult::Accepted
            | driver_common::ConfigResult::Negotiated { .. } => {
                log::info!("Sample rate set to {}Hz via driver", rate);
                Response::ok(serde_json::json!({ "sample_rate": rate }))
            }
            driver_common::ConfigResult::Error(e) => {
                Response::err(format!("Failed to set sample rate: {}", e))
            }
            _ => Response::err("Driver returned an unknown configuration result"),
        }
    }

    pub(super) fn handle_set_buffer_frames(&self, frames: u32) -> Response {
        if !(64..=4096).contains(&frames) {
            return Response::err(format!(
                "Buffer frames must be between 64 and 4096, got: {}",
                frames
            ));
        }

        let state = self.manager.lock().get_state();
        if state != sotf_audio::manager::StreamingState::Idle {
            return Response::err(
                "Cannot change buffer size while playback is active; stop playback and retry",
            );
        }

        let mut driver = self.driver_manager.lock();
        let result = driver.request_config(DriverConfig::with_buffer_frames(frames));

        match result {
            driver_common::ConfigResult::Accepted
            | driver_common::ConfigResult::Negotiated { .. } => {
                log::info!("Buffer frames set to {} via driver", frames);
                Response::ok(serde_json::json!({ "buffer_frames": frames }))
            }
            driver_common::ConfigResult::Error(e) => {
                Response::err(format!("Failed to set buffer frames: {}", e))
            }
            _ => Response::err("Driver returned an unknown configuration result"),
        }
    }

    pub(super) fn handle_get_driver_config(&self) -> Response {
        let driver = self.driver_manager.lock();
        let status = driver.status();
        let wire = DriverConfigWire::from(&status);

        match serde_json::to_value(wire) {
            Ok(data) => Response::ok(data),
            Err(error) => Response::err(format!("Failed to serialize driver config: {error}")),
        }
    }

    pub(super) fn handle_client(&self, mut stream: UnixStream, peer_class: PeerClass) {
        if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_secs(
            super::consts::IPC_CLIENT_IDLE_TIMEOUT_SECS,
        ))) {
            log::warn!("Failed to set IPC client idle timeout: {}", e);
        }
        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to clone stream for reading: {}", e);
                return;
            }
        };
        let mut reader = BufReader::new(reader_stream);
        let mut line = Vec::new();

        loop {
            match read_ipc_line_bounded(&mut reader, &mut line) {
                Ok(IpcLine::Eof) => break,
                Ok(IpcLine::Empty) => continue,
                Ok(IpcLine::TooLarge) => {
                    let response = Response::err("Request too large");
                    let json = serialize_response_safely(&response);
                    let _ = writeln!(stream, "{}", json);
                    break;
                }
                Ok(IpcLine::InvalidUtf8) => {
                    let response = Response::err("Invalid UTF-8 in command");
                    let json = serialize_response_safely(&response);
                    if let Err(e) = writeln!(stream, "{}", json) {
                        log::error!("Failed to write response: {}", e);
                        break;
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    log::debug!("Closing idle IPC client after read timeout");
                    break;
                }
                Err(e) => {
                    log::warn!("IPC client read failed: {}", e);
                    break;
                }
                Ok(IpcLine::Line(command_line)) => {
                    let mut command_telemetry = None;
                    let response = match serde_json::from_str::<Command>(&command_line) {
                        Ok(cmd) => {
                            let command_name = cmd.name();
                            let command_started = std::time::Instant::now();
                            // Defense-in-depth: gate which commands the
                            // peer's UID class may invoke. The macOS HAL
                            // (UID 202) is authenticated but should only
                            // be allowed to query status -- NOT issue
                            // arbitrary plugin loads, shutdowns, etc.
                            let response = if !peer_allows_command(peer_class, cmd.name()) {
                                log::warn!(
                                    "Rejecting command '{}' from peer class {:?}: not allowed",
                                    cmd.name(),
                                    peer_class
                                );
                                Response::err(format!(
                                    "Command '{}' not permitted for this peer",
                                    cmd.name()
                                ))
                            } else {
                                self.handle_command(cmd)
                            };
                            command_telemetry = Some((command_name, command_started.elapsed()));
                            response
                        }
                        Err(e) => Response::err(format!("Invalid command: {}", e)),
                    };

                    // Hot-path IPC writer: serialization can fail if a
                    // client managed to inject NaN / Infinity into a
                    // `Value::Number` via UpdatePlugin parameters that
                    // gets reflected back through get_plugins. Never
                    // panic the client thread -- emit a static, safe
                    // fallback instead.
                    let json = serialize_response_safely(&response);
                    if let Some((command_name, elapsed)) = command_telemetry {
                        self.runtime_telemetry
                            .record_command(command_name, elapsed, json.len());
                    }
                    if let Err(e) = writeln!(stream, "{}", json) {
                        log::error!("Failed to write response: {}", e);
                        break;
                    }
                }
            }
        }
    }

    pub(super) fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = get_socket_path();

        // Ensure socket directory exists with secure permissions
        ensure_secure_socket_dir(&socket_path).map_err(|error| {
            format!(
                "failed to prepare daemon socket directory {}: {error}",
                socket_path.display()
            )
        })?;

        // Start driver config watcher thread
        let config_watcher = {
            let driver_manager = Arc::clone(&self.driver_manager);
            let audio_manager = Arc::clone(&self.manager);
            let running = Arc::clone(&self.running);
            let pipeline = Arc::clone(&self.system_state);
            let pipeline_mutation = Arc::clone(&self.pipeline_mutation);
            spawn_driver_config_watcher(
                driver_manager,
                audio_manager,
                running,
                pipeline,
                pipeline_mutation,
            )
        };

        // Bind the socket. To avoid a TOCTOU race window between an
        // existence check and a follow-up unlink (which would allow a
        // same-UID hostile actor to swap in their own socket or unrelated
        // file at the path), we try `bind()` first and only fall back to
        // unlinking when we have positively confirmed the existing entry
        // is a stale `AF_UNIX` socket -- never a regular file, FIFO, or
        // symlink. See `bind_unix_socket` below for the full strategy.
        let listener = bind_unix_socket(&socket_path).map_err(|error| {
            format!(
                "failed to bind daemon socket {}: {error}",
                socket_path.display()
            )
        })?;
        println!("Audio daemon listening on {}", socket_path.display());

        // NOTE: the legacy `/tmp/autoeq_audio.sock` symlink that previous
        // versions of the daemon created on each startup has been
        // removed. `/tmp` is world-writable on macOS/Linux, and the prior
        // `remove_file(LEGACY_SOCKET_PATH)` would happily unlink whatever
        // a same-host attacker pre-staged at that path (regular file,
        // FIFO, symlink-to-/etc/passwd, etc.). The `SOTF_LEGACY_SOCKET`
        // opt-in still works for callers that *must* use the legacy
        // path: they get a real socket bound at `LEGACY_SOCKET_PATH`,
        // not a symlink. New clients should use `get_secure_socket_path`.
        let _ = LEGACY_SOCKET_PATH; // keep the constant referenced

        // Accept connections (non-blocking so Ctrl-C can interrupt)
        listener.set_nonblocking(true)?;
        self.spawn_initial_driver_playback();
        let active_clients = Arc::new(AtomicUsize::new(0));
        let mut client_threads: Vec<(std::thread::JoinHandle<()>, UnixStream)> = Vec::new();

        loop {
            if !*self.running.lock() {
                println!("Shutdown requested, exiting");
                break;
            }

            let mut client_index = 0;
            while client_index < client_threads.len() {
                if client_threads[client_index].0.is_finished() {
                    let (thread, _shutdown_stream) = client_threads.swap_remove(client_index);
                    if thread.join().is_err() {
                        log::warn!("IPC client handler panicked");
                    }
                } else {
                    client_index += 1;
                }
            }

            match listener.accept() {
                Ok((stream, _addr)) => {
                    if let Err(e) = stream.set_nonblocking(false) {
                        log::error!("Failed to set client stream to blocking: {}", e);
                        continue;
                    }

                    let peer_class = match verify_peer_credentials(&stream) {
                        Ok(peer_uid) => {
                            let class = classify_peer(peer_uid, security_current_uid());
                            log::debug!(
                                "Accepted connection from UID {} (class {:?})",
                                peer_uid,
                                class
                            );
                            class
                        }
                        Err(e) => {
                            log::warn!("Rejected unauthorized connection: {}", e);
                            continue;
                        }
                    };

                    if !try_acquire_client_slot(&active_clients) {
                        log::warn!(
                            "Rejecting IPC client: maximum of {} active clients reached",
                            MAX_IPC_CLIENTS
                        );
                        continue;
                    }

                    let shutdown_stream = match stream.try_clone() {
                        Ok(stream) => stream,
                        Err(error) => {
                            log::warn!("Failed to clone IPC client for shutdown: {error}");
                            continue;
                        }
                    };
                    let daemon = self.clone();
                    let client_slot = ClientSlot(Arc::clone(&active_clients));

                    let thread = std::thread::spawn(move || {
                        let _client_slot = client_slot;
                        daemon.handle_client(stream, peer_class);
                    });
                    client_threads.push((thread, shutdown_stream));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    // A persistent listener error must not turn the accept
                    // loop into a hot spin. Keep the daemon responsive to
                    // shutdown while applying a small bounded backoff.
                    log::error!("Failed to accept connection: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }

        // Actively unblock and join every daemon-owned client handler. Persistent
        // polling connections otherwise outlive the accept loop until their
        // idle timeout and can retain daemon resources during restart.
        for (_thread, stream) in &client_threads {
            let _ = stream.shutdown(Shutdown::Both);
        }
        for (thread, _shutdown_stream) in client_threads {
            if thread.join().is_err() {
                log::warn!("IPC client handler panicked during shutdown");
            }
        }

        // Cleanup -- only remove our own socket entry, after re-verifying
        // it is still a socket. We deliberately do NOT unlink the legacy
        // `/tmp/autoeq_audio.sock` here: if it exists and is not ours,
        // it's not our business to remove (avoid the prior TOCTOU /
        // symlink-following hazard at shutdown).
        if socket_is_unix_socket(&socket_path) {
            let _ = std::fs::remove_file(&socket_path);
        }

        let _ = config_watcher.join();

        Ok(())
    }
}
