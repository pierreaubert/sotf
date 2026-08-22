use driver_common::DriverStatus;
use serde_json::Value;
use sotf_audio::PluginConfig;
use std::os::unix::net::{UnixListener, UnixStream};

/// Process-lifetime locks serialize ownership of every runtime resource that
/// can affect the HAL transport.
///
/// The control socket is only one resource. Shared memory and both session-key
/// copies can be overridden independently, so locking beside the socket alone
/// allows a daemon with a different socket to rotate another daemon's key.
/// Canonical lock paths and sorted acquisition also prevent alias-based bypasses
/// and overlapping-resource deadlocks.
#[derive(Debug)]
pub(super) struct DaemonInstanceLock {
    _files: Vec<std::fs::File>,
}

pub(super) fn acquire_daemon_instance_lock(
    resource_paths: &[std::path::PathBuf],
) -> std::io::Result<DaemonInstanceLock> {
    if resource_paths.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "daemon ownership requires at least one runtime resource",
        ));
    }

    let mut lock_paths = Vec::with_capacity(resource_paths.len());
    for resource_path in resource_paths {
        let parent = resource_path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "runtime resource {} has no parent directory",
                    resource_path.display()
                ),
            )
        })?;
        let file_name = resource_path.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "runtime resource {} has no file name",
                    resource_path.display()
                ),
            )
        })?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "failed to resolve runtime resource parent {}: {error}",
                    parent.display()
                ),
            )
        })?;
        let mut lock_name = file_name.to_os_string();
        lock_name.push(".owner.lock");
        lock_paths.push(canonical_parent.join(lock_name));
    }
    lock_paths.sort();
    lock_paths.dedup();

    let mut files = Vec::with_capacity(lock_paths.len());
    for lock_path in lock_paths {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .mode(0o600);
        }

        let file = options.open(&lock_path).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "failed to open ownership lock {}: {error}",
                    lock_path.display()
                ),
            )
        })?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "ownership lock {} is not a regular file",
                    lock_path.display()
                ),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }

        file.try_lock().map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                format!(
                    "another sotf-daemon instance owns runtime resource lock {}: {}",
                    lock_path.display(),
                    error
                ),
            )
        })?;
        files.push(file);
    }

    Ok(DaemonInstanceLock { _files: files })
}

pub(super) fn env_path_is_set(key: &str) -> bool {
    std::env::var_os(key).is_some_and(|value| !value.as_os_str().is_empty())
}

pub(super) fn transport_snapshot_and_faults(
    state_name: &str,
    driver_status: &DriverStatus,
    engine_state: &sotf_audio::AudioEngineState,
) -> (Value, Vec<Value>) {
    let playing = state_name == "Playing";
    let driver_unavailable =
        playing && (!driver_status.driver_installed || !driver_status.driver_ready);
    let hal_stream_inactive = playing && !driver_unavailable && !driver_status.capture_active;
    let input_frames_missing =
        playing && !driver_unavailable && engine_state.playback_frames_received == 0;
    let output_callbacks_missing = playing && engine_state.playback_callback_count == 0;
    let output_frames_missing = playing
        && engine_state.playback_frames_received > 0
        && engine_state.playback_frames_written == 0;
    let output_device_unresolved = playing && engine_state.playback_output_device.is_none();

    let input_status = if !playing {
        "idle"
    } else if driver_unavailable {
        "driver_unavailable"
    } else if hal_stream_inactive {
        "hal_stream_inactive"
    } else if input_frames_missing {
        "input_frames_missing"
    } else {
        "flowing"
    };

    let output_status = if !playing {
        "idle"
    } else if output_callbacks_missing {
        "output_callbacks_missing"
    } else if output_frames_missing {
        "output_frames_missing"
    } else if output_device_unresolved {
        "output_device_unresolved"
    } else {
        "flowing"
    };

    let mut faults = Vec::new();
    if playing {
        if driver_unavailable {
            faults.push(serde_json::json!({
                "code": "driver_unavailable",
                "severity": "error",
                "message": "Playback is marked Playing but the HAL driver is not ready.",
            }));
        }
        if hal_stream_inactive {
            faults.push(serde_json::json!({
                "code": "hal_stream_inactive",
                "severity": "warning",
                "message": "Playback is marked Playing but the HAL stream is not active.",
            }));
        }
        if input_frames_missing {
            faults.push(serde_json::json!({
                "code": "input_frames_missing",
                "severity": "error",
                "message": "Playback is marked Playing but no frames have reached the playback thread.",
            }));
        }

        if output_callbacks_missing {
            faults.push(serde_json::json!({
                "code": "output_callbacks_missing",
                "severity": "error",
                "message": "Playback is marked Playing but no hardware output callbacks have been observed.",
            }));
        }
        if output_frames_missing {
            faults.push(serde_json::json!({
                "code": "output_frames_missing",
                "severity": "error",
                "message": "The playback thread received frames but has not written any to the hardware ring.",
            }));
        }
        if output_device_unresolved {
            faults.push(serde_json::json!({
                "code": "output_device_unresolved",
                "severity": "warning",
                "message": "Playback is active but no hardware output device has been resolved yet.",
            }));
        }
    }

    (
        serde_json::json!({
            "input_status": input_status,
            "output_status": output_status,
            "input": {
                "status": input_status,
                "frames_received": engine_state.playback_frames_received,
                "hal_capture_active": driver_status.capture_active,
                "driver_installed": driver_status.driver_installed,
                "driver_ready": driver_status.driver_ready,
            },
            "output": {
                "status": output_status,
                "callbacks": engine_state.playback_callback_count,
                "frames_written": engine_state.playback_frames_written,
                "frames_dropped": engine_state.playback_frames_dropped,
                "device": engine_state.playback_output_device.as_deref(),
                "effective_sample_rate": engine_state.playback_effective_sample_rate,
            },
            "input_frames_received": engine_state.playback_frames_received,
            "output_callbacks": engine_state.playback_callback_count,
            "output_frames_written": engine_state.playback_frames_written,
            "frames_dropped": engine_state.playback_frames_dropped,
            "hal_capture_active": driver_status.capture_active,
        }),
        faults,
    )
}

pub(super) fn push_metering_faults(state_name: &str, metering: &Value, faults: &mut Vec<Value>) {
    if state_name != "Playing" {
        return;
    }

    if metering["sources"]["input"]["status"].as_str() == Some("fallback_zero") {
        faults.push(serde_json::json!({
            "code": "input_metering_unavailable",
            "severity": "warning",
            "message": "Input meters are channel-shaped fallback zeros, not analyzer data.",
        }));
    }
    if metering["sources"]["output"]["status"].as_str() == Some("fallback_zero") {
        faults.push(serde_json::json!({
            "code": "output_metering_unavailable",
            "severity": "warning",
            "message": "Output meters are channel-shaped fallback zeros, not analyzer data.",
        }));
    }
}

pub(super) fn parameter_descriptor_to_json(spec: &sotf_plugins::param_specs::ParamSpec) -> Value {
    use sotf_plugins::param_specs::{ParamType, UpdateMode};

    let mut descriptor = serde_json::json!({
        "key": spec.engine_key,
        "name": spec.name,
        "unit": spec.unit,
        "group": spec.group,
        "doc": spec.doc,
        "update_mode": match spec.update_mode {
            UpdateMode::Realtime => "realtime",
            UpdateMode::Structural => "structural",
        },
    });

    let Some(object) = descriptor.as_object_mut() else {
        // The value is constructed above with json! and should always be an
        // object. Keep this helper total anyway so malformed future edits do
        // not panic an IPC handler while building plugin metadata.
        return descriptor;
    };

    match spec.param_type {
        ParamType::Float {
            default,
            min,
            max,
            step,
        } => {
            object.insert("type".to_string(), serde_json::json!("float"));
            object.insert("default".to_string(), serde_json::json!(default));
            object.insert("min".to_string(), serde_json::json!(min));
            object.insert("max".to_string(), serde_json::json!(max));
            object.insert("step".to_string(), serde_json::json!(step));
        }
        ParamType::Int {
            default,
            min,
            max,
            step,
        } => {
            object.insert("type".to_string(), serde_json::json!("int"));
            object.insert("default".to_string(), serde_json::json!(default));
            object.insert("min".to_string(), serde_json::json!(min));
            object.insert("max".to_string(), serde_json::json!(max));
            object.insert("step".to_string(), serde_json::json!(step));
        }
        ParamType::Bool {
            default,
            true_label,
            false_label,
        } => {
            object.insert("type".to_string(), serde_json::json!("bool"));
            object.insert("default".to_string(), serde_json::json!(default));
            object.insert("true_label".to_string(), serde_json::json!(true_label));
            object.insert("false_label".to_string(), serde_json::json!(false_label));
        }
        ParamType::Choice {
            default_index,
            labels,
        } => {
            object.insert("type".to_string(), serde_json::json!("choice"));
            object.insert("default".to_string(), serde_json::json!(default_index));
            object.insert("choices".to_string(), serde_json::json!(labels));
        }
        ParamType::FilePath => {
            object.insert("type".to_string(), serde_json::json!("file_path"));
        }
    }

    descriptor
}

pub(super) fn sanitize_user_plugins(plugins: Vec<PluginConfig>) -> Vec<PluginConfig> {
    plugins
        .into_iter()
        .filter(|p| {
            let pt = p.plugin_type.as_str();
            if pt == "hal_input" || pt == "hal_output" {
                log::warn!(
                    "Stripping obsolete '{}' plugin from chain - decoder thread handles driver I/O directly",
                    pt
                );
                false
            } else if pt == "loudness_monitor" {
                log::warn!("Stripping user-supplied loudness_monitor - daemon injects metering");
                false
            } else {
                true
            }
        })
        .collect()
}

pub(super) fn build_driver_plugin_chain(
    plugins: Vec<PluginConfig>,
) -> (Vec<PluginConfig>, usize, usize) {
    let mut final_plugins = Vec::with_capacity(plugins.len() + 2);

    let input_monitor_index = 0;
    final_plugins.push(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters: serde_json::json!({}),
    });

    final_plugins.extend(plugins);

    let output_monitor_index = final_plugins.len();
    final_plugins.push(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters: serde_json::json!({}),
    });

    (final_plugins, input_monitor_index, output_monitor_index)
}

pub(super) fn build_driver_plugin_graph(
    mut graph: sotf_audio::engine::PluginGraphConfig,
    input_channels: usize,
    output_channels: usize,
) -> Result<(sotf_audio::engine::PluginGraphConfig, usize, usize), String> {
    use sotf_audio::engine::{PluginGraphEdgeConfig, PluginGraphNodeConfig};
    use std::collections::HashSet;

    graph
        .validate()
        .map_err(|error| format!("Invalid plugin graph: {error}"))?;
    if graph.nodes.is_empty() {
        return Err("Plugin graph must contain at least one user node".to_string());
    }
    if let Some(node) = graph.nodes.iter().find(|node| {
        matches!(
            node.plugin_type.as_str(),
            "hal_input" | "hal_output" | "loudness_monitor" | "spectrum_analyzer"
        )
    }) {
        return Err(format!(
            "Plugin graph node {} uses daemon-owned system plugin type '{}'",
            node.id, node.plugin_type
        ));
    }

    let incoming = graph
        .edges
        .iter()
        .map(|edge| edge.to_node)
        .collect::<HashSet<_>>();
    let outgoing = graph
        .edges
        .iter()
        .map(|edge| edge.from_node)
        .collect::<HashSet<_>>();
    let roots = graph
        .nodes
        .iter()
        .filter_map(|node| (!incoming.contains(&node.id)).then_some(node.id))
        .collect::<Vec<_>>();
    let leaves = graph
        .nodes
        .iter()
        .filter_map(|node| (!outgoing.contains(&node.id)).then_some(node.id))
        .collect::<Vec<_>>();
    if roots.is_empty() || leaves.is_empty() {
        return Err("Plugin graph must have at least one input and output path".to_string());
    }

    let input_monitor_id = graph
        .nodes
        .iter()
        .map(|node| node.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "Plugin graph node id space exhausted".to_string())?;
    let output_monitor_id = input_monitor_id
        .checked_add(1)
        .ok_or_else(|| "Plugin graph node id space exhausted".to_string())?;
    let input_monitor = PluginGraphNodeConfig::try_new(
        input_monitor_id,
        "loudness_monitor",
        serde_json::json!({}),
        input_channels,
    )
    .map_err(|error| error.to_string())?;
    let output_monitor = PluginGraphNodeConfig::try_new(
        output_monitor_id,
        "loudness_monitor",
        serde_json::json!({}),
        output_channels,
    )
    .map_err(|error| error.to_string())?;

    graph.nodes.insert(0, input_monitor);
    graph.nodes.push(output_monitor);
    graph.edges.extend(
        roots
            .into_iter()
            .map(|root| PluginGraphEdgeConfig::new(input_monitor_id, root)),
    );
    graph.edges.extend(
        leaves
            .into_iter()
            .map(|leaf| PluginGraphEdgeConfig::new(leaf, output_monitor_id)),
    );
    graph
        .validate()
        .map_err(|error| format!("Invalid monitored plugin graph: {error}"))?;

    let output_monitor_index = graph.nodes.len() - 1;
    Ok((graph, 0, output_monitor_index))
}

/// Bind a `UnixListener` at `socket_path` defending against TOCTOU on
/// stale-socket cleanup.
///
/// Strategy:
/// 1. Try `bind` directly -- if it succeeds, we own a fresh socket.
/// 2. On `AddrInUse`, probe with `connect`. If something accepts, another
///    daemon is alive; bail out.
/// 3. Otherwise, `lstat` the existing entry. Only unlink it if it is
///    *actually* a Unix socket (`S_ISSOCK`). Regular files, FIFOs, and
///    symlinks (which an attacker could plant) are left alone and bind
///    fails. This means we never follow a symlink at the socket path,
///    and never unlink unrelated files.
/// 4. Retry `bind` once after a successful unlink. If a racing process
///    re-creates the entry between unlink and bind, the second bind
///    fails and we return the error to the caller (no infinite retry).
pub(super) fn bind_unix_socket(socket_path: &std::path::Path) -> std::io::Result<UnixListener> {
    match UnixListener::bind(socket_path) {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // Existing entry. See if it's a live daemon.
            if let Ok(_stream) = UnixStream::connect(socket_path) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "Another daemon instance is already running",
                ));
            }
            // Refuse to unlink anything that isn't an AF_UNIX socket.
            // `lstat` -- explicitly do NOT follow symlinks.
            if !socket_is_unix_socket(socket_path) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!(
                        "{} exists and is not a Unix socket; refusing to remove",
                        socket_path.display()
                    ),
                ));
            }
            std::fs::remove_file(socket_path)?;
            UnixListener::bind(socket_path)
        }
        Err(e) => Err(e),
    }
}

/// Return true iff `path` is a Unix-domain socket (lstat, does NOT
/// follow symlinks).
pub(super) fn socket_is_unix_socket(path: &std::path::Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::symlink_metadata(path) {
        Ok(md) => md.file_type().is_socket(),
        Err(_) => false,
    }
}

pub(super) fn is_safe_output_device_name(name: &str) -> bool {
    let lowercase_name = name.to_ascii_lowercase();
    ![
        "sotf",
        "blackhole",
        "zoomaudio",
        "loopback",
        "virtual",
        "soundflower",
        "background music",
        "audio bridge",
    ]
    .iter()
    .any(|virtual_name| lowercase_name.contains(virtual_name))
}

pub(super) fn list_audio_devices() -> Result<Vec<serde_json::Value>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let mut devices = Vec::new();

    if let Some(default_out) = host.default_output_device() {
        let name = default_out
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        devices.push(serde_json::json!({
            "name": name,
            "is_default": true,
        }));
    }

    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            let name = device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string());

            if devices
                .iter()
                .any(|d| d["name"] == name && d["is_default"] == true)
            {
                continue;
            }

            let mut device_info = serde_json::json!({
                "name": name,
                "is_default": false,
            });

            if let Ok(config) = device.default_output_config() {
                device_info["channels"] = config.channels().into();
                device_info["sample_rate"] = config.sample_rate().into();
            }

            devices.push(device_info);
        }
    }

    // Include ASIO devices (Windows only, requires asio feature)
    for asio_name in sotf_audio::devices::list_asio_devices() {
        devices.push(serde_json::json!({
            "name": asio_name,
            "is_default": false,
            "backend": "ASIO",
        }));
    }

    if devices.is_empty() {
        Err("No audio devices found".to_string())
    } else {
        Ok(devices)
    }
}

#[cfg(test)]
mod tests {
    use super::{acquire_daemon_instance_lock, bind_unix_socket, socket_is_unix_socket};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn daemon_ownership_lock_rejects_overlapping_runtime_resources() {
        let directory = tempdir().expect("create temporary directory");
        let shared_memory = directory.path().join("audio.shm");
        let session_key = directory.path().join("session.key");
        let alternate_socket = directory.path().join("alternate.sock");

        let first = acquire_daemon_instance_lock(&[shared_memory.clone(), session_key.clone()])
            .expect("acquire first transport ownership");
        let error = acquire_daemon_instance_lock(&[alternate_socket, session_key.clone()])
            .expect_err("overlapping session key must have one owner");
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);

        drop(first);
        acquire_daemon_instance_lock(&[shared_memory, session_key])
            .expect("ownership must be released when the daemon lock drops");
    }

    #[cfg(unix)]
    #[test]
    fn daemon_ownership_lock_rejects_a_symlink_lock_file() {
        let directory = tempdir().expect("create temporary directory");
        let resource = directory.path().join("audio.shm");
        let target = directory.path().join("attacker-target");
        let lock_path = directory.path().join("audio.shm.owner.lock");
        fs::write(&target, b"preserve target").expect("create symlink target");
        std::os::unix::fs::symlink(&target, &lock_path).expect("plant lock symlink");

        let error = acquire_daemon_instance_lock(&[resource])
            .expect_err("ownership lock must not follow a symlink");
        assert_ne!(error.kind(), std::io::ErrorKind::AddrInUse);
        assert_eq!(
            fs::read(&target).expect("read preserved target"),
            b"preserve target"
        );
        assert!(
            fs::symlink_metadata(&lock_path)
                .expect("read preserved lock symlink")
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn bind_unix_socket_accepts_a_fresh_path_and_reports_socket_type() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("daemon.sock");

        let listener = bind_unix_socket(&path).expect("bind fresh socket");
        assert!(socket_is_unix_socket(&path));
        drop(listener);
    }

    #[test]
    fn bind_unix_socket_rejects_a_live_daemon_without_unlinking_it() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("daemon.sock");
        let listener = bind_unix_socket(&path).expect("bind live socket");

        let error = bind_unix_socket(&path).expect_err("live daemon must remain authoritative");
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        assert!(socket_is_unix_socket(&path));
        drop(listener);
    }

    #[test]
    fn bind_unix_socket_rejects_regular_files_without_removing_them() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("daemon.sock");
        fs::write(&path, b"do not remove").expect("create regular file");

        assert!(bind_unix_socket(&path).is_err());
        assert_eq!(
            fs::read(&path).expect("read preserved file"),
            b"do not remove"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bind_unix_socket_rejects_symlinks_without_following_or_removing_them() {
        let directory = tempdir().expect("create temporary directory");
        let target = directory.path().join("target");
        let path = directory.path().join("daemon.sock");
        fs::write(&target, b"target file").expect("create symlink target");
        std::os::unix::fs::symlink(&target, &path).expect("create symlink");

        assert!(bind_unix_socket(&path).is_err());
        assert!(
            fs::symlink_metadata(&path)
                .expect("read preserved symlink")
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn bind_unix_socket_reclaims_a_stale_socket_only() {
        let directory = tempdir().expect("create temporary directory");
        let path = directory.path().join("daemon.sock");
        let stale = bind_unix_socket(&path).expect("bind stale fixture");
        drop(stale);

        let replacement = bind_unix_socket(&path).expect("replace stale socket");
        assert!(socket_is_unix_socket(&path));
        drop(replacement);
    }
}
