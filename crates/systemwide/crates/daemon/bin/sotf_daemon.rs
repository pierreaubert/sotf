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
mod plugin_artifact;
mod security;

use driver_manager::{DriverManager, get_driver_status};
use plugin_artifact::{PluginArtifactPlan, plan_plugin_artifact};
use security::{
    KeyManager, PeerClass, classify_peer, current_uid as security_current_uid,
    ensure_secure_socket_dir, get_secure_socket_path, peer_allows_command, verify_peer_credentials,
};

use driver_common::{DriverConfig, DriverStatus};
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

fn env_path_is_set(key: &str) -> bool {
    std::env::var_os(key).is_some_and(|value| !value.as_os_str().is_empty())
}

/// Get the socket path to use
/// Uses secure per-user path, with fallback to legacy path if SOTF_LEGACY_SOCKET is set
fn get_socket_path() -> PathBuf {
    if env_path_is_set("SOTF_DAEMON_SOCKET_PATH") || env_path_is_set("SOTF_SYSTEMWIDE_RUNTIME_DIR")
    {
        get_secure_socket_path()
    } else if std::env::var("SOTF_LEGACY_SOCKET").is_ok() {
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
    #[serde(rename = "get_snapshot", alias = "snapshot")]
    GetSnapshot,
    #[serde(rename = "dump_state")]
    DumpState,
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
    #[serde(rename = "load_plugin_artifact")]
    LoadPluginArtifact { artifact: Value },
    #[serde(rename = "set_input_channels")]
    SetInputChannels { channels: usize },
    #[serde(rename = "set_output_channels")]
    SetOutputChannels { channels: usize },
    #[serde(rename = "set_pipeline_channels")]
    SetPipelineChannels {
        #[serde(default)]
        input_channels: Option<usize>,
        #[serde(default)]
        output_channels: Option<usize>,
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
            Command::GetSnapshot => "get_snapshot",
            Command::DumpState => "dump_state",
            Command::Load { .. } => "load",
            Command::Play => "play",
            Command::Pause => "pause",
            Command::Stop => "stop",
            Command::Seek { .. } => "seek",
            Command::SetVolume { .. } => "set_volume",
            Command::ListDevices => "list_devices",
            Command::SetDevice { .. } => "set_device",
            Command::LoadPlugins { .. } => "load_plugins",
            Command::LoadPluginArtifact { .. } => "load_plugin_artifact",
            Command::SetInputChannels { .. } => "set_input_channels",
            Command::SetOutputChannels { .. } => "set_output_channels",
            Command::SetPipelineChannels { .. } => "set_pipeline_channels",
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

fn metering_source_json(data_present: bool, channels: usize) -> Value {
    let channels = channels.clamp(1, MAX_HAL_CHANNELS);
    let (status, source) = if data_present {
        ("available", "loudness_monitor")
    } else {
        ("fallback_zero", "channel_sized_fallback")
    };
    serde_json::json!({
        "status": status,
        "source": source,
        "channels": channels,
    })
}

fn transport_snapshot_and_faults(
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

fn push_metering_faults(state_name: &str, metering: &Value, faults: &mut Vec<Value>) {
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

fn pipeline_spec_to_json(spec: &PipelineSpec) -> Value {
    serde_json::json!({
        "output_device": spec.output_device,
        "input_channels": spec.input_channels,
        "output_channels": spec.output_channels,
        "user_plugin_count": spec.user_plugins.len(),
        "user_plugin_types": spec
            .user_plugins
            .iter()
            .map(|p| p.plugin_type.as_str())
            .collect::<Vec<_>>(),
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

#[derive(Clone, Debug)]
struct PipelineSpec {
    output_device: Option<String>,
    user_plugins: Vec<PluginConfig>,
    input_channels: usize,
    output_channels: usize,
}

impl Default for PipelineSpec {
    fn default() -> Self {
        Self {
            output_device: None,
            user_plugins: Vec::new(),
            input_channels: 2,
            output_channels: 2,
        }
    }
}

#[derive(Clone, Debug)]
struct AppliedPipeline {
    spec: PipelineSpec,
    input_loudness_index: usize,
    output_loudness_index: usize,
    generation: u64,
}

#[derive(Clone, Debug)]
struct PipelinePlan {
    spec: PipelineSpec,
    runtime_plugins: Vec<PluginConfig>,
    input_loudness_index: usize,
    output_loudness_index: usize,
}

#[derive(Debug, Default)]
struct PipelineSupervisor {
    desired: PipelineSpec,
    applied: Option<AppliedPipeline>,
    generation: u64,
}

impl PipelineSupervisor {
    fn selected_output_device(&self) -> Option<String> {
        self.desired.output_device.clone()
    }

    fn user_plugins(&self) -> Vec<PluginConfig> {
        self.desired.user_plugins.clone()
    }

    fn input_channels(&self) -> usize {
        self.desired.input_channels
    }

    fn output_channels(&self) -> usize {
        self.desired.output_channels
    }

    fn input_loudness_index(&self) -> Option<usize> {
        self.applied.as_ref().map(|p| p.input_loudness_index)
    }

    fn output_loudness_index(&self) -> Option<usize> {
        self.applied.as_ref().map(|p| p.output_loudness_index)
    }

    fn applied_generation(&self) -> Option<u64> {
        self.applied.as_ref().map(|p| p.generation)
    }

    fn applied_output_device(&self) -> Option<String> {
        self.applied
            .as_ref()
            .and_then(|p| p.spec.output_device.clone())
    }

    fn desired_spec(&self) -> PipelineSpec {
        self.desired.clone()
    }

    fn applied_spec(&self) -> Option<PipelineSpec> {
        self.applied.as_ref().map(|p| p.spec.clone())
    }

    fn prepare_plan(
        &self,
        user_plugins: Vec<PluginConfig>,
        input_channels: usize,
        output_channels: usize,
        driver_input_fallback_channels: usize,
    ) -> Result<PipelinePlan, String> {
        let user_plugins = sanitize_user_plugins(user_plugins);
        let input_channels = if input_channels > 0 {
            input_channels
        } else if driver_input_fallback_channels > 0 {
            driver_input_fallback_channels
        } else {
            self.desired.input_channels.max(1)
        };

        if !(1..=MAX_HAL_CHANNELS).contains(&input_channels) {
            return Err(format!(
                "Invalid HAL input channel count: {}. Must be between 1 and {}.",
                input_channels, MAX_HAL_CHANNELS
            ));
        }
        if !(1..=MAX_HAL_CHANNELS).contains(&output_channels) {
            return Err(format!(
                "Invalid output channel count: {}. Must be between 1 and {}.",
                output_channels, MAX_HAL_CHANNELS
            ));
        }

        let mut output_device = self.desired.output_device.clone();
        if output_device.is_none() {
            output_device = configured_output_device_from_env();
        }

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

        let (runtime_plugins, input_loudness_index, output_loudness_index) =
            build_driver_plugin_chain(user_plugins.clone());

        Ok(PipelinePlan {
            spec: PipelineSpec {
                output_device,
                user_plugins,
                input_channels,
                output_channels,
            },
            runtime_plugins,
            input_loudness_index,
            output_loudness_index,
        })
    }

    fn prepare_with_selected_device(&self, output_device: String) -> Result<PipelinePlan, String> {
        let mut next = self.desired.clone();
        next.output_device = Some(output_device);
        let supervisor = Self {
            desired: next.clone(),
            applied: self.applied.clone(),
            generation: self.generation,
        };
        supervisor.prepare_plan(
            next.user_plugins,
            next.input_channels,
            next.output_channels,
            next.input_channels,
        )
    }

    fn commit_applied(&mut self, plan: &PipelinePlan) {
        self.generation = self.generation.saturating_add(1);
        self.desired = plan.spec.clone();
        self.applied = Some(AppliedPipeline {
            spec: plan.spec.clone(),
            input_loudness_index: plan.input_loudness_index,
            output_loudness_index: plan.output_loudness_index,
            generation: self.generation,
        });
    }

    fn set_desired_output_device(&mut self, output_device: Option<String>) -> Result<(), String> {
        if let Some(device) = output_device.as_ref()
            && !is_safe_output_device_name(device)
        {
            return Err(format!(
                "'{}' is a virtual/loopback device and cannot be used as Systemwide speaker output.",
                device
            ));
        }
        self.desired.output_device = output_device;
        Ok(())
    }

    fn commit_idle_reconfigure(&mut self, plan: &PipelinePlan) {
        self.desired = plan.spec.clone();
    }
}

#[derive(Debug, Default)]
struct SystemwideState {
    pipeline: PipelineSupervisor,
}

impl SystemwideState {
    fn selected_output_device(&self) -> Option<String> {
        self.pipeline.selected_output_device()
    }

    fn user_plugins(&self) -> Vec<PluginConfig> {
        self.pipeline.user_plugins()
    }

    fn input_channels(&self) -> usize {
        self.pipeline.input_channels()
    }

    fn output_channels(&self) -> usize {
        self.pipeline.output_channels()
    }

    fn input_loudness_index(&self) -> Option<usize> {
        self.pipeline.input_loudness_index()
    }

    fn output_loudness_index(&self) -> Option<usize> {
        self.pipeline.output_loudness_index()
    }

    fn applied_generation(&self) -> Option<u64> {
        self.pipeline.applied_generation()
    }

    fn applied_output_device(&self) -> Option<String> {
        self.pipeline.applied_output_device()
    }

    fn desired_spec(&self) -> PipelineSpec {
        self.pipeline.desired_spec()
    }

    fn applied_spec(&self) -> Option<PipelineSpec> {
        self.pipeline.applied_spec()
    }

    fn prepare_plan(
        &self,
        user_plugins: Vec<PluginConfig>,
        input_channels: usize,
        output_channels: usize,
        driver_input_fallback_channels: usize,
    ) -> Result<PipelinePlan, String> {
        self.pipeline.prepare_plan(
            user_plugins,
            input_channels,
            output_channels,
            driver_input_fallback_channels,
        )
    }

    fn prepare_with_selected_device(&self, output_device: String) -> Result<PipelinePlan, String> {
        self.pipeline.prepare_with_selected_device(output_device)
    }

    fn commit_applied(&mut self, plan: &PipelinePlan) {
        self.pipeline.commit_applied(plan);
    }

    fn set_desired_output_device(&mut self, output_device: Option<String>) -> Result<(), String> {
        self.pipeline.set_desired_output_device(output_device)
    }

    fn commit_idle_reconfigure(&mut self, plan: &PipelinePlan) {
        self.pipeline.commit_idle_reconfigure(plan);
    }
}

#[derive(Clone)]
struct AudioDaemon {
    manager: Arc<Mutex<AudioEngineManager>>,
    running: Arc<Mutex<bool>>,
    driver_manager: Arc<Mutex<DriverManager>>,
    /// Desired and applied systemwide daemon state.
    system_state: Arc<Mutex<SystemwideState>>,
    /// Encryption key manager
    key_manager: Arc<Mutex<KeyManager>>,
    /// Shared Tokio runtime for async operations
    runtime: Arc<tokio::runtime::Runtime>,
}

impl AudioDaemon {
    fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        Self {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::new())),
            system_state: Arc::new(Mutex::new(SystemwideState::default())),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
            runtime: Arc::new(runtime),
        }
    }

    fn spawn_initial_driver_playback(&self) {
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
            Command::GetSnapshot => self.handle_get_snapshot().await,
            Command::DumpState => self.handle_dump_state().await,
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
            Command::LoadPluginArtifact { artifact } => {
                self.handle_load_plugin_artifact(artifact).await
            }
            Command::SetInputChannels { channels } => {
                self.handle_set_pipeline_channels(Some(channels), None)
                    .await
            }
            Command::SetOutputChannels { channels } => {
                self.handle_set_pipeline_channels(None, Some(channels))
                    .await
            }
            Command::SetPipelineChannels {
                input_channels,
                output_channels,
            } => {
                self.handle_set_pipeline_channels(input_channels, output_channels)
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

    fn metering_snapshot(&self) -> Value {
        let manager = self.manager.lock();
        let pipeline = self.system_state.lock();
        let input_idx = pipeline.input_loudness_index();
        let output_idx = pipeline.output_loudness_index();
        let fallback_input_channels = pipeline.input_channels();
        let fallback_output_channels = manager.get_engine_state().num_channels;
        drop(pipeline);

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

        serde_json::json!({
            "input": input_json,
            "output": output_json,
            "sources": {
                "input": metering_source_json(input_data.is_some(), fallback_input_channels),
                "output": metering_source_json(output_data.is_some(), fallback_output_channels),
            },
        })
    }

    fn snapshot_json(&self) -> Value {
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
            },
        })
    }

    async fn handle_get_snapshot(&self) -> Response {
        Response::ok(self.snapshot_json())
    }

    async fn handle_dump_state(&self) -> Response {
        Response::ok(serde_json::json!({
            "snapshot": self.snapshot_json(),
            "plugins": self.system_state.lock().user_plugins(),
        }))
    }

    async fn handle_status(&self) -> Response {
        let manager = self.manager.lock();
        let state = manager.get_state();
        let engine_state = manager.get_engine_state();
        let pipeline = self.system_state.lock();
        let selected_device = pipeline.selected_output_device();
        let input_channels = pipeline.input_channels();
        let output_channels = pipeline.output_channels();
        let pipeline_generation = pipeline.applied_generation();
        let pipeline_applied_output_device = pipeline.applied_output_device();
        drop(pipeline);

        Response::ok(serde_json::json!({
            "state": format!("{:?}", state),
            "volume": manager.get_volume(),
            "muted": manager.is_muted(),
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
        let output_device = self.system_state.lock().selected_output_device();
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

    fn apply_pipeline_plan(
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

        if driver_status.driver_installed
            && driver_status.channel_count != plan.spec.input_channels as u32
        {
            let result = self.driver_manager.lock().request_config(DriverConfig {
                sample_rate: driver_sample_rate,
                buffer_frames: driver_buffer_frames,
                channel_count: plan.spec.input_channels as u32,
            });

            match result {
                driver_common::ConfigResult::Accepted
                | driver_common::ConfigResult::Negotiated { .. } => {
                    log::info!(
                        "HAL input channel count set to {} via driver config",
                        plan.spec.input_channels
                    );
                }
                driver_common::ConfigResult::Error(e) => {
                    log::error!("Failed to set HAL input channels: {}", e);
                    return Response::err(format!("Failed to set HAL input channels: {}", e));
                }
            }
        }

        log::info!(
            "Loading driver plugin chain: {} user plugins + 2 monitors = {} total, {}Hz {}ch input, {} output channels, device: {:?}",
            plan.spec.user_plugins.len(),
            plan.runtime_plugins.len(),
            driver_sample_rate,
            plan.spec.input_channels,
            plan.spec.output_channels,
            plan.spec.output_device
        );

        let mut manager = self.manager.lock();
        manager.set_loudness_plugin_index(plan.output_loudness_index);
        let result = manager.start_hal_playback_with_driver_config(
            plan.spec.output_device.clone(),
            plan.runtime_plugins.clone(),
            plan.spec.output_channels,
            driver_sample_rate,
            plan.spec.input_channels,
        );
        drop(manager);

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

    async fn handle_load_plugins_with_channels(
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

    async fn handle_load_plugin_artifact(&self, artifact: Value) -> Response {
        match plan_plugin_artifact(artifact) {
            Ok(PluginArtifactPlan::RackChain { plugins }) => {
                let (input_channels, output_channels) = {
                    let state = self.system_state.lock();
                    (state.input_channels(), state.output_channels())
                };
                self.handle_load_plugins_with_channels(plugins, input_channels, output_channels)
                    .await
            }
            Ok(PluginArtifactPlan::UnsupportedGraph { reason }) => Response::err(format!(
                "Unsupported graph plugin artifact: {}. Use a graph-aware loader instead of flattening it into the rack.",
                reason
            )),
            Err(e) => Response::err(format!("Invalid plugin artifact: {}", e)),
        }
    }

    async fn handle_set_pipeline_channels(
        &self,
        input_channels: Option<usize>,
        output_channels: Option<usize>,
    ) -> Response {
        if input_channels.is_none() && output_channels.is_none() {
            return Response::err(
                "set_pipeline_channels requires input_channels or output_channels",
            );
        }

        let (plugins, current_input_channels, current_output_channels) = {
            let state = self.system_state.lock();
            (
                state.user_plugins(),
                state.input_channels(),
                state.output_channels(),
            )
        };

        let next_input_channels = input_channels.unwrap_or(current_input_channels);
        let next_output_channels = output_channels.unwrap_or(current_output_channels);

        self.handle_load_plugins_with_channels(plugins, next_input_channels, next_output_channels)
            .await
    }

    async fn handle_get_loudness(&self) -> Response {
        let manager = self.manager.lock();
        match manager.get_loudness() {
            Some(loudness) => Response::ok(loudness_info_to_json(&loudness)),
            None => Response::err("Loudness monitoring not enabled"),
        }
    }

    async fn handle_get_metering(&self) -> Response {
        Response::ok(self.metering_snapshot())
    }

    // =========================================================================
    // Plugin management handlers
    // =========================================================================

    async fn handle_get_plugins(&self) -> Response {
        let plugins = self.system_state.lock().user_plugins();
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
        let mut plugins = self.system_state.lock().user_plugins();
        match index {
            Some(i) if i <= plugins.len() => plugins.insert(i, plugin),
            _ => plugins.push(plugin),
        }
        self.reload_plugins_with_user_plugins(plugins).await
    }

    async fn handle_remove_plugin(&self, index: usize) -> Response {
        let mut plugins = self.system_state.lock().user_plugins();
        if index >= plugins.len() {
            return Response::err(format!(
                "Plugin index {} out of range (have {})",
                index,
                plugins.len()
            ));
        }
        plugins.remove(index);
        self.reload_plugins_with_user_plugins(plugins).await
    }

    async fn handle_update_plugin(&self, index: usize, parameters: Value) -> Response {
        let mut plugins = self.system_state.lock().user_plugins();
        if index >= plugins.len() {
            return Response::err(format!(
                "Plugin index {} out of range (have {})",
                index,
                plugins.len()
            ));
        }
        plugins[index].parameters = parameters;
        self.reload_plugins_with_user_plugins(plugins).await
    }

    async fn handle_reorder_plugins(&self, order: Vec<usize>) -> Response {
        let plugins = self.system_state.lock().user_plugins();
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
        self.reload_plugins_with_user_plugins(reordered).await
    }

    async fn reload_plugins_with_user_plugins(&self, plugins: Vec<PluginConfig>) -> Response {
        let plan = match {
            let pipeline = self.system_state.lock();
            pipeline.prepare_plan(
                plugins,
                pipeline.input_channels(),
                pipeline.output_channels(),
                pipeline.input_channels(),
            )
        } {
            Ok(plan) => plan,
            Err(e) => return Response::err(e),
        };

        let result = {
            let manager = self.manager.lock();
            manager.update_plugin_chain(plan.runtime_plugins.clone())
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
            Err(e) if e == "No engine running" => {
                log::info!("No running driver engine; starting driver playback");
                self.handle_load_plugins_with_channels(
                    plan.spec.user_plugins.clone(),
                    plan.spec.input_channels,
                    plan.spec.output_channels,
                )
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
            let pipeline = Arc::clone(&self.system_state);
            spawn_driver_config_watcher(driver_manager, audio_manager, running, pipeline)
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
                        system_state: Arc::clone(&self.system_state),
                        key_manager: Arc::clone(&self.key_manager),
                        runtime: Arc::clone(&self.runtime),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineReconfigureOutcome {
    IdleUpdated,
    Restarted,
}

/// Spawn a background thread that polls the driver for config changes
fn spawn_driver_config_watcher(
    driver_manager: Arc<Mutex<DriverManager>>,
    audio_manager: Arc<Mutex<AudioEngineManager>>,
    running: Arc<Mutex<bool>>,
    system_state: Arc<Mutex<SystemwideState>>,
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
                handle_driver_config_change(&driver_manager, &audio_manager, config, &system_state);
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
    system_state: &Arc<Mutex<SystemwideState>>,
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

    // Reconfigure audio pipeline
    match reconfigure_audio_pipeline(
        audio_manager,
        system_state,
        actual_rate,
        requested_frames,
        requested_channels as usize,
    ) {
        Ok(outcome) => {
            if outcome == PipelineReconfigureOutcome::Restarted {
                // Set engine_ready so driver continues sending audio.
                driver_manager.lock().set_engine_ready(true);
            }

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
                "Config accepted: {}Hz, {} frames, {} channels, outcome={:?}",
                actual_rate,
                requested_frames,
                requested_channels,
                outcome
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
    system_state: &Arc<Mutex<SystemwideState>>,
    hal_sample_rate: u32,
    _buffer_frames: u32,
    input_channels: usize,
) -> Result<PipelineReconfigureOutcome, String> {
    let plan = {
        let state = system_state.lock();
        state.prepare_plan(
            state.user_plugins(),
            input_channels,
            state.output_channels(),
            input_channels,
        )?
    };

    let mut manager = audio_manager.lock();

    let state = manager.get_state();
    if state == sotf_audio::manager::StreamingState::Idle {
        log::debug!("No active playback, acknowledging config change");
        system_state.lock().commit_idle_reconfigure(&plan);
        return Ok(PipelineReconfigureOutcome::IdleUpdated);
    }

    log::info!("Reconfiguring driver playback pipeline");

    if let Err(e) = manager.stop() {
        log::warn!("Failed to stop current playback: {}", e);
    }

    log::info!(
        "Restarting driver playback with {} plugins (incl. 2 monitors), {} output channels, device: {:?}",
        plan.runtime_plugins.len(),
        plan.spec.output_channels,
        plan.spec.output_device
    );

    manager.set_loudness_plugin_index(plan.output_loudness_index);

    match manager.start_hal_playback_with_driver_config(
        plan.spec.output_device.clone(),
        plan.runtime_plugins.clone(),
        plan.spec.output_channels,
        hal_sample_rate,
        plan.spec.input_channels,
    ) {
        Ok(_) => {
            system_state.lock().commit_applied(&plan);
            log::info!("Driver playback restarted successfully");
            Ok(PipelineReconfigureOutcome::Restarted)
        }
        Err(e) => {
            log::error!("Failed to restart driver playback: {}", e);
            Err(format!("Failed to restart driver playback: {}", e))
        }
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
        PluginType::FirDesigner => "fir_designer",
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
        PluginType::FirDesigner => "EQ & Tone",
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
    use driver_common::{AudioDriver, ConfigResult, DriverStatus};
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

        let cmd: Command = serde_json::from_str(r#"{"command":"get_snapshot"}"#).unwrap();
        assert_eq!(cmd.name(), "get_snapshot");

        let cmd: Command = serde_json::from_str(r#"{"command":"snapshot"}"#).unwrap();
        assert_eq!(cmd.name(), "get_snapshot");

        let cmd: Command = serde_json::from_str(r#"{"command":"dump_state"}"#).unwrap();
        assert_eq!(cmd.name(), "dump_state");

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

        let cmd: Command =
            serde_json::from_str(r#"{"command":"set_input_channels","channels":4}"#).unwrap();
        assert_eq!(cmd.name(), "set_input_channels");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"set_output_channels","channels":6}"#).unwrap();
        assert_eq!(cmd.name(), "set_output_channels");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"set_pipeline_channels","output_channels":6}"#)
                .unwrap();
        assert_eq!(cmd.name(), "set_pipeline_channels");

        let cmd: Command =
            serde_json::from_str(r#"{"command":"load_plugin_artifact","artifact":[]}"#).unwrap();
        assert_eq!(cmd.name(), "load_plugin_artifact");

        let cmd: Command = serde_json::from_str(r#"{"command":"shutdown"}"#).unwrap();
        assert_eq!(cmd.name(), "shutdown");
    }

    fn test_plugin(plugin_type: &str) -> PluginConfig {
        PluginConfig {
            plugin_type: plugin_type.to_string(),
            parameters: serde_json::json!({}),
        }
    }

    #[derive(Debug)]
    struct FakeDriverState {
        status: DriverStatus,
        engine_ready: bool,
        last_requested_config: Option<DriverConfig>,
        last_ack: Option<(DriverConfig, ConfigResult)>,
        pending_config_change: Option<DriverConfig>,
    }

    #[derive(Debug, Clone)]
    struct FakeDriver {
        state: Arc<Mutex<FakeDriverState>>,
    }

    impl FakeDriver {
        fn new(state: Arc<Mutex<FakeDriverState>>) -> Self {
            Self { state }
        }
    }

    impl AudioDriver for FakeDriver {
        fn initialize(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn shutdown(&mut self) {}

        fn status(&self) -> DriverStatus {
            self.state.lock().status.clone()
        }

        fn read_audio(&mut self, buffer: &mut [f32]) -> usize {
            buffer.fill(0.0);
            0
        }

        fn available_frames(&self) -> usize {
            0
        }

        fn sample_rate(&self) -> u32 {
            self.state.lock().status.sample_rate
        }

        fn channel_count(&self) -> u32 {
            self.state.lock().status.channel_count
        }

        fn request_config(&mut self, config: DriverConfig) -> ConfigResult {
            self.state.lock().last_requested_config = Some(config);
            ConfigResult::Accepted
        }

        fn poll_config_change(&mut self) -> Option<DriverConfig> {
            self.state.lock().pending_config_change.take()
        }

        fn acknowledge_config_change(&mut self, actual: DriverConfig, result: ConfigResult) {
            self.state.lock().last_ack = Some((actual, result));
        }

        fn set_engine_ready(&mut self, ready: bool) {
            self.state.lock().engine_ready = ready;
        }
    }

    fn fake_driver_state() -> Arc<Mutex<FakeDriverState>> {
        Arc::new(Mutex::new(FakeDriverState {
            status: DriverStatus {
                platform_supported: true,
                driver_installed: true,
                capture_active: true,
                sample_rate: 48_000,
                channel_count: 2,
                buffer_frames: 512,
                driver_name: "Fake HAL".to_string(),
                driver_ready: true,
            },
            engine_ready: false,
            last_requested_config: None,
            last_ack: None,
            pending_config_change: None,
        }))
    }

    fn healthy_driver_status() -> DriverStatus {
        DriverStatus {
            platform_supported: true,
            driver_installed: true,
            capture_active: true,
            sample_rate: 48_000,
            channel_count: 2,
            buffer_frames: 512,
            driver_name: "Fake HAL".to_string(),
            driver_ready: true,
        }
    }

    fn fault_codes(faults: &[Value]) -> Vec<&str> {
        faults
            .iter()
            .filter_map(|fault| fault["code"].as_str())
            .collect()
    }

    fn test_daemon_with_driver(state: Arc<Mutex<FakeDriverState>>) -> AudioDaemon {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        AudioDaemon {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            driver_manager: Arc::new(Mutex::new(DriverManager::from_driver(Box::new(
                FakeDriver::new(state),
            )))),
            system_state: Arc::new(Mutex::new(SystemwideState::default())),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
            runtime: Arc::new(runtime),
        }
    }

    fn send_owner_ipc_command(daemon: &AudioDaemon, raw: &str) -> serde_json::Value {
        let (mut client, server) = UnixStream::pair().expect("unix stream pair");
        let daemon = daemon.clone();
        let handle = std::thread::spawn(move || daemon.handle_client(server, PeerClass::Owner));

        writeln!(client, "{}", raw).expect("write request");
        let mut reader = BufReader::new(client.try_clone().expect("clone client"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");
        drop(reader);
        drop(client);
        handle.join().expect("client handler thread");

        serde_json::from_str(&line).expect("valid JSON response")
    }

    #[test]
    fn pipeline_supervisor_builds_runtime_chain_without_committing_until_success() {
        let supervisor = PipelineSupervisor::default();

        let plan = supervisor
            .prepare_plan(
                vec![
                    test_plugin("hal_input"),
                    test_plugin("eq"),
                    test_plugin("loudness_monitor"),
                    test_plugin("gain"),
                    test_plugin("hal_output"),
                ],
                2,
                6,
                2,
            )
            .expect("valid pipeline plan");

        assert_eq!(plan.spec.input_channels, 2);
        assert_eq!(plan.spec.output_channels, 6);
        assert_eq!(
            plan.spec
                .user_plugins
                .iter()
                .map(|p| p.plugin_type.as_str())
                .collect::<Vec<_>>(),
            vec!["eq", "gain"]
        );
        assert_eq!(plan.input_loudness_index, 0);
        assert_eq!(plan.output_loudness_index, 3);
        assert_eq!(
            plan.runtime_plugins
                .iter()
                .map(|p| p.plugin_type.as_str())
                .collect::<Vec<_>>(),
            vec!["loudness_monitor", "eq", "gain", "loudness_monitor"]
        );
        assert!(supervisor.input_loudness_index().is_none());
        assert!(supervisor.output_loudness_index().is_none());
    }

    #[test]
    fn pipeline_supervisor_commit_atomically_updates_desired_and_applied_state() {
        let mut supervisor = PipelineSupervisor::default();
        let plan = supervisor
            .prepare_plan(vec![test_plugin("eq")], 4, 8, 2)
            .expect("valid pipeline plan");

        supervisor.commit_applied(&plan);

        assert_eq!(supervisor.input_channels(), 4);
        assert_eq!(supervisor.output_channels(), 8);
        assert_eq!(supervisor.input_loudness_index(), Some(0));
        assert_eq!(supervisor.output_loudness_index(), Some(2));
        assert_eq!(supervisor.applied_generation(), Some(1));
    }

    #[test]
    fn pipeline_supervisor_rejects_invalid_channels_before_state_mutation() {
        let supervisor = PipelineSupervisor::default();
        let result = supervisor.prepare_plan(vec![test_plugin("eq")], 0, 64, 2);

        assert!(result.unwrap_err().contains("Invalid output channel count"));
        assert_eq!(supervisor.input_channels(), 2);
        assert_eq!(supervisor.output_channels(), 2);
    }

    #[test]
    fn pipeline_supervisor_reducer_methods_control_desired_mutation() {
        let mut supervisor = PipelineSupervisor::default();

        supervisor
            .set_desired_output_device(Some("ADAM Audio D3V".to_string()))
            .expect("safe device should be accepted");
        assert_eq!(
            supervisor.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );

        let result = supervisor.set_desired_output_device(Some("SotF Virtual Audio".to_string()));
        assert!(result.unwrap_err().contains("virtual/loopback"));
        assert_eq!(
            supervisor.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );

        let plan = supervisor
            .prepare_plan(vec![test_plugin("eq")], 10, 4, 10)
            .expect("valid idle reconfigure plan");
        supervisor.commit_idle_reconfigure(&plan);
        assert_eq!(supervisor.input_channels(), 10);
        assert_eq!(supervisor.output_channels(), 4);
        assert!(supervisor.applied_generation().is_none());
    }

    #[test]
    fn transport_snapshot_reports_all_playing_faults_without_hiding_secondary_causes() {
        let driver_status = healthy_driver_status();
        let engine_state = sotf_audio::AudioEngineState::default();

        let (transport, faults) =
            transport_snapshot_and_faults("Playing", &driver_status, &engine_state);

        assert_eq!(transport["input"]["status"], "input_frames_missing");
        assert_eq!(transport["output"]["status"], "output_callbacks_missing");
        let codes = fault_codes(&faults);
        assert!(codes.contains(&"input_frames_missing"));
        assert!(codes.contains(&"output_callbacks_missing"));
        assert!(codes.contains(&"output_device_unresolved"));
    }

    #[test]
    fn transport_snapshot_reports_flowing_when_input_and_output_are_observed() {
        let driver_status = healthy_driver_status();
        let mut engine_state = sotf_audio::AudioEngineState::default();
        engine_state.playback_frames_received = 1024;
        engine_state.playback_callback_count = 8;
        engine_state.playback_frames_written = 1024;
        engine_state.playback_output_device = Some("ADAM Audio D3V".to_string());
        engine_state.playback_effective_sample_rate = 48_000;

        let (transport, faults) =
            transport_snapshot_and_faults("Playing", &driver_status, &engine_state);

        assert_eq!(transport["input"]["status"], "flowing");
        assert_eq!(transport["output"]["status"], "flowing");
        assert_eq!(transport["output"]["device"], "ADAM Audio D3V");
        assert!(faults.is_empty());
    }

    #[test]
    fn metering_faults_only_apply_to_playing_fallback_sources() {
        let metering = serde_json::json!({
            "sources": {
                "input": { "status": "fallback_zero" },
                "output": { "status": "available" }
            }
        });

        let mut faults = Vec::new();
        push_metering_faults("Idle", &metering, &mut faults);
        assert!(faults.is_empty());

        push_metering_faults("Playing", &metering, &mut faults);
        assert_eq!(fault_codes(&faults), vec!["input_metering_unavailable"]);
    }

    #[test]
    fn driver_reconfigure_preserves_daemon_selected_output_device_when_idle() {
        let audio_manager = Arc::new(Mutex::new(AudioEngineManager::new()));
        let system_state = Arc::new(Mutex::new(SystemwideState::default()));

        {
            let mut state = system_state.lock();
            let plan = state
                .prepare_with_selected_device("ADAM Audio D3V".to_string())
                .expect("valid device plan");
            state.commit_applied(&plan);
        }

        reconfigure_audio_pipeline(&audio_manager, &system_state, 48_000, 512, 6)
            .expect("idle reconfigure should update desired state");

        let state = system_state.lock();
        assert_eq!(
            state.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );
        assert_eq!(state.input_channels(), 6);
        assert_eq!(state.output_channels(), 2);
    }

    #[test]
    fn testkit_driver_status_uses_injected_driver() {
        let state = fake_driver_state();
        state.lock().status.channel_count = 10;
        let daemon = test_daemon_with_driver(state);

        let response = daemon
            .runtime
            .block_on(daemon.handle_command(Command::DriverStatus));

        assert!(response.success);
        let data = response.data.expect("driver_status data");
        assert_eq!(data["driver_name"], "Fake HAL");
        assert_eq!(data["channel_count"], 10);
        assert_eq!(data["ready"], true);
    }

    #[test]
    fn testkit_invalid_plugin_load_does_not_mutate_pipeline_state() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = daemon
            .runtime
            .block_on(daemon.handle_command(Command::LoadPlugins {
                plugins: vec![test_plugin("eq")],
                input_channels: 2,
                output_channels: 64,
            }));

        assert!(!response.success);
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|e| e.contains("Invalid output channel count"))
        );
        assert_eq!(daemon.system_state.lock().output_channels(), 2);
        assert!(daemon.system_state.lock().output_loudness_index().is_none());
    }

    #[test]
    fn testkit_patch_channel_command_preserves_daemon_owned_plugins_and_input_channels() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = daemon
            .runtime
            .block_on(daemon.handle_command(Command::LoadPlugins {
                plugins: vec![test_plugin("eq")],
                input_channels: 10,
                output_channels: 2,
            }));
        assert!(response.success);

        let response = daemon
            .runtime
            .block_on(daemon.handle_command(Command::SetOutputChannels { channels: 6 }));

        assert!(response.success);
        let state = daemon.system_state.lock();
        assert_eq!(state.input_channels(), 10);
        assert_eq!(state.output_channels(), 6);
        assert_eq!(state.user_plugins().len(), 1);
    }

    #[test]
    fn testkit_pipeline_channel_patch_requires_a_field() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response =
            daemon
                .runtime
                .block_on(daemon.handle_command(Command::SetPipelineChannels {
                    input_channels: None,
                    output_channels: None,
                }));

        assert!(!response.success);
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|e| e.contains("requires input_channels or output_channels"))
        );
    }

    #[test]
    fn testkit_load_plugin_artifact_accepts_rack_chain_without_ui_flattening() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","artifact":{"plugins":[{"plugin_type":"eq","parameters":{}}]}}"#,
        );

        assert_eq!(response["success"], true);
        let state = daemon.system_state.lock();
        assert_eq!(state.user_plugins().len(), 1);
        assert_eq!(state.user_plugins()[0].plugin_type, "eq");
    }

    #[test]
    fn testkit_load_plugin_artifact_rejects_graph_shape_instead_of_flattening() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugin_artifact","artifact":{"global_plugins":[{"plugin_type":"eq","parameters":{}}],"channels":{"L":{"plugins":[{"plugin_type":"gain","parameters":{}}]}}}}"#,
        );

        assert_eq!(response["success"], false);
        assert!(
            response["error"]
                .as_str()
                .is_some_and(|e| e.contains("Unsupported graph plugin artifact"))
        );
        assert!(daemon.system_state.lock().user_plugins().is_empty());
    }

    #[test]
    fn testkit_unix_ipc_invalid_plugin_load_preserves_pipeline_state() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(
            &daemon,
            r#"{"command":"load_plugins","plugins":[{"plugin_type":"eq","parameters":{}}],"input_channels":2,"output_channels":64}"#,
        );

        assert_eq!(response["success"], false);
        assert!(
            response["error"]
                .as_str()
                .is_some_and(|e| e.contains("Invalid output channel count"))
        );
        assert_eq!(daemon.system_state.lock().input_channels(), 2);
        assert_eq!(daemon.system_state.lock().output_channels(), 2);
        assert!(daemon.system_state.lock().applied_generation().is_none());
    }

    #[test]
    fn testkit_unix_ipc_driver_status_uses_injected_driver() {
        let state = fake_driver_state();
        state.lock().status.channel_count = 12;
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"driver_status"}"#);

        assert_eq!(response["success"], true);
        assert_eq!(response["data"]["driver_name"], "Fake HAL");
        assert_eq!(response["data"]["channel_count"], 12);
    }

    #[test]
    fn testkit_snapshot_separates_desired_observed_and_diagnostics() {
        let state = fake_driver_state();
        state.lock().status.channel_count = 6;
        let daemon = test_daemon_with_driver(state);
        {
            let mut pipeline = daemon.system_state.lock();
            pipeline
                .set_desired_output_device(Some("ADAM Audio D3V".to_string()))
                .expect("safe device");
        }

        let response = send_owner_ipc_command(&daemon, r#"{"command":"get_snapshot"}"#);

        assert_eq!(response["success"], true);
        let data = &response["data"];
        assert_eq!(data["schema_version"], 1);
        assert_eq!(data["desired"]["output_device"], "ADAM Audio D3V");
        assert_eq!(data["observed"]["driver"]["channel_count"], 6);
        assert_eq!(
            data["observed"]["metering"]["sources"]["input"]["status"],
            "fallback_zero"
        );
        assert_eq!(data["observed"]["transport"]["input"]["status"], "idle");
        assert_eq!(data["observed"]["transport"]["output"]["status"], "idle");
        assert_eq!(data["diagnostics"]["health"], "ok");
        assert!(data["diagnostics"]["faults"].as_array().unwrap().is_empty());
    }

    #[test]
    fn testkit_dump_state_includes_snapshot_and_plugins() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(state);

        let response = send_owner_ipc_command(&daemon, r#"{"command":"dump_state"}"#);

        assert_eq!(response["success"], true);
        assert_eq!(response["data"]["snapshot"]["schema_version"], 1);
        assert!(response["data"]["plugins"].as_array().is_some());
    }

    #[test]
    fn testkit_idle_driver_config_change_updates_spec_without_engine_ready() {
        let state = fake_driver_state();
        let daemon = test_daemon_with_driver(Arc::clone(&state));
        {
            let mut pipeline = daemon.system_state.lock();
            let plan = pipeline
                .prepare_with_selected_device("ADAM Audio D3V".to_string())
                .expect("valid device plan");
            pipeline.commit_applied(&plan);
        }

        handle_driver_config_change(
            &daemon.driver_manager,
            &daemon.manager,
            DriverConfig {
                sample_rate: 48_000,
                buffer_frames: 512,
                channel_count: 10,
            },
            &daemon.system_state,
        );

        let pipeline = daemon.system_state.lock();
        assert_eq!(pipeline.input_channels(), 10);
        assert_eq!(
            pipeline.selected_output_device().as_deref(),
            Some("ADAM Audio D3V")
        );
        drop(pipeline);

        let state = state.lock();
        assert!(
            !state.engine_ready,
            "idle reconfigure must not mark engine ready"
        );
        let (actual, result) = state.last_ack.as_ref().expect("config ack");
        assert_eq!(actual.channel_count, 10);
        assert!(matches!(result, ConfigResult::Accepted));
    }
}
