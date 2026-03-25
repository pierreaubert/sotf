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
mod security;

use driver_manager::{DriverManager, get_driver_status};
use security::{
    KeyManager, ensure_secure_socket_dir, get_secure_socket_path, verify_peer_credentials,
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
            current_output_channels: Arc::new(Mutex::new(2)),
            input_loudness_index: Arc::new(Mutex::new(None)),
            output_loudness_index: Arc::new(Mutex::new(None)),
        }
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
                output_channels,
            } => {
                self.handle_load_plugins_with_channels(plugins, output_channels)
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

        Response::ok(serde_json::json!({
            "state": format!("{:?}", state),
            "volume": manager.get_volume(),
            "muted": manager.is_muted(),
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
        // Clear engine_ready BEFORE acquiring manager lock to avoid
        // lock-order inversion with the config watcher thread
        // (which acquires driver_manager -> manager).
        self.driver_manager.lock().set_engine_ready(false);
        log::debug!("Cleared engine_ready flag via driver");

        let mut manager = self.manager.lock();
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

                // Store with ASIO prefix preserved so playback thread selects the right host
                let stored_name = if is_asio {
                    format!("{}{}", sotf_audio::devices::ASIO_DEVICE_PREFIX, resolved_name)
                } else {
                    resolved_name.clone()
                };
                *self.selected_device.lock() = Some(stored_name);
                log::info!("Output device set to: {} (matched from '{}')", resolved_name, device);

                let manager = self.manager.lock();
                let state = manager.get_state();
                if state != sotf_audio::manager::StreamingState::Idle {
                    log::warn!(
                        "Device change will take effect on next playback start (current state: {:?})",
                        state
                    );
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
        output_channels: usize,
    ) -> Response {
        // Strip obsolete hal_input/hal_output plugins (toolbar may still send them)
        let plugins: Vec<PluginConfig> = plugins
            .into_iter()
            .filter(|p| {
                let pt = p.plugin_type.as_str();
                if pt == "hal_input" || pt == "hal_output" {
                    log::warn!("Stripping obsolete '{}' plugin from chain — decoder thread handles driver I/O directly", pt);
                    false
                } else {
                    true
                }
            })
            .collect();

        // Also strip loudness_monitor — we auto-inject them at known positions
        let plugins: Vec<PluginConfig> = plugins
            .into_iter()
            .filter(|p| p.plugin_type != "loudness_monitor")
            .collect();

        // Store user's plugin configuration BEFORE adding monitors
        *self.current_plugins.lock() = plugins.clone();
        *self.current_output_channels.lock() = output_channels;

        let mut manager = self.manager.lock();
        let mut output_device = self.selected_device.lock().clone();

        // If no device selected (or it's the virtual device), find a safe physical fallback
        if output_device.is_none()
            || output_device
                .as_ref()
                .map(|d| d.contains("SotF"))
                .unwrap_or(false)
        {
            output_device = find_fallback_output_device();
            if let Some(ref dev) = output_device {
                log::info!("Using fallback output device for driver playback: {}", dev);
            }
        }

        // Stop current playback if running
        let _ = manager.stop();

        // Build final plugin chain: input_monitor + user plugins + output_monitor
        let mut final_plugins = Vec::with_capacity(plugins.len() + 2);

        // Index 0: input loudness monitor (measures signal before processing)
        let input_monitor_index = 0;
        final_plugins.push(PluginConfig {
            plugin_type: "loudness_monitor".to_string(),
            parameters: serde_json::json!({}),
        });

        // User's processing plugins
        final_plugins.extend(plugins);

        // Last: output loudness monitor (measures signal after processing)
        let output_monitor_index = final_plugins.len();
        final_plugins.push(PluginConfig {
            plugin_type: "loudness_monitor".to_string(),
            parameters: serde_json::json!({}),
        });

        // Store monitor indices for get_metering
        *self.input_loudness_index.lock() = Some(input_monitor_index);
        *self.output_loudness_index.lock() = Some(output_monitor_index);

        log::info!(
            "Loading driver plugin chain: {} user plugins + 2 monitors = {} total, {} output channels, device: {:?}",
            final_plugins.len() - 2,
            final_plugins.len(),
            output_channels,
            output_device
        );

        // Set the output loudness index for backward compat (get_loudness command)
        manager.set_loudness_plugin_index(output_monitor_index);

        // Start driver playback (no file source needed)
        let result = manager.start_hal_playback(output_device, final_plugins, output_channels);

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
            Some(loudness) => Response::ok(serde_json::json!({
                "momentary": loudness.momentary_lufs,
                "short_term": loudness.shortterm_lufs,
                "integrated": loudness.integrated_lufs,
                "peak": loudness.peak,
            })),
            None => Response::err("Loudness monitoring not enabled"),
        }
    }

    async fn handle_get_metering(&self) -> Response {
        let manager = self.manager.lock();
        let input_idx = *self.input_loudness_index.lock();
        let output_idx = *self.output_loudness_index.lock();

        let loudness_to_json = |info: &sotf_audio::LoudnessInfo| -> Value {
            serde_json::json!({
                "momentary": info.momentary_lufs,
                "short_term": info.shortterm_lufs,
                "integrated": info.integrated_lufs,
                "peak": info.peak,
            })
        };

        let input_data = input_idx.and_then(|idx| {
            manager
                .get_cached_plugin_data(idx)
                .and_then(|data| data.downcast_ref::<sotf_audio::LoudnessInfo>().cloned())
        });

        let output_data = output_idx.and_then(|idx| {
            manager
                .get_cached_plugin_data(idx)
                .and_then(|data| data.downcast_ref::<sotf_audio::LoudnessInfo>().cloned())
        });

        let input_json = input_data
            .as_ref()
            .map(loudness_to_json)
            .unwrap_or(serde_json::json!(null));
        let output_json = output_data
            .as_ref()
            .map(loudness_to_json)
            .unwrap_or(serde_json::json!(null));

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
                serde_json::json!({
                    "type": engine_type,
                    "name": pt.name(),
                    "description": pt.description(),
                    "category": category,
                    "maturity": format!("{:?}", pt.maturity()),
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
        let plugins = self.current_plugins.lock().clone();
        let output_channels = *self.current_output_channels.lock();
        self.handle_load_plugins_with_channels(plugins, output_channels)
            .await
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

    async fn handle_set_encryption(&self, enabled: bool) -> Response {
        let mut key_manager = self.key_manager.lock();
        key_manager.set_enabled(enabled);

        // On macOS with HAL, update shared memory encryption flag
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            if let Ok(mut buffer) = driver_hal::SharedAudioBuffer::open_default() {
                buffer.set_encrypted(enabled);
                if enabled {
                    buffer.set_key_fingerprint(*key_manager.fingerprint());
                }
                buffer.set_config_changed();
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
                    if key_manager.is_enabled()
                        && let Ok(mut buffer) = driver_hal::SharedAudioBuffer::open_default()
                    {
                        buffer.set_key_fingerprint(*key_manager.fingerprint());
                        buffer.set_config_changed();
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
        });

        match result {
            driver_common::ConfigResult::Accepted | driver_common::ConfigResult::Negotiated { .. } => {
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
        });

        match result {
            driver_common::ConfigResult::Accepted | driver_common::ConfigResult::Negotiated { .. } => {
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
            "buffer_frames": status.buffer_frames,
            "channel_count": status.channel_count,
            "active": status.capture_active,
            "driver_name": status.driver_name,
            "driver_installed": status.driver_installed,
            "platform_supported": status.platform_supported,
        }))
    }

    fn handle_client(&self, mut stream: UnixStream) {
        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to clone stream for reading: {}", e);
                return;
            }
        };
        let mut reader = BufReader::new(reader_stream);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let response = match serde_json::from_str::<Command>(trimmed) {
                        Ok(cmd) => {
                            self.runtime.block_on(self.handle_command(cmd))
                        }
                        Err(e) => Response::err(format!("Invalid command: {}", e)),
                    };

                    let json = serde_json::to_string(&response).unwrap();
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
            let current_output_channels = Arc::clone(&self.current_output_channels);
            let input_loudness_index = Arc::clone(&self.input_loudness_index);
            let output_loudness_index = Arc::clone(&self.output_loudness_index);
            spawn_driver_config_watcher(
                driver_manager,
                audio_manager,
                running,
                current_plugins,
                current_output_channels,
                input_loudness_index,
                output_loudness_index,
            )
        };

        // Try to bind the socket, handling TOCTOU race properly
        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if let Ok(_stream) = UnixStream::connect(&socket_path) {
                    return Err("Another daemon instance is already running".into());
                }
                let _ = std::fs::remove_file(&socket_path);
                UnixListener::bind(&socket_path)?
            }
            Err(e) => return Err(e.into()),
        };
        println!("Audio daemon listening on {}", socket_path.display());

        // Also create legacy symlink for backwards compatibility with old clients
        if socket_path.to_string_lossy() != LEGACY_SOCKET_PATH {
            let _ = std::fs::remove_file(LEGACY_SOCKET_PATH);
            if std::os::unix::fs::symlink(&socket_path, LEGACY_SOCKET_PATH).is_ok() {
                println!("Legacy socket symlink: {}", LEGACY_SOCKET_PATH);
            }
        }

        // Accept connections (non-blocking so Ctrl-C can interrupt)
        listener.set_nonblocking(true)?;
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

                    match verify_peer_credentials(&stream) {
                        Ok(peer_uid) => {
                            log::debug!("Accepted connection from UID {}", peer_uid);
                        }
                        Err(e) => {
                            log::warn!("Rejected unauthorized connection: {}", e);
                            continue;
                        }
                    }

                    // Clone daemon for client thread
                    let daemon = AudioDaemon {
                        manager: Arc::clone(&self.manager),
                        running: Arc::clone(&self.running),
                        driver_manager: Arc::clone(&self.driver_manager),
                        selected_device: Arc::clone(&self.selected_device),
                        key_manager: Arc::clone(&self.key_manager),
                        runtime: Arc::clone(&self.runtime),
                        current_plugins: Arc::clone(&self.current_plugins),
                        current_output_channels: Arc::clone(&self.current_output_channels),
                        input_loudness_index: Arc::clone(&self.input_loudness_index),
                        output_loudness_index: Arc::clone(&self.output_loudness_index),
                    };

                    std::thread::spawn(move || {
                        daemon.handle_client(stream);
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

        // Cleanup
        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_file(LEGACY_SOCKET_PATH);

        let _ = config_watcher.join();

        Ok(())
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
    current_output_channels: &Arc<Mutex<usize>>,
    input_loudness_index: &Arc<Mutex<Option<usize>>>,
    output_loudness_index: &Arc<Mutex<Option<usize>>>,
) {
    let requested_rate = config.sample_rate;
    let requested_frames = config.buffer_frames;

    log::info!(
        "Driver config change request: sample_rate={}, buffer_frames={}",
        requested_rate,
        requested_frames
    );

    // Validate requested values
    if requested_rate == 0 {
        log::warn!("Invalid config request: sample_rate=0, ignoring");
        driver_manager.lock().acknowledge_config_change(
            DriverConfig { sample_rate: 48000, buffer_frames: requested_frames },
            driver_common::ConfigResult::Error("Invalid sample rate".to_string()),
        );
        return;
    }
    if requested_frames == 0 || requested_frames > 65536 {
        log::warn!("Invalid config request: buffer_frames={}, out of range", requested_frames);
        driver_manager.lock().acknowledge_config_change(
            DriverConfig { sample_rate: requested_rate, buffer_frames: 512 },
            driver_common::ConfigResult::Error("Invalid buffer frames".to_string()),
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
                log::info!("Config negotiated: requested {}Hz, using {}Hz", requested_rate, actual_rate);
                driver_common::ConfigResult::Negotiated {
                    actual_rate,
                    actual_frames: requested_frames,
                }
            } else {
                driver_common::ConfigResult::Accepted
            };

            driver_manager.lock().acknowledge_config_change(
                DriverConfig { sample_rate: actual_rate, buffer_frames: requested_frames },
                result,
            );
            log::info!("Config accepted: {}Hz, {} frames, engine_ready=true", actual_rate, requested_frames);
        }
        Err(e) => {
            log::error!("Pipeline reconfiguration failed: {}", e);
            driver_manager.lock().acknowledge_config_change(
                DriverConfig { sample_rate: actual_rate, buffer_frames: requested_frames },
                driver_common::ConfigResult::Error(e),
            );
        }
    }
}

/// Reconfigure the audio pipeline with new sample rate and buffer size
fn reconfigure_audio_pipeline(
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    _hal_sample_rate: u32,
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

    let output_device = find_fallback_output_device();

    log::info!(
        "Restarting driver playback with {} plugins (incl. 2 monitors), {} output channels, device: {:?}",
        final_plugins.len(),
        output_channels,
        output_device
    );

    manager.set_loudness_plugin_index(output_monitor_index);

    match manager.start_hal_playback(output_device, final_plugins, output_channels) {
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

/// Map PluginType enum to the string the engine's create_plugin() expects
fn plugin_type_to_engine_str(pt: &PluginType) -> &'static str {
    match pt {
        PluginType::EQ => "eq",
        PluginType::Gain => "gain",
        PluginType::Upmixer => "upmixer",
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
        PluginType::Upmixer
        | PluginType::Downmix
        | PluginType::MonoToStereo
        | PluginType::Matrix
        | PluginType::ChannelMuteSolo => "Spatial & Routing",
        PluginType::BinauralDecoder | PluginType::XTC => "Spatial & Routing",
        PluginType::Convolution => "Effects",
        PluginType::Denoiser | PluginType::Pnd => "Restoration",
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

fn find_fallback_output_device() -> Option<String> {
    if let Ok(devices) = list_audio_devices() {
        let physical_device = devices.iter().find(|d| {
            let name = d.get("name").and_then(|n| n.as_str()).unwrap_or("");
            !name.contains("SotF")
                && !name.contains("BlackHole")
                && !name.contains("ZoomAudio")
                && !name.contains("Loopback")
        });

        if let Some(device) = physical_device {
            return device
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
        }
    }
    None
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
                    if status.platform_supported { "Yes" } else { "No" }
                );
                println!(
                    "   Driver installed:   {}",
                    if status.driver_installed { "Yes" } else { "No (optional)" }
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

    // Auto-start driver playback with empty plugin chain
    {
        println!("Auto-starting driver playback (2ch)...");

        let output_device = find_fallback_output_device();
        println!("   Output device: {:?}", output_device);

        if output_device.is_none() {
            println!("   WARNING: No physical output device found!");
        }

        let plugins: Vec<PluginConfig> = vec![];

        let result = daemon
            .runtime
            .block_on(daemon.handle_load_plugins_with_channels(plugins, 2));
        if result.success {
            println!("   Driver playback started successfully");
        } else {
            println!("   Driver playback failed: {:?}", result.error);
        }
    }

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
