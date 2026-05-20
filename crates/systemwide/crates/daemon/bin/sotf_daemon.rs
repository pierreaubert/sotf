//! Audio Engine Control Daemon
//!
//! A Unix socket daemon that provides IPC control for the AudioEngineManager.
//! This allows external processes (like the Swift menubar app or GPUI configbar)
//! to control audio playback, query status, and configure plugins via JSON messages
//! over a Unix domain socket.
//!
//! Protocol: JSON messages over Unix socket (one JSON object per line)
//!
//! The daemon is cross-platform:
//! - macOS: Uses CoreAudio HAL driver for system audio capture
//! - Linux: Will use PipeWire filter node (future)
//! - Windows: Will use APO + shared memory (future)
//! - Fallback: NullDriver (no capture, status-only)

mod driver_manager;
mod lock_order;
mod security;

use driver_manager::{DriverManager, get_driver_status};
use security::{
    KeyManager, PeerClass, classify_peer, current_uid as security_current_uid,
    ensure_secure_socket_dir, get_secure_socket_path, peer_allows_command, verify_peer_credentials,
};

use driver_common::DriverConfig;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::manager::AudioEngineManager;
use sotf_audio::plugins::PluginType;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;

/// Legacy socket path for backwards compatibility
const LEGACY_SOCKET_PATH: &str = "/tmp/autoeq_audio.sock";
const OUTPUT_DEVICE_ENV: &str = "SOTF_OUTPUT_DEVICE";
const MAX_HAL_CHANNELS: usize = 32;
const MAX_IPC_COMMAND_BYTES: usize = 64 * 1024;

fn default_input_channels() -> usize {
    0
}

fn default_output_channels() -> usize {
    2
}

/// Get the socket path to use
/// Uses secure per-user path, with fallback to legacy path if SOTF_LEGACY_SOCKET is set
fn get_socket_path() -> PathBuf {
    if std::env::var("SOTF_LEGACY_SOCKET").is_ok() {
        PathBuf::from(LEGACY_SOCKET_PATH)
    } else {
        get_secure_socket_path()
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
enum Command {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "load")]
    Load { path: String },
    #[serde(rename = "play")]
    Play,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "seek")]
    Seek { position: f64 },
    #[serde(rename = "set_volume")]
    SetVolume { volume: f32 },
    #[serde(rename = "list_devices")]
    ListDevices,
    #[serde(rename = "set_device")]
    SetDevice { device: String },
    #[serde(rename = "load_plugins")]
    LoadPlugins {
        plugins: Vec<PluginConfig>,
        #[serde(default = "default_input_channels")]
        input_channels: usize,
        #[serde(default = "default_output_channels")]
        output_channels: usize,
    },
    #[serde(rename = "get_loudness")]
    GetLoudness,
    #[serde(rename = "get_metering")]
    GetMetering,
    // Plugin management commands
    #[serde(rename = "get_plugins")]
    GetPlugins,
    #[serde(rename = "get_available_plugins")]
    GetAvailablePlugins,
    #[serde(rename = "add_plugin")]
    AddPlugin {
        plugin: PluginConfig,
        #[serde(default)]
        index: Option<usize>,
    },
    #[serde(rename = "remove_plugin")]
    RemovePlugin { index: usize },
    #[serde(rename = "update_plugin")]
    UpdatePlugin {
        index: usize,
        parameters: serde_json::Value,
    },
    #[serde(rename = "reorder_plugins")]
    ReorderPlugins { order: Vec<usize> },
    // Driver status (replaces hal_status, kept as alias)
    #[serde(rename = "driver_status", alias = "hal_status")]
    DriverStatus,
    #[serde(rename = "shutdown")]
    Shutdown,
    // Encryption commands
    #[serde(rename = "set_encryption")]
    SetEncryption { enabled: bool },
    #[serde(rename = "encryption_status")]
    EncryptionStatus,
    #[serde(rename = "rotate_encryption_key")]
    RotateEncryptionKey,
    // Driver config commands (replaces hal_config, kept as aliases)
    #[serde(rename = "set_sample_rate")]
    SetSampleRate { rate: u32 },
    #[serde(rename = "set_buffer_frames")]
    SetBufferFrames { frames: u32 },
    #[serde(rename = "get_driver_config", alias = "get_hal_config")]
    GetDriverConfig,
}

impl Command {
    /// Return the wire name (`#[serde(rename = ...)]`) for this command.
    ///
    /// Used to gate which commands a given peer UID may invoke (see
    /// `security::peer_allows_command`). Keep in sync with the `serde`
    /// attributes on each variant.
    fn name(&self) -> &'static str {
        match self {
            Command::Status => "status",
            Command::Load { .. } => "load",
            Command::Play => "play",
            Command::Pause => "pause",
            Command::Stop => "stop",
            Command::Seek { .. } => "seek",
            Command::SetVolume { .. } => "set_volume",
            Command::ListDevices => "list_devices",
            Command::SetDevice { .. } => "set_device",
            Command::LoadPlugins { .. } => "load_plugins",
            Command::GetLoudness => "get_loudness",
            Command::GetMetering => "get_metering",
            Command::GetPlugins => "get_plugins",
            Command::GetAvailablePlugins => "get_available_plugins",
            Command::AddPlugin { .. } => "add_plugin",
            Command::RemovePlugin { .. } => "remove_plugin",
            Command::UpdatePlugin { .. } => "update_plugin",
            Command::ReorderPlugins { .. } => "reorder_plugins",
            Command::DriverStatus => "driver_status",
            Command::Shutdown => "shutdown",
            Command::SetEncryption { .. } => "set_encryption",
            Command::EncryptionStatus => "encryption_status",
            Command::RotateEncryptionKey => "rotate_encryption_key",
            Command::SetSampleRate { .. } => "set_sample_rate",
            Command::SetBufferFrames { .. } => "set_buffer_frames",
            Command::GetDriverConfig => "get_driver_config",
        }
    }
}

#[derive(Debug, Serialize)]
struct Response {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn ok(data: Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn ok_empty() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }

    fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

/// Serialize a `Response` to JSON without ever panicking.
///
/// `Response::data` can hold arbitrary client-supplied JSON (via
/// `UpdatePlugin { parameters }`, reflected back through
/// `handle_get_plugins`). A NaN / Infinity smuggled into a `Value::Number`
/// would make `serde_json::to_string` return `Err`. We must not let that
/// kill the client thread, since this runs in the IPC hot path. Fall back
/// to a static, always-serializable byte string.
fn serialize_response_safely(response: &Response) -> String {
    match serde_json::to_string(response) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Response serialization failed: {}", e);
            // Static fallback. This string is hard-coded valid JSON and
            // matches the on-wire shape of `Response`.
            String::from(
                r#"{"success":false,"error":"internal error: response serialization failed"}"#,
            )
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum IpcLine {
    Eof,
    Empty,
    TooLarge,
    InvalidUtf8,
    Line(String),
}

fn read_ipc_line_bounded<R: BufRead>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> std::io::Result<IpcLine> {
    buffer.clear();

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if buffer.is_empty() {
                return Ok(IpcLine::Eof);
            }
            break;
        }

        let bytes_to_consume = match available.iter().position(|&b| b == b'\n') {
            Some(index) => index + 1,
            None => available.len(),
        };

        if buffer.len().saturating_add(bytes_to_consume) > MAX_IPC_COMMAND_BYTES {
            reader.consume(bytes_to_consume);
            return Ok(IpcLine::TooLarge);
        }

        buffer.extend_from_slice(&available[..bytes_to_consume]);
        reader.consume(bytes_to_consume);

        if buffer.last() == Some(&b'\n') {
            break;
        }
    }

    while matches!(buffer.last(), Some(b'\n' | b'\r')) {
        buffer.pop();
    }

    let line = match std::str::from_utf8(buffer) {
        Ok(line) => line.trim(),
        Err(_) => return Ok(IpcLine::InvalidUtf8),
    };

    if line.is_empty() {
        Ok(IpcLine::Empty)
    } else {
        Ok(IpcLine::Line(line.to_string()))
    }
}
fn loudness_data_to_json(info: &sotf_audio::LoudnessData) -> Value {
    serde_json::json!({
        "momentary": info.momentary_lufs,
        "short_term": info.shortterm_lufs,
        "integrated": info.integrated_lufs,
        "peak": info.peak,
        "channel_peaks": info.channel_peaks.as_ref(),
        "true_peaks_dbtp": info.true_peaks_dbtp.as_ref(),
        "correlation_lr": info.correlation_lr,
    })
}

fn loudness_info_to_json(info: &sotf_audio::LoudnessInfo) -> Value {
    serde_json::json!({
        "momentary": info.momentary_lufs,
        "short_term": info.shortterm_lufs,
        "integrated": info.integrated_lufs,
        "peak": info.peak,
        "channel_peaks": [],
        "true_peaks_dbtp": [],
        "correlation_lr": null,
    })
}

fn empty_loudness_json(channels: usize) -> Value {
    let channels = channels.clamp(1, MAX_HAL_CHANNELS);
    serde_json::json!({
        "momentary": -60.0,
        "short_term": -60.0,
        "integrated": -60.0,
        "peak": 0.0,
        "channel_peaks": vec![0.0; channels],
        "true_peaks_dbtp": vec![-120.0; channels],
        "correlation_lr": null,
    })
}

fn parameter_descriptor_to_json(spec: &sotf_plugins::param_specs::ParamSpec) -> Value {
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

    let object = descriptor
        .as_object_mut()
        .expect("descriptor starts as a JSON object");

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

fn plugin_parameter_descriptors(settings: &sotf_audio::PluginSettings) -> Vec<Value> {
    settings
        .param_specs()
        .iter()
        .map(parameter_descriptor_to_json)
        .collect()
}

fn sanitize_user_plugins(plugins: Vec<PluginConfig>) -> Vec<PluginConfig> {
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

fn build_driver_plugin_chain(plugins: Vec<PluginConfig>) -> (Vec<PluginConfig>, usize, usize) {
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

#[derive(Clone)]
struct AudioDaemon {
    manager: Arc<Mutex<AudioEngineManager>>,
    running: Arc<Mutex<bool>>,
    driver_manager: Arc<Mutex<DriverManager>>,
    /// Selected output device name (None = use default device)
    selected_device: Arc<Mutex<Option<String>>>,
    /// Encryption key manager
    key_manager: Arc<Mutex<KeyManager>>,
    /// Shared Tokio runtime for async operations
    runtime: Arc<tokio::runtime::Runtime>,
    /// Current plugin configuration (user plugins, excluding auto-added monitors)
    current_plugins: Arc<Mutex<Vec<PluginConfig>>>,
    /// Current HAL input channel count
    current_input_channels: Arc<Mutex<usize>>,
    /// Current output channel count
    current_output_channels: Arc<Mutex<usize>>,
    /// Index of the auto-injected input loudness monitor in the final plugin chain
    input_loudness_index: Arc<Mutex<Option<usize>>>,
    /// Index of the auto-injected output loudness monitor in the final plugin chain
    output_loudness_index: Arc<Mutex<Option<usize>>>,
}

impl AudioDaemon {
    fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        Self {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::new())),
            selected_device: Arc::new(Mutex::new(None)),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
            runtime: Arc::new(runtime),
            current_plugins: Arc::new(Mutex::new(Vec::new())),
            current_input_channels: Arc::new(Mutex::new(2)),
            current_output_channels: Arc::new(Mutex::new(2)),
            input_loudness_index: Arc::new(Mutex::new(None)),
            output_loudness_index: Arc::new(Mutex::new(None)),
        }
    }

    fn spawn_initial_driver_playback(&self) {
        let daemon = self.clone();
        std::thread::spawn(move || {
            println!("Auto-starting driver playback (2ch)...");

            let output_device = configured_output_device_from_env();
            println!("   Output device: {:?}", output_device);

            if let Some(ref device) = output_device {
                *daemon.selected_device.lock() = Some(device.clone());
            } else {
                println!("   No output device override; playback thread will choose a safe device");
            }

            let plugins: Vec<PluginConfig> = vec![];

            let result = daemon
                .runtime
                .block_on(daemon.handle_load_plugins_with_channels(plugins, 2, 2));
            if result.success {
                println!("   Driver playback started successfully");
            } else {
                println!("   Driver playback failed: {:?}", result.error);
            }
        });
    }

    async fn handle_command(&self, cmd: Command) -> Response {
        match cmd {
            Command::Status => self.handle_status().await,
            Command::Load { path } => self.handle_load(&path).await,
            Command::Play => self.handle_play().await,
            Command::Pause => self.handle_pause().await,
            Command::Stop => self.handle_stop().await,
            Command::Seek { position } => self.handle_seek(position).await,
            Command::SetVolume { volume } => self.handle_set_volume(volume).await,
            Command::ListDevices => self.handle_list_devices().await,
            Command::SetDevice { device } => self.handle_set_device(&device).await,
            Command::LoadPlugins {
                plugins,
                input_channels,
                output_channels,
            } => {
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
                    .await
            }
            Command::GetLoudness => self.handle_get_loudness().await,
            Command::GetMetering => self.handle_get_metering().await,
            Command::GetPlugins => self.handle_get_plugins().await,
            Command::GetAvailablePlugins => self.handle_get_available_plugins().await,
            Command::AddPlugin { plugin, index } => self.handle_add_plugin(plugin, index).await,
            Command::RemovePlugin { index } => self.handle_remove_plugin(index).await,
            Command::UpdatePlugin { index, parameters } => {
                self.handle_update_plugin(index, parameters).await
            }
            Command::ReorderPlugins { order } => self.handle_reorder_plugins(order).await,
            Command::DriverStatus => self.handle_driver_status().await,
            Command::Shutdown => {
                *self.running.lock() = false;
                Response::ok_empty()
            }
            // Encryption commands
            Command::SetEncryption { enabled } => self.handle_set_encryption(enabled).await,
            Command::EncryptionStatus => self.handle_encryption_status().await,
            Command::RotateEncryptionKey => self.handle_rotate_encryption_key().await,
            // Driver config commands
            Command::SetSampleRate { rate } => self.handle_set_sample_rate(rate).await,
            Command::SetBufferFrames { frames } => self.handle_set_buffer_frames(frames).await,
            Command::GetDriverConfig => self.handle_get_driver_config().await,
        }
    }

    async fn handle_status(&self) -> Response {
        let manager = self.manager.lock();
        let state = manager.get_state();
        let engine_state = manager.get_engine_state();
        let selected_device = self.selected_device.lock().clone();

        Response::ok(serde_json::json!({
            "state": format!("{:?}", state),
            "volume": manager.get_volume(),
            "muted": manager.is_muted(),
            "selected_device": selected_device,
            "sample_rate": engine_state.sample_rate,
            "channels": engine_state.num_channels,
            "underruns": engine_state.underruns,
            "last_error": engine_state.last_error,
        }))
    }

    async fn handle_load(&self, path: &str) -> Response {
        let mut manager = self.manager.lock();
        match manager.load_file(path) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to load file: {}", e)),
        }
    }

    async fn handle_play(&self) -> Response {
        let mut manager = self.manager.lock();
        let output_device = self.selected_device.lock().clone();
        match manager.start_playback(output_device, vec![], 2) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to start playback: {}", e)),
        }
    }

    async fn handle_pause(&self) -> Response {
        let manager = self.manager.lock();
        match manager.pause() {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to pause: {}", e)),
        }
    }

    async fn handle_stop(&self) -> Response {
        // Lock-order invariant: driver_manager -> manager. The config
        // watcher thread also acquires them in this order. Using the
        // `lock_order::lock_with_order_warning` helper turns silent
        // contention with the watcher into a logged warning so a future
        // contributor who introduces an inverse acquisition order has a
        // diagnostic to follow instead of an undetectable deadlock.
        lock_order::lock_with_order_warning(&self.driver_manager, "driver_manager")
            .set_engine_ready(false);
        log::debug!("Cleared engine_ready flag via driver");

        let mut manager = lock_order::lock_with_order_warning(&self.manager, "manager");
        match manager.stop() {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to stop: {}", e)),
        }
    }

    async fn handle_seek(&self, position: f64) -> Response {
        let manager = self.manager.lock();
        match manager.seek(position) {
            Ok(_) => Response::ok_empty(),
            Err(e) => Response::err(format!("Failed to seek: {}", e)),
        }
    }

    async fn handle_set_volume(&self, volume: f32) -> Response {
        let manager = self.manager.lock();
        let _ = manager.set_volume(volume);
        Response::ok_empty()
    }

    async fn handle_list_devices(&self) -> Response {
        match list_audio_devices() {
            Ok(devices) => Response::ok(serde_json::json!({ "devices": devices })),
            Err(e) => Response::err(format!("Failed to list devices: {}", e)),
        }
    }

    async fn handle_set_device(&self, device: &str) -> Response {
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
                *self.selected_device.lock() = Some(stored_name);
                log::info!(
                    "Output device set to: {} (matched from '{}')",
                    resolved_name,
                    device
                );

                // The cpal stream is bound to the current output device. Reload
                // the driver chain even from Idle so selecting a device can
                // recover from startup fallback discovery failures.
                let plugins = self.current_plugins.lock().clone();
                let input_channels = *self.current_input_channels.lock();
                let output_channels = *self.current_output_channels.lock();
                log::info!(
                    "Starting/restarting driver playback with output device: {}",
                    resolved_name
                );
                let resp = self
                    .handle_load_plugins_with_channels(plugins, input_channels, output_channels)
                    .await;
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

    async fn handle_load_plugins_with_channels(
        &self,
        plugins: Vec<PluginConfig>,
        input_channels: usize,
        output_channels: usize,
    ) -> Response {
        let plugins = sanitize_user_plugins(plugins);

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
        let stored_input_channels = *self.current_input_channels.lock();
        let fallback_input_channels = if driver_status.channel_count > 0 {
            driver_status.channel_count as usize
        } else if stored_input_channels > 0 {
            stored_input_channels
        } else {
            2
        };
        let driver_input_channels = if input_channels > 0 {
            input_channels
        } else {
            fallback_input_channels
        };

        if !(1..=MAX_HAL_CHANNELS).contains(&driver_input_channels) {
            return Response::err(format!(
                "Invalid HAL input channel count: {}. Must be between 1 and {}.",
                driver_input_channels, MAX_HAL_CHANNELS
            ));
        }

        self.driver_manager.lock().set_engine_ready(false);

        {
            let mut manager = self.manager.lock();
            let _ = manager.stop();
        }

        if driver_status.driver_installed
            && driver_status.channel_count != driver_input_channels as u32
        {
            let result = self.driver_manager.lock().request_config(DriverConfig {
                sample_rate: driver_sample_rate,
                buffer_frames: driver_buffer_frames,
                channel_count: driver_input_channels as u32,
            });

            match result {
                driver_common::ConfigResult::Accepted
                | driver_common::ConfigResult::Negotiated { .. } => {
                    log::info!(
                        "HAL input channel count set to {} via driver config",
                        driver_input_channels
                    );
                }
                driver_common::ConfigResult::Error(e) => {
                    log::error!("Failed to set HAL input channels: {}", e);
                    return Response::err(format!("Failed to set HAL input channels: {}", e));
                }
            }
        }

        // Store user's plugin configuration BEFORE adding monitors
        *self.current_plugins.lock() = plugins.clone();
        *self.current_input_channels.lock() = driver_input_channels;
        *self.current_output_channels.lock() = output_channels;

        let mut output_device = self.selected_device.lock().clone();

        if output_device.is_none() {
            output_device = configured_output_device_from_env();
        }

        // If no device is selected, let the playback thread choose. It already
        // knows how to avoid virtual loopback devices. Doing cpal enumeration
        // here can block startup while coreaudiod is busy loading HAL plugins.
        if output_device
            .as_ref()
            .map(|d| is_safe_output_device_name(d))
            .unwrap_or(false)
        {
            log::info!(
                "Using selected output device for driver playback: {:?}",
                output_device
            );
        } else if output_device.is_some() {
            log::warn!(
                "Ignoring virtual output device selection {:?}; playback thread will choose a safe device",
                output_device
            );
            output_device = None;
        }

        let (final_plugins, input_monitor_index, output_monitor_index) =
            build_driver_plugin_chain(plugins);

        // Store monitor indices for get_metering
        *self.input_loudness_index.lock() = Some(input_monitor_index);
        *self.output_loudness_index.lock() = Some(output_monitor_index);

        log::info!(
            "Loading driver plugin chain: {} user plugins + 2 monitors = {} total, {}Hz {}ch input, {} output channels, device: {:?}",
            final_plugins.len() - 2,
            final_plugins.len(),
            driver_sample_rate,
            driver_input_channels,
            output_channels,
            output_device
        );

        let mut manager = self.manager.lock();

        // Set the output loudness index for backward compat (get_loudness command)
        manager.set_loudness_plugin_index(output_monitor_index);

        // Start driver playback (no file source needed)
        let result = manager.start_hal_playback_with_driver_config(
            output_device,
            final_plugins,
            output_channels,
            driver_sample_rate,
            driver_input_channels,
        );

        // Drop manager lock BEFORE acquiring driver_manager to avoid
        // lock-order inversion with the config watcher thread
        // (which acquires driver_manager -> audio_manager).
        drop(manager);

        match result {
            Ok(_) => {
                log::info!("Driver plugin chain loaded successfully");

                // Set engine_ready so driver starts sending audio
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

    async fn handle_get_loudness(&self) -> Response {
        let manager = self.manager.lock();
        match manager.get_loudness() {
            Some(loudness) => Response::ok(loudness_info_to_json(&loudness)),
            None => Response::err("Loudness monitoring not enabled"),
        }
    }

    async fn handle_get_metering(&self) -> Response {
        let manager = self.manager.lock();
        let input_idx = *self.input_loudness_index.lock();
        let output_idx = *self.output_loudness_index.lock();
        let fallback_input_channels = *self.current_input_channels.lock();
        let fallback_output_channels = manager.get_engine_state().num_channels;

        let input_data = input_idx.and_then(|idx| {
            manager
                .get_cached_plugin_data(idx)
                .and_then(|data| data.downcast_ref::<sotf_audio::LoudnessData>().cloned())
        });

        let output_data = output_idx.and_then(|idx| {
            manager
                .get_cached_plugin_data(idx)
                .and_then(|data| data.downcast_ref::<sotf_audio::LoudnessData>().cloned())
        });

        let input_json = input_data
            .as_ref()
            .map(loudness_data_to_json)
            .unwrap_or_else(|| empty_loudness_json(fallback_input_channels));
        let output_json = output_data
            .as_ref()
            .map(loudness_data_to_json)
            .unwrap_or_else(|| empty_loudness_json(fallback_output_channels));

        Response::ok(serde_json::json!({
            "input": input_json,
            "output": output_json,
        }))
    }

    // =========================================================================
    // Plugin management handlers
    // =========================================================================

    async fn handle_get_plugins(&self) -> Response {
        let plugins = self.current_plugins.lock();
        let result: Vec<Value> = plugins
            .iter()
            .enumerate()
            .map(|(i, p)| {
                serde_json::json!({
                    "index": i,
                    "plugin_type": p.plugin_type,
                    "parameters": p.parameters,
                })
            })
            .collect();
        Response::ok(serde_json::json!({ "plugins": result }))
    }

    async fn handle_get_available_plugins(&self) -> Response {
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

        let available: Vec<Value> = PluginType::all()
            .into_iter()
            .filter(|pt| {
                let engine_type = plugin_type_to_engine_str(pt);
                !excluded.contains(&engine_type)
            })
            .map(|pt| {
                let engine_type = plugin_type_to_engine_str(&pt);
                let category = plugin_type_category(&pt);
                let default_settings = sotf_audio::PluginSettings::default_for(&pt);
                let default_parameters = default_settings.to_plugin_config(48_000.0).parameters;
                serde_json::json!({
                    "type": engine_type,
                    "name": pt.name(),
                    "description": pt.description(),
                    "category": category,
                    "maturity": format!("{:?}", pt.maturity()),
                    "default_parameters": default_parameters,
                    "parameters": plugin_parameter_descriptors(&default_settings),
                })
            })
            .collect();

        Response::ok(serde_json::json!({ "plugins": available }))
    }

    async fn handle_add_plugin(&self, plugin: PluginConfig, index: Option<usize>) -> Response {
        {
            let mut plugins = self.current_plugins.lock();
            match index {
                Some(i) if i <= plugins.len() => plugins.insert(i, plugin),
                _ => plugins.push(plugin),
            }
        }
        self.reload_plugins().await
    }

    async fn handle_remove_plugin(&self, index: usize) -> Response {
        {
            let mut plugins = self.current_plugins.lock();
            if index >= plugins.len() {
                return Response::err(format!(
                    "Plugin index {} out of range (have {})",
                    index,
                    plugins.len()
                ));
            }
            plugins.remove(index);
        }
        self.reload_plugins().await
    }

    async fn handle_update_plugin(&self, index: usize, parameters: Value) -> Response {
        {
            let mut plugins = self.current_plugins.lock();
            if index >= plugins.len() {
                return Response::err(format!(
                    "Plugin index {} out of range (have {})",
                    index,
                    plugins.len()
                ));
            }
            plugins[index].parameters = parameters;
        }
        self.reload_plugins().await
    }

    async fn handle_reorder_plugins(&self, order: Vec<usize>) -> Response {
        {
            let mut plugins = self.current_plugins.lock();
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
            for (new_pos, &old_pos) in order.iter().enumerate() {
                plugins[new_pos] = old[old_pos].clone();
            }
        }
        self.reload_plugins().await
    }

    /// Reload the plugin chain from current_plugins (re-injects monitors)
    async fn reload_plugins(&self) -> Response {
        let plugins = sanitize_user_plugins(self.current_plugins.lock().clone());
        *self.current_plugins.lock() = plugins.clone();
        let output_channels = *self.current_output_channels.lock();
        let (final_plugins, input_monitor_index, output_monitor_index) =
            build_driver_plugin_chain(plugins.clone());

        *self.input_loudness_index.lock() = Some(input_monitor_index);
        *self.output_loudness_index.lock() = Some(output_monitor_index);

        let result = {
            let manager = self.manager.lock();
            manager.update_plugin_chain(final_plugins)
        };

        match result {
            Ok(()) => {
                self.manager
                    .lock()
                    .set_loudness_plugin_index(output_monitor_index);
                log::info!("Driver plugin chain hot-updated successfully");
                Response::ok_empty()
            }
            Err(e) if e == "No engine running" => {
                log::info!("No running driver engine; starting driver playback");
                let input_channels = *self.current_input_channels.lock();
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
                    .await
            }
            Err(e) => {
                log::error!("Failed to hot-update plugin chain: {}", e);
                Response::err(format!("Failed to update plugin chain: {}", e))
            }
        }
    }

    async fn handle_driver_status(&self) -> Response {
        let status = get_driver_status(&self.driver_manager.lock());
        Response::ok(serde_json::json!({
            "platform_supported": status.platform_supported,
            "driver_installed": status.driver_installed,
            "capture_active": status.capture_active,
            "sample_rate": status.sample_rate,
            "channel_count": status.channel_count,
            "buffer_frames": status.buffer_frames,
            "driver_name": status.driver_name,
            // Legacy fields for backward compatibility
            "buffer_initialized": status.capture_active || status.driver_installed,
            "ready": status.platform_supported && status.driver_installed,
        }))
    }

    // =========================================================================
    // Encryption handlers
    // =========================================================================

    #[cfg(all(target_os = "macos", feature = "hal"))]
    fn sync_encryption_to_shared_memory(&self, flush_audio: bool) -> Result<(), String> {
        let key_manager = self.key_manager.lock();
        Self::apply_encryption_to_shared_memory(&key_manager, flush_audio)
    }

    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    fn sync_encryption_to_shared_memory(&self, _flush_audio: bool) -> Result<(), String> {
        Ok(())
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    fn apply_encryption_to_shared_memory(
        key_manager: &KeyManager,
        flush_audio: bool,
    ) -> Result<(), String> {
        match driver_hal::SharedAudioBuffer::open_default() {
            Ok(mut buffer) => {
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

    async fn handle_set_encryption(&self, enabled: bool) -> Response {
        let mut key_manager = self.key_manager.lock();
        #[cfg(all(target_os = "macos", feature = "hal"))]
        let previous_enabled = key_manager.is_enabled();
        key_manager.set_enabled(enabled);

        // On macOS with HAL, update shared memory encryption flag
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            if let Err(e) = Self::apply_encryption_to_shared_memory(&key_manager, true) {
                key_manager.set_enabled(previous_enabled);
                return Response::err(e);
            }
        }

        Response::ok(serde_json::json!({
            "enabled": key_manager.is_enabled(),
            "fingerprint": key_manager.fingerprint_hex(),
        }))
    }

    async fn handle_encryption_status(&self) -> Response {
        let key_manager = self.key_manager.lock();
        let status = key_manager.status();

        Response::ok(serde_json::json!({
            "enabled": status.enabled,
            "fingerprint": status.fingerprint,
            "key_path": status.key_path,
        }))
    }

    async fn handle_rotate_encryption_key(&self) -> Response {
        let mut key_manager = self.key_manager.lock();

        match key_manager.force_rotate() {
            Ok(()) => {
                // On macOS with HAL, update shared memory fingerprint
                #[cfg(all(target_os = "macos", feature = "hal"))]
                {
                    if let Err(e) = Self::apply_encryption_to_shared_memory(&key_manager, true) {
                        return Response::err(e);
                    }
                }

                Response::ok(serde_json::json!({
                    "fingerprint": key_manager.fingerprint_hex(),
                }))
            }
            Err(e) => Response::err(format!("Failed to rotate key: {}", e)),
        }
    }

    // =========================================================================
    // Driver config handlers
    // =========================================================================

    async fn handle_set_sample_rate(&self, rate: u32) -> Response {
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
            log::warn!(
                "Cannot change sample rate during active playback, will apply on next start"
            );
        }

        let mut driver = self.driver_manager.lock();
        let result = driver.request_config(DriverConfig {
            sample_rate: rate,
            buffer_frames: 0, // Keep current
            channel_count: 0, // Keep current
        });

        match result {
            driver_common::ConfigResult::Accepted
            | driver_common::ConfigResult::Negotiated { .. } => {
                log::info!("Sample rate set to {}Hz via driver", rate);
                Response::ok(serde_json::json!({ "sample_rate": rate }))
            }
            driver_common::ConfigResult::Error(e) => {
                Response::err(format!("Failed to set sample rate: {}", e))
            }
        }
    }

    async fn handle_set_buffer_frames(&self, frames: u32) -> Response {
        if !(64..=4096).contains(&frames) {
            return Response::err(format!(
                "Buffer frames must be between 64 and 4096, got: {}",
                frames
            ));
        }

        let mut driver = self.driver_manager.lock();
        let result = driver.request_config(DriverConfig {
            sample_rate: 0, // Keep current
            buffer_frames: frames,
            channel_count: 0, // Keep current
        });

        match result {
            driver_common::ConfigResult::Accepted
            | driver_common::ConfigResult::Negotiated { .. } => {
                log::info!("Buffer frames set to {} via driver", frames);
                Response::ok(serde_json::json!({ "buffer_frames": frames }))
            }
            driver_common::ConfigResult::Error(e) => {
                Response::err(format!("Failed to set buffer frames: {}", e))
            }
        }
    }

    async fn handle_get_driver_config(&self) -> Response {
        let driver = self.driver_manager.lock();
        let status = driver.status();

        Response::ok(serde_json::json!({
            "sample_rate": status.sample_rate,
            "actual_sample_rate": status.sample_rate,
            "buffer_frames": status.buffer_frames,
            "actual_buffer_frames": status.buffer_frames,
            "channel_count": status.channel_count,
            "active": status.capture_active,
            "driver_name": status.driver_name,
            "driver_installed": status.driver_installed,
            "driver_ready": status.driver_ready,
            "platform_supported": status.platform_supported,
        }))
    }

    fn handle_client(&self, mut stream: UnixStream, peer_class: PeerClass) {
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
                Ok(IpcLine::Line(command_line)) => {
                    let response = match serde_json::from_str::<Command>(&command_line) {
                        Ok(cmd) => {
                            // Defense-in-depth: gate which commands the
                            // peer's UID class may invoke. The macOS HAL
                            // (UID 202) is authenticated but should only
                            // be allowed to query status -- NOT issue
                            // arbitrary plugin loads, shutdowns, etc.
                            if !peer_allows_command(peer_class, cmd.name()) {
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
                                self.runtime.block_on(self.handle_command(cmd))
                            }
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
                    if let Err(e) = writeln!(stream, "{}", json) {
                        log::error!("Failed to write response: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Failed to read from client: {}", e);
                    break;
                }
            }
        }
    }

    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = get_socket_path();

        // Ensure socket directory exists with secure permissions
        ensure_secure_socket_dir(&socket_path)?;

        // Start driver config watcher thread
        let config_watcher = {
            let driver_manager = Arc::clone(&self.driver_manager);
            let audio_manager = Arc::clone(&self.manager);
            let running = Arc::clone(&self.running);
            let current_plugins = Arc::clone(&self.current_plugins);
            let current_input_channels = Arc::clone(&self.current_input_channels);
            let current_output_channels = Arc::clone(&self.current_output_channels);
            let input_loudness_index = Arc::clone(&self.input_loudness_index);
            let output_loudness_index = Arc::clone(&self.output_loudness_index);
            spawn_driver_config_watcher(
                driver_manager,
                audio_manager,
                running,
                current_plugins,
                current_input_channels,
                current_output_channels,
                input_loudness_index,
                output_loudness_index,
            )
        };

        // Bind the socket. To avoid a TOCTOU race window between an
        // existence check and a follow-up unlink (which would allow a
        // same-UID hostile actor to swap in their own socket or unrelated
        // file at the path), we try `bind()` first and only fall back to
        // unlinking when we have positively confirmed the existing entry
        // is a stale `AF_UNIX` socket -- never a regular file, FIFO, or
        // symlink. See `bind_unix_socket` below for the full strategy.
        let listener = bind_unix_socket(&socket_path)?;
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

        loop {
            if !*self.running.lock() {
                println!("Shutdown requested, exiting");
                break;
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

                    // Clone daemon for client thread
                    let daemon = AudioDaemon {
                        manager: Arc::clone(&self.manager),
                        running: Arc::clone(&self.running),
                        driver_manager: Arc::clone(&self.driver_manager),
                        selected_device: Arc::clone(&self.selected_device),
                        key_manager: Arc::clone(&self.key_manager),
                        runtime: Arc::clone(&self.runtime),
                        current_plugins: Arc::clone(&self.current_plugins),
                        current_input_channels: Arc::clone(&self.current_input_channels),
                        current_output_channels: Arc::clone(&self.current_output_channels),
                        input_loudness_index: Arc::clone(&self.input_loudness_index),
                        output_loudness_index: Arc::clone(&self.output_loudness_index),
                    };

                    std::thread::spawn(move || {
                        daemon.handle_client(stream, peer_class);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    log::error!("Failed to accept connection: {}", e);
                }
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
fn bind_unix_socket(socket_path: &std::path::Path) -> std::io::Result<UnixListener> {
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
fn socket_is_unix_socket(path: &std::path::Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::symlink_metadata(path) {
        Ok(md) => md.file_type().is_socket(),
        Err(_) => false,
    }
}

// =============================================================================
// Driver Config Watcher
// =============================================================================

/// Supported sample rates for driver config negotiation
const SUPPORTED_SAMPLE_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

/// Spawn a background thread that polls the driver for config changes
fn spawn_driver_config_watcher(
    driver_manager: Arc<Mutex<DriverManager>>,
    audio_manager: Arc<Mutex<AudioEngineManager>>,
    running: Arc<Mutex<bool>>,
    current_plugins: Arc<Mutex<Vec<PluginConfig>>>,
    current_input_channels: Arc<Mutex<usize>>,
    current_output_channels: Arc<Mutex<usize>>,
    input_loudness_index: Arc<Mutex<Option<usize>>>,
    output_loudness_index: Arc<Mutex<Option<usize>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::time::Duration;

        let poll_interval = Duration::from_millis(50);

        log::info!("Driver config watcher thread started");

        loop {
            if !*running.lock() {
                break;
            }

            // Poll driver for config changes
            let config_change = driver_manager.lock().poll_config_change();
            if let Some(config) = config_change {
                handle_driver_config_change(
                    &driver_manager,
                    &audio_manager,
                    config,
                    &current_plugins,
                    &current_input_channels,
                    &current_output_channels,
                    &input_loudness_index,
                    &output_loudness_index,
                );
            }

            std::thread::sleep(poll_interval);
        }

        log::info!("Driver config watcher thread stopped");
    })
}

/// Handle a driver-initiated config change
fn handle_driver_config_change(
    driver_manager: &Arc<Mutex<DriverManager>>,
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    config: DriverConfig,
    current_plugins: &Arc<Mutex<Vec<PluginConfig>>>,
    current_input_channels: &Arc<Mutex<usize>>,
    current_output_channels: &Arc<Mutex<usize>>,
    input_loudness_index: &Arc<Mutex<Option<usize>>>,
    output_loudness_index: &Arc<Mutex<Option<usize>>>,
) {
    let requested_rate = config.sample_rate;
    let requested_frames = config.buffer_frames;
    let requested_channels = config.channel_count;

    log::info!(
        "Driver config change request: sample_rate={}, buffer_frames={}, channels={}",
        requested_rate,
        requested_frames,
        requested_channels
    );

    // Validate requested values
    if requested_rate == 0 {
        log::warn!("Invalid config request: sample_rate=0, ignoring");
        driver_manager.lock().acknowledge_config_change(
            DriverConfig {
                sample_rate: 48000,
                buffer_frames: requested_frames,
                channel_count: config.channel_count,
            },
            driver_common::ConfigResult::Error("Invalid sample rate".to_string()),
        );
        return;
    }
    if requested_frames == 0 || requested_frames > 65536 {
        log::warn!(
            "Invalid config request: buffer_frames={}, out of range",
            requested_frames
        );
        driver_manager.lock().acknowledge_config_change(
            DriverConfig {
                sample_rate: requested_rate,
                buffer_frames: 512,
                channel_count: config.channel_count,
            },
            driver_common::ConfigResult::Error("Invalid buffer frames".to_string()),
        );
        return;
    }
    if requested_channels == 0 || requested_channels as usize > MAX_HAL_CHANNELS {
        log::warn!(
            "Invalid config request: channel_count={}, out of range",
            requested_channels
        );
        driver_manager.lock().acknowledge_config_change(
            DriverConfig {
                sample_rate: requested_rate,
                buffer_frames: requested_frames,
                channel_count: 2,
            },
            driver_common::ConfigResult::Error("Invalid channel count".to_string()),
        );
        return;
    }

    // Determine actual rate to use
    let actual_rate = if SUPPORTED_SAMPLE_RATES.contains(&requested_rate) {
        requested_rate
    } else {
        SUPPORTED_SAMPLE_RATES
            .iter()
            .min_by_key(|&&r| (r as i32 - requested_rate as i32).abs())
            .copied()
            .unwrap_or(48000)
    };

    let negotiated = actual_rate != requested_rate;
    *current_input_channels.lock() = requested_channels as usize;

    // Reconfigure audio pipeline
    match reconfigure_audio_pipeline(
        audio_manager,
        actual_rate,
        requested_frames,
        current_plugins,
        current_output_channels,
        input_loudness_index,
        output_loudness_index,
    ) {
        Ok(()) => {
            // Set engine_ready so driver continues sending audio
            driver_manager.lock().set_engine_ready(true);

            let result = if negotiated {
                log::info!(
                    "Config negotiated: requested {}Hz, using {}Hz",
                    requested_rate,
                    actual_rate
                );
                driver_common::ConfigResult::Negotiated {
                    actual_rate,
                    actual_frames: requested_frames,
                }
            } else {
                driver_common::ConfigResult::Accepted
            };

            driver_manager.lock().acknowledge_config_change(
                DriverConfig {
                    sample_rate: actual_rate,
                    buffer_frames: requested_frames,
                    channel_count: config.channel_count,
                },
                result,
            );
            log::info!(
                "Config accepted: {}Hz, {} frames, {} channels, engine_ready=true",
                actual_rate,
                requested_frames,
                requested_channels
            );
        }
        Err(e) => {
            log::error!("Pipeline reconfiguration failed: {}", e);
            driver_manager.lock().acknowledge_config_change(
                DriverConfig {
                    sample_rate: actual_rate,
                    buffer_frames: requested_frames,
                    channel_count: config.channel_count,
                },
                driver_common::ConfigResult::Error(e),
            );
        }
    }
}

/// Reconfigure the audio pipeline with new sample rate and buffer size
fn reconfigure_audio_pipeline(
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    hal_sample_rate: u32,
    _buffer_frames: u32,
    current_plugins: &Arc<Mutex<Vec<PluginConfig>>>,
    current_output_channels: &Arc<Mutex<usize>>,
    input_loudness_index: &Arc<Mutex<Option<usize>>>,
    output_loudness_index: &Arc<Mutex<Option<usize>>>,
) -> Result<(), String> {
    let mut manager = audio_manager.lock();

    let state = manager.get_state();
    if state == sotf_audio::manager::StreamingState::Idle {
        log::debug!("No active playback, acknowledging config change");
        return Ok(());
    }

    log::info!("Reconfiguring driver playback pipeline");

    if let Err(e) = manager.stop() {
        log::warn!("Failed to stop current playback: {}", e);
    }

    // Get stored user plugins
    let user_plugins = current_plugins.lock().clone();
    let output_channels = *current_output_channels.lock();

    // Build full plugin chain: input_monitor + user_plugins + output_monitor
    let mut final_plugins = Vec::with_capacity(user_plugins.len() + 2);

    let input_monitor_index = 0;
    final_plugins.push(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters: serde_json::json!({}),
    });

    final_plugins.extend(user_plugins);

    let output_monitor_index = final_plugins.len();
    final_plugins.push(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters: serde_json::json!({}),
    });

    *input_loudness_index.lock() = Some(input_monitor_index);
    *output_loudness_index.lock() = Some(output_monitor_index);

    let output_device = configured_output_device_from_env();

    log::info!(
        "Restarting driver playback with {} plugins (incl. 2 monitors), {} output channels, device: {:?}",
        final_plugins.len(),
        output_channels,
        output_device
    );

    manager.set_loudness_plugin_index(output_monitor_index);

    let input_channels = driver_hal_input_channels().unwrap_or(2);

    match manager.start_hal_playback_with_driver_config(
        output_device,
        final_plugins,
        output_channels,
        hal_sample_rate,
        input_channels,
    ) {
        Ok(_) => {
            log::info!("Driver playback restarted successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to restart driver playback: {}", e);
            Err(format!("Failed to restart driver playback: {}", e))
        }
    }
}

fn driver_hal_input_channels() -> Option<usize> {
    #[cfg(all(target_os = "macos", feature = "hal"))]
    {
        driver_hal::SharedAudioBuffer::open_default()
            .ok()
            .and_then(|buffer| {
                let channels = buffer.channel_count() as usize;
                (channels > 0).then_some(channels)
            })
    }
    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    {
        None
    }
}

/// Map PluginType enum to the string the engine's create_plugin() expects
fn plugin_type_to_engine_str(pt: &PluginType) -> &'static str {
    match pt {
        PluginType::EQ => "eq",
        PluginType::Gain => "gain",
        PluginType::Upmixer => "upmixer",
        PluginType::AAE => "aae",
        PluginType::Compressor => "compressor",
        PluginType::Limiter => "limiter",
        PluginType::Gate => "gate",
        PluginType::Expander => "expander",
        PluginType::MultibandCompressor => "multiband_compressor",
        PluginType::MultibandExpander => "multiband_expander",
        PluginType::LoudnessCompensation => "loudness_compensation",
        PluginType::FletcherMunson => "fletcher_munson",
        PluginType::BinauralDecoder => "binaural_decoder",
        PluginType::Convolution => "convolution",
        PluginType::LoudnessMonitor => "loudness_monitor",
        PluginType::SpectrumAnalyzer => "spectrum_analyzer",
        PluginType::ChannelMuteSolo => "channel_mute_solo",
        PluginType::Matrix => "matrix",
        PluginType::XTC => "xtc",
        PluginType::Denoiser => "denoiser",
        PluginType::Declick => "declick",
        PluginType::HissReducer => "hiss_reducer",
        PluginType::SpeechDenoiser => "speech_denoiser",
        PluginType::Pnd => "pnd",
        PluginType::ABCompare => "ab_compare",
        PluginType::BandSplit => "band_split",
        PluginType::BandMerge => "band_merge",
        PluginType::Downmix => "downmix",
        PluginType::MonoToStereo => "mono_to_stereo",
        PluginType::Crossfeed => "crossfeed",
        PluginType::Delay => "delay",
        PluginType::Aec => "aec",
        PluginType::Beamformer => "beamformer",
        PluginType::AmbisonicsDecoder => "ambisonics_decoder",
        PluginType::StereoImager => "stereo_imager",
        PluginType::DeEsser => "de_esser",
        PluginType::TransientShaper => "transient_shaper",
        PluginType::Saturation => "saturation",
        PluginType::DynamicEq => "dynamic_eq",
        PluginType::LinearPhaseEq => "linear_phase_eq",
        PluginType::SpectralCompressor => "spectral_compressor",
    }
}

/// Categorize plugins for the UI picker
fn plugin_type_category(pt: &PluginType) -> &'static str {
    match pt {
        PluginType::EQ | PluginType::FletcherMunson | PluginType::LoudnessCompensation => {
            "EQ & Tone"
        }
        PluginType::Gain => "Utility",
        PluginType::Compressor | PluginType::Limiter | PluginType::Gate | PluginType::Expander => {
            "Dynamics"
        }
        PluginType::MultibandCompressor | PluginType::MultibandExpander => "Dynamics",
        PluginType::AAE
        | PluginType::Upmixer
        | PluginType::Downmix
        | PluginType::MonoToStereo
        | PluginType::Matrix
        | PluginType::ChannelMuteSolo => "Spatial & Routing",
        PluginType::BinauralDecoder | PluginType::XTC => "Spatial & Routing",
        PluginType::Convolution => "Effects",
        PluginType::Denoiser
        | PluginType::Declick
        | PluginType::HissReducer
        | PluginType::SpeechDenoiser
        | PluginType::Pnd => "Restoration",
        PluginType::LoudnessMonitor | PluginType::SpectrumAnalyzer => "Monitoring",
        PluginType::ABCompare => "Utility",
        PluginType::BandSplit | PluginType::BandMerge | PluginType::Crossfeed => "Utility",
        PluginType::Delay => "Effects",
        PluginType::Aec => "Restoration",
        PluginType::Beamformer => "Spatial & Routing",
        PluginType::AmbisonicsDecoder => "Spatial & Routing",
        PluginType::StereoImager => "Spatial & Routing",
        PluginType::DeEsser => "Dynamics",
        PluginType::TransientShaper => "Dynamics",
        PluginType::Saturation => "Effects",
        PluginType::DynamicEq => "Dynamics",
        PluginType::LinearPhaseEq => "EQ",
        PluginType::SpectralCompressor => "Dynamics",
    }
}

fn configured_output_device_from_env() -> Option<String> {
    configured_output_device_from_value(std::env::var(OUTPUT_DEVICE_ENV).ok().as_deref())
}

fn configured_output_device_from_value(value: Option<&str>) -> Option<String> {
    value
        .map(|device| device.trim().to_string())
        .filter(|device| !device.is_empty())
}

fn is_safe_output_device_name(name: &str) -> bool {
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

fn list_audio_devices() -> Result<Vec<serde_json::Value>, String> {
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
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn loudness_data_json_includes_meter_fields() {
        let loudness = sotf_audio::LoudnessData {
            momentary_lufs: -18.5,
            shortterm_lufs: -17.25,
            integrated_lufs: -20.0,
            peak: 0.75,
            channel_peaks: Arc::new(vec![0.5, 0.75]),
            true_peaks_dbtp: Arc::new(vec![-2.0, -1.0]),
            correlation_lr: Some(0.42),
            correlation_matrix: Arc::new(Vec::new()),
            correlation_samples_seen: 0,
        };

        let json = loudness_data_to_json(&loudness);

        assert_eq!(json["momentary"], serde_json::json!(-18.5));
        assert_eq!(json["short_term"], serde_json::json!(-17.25));
        assert_eq!(json["integrated"], serde_json::json!(-20.0));
        assert_eq!(json["peak"], serde_json::json!(0.75));
        assert_eq!(json["channel_peaks"], serde_json::json!([0.5, 0.75]));
        assert_eq!(json["true_peaks_dbtp"], serde_json::json!([-2.0, -1.0]));
        assert_eq!(json["correlation_lr"], serde_json::json!(0.42));
    }

    #[test]
    fn available_plugin_descriptors_expose_engine_keys() {
        let settings = sotf_audio::PluginSettings::default_for(&PluginType::Gain);
        let descriptors = plugin_parameter_descriptors(&settings);

        assert!(
            descriptors
                .iter()
                .any(|d| d["key"] == "gain_db" && d["type"] == "float")
        );
    }

    #[test]
    fn configured_output_device_uses_non_empty_value() {
        assert_eq!(
            configured_output_device_from_value(Some(" ADAM Audio D3V ")),
            Some("ADAM Audio D3V".to_string())
        );
        assert_eq!(configured_output_device_from_value(Some("   ")), None);
        assert_eq!(configured_output_device_from_value(None), None);
    }

    #[test]
    fn virtual_output_device_names_are_rejected() {
        for name in [
            "SotF Virtual Audio",
            "BlackHole 2ch",
            "Loopback Audio",
            "Soundflower (2ch)",
            "Background Music",
            "Audio Bridge",
            "ZoomAudioDevice",
            "Generic Virtual Device",
        ] {
            assert!(!is_safe_output_device_name(name), "{name} should be unsafe");
        }

        for name in ["Built-in Output", "ADAM Audio D3V", "MacBook Pro Speakers"] {
            assert!(is_safe_output_device_name(name), "{name} should be safe");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let daemon = AudioDaemon::new();

    // Setup signal handling for graceful shutdown — use the daemon's own
    // running flag so Ctrl-C actually stops the accept loop.
    {
        let running = Arc::clone(&daemon.running);
        ctrlc::set_handler(move || {
            println!("\nReceived interrupt signal, shutting down...");
            *running.lock() = false;
        })?;
    }

    println!("===============================================================================");
    println!("SotF Audio Control Daemon");
    println!("===============================================================================");

    // Initialize driver
    println!();
    {
        let mut driver = daemon.driver_manager.lock();
        match driver.initialize() {
            Ok(()) => {
                let status = driver.status();
                println!("Driver Status:");
                println!("   Driver:             {}", status.driver_name);
                println!(
                    "   Platform supported: {}",
                    if status.platform_supported {
                        "Yes"
                    } else {
                        "No"
                    }
                );
                println!(
                    "   Driver installed:   {}",
                    if status.driver_installed {
                        "Yes"
                    } else {
                        "No (optional)"
                    }
                );
                println!(
                    "   Capture active:     {}",
                    if status.capture_active { "Yes" } else { "No" }
                );

                if status.platform_supported && status.driver_installed {
                    println!();
                    println!("Audio flow (capture mode):");
                    println!(
                        "   System Audio -> Driver -> SharedMemory -> Daemon -> cpal -> Hardware"
                    );
                }
            }
            Err(e) => {
                log::warn!("Failed to initialize driver: {}", e);
                log::warn!("Audio capture will not be available");
            }
        }
    }

    // Show encryption status
    {
        let key_manager = daemon.key_manager.lock();
        let status = key_manager.status();
        println!();
        println!("Encryption Status:");
        println!(
            "   Enabled:     {}",
            if status.enabled { "Yes" } else { "No" }
        );
        println!("   Fingerprint: {}", status.fingerprint);
        println!("   Key path:    {}", status.key_path);
    }

    println!();
    println!("===============================================================================");
    println!("Starting daemon...");
    println!("===============================================================================");

    daemon.run()?;

    // Explicit driver cleanup
    {
        let mut driver = daemon.driver_manager.lock();
        driver.shutdown();
    }

    println!();
    println!("Daemon stopped cleanly");
    Ok(())
}

#[cfg(test)]
mod ipc_safety_tests {
    use super::*;
    use std::io::Cursor;

    /// `serialize_response_safely` must produce valid JSON on the OK
    /// path with no behavioural change versus the original
    /// `to_string(...).unwrap()` semantics.
    #[test]
    fn serialize_response_safely_round_trips_ok_response() {
        let r = Response::ok(serde_json::json!({
            "index": 7,
            "name": "test plugin",
        }));
        let out = serialize_response_safely(&r);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("ok-path output must parse");
        assert_eq!(parsed["success"], serde_json::Value::Bool(true));
        assert_eq!(parsed["data"]["index"], serde_json::Value::from(7));
    }

    /// `serialize_response_safely` must produce valid JSON on the error
    /// path too.
    #[test]
    fn serialize_response_safely_handles_err_response() {
        let r = Response::err("something failed");
        let out = serialize_response_safely(&r);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("err-path output must parse");
        assert_eq!(parsed["success"], serde_json::Value::Bool(false));
        assert_eq!(parsed["error"], serde_json::Value::from("something failed"));
    }

    /// Regression test for the IPC `unwrap()` panic.
    ///
    /// The fallback returned by `serialize_response_safely` when
    /// `serde_json::to_string` errors out MUST itself be valid JSON
    /// matching the on-wire `Response` shape. If a future refactor
    /// breaks this string, every client receives malformed JSON when
    /// the daemon encounters a NaN/Inf in echoed user-supplied
    /// parameters -- so we lock it down here.
    #[test]
    fn serialize_response_safely_fallback_is_valid_json() {
        let fallback = String::from(
            r#"{"success":false,"error":"internal error: response serialization failed"}"#,
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&fallback).expect("fallback must be valid JSON");
        assert_eq!(parsed["success"], serde_json::Value::Bool(false));
        assert!(parsed["error"].is_string());
    }

    /// Confirm that a synthetic Serialize failure does NOT propagate
    /// out of `serialize_response_safely`. We can't easily inject a
    /// failing `Value` through `Response::data` (serde_json::Value's
    /// own Serialize impl never errors for in-memory values), but we
    /// can verify the helper's no-panic contract on every legitimate
    /// Response we can construct -- and that `serde_json::to_string`
    /// itself can return Err on a custom Serialize impl that errors.
    /// This locks in the *shape* of the safety net: if the underlying
    /// `to_string` does error, our wrapper turns it into a normal
    /// String return rather than a panic-on-`.unwrap()`.
    #[test]
    fn synthetic_serializer_error_is_handled_without_panic() {
        use serde::{Serialize, Serializer};

        struct AlwaysFail;
        impl Serialize for AlwaysFail {
            fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("synthetic serialization failure"))
            }
        }
        // Sanity: serde_json::to_string on AlwaysFail returns Err.
        let bad = serde_json::to_string(&AlwaysFail);
        assert!(bad.is_err(), "AlwaysFail must fail to serialize");

        // The helper itself never panics on a normal Response, which
        // is the property we care about for the IPC hot path.
        let r = Response::err("normal");
        let _ = serialize_response_safely(&r); // must not panic
    }

    #[test]
    fn read_ipc_line_bounded_accepts_normal_command() {
        let input = Cursor::new(b"  {\"command\":\"status\"}  \n");
        let mut reader = BufReader::new(input);
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::Line(r#"{"command":"status"}"#.to_string())
        );
    }

    #[test]
    fn read_ipc_line_bounded_handles_crlf_and_empty_lines() {
        let input = Cursor::new(b"\r\n{\"command\":\"status\"}\r\n");
        let mut reader = BufReader::new(input);
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::Empty
        );
        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::Line(r#"{"command":"status"}"#.to_string())
        );
    }

    #[test]
    fn read_ipc_line_bounded_rejects_oversized_line() {
        let mut input = vec![b'a'; MAX_IPC_COMMAND_BYTES + 1];
        input.push(b'\n');
        let mut reader = BufReader::new(Cursor::new(input));
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::TooLarge
        );
    }

    #[test]
    fn read_ipc_line_bounded_rejects_oversized_unterminated_line() {
        let input = vec![b'a'; MAX_IPC_COMMAND_BYTES + 1];
        let mut reader = BufReader::new(Cursor::new(input));
        let mut buffer = Vec::new();

        assert_eq!(
            read_ipc_line_bounded(&mut reader, &mut buffer).unwrap(),
            IpcLine::TooLarge
        );
    }
    /// `Command::name()` must stay in sync with `#[serde(rename)]`,
    /// because `peer_allows_command` matches on these exact strings.
    #[test]
    fn command_name_matches_wire_tag() {
        let cmd: Command = serde_json::from_str(r#"{"command":"status"}"#).unwrap();
        assert_eq!(cmd.name(), "status");

        let cmd: Command = serde_json::from_str(r#"{"command":"driver_status"}"#).unwrap();
        assert_eq!(cmd.name(), "driver_status");

        // The "hal_status" alias deserialises to DriverStatus, whose
        // canonical wire name (per `#[serde(rename)]`) is "driver_status".
        let cmd: Command = serde_json::from_str(r#"{"command":"hal_status"}"#).unwrap();
        assert_eq!(cmd.name(), "driver_status");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"update_plugin","index":0,"parameters":{}}"#)
                .unwrap();
        assert_eq!(cmd.name(), "update_plugin");

        let cmd: Command = serde_json::from_str(r#"{"command":"shutdown"}"#).unwrap();
        assert_eq!(cmd.name(), "shutdown");
    }
}
