//! Audio Engine Control Daemon
//!
//! A Unix socket daemon that provides IPC control for the AudioEngineManager.
//! This allows external processes (like the Swift menubar app) to control audio playback,
//! query status, and configure plugins via JSON messages over a Unix domain socket.
//!
//! Socket location: /tmp/autoeq_audio.sock
//!
//! Protocol: JSON messages over Unix socket
//!
//! Commands:
//! - {"command": "status"} -> Returns current state
//! - {"command": "load", "path": "/path/to/file.flac"} -> Load audio file
//! - {"command": "play"} -> Start playback
//! - {"command": "pause"} -> Pause playback
//! - {"command": "stop"} -> Stop playback
//! - {"command": "seek", "position": 10.5} -> Seek to position in seconds
//! - {"command": "set_volume", "volume": 0.8} -> Set volume (0.0-1.0)
//! - {"command": "list_devices"} -> List audio output devices
//! - {"command": "set_device", "device": "device_name"} -> Set output device
//! - {"command": "load_plugins", "plugins": [...]} -> Load plugin chain
//! - {"command": "get_loudness"} -> Get current loudness (LUFS)
//! - {"command": "hal_status"} -> Get HAL driver status (macOS only)
//! - {"command": "shutdown"} -> Gracefully shutdown daemon

mod hal_manager;
mod security;

use hal_manager::{HalManager, get_hal_status};
use security::{get_secure_socket_path, verify_peer_credentials, ensure_secure_socket_dir, KeyManager};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::manager::AudioEngineManager;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;

/// Legacy socket path for backwards compatibility
const LEGACY_SOCKET_PATH: &str = "/tmp/autoeq_audio.sock";

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
    LoadPlugins { plugins: Vec<PluginConfig> },
    #[serde(rename = "get_loudness")]
    GetLoudness,
    #[serde(rename = "hal_status")]
    HalStatus,
    #[serde(rename = "shutdown")]
    Shutdown,
    // Encryption commands
    #[serde(rename = "set_encryption")]
    SetEncryption { enabled: bool },
    #[serde(rename = "encryption_status")]
    EncryptionStatus,
    #[serde(rename = "rotate_encryption_key")]
    RotateEncryptionKey,
    // HAL config commands
    #[serde(rename = "set_sample_rate")]
    SetSampleRate { rate: u32 },
    #[serde(rename = "set_buffer_frames")]
    SetBufferFrames { frames: u32 },
    #[serde(rename = "get_hal_config")]
    GetHalConfig,
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
    hal_manager: Arc<Mutex<HalManager>>,
    /// Selected output device name (None = use default device)
    selected_device: Arc<Mutex<Option<String>>>,
    /// Encryption key manager
    key_manager: Arc<Mutex<KeyManager>>,
    /// Cached shared audio buffer (opened lazily)
    #[cfg(target_os = "macos")]
    shared_buffer: Arc<Mutex<Option<driver_hal::SharedAudioBuffer>>>,
    /// Shared Tokio runtime for async operations
    runtime: Arc<tokio::runtime::Runtime>,
    /// Current plugin configuration (user plugins, excluding auto-added resampler)
    current_plugins: Arc<Mutex<Vec<PluginConfig>>>,
    /// Current output channel count
    current_output_channels: Arc<Mutex<usize>>,
}

impl AudioDaemon {
    fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime");

        Self {
            manager: Arc::new(Mutex::new(AudioEngineManager::new())),
            running: Arc::new(Mutex::new(true)),
            hal_manager: Arc::new(Mutex::new(HalManager::new())),
            selected_device: Arc::new(Mutex::new(None)),
            key_manager: Arc::new(Mutex::new(KeyManager::default())),
            #[cfg(target_os = "macos")]
            shared_buffer: Arc::new(Mutex::new(None)),
            runtime: Arc::new(runtime),
            current_plugins: Arc::new(Mutex::new(Vec::new())),
            current_output_channels: Arc::new(Mutex::new(2)),
        }
    }

    /// Get the cached shared audio buffer, opening lazily if needed
    #[cfg(target_os = "macos")]
    fn get_shared_buffer(
        &self,
    ) -> Option<parking_lot::MappedMutexGuard<'_, driver_hal::SharedAudioBuffer>> {
        let mut guard = self.shared_buffer.lock();
        if guard.is_none() {
            match driver_hal::SharedAudioBuffer::open_default() {
                Ok(buffer) => {
                    *guard = Some(buffer);
                }
                Err(e) => {
                    log::debug!("Shared buffer not available: {}", e);
                    return None;
                }
            }
        }
        // Use MutexGuard::map to return guard over the inner buffer
        if guard.is_some() {
            Some(parking_lot::MutexGuard::map(guard, |opt| {
                opt.as_mut().unwrap()
            }))
        } else {
            None
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
            Command::LoadPlugins { plugins } => self.handle_load_plugins(plugins).await,
            Command::GetLoudness => self.handle_get_loudness().await,
            Command::HalStatus => self.handle_hal_status().await,
            Command::Shutdown => {
                *self.running.lock() = false;
                Response::ok_empty()
            }
            // Encryption commands
            Command::SetEncryption { enabled } => self.handle_set_encryption(enabled).await,
            Command::EncryptionStatus => self.handle_encryption_status().await,
            Command::RotateEncryptionKey => self.handle_rotate_encryption_key().await,
            // HAL config commands
            Command::SetSampleRate { rate } => self.handle_set_sample_rate(rate).await,
            Command::SetBufferFrames { frames } => self.handle_set_buffer_frames(frames).await,
            Command::GetHalConfig => self.handle_get_hal_config().await,
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
        let mut manager = self.manager.lock();

        // Clear engine_ready flag so Swift HAL driver stops sending audio
        #[cfg(target_os = "macos")]
        {
            if let Some(buffer) = self.get_shared_buffer() {
                buffer.set_engine_ready(false);
                log::debug!("Cleared engine_ready flag in shared memory");
            }
        }

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
        // Validate that the device exists
        match list_audio_devices() {
            Ok(devices) => {
                let device_exists = devices.iter().any(|d| {
                    d.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| n == device)
                        .unwrap_or(false)
                });

                if !device_exists {
                    return Response::err(format!("Device '{}' not found", device));
                }
            }
            Err(e) => {
                return Response::err(format!("Failed to list devices: {}", e));
            }
        }

        // Store the selected device
        *self.selected_device.lock() = Some(device.to_string());
        log::info!("Output device set to: {}", device);

        Response::ok_empty()
    }

    async fn handle_load_plugins(&self, plugins: Vec<PluginConfig>) -> Response {
        self.handle_load_plugins_with_channels(plugins, 2).await
    }

    async fn handle_load_plugins_with_channels(&self, mut plugins: Vec<PluginConfig>, output_channels: usize) -> Response {
        // Store user's plugin configuration BEFORE adding resampler
        *self.current_plugins.lock() = plugins.clone();
        *self.current_output_channels.lock() = output_channels;

        let mut manager = self.manager.lock();
        let mut output_device = self.selected_device.lock().clone();

        // If no device selected (or it's the virtual device), find a safe physical fallback
        if output_device.is_none() || output_device.as_ref().map(|d| d.contains("SotF")).unwrap_or(false) {
            output_device = find_fallback_output_device();
            if let Some(ref dev) = output_device {
                log::info!("Using fallback output device for HAL playback: {}", dev);
            }
        }

        // Stop current playback if running
        let _ = manager.stop();

        // Note: hal_input/hal_output plugins are no longer required.
        // The decoder thread's HalInputReader is the audio source, and cpal is the output.
        // Any plugins in the chain are purely for processing (EQ, upmixer, etc.)

        // Check HAL sample rate and add resampler if needed
        #[cfg(target_os = "macos")]
        {
            if let Some(buffer) = self.get_shared_buffer() {
                let hal_rate = buffer.sample_rate();
                let target_rate = 48000u32; // Engine's target sample rate

                if hal_rate != 0 && hal_rate != target_rate {
                    log::info!(
                        "HAL sample rate ({}Hz) differs from target ({}Hz), but DecoderThread handles resampling internally.",
                        hal_rate, target_rate
                    );
                }
            }
        }

        log::info!(
            "Loading HAL plugin chain: {} plugins, {} output channels, device: {:?}",
            plugins.len(),
            output_channels,
            output_device
        );

        // Start HAL playback (no file source needed)
        match manager.start_hal_playback(output_device, plugins, output_channels) {
            Ok(_) => {
                log::info!("HAL plugin chain loaded successfully");

                // CRITICAL: Set engine_ready flag so Swift HAL driver starts sending audio
                #[cfg(target_os = "macos")]
                {
                    use std::sync::atomic::Ordering;

                    if let Some(buffer) = self.get_shared_buffer() {
                        // Log the current state before setting
                        log::info!(
                            "[AUDIO FLOW] SharedMemory state BEFORE: driver_ready={}, engine_ready={}, active={}, wpos={}, rpos={}",
                            buffer.driver_ready(),
                            buffer.header().engine_ready.load(Ordering::Acquire) != 0,
                            buffer.is_active(),
                            buffer.header().write_position.load(Ordering::Acquire),
                            buffer.header().read_position.load(Ordering::Acquire)
                        );

                        buffer.set_engine_ready(true);

                        // Log the state after setting
                        log::info!(
                            "[AUDIO FLOW] SharedMemory state AFTER: engine_ready={}",
                            buffer.header().engine_ready.load(Ordering::Acquire) != 0
                        );

                        log::info!("Set engine_ready=true in shared memory");
                    } else {
                        log::error!("[AUDIO FLOW] CRITICAL: Could not open shared memory to set engine_ready flag!");
                        log::error!("[AUDIO FLOW] Expected path: /tmp/sotf-{}/audio.shm", unsafe { libc::getuid() });
                    }
                }

                Response::ok_empty()
            }
            Err(e) => {
                log::error!("Failed to load HAL plugins: {}", e);
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
                "channel_peaks": loudness.channel_peaks,
                "true_peaks_dbtp": loudness.true_peaks_dbtp,
                "correlation_lr": loudness.correlation_lr,
            })),
            None => Response::err("Loudness monitoring not enabled"),
        }
    }

    async fn handle_hal_status(&self) -> Response {
        let status = get_hal_status();
        Response::ok(serde_json::json!({
            "platform_supported": status.platform_supported,
            "buffer_initialized": status.buffer_initialized,
            "driver_installed": status.driver_installed,
            "ready": status.is_ready(),
        }))
    }

    // =========================================================================
    // Encryption handlers
    // =========================================================================

    async fn handle_set_encryption(&self, enabled: bool) -> Response {
        let mut key_manager = self.key_manager.lock();
        key_manager.set_enabled(enabled);

        // Update shared memory encryption flag
        #[cfg(target_os = "macos")]
        {
            if let Some(mut buffer) = self.get_shared_buffer() {
                buffer.set_encrypted(enabled);
                if enabled {
                    buffer.set_key_fingerprint(*key_manager.fingerprint());
                }
                buffer.set_config_changed();
            }
        }

        Response::ok(serde_json::json!({
            "enabled": enabled,
            "fingerprint": key_manager.fingerprint_hex(),
        }))
    }

    async fn handle_encryption_status(&self) -> Response {
        let key_manager = self.key_manager.lock();
        let status = key_manager.status();

        // Also get frame counter from shared memory if available
        #[cfg(target_os = "macos")]
        let frame_count = self.get_shared_buffer()
            .map(|b| b.frame_counter())
            .unwrap_or(0);
        #[cfg(not(target_os = "macos"))]
        let frame_count: u64 = 0;

        Response::ok(serde_json::json!({
            "enabled": status.enabled,
            "fingerprint": status.fingerprint,
            "key_path": status.key_path,
            "frame_count": frame_count,
        }))
    }

    async fn handle_rotate_encryption_key(&self) -> Response {
        let mut key_manager = self.key_manager.lock();

        match key_manager.force_rotate() {
            Ok(()) => {
                // Update shared memory fingerprint if encryption is enabled
                #[cfg(target_os = "macos")]
                {
                    if key_manager.is_enabled() {
                        if let Some(mut buffer) = self.get_shared_buffer() {
                            buffer.set_key_fingerprint(*key_manager.fingerprint());
                            buffer.set_config_changed();
                        }
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
    // HAL config handlers
    // =========================================================================

    /// Set sample rate and notify HAL driver
    async fn handle_set_sample_rate(&self, rate: u32) -> Response {
        #[cfg(target_os = "macos")]
        {
            const SUPPORTED: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

            if !SUPPORTED.contains(&rate) {
                return Response::err(format!(
                    "Unsupported sample rate: {}. Supported: {:?}",
                    rate, SUPPORTED
                ));
            }

            // Reconfigure audio pipeline if needed
            let manager = self.manager.lock();
            let state = manager.get_state();
            drop(manager); // Release lock before potential reconfiguration

            if state != sotf_audio::manager::StreamingState::Idle {
                // Active playback - would need to restart with new rate
                log::warn!("Cannot change sample rate during active playback, will apply on next start");
            }

            // Notify HAL driver via shared memory
            if let Some(mut buffer) = self.get_shared_buffer() {
                let current_frames = buffer.buffer_frames();
                buffer.set_actual_sample_rate(rate);
                buffer.set_actual_buffer_frames(current_frames);
                buffer.set_config_source(2); // Daemon initiated
                buffer.set_config_changed();

                log::info!("Sample rate set to {}Hz, HAL driver notified", rate);

                Response::ok(serde_json::json!({
                    "sample_rate": rate,
                }))
            } else {
                Response::err("Failed to open shared memory")
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = rate;
            Response::err("HAL driver only available on macOS")
        }
    }

    /// Set buffer frames and notify HAL driver
    async fn handle_set_buffer_frames(&self, frames: u32) -> Response {
        #[cfg(target_os = "macos")]
        {
            if frames < 64 || frames > 4096 {
                return Response::err(format!(
                    "Buffer frames must be between 64 and 4096, got: {}",
                    frames
                ));
            }

            // Notify HAL driver via shared memory
            // Note: Only update buffer_frames, preserve the current sample rate
            // to avoid accidentally reverting concurrent sample rate changes
            if let Some(mut buffer) = self.get_shared_buffer() {
                // Read current actual sample rate to preserve it
                let current_rate = buffer.actual_sample_rate();
                let rate_to_use = if current_rate > 0 { current_rate } else { buffer.sample_rate() };

                buffer.set_actual_sample_rate(rate_to_use);
                buffer.set_actual_buffer_frames(frames);
                buffer.set_config_source(2); // Daemon initiated
                buffer.set_config_changed();

                log::info!("Buffer frames set to {}, HAL driver notified (sample rate preserved at {})", frames, rate_to_use);

                Response::ok(serde_json::json!({
                    "buffer_frames": frames,
                }))
            } else {
                Response::err("Failed to open shared memory")
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = frames;
            Response::err("HAL driver only available on macOS")
        }
    }

    /// Get current HAL driver configuration
    async fn handle_get_hal_config(&self) -> Response {
        #[cfg(target_os = "macos")]
        {
            if let Some(buffer) = self.get_shared_buffer() {
                Response::ok(serde_json::json!({
                    "sample_rate": buffer.sample_rate(),
                    "actual_sample_rate": buffer.actual_sample_rate(),
                    "buffer_frames": buffer.buffer_frames(),
                    "actual_buffer_frames": buffer.actual_buffer_frames(),
                    "channel_count": buffer.channel_count(),
                    "active": buffer.is_active(),
                    "driver_ready": buffer.driver_ready(),
                    "config_status": buffer.config_status(),
                    "config_source": buffer.config_source(),
                }))
            } else {
                Response::err("Failed to open shared memory")
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Response::err("HAL driver only available on macOS")
        }
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
                            // Use shared runtime for async operations
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

        // Start HAL config watcher thread (macOS only)
        #[cfg(target_os = "macos")]
        let hal_config_watcher = {
            let manager = Arc::clone(&self.manager);
            let running = Arc::clone(&self.running);
            let current_plugins = Arc::clone(&self.current_plugins);
            let current_output_channels = Arc::clone(&self.current_output_channels);
            spawn_hal_config_watcher(manager, running, current_plugins, current_output_channels)
        };

        // Try to bind the socket, handling TOCTOU race properly
        // Instead of check-then-remove-then-bind, we attempt bind directly
        // and handle AddrInUse by checking if another daemon is actually running
        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // Socket exists - check if another daemon is running
                if let Ok(_stream) = UnixStream::connect(&socket_path) {
                    return Err("Another daemon instance is already running".into());
                }
                // Stale socket - remove it and try again
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

        // Accept connections
        for stream in listener.incoming() {
            if !*self.running.lock() {
                println!("Shutdown requested, exiting");
                break;
            }

            match stream {
                Ok(stream) => {
                    // Verify peer credentials before handling
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
                    // Note: hal_manager uses Arc, so Drop won't shutdown until last Arc drops
                    // Actual shutdown is called explicitly in main()
                    let daemon = AudioDaemon {
                        manager: Arc::clone(&self.manager),
                        running: Arc::clone(&self.running),
                        hal_manager: Arc::clone(&self.hal_manager),
                        selected_device: Arc::clone(&self.selected_device),
                        key_manager: Arc::clone(&self.key_manager),
                        #[cfg(target_os = "macos")]
                        shared_buffer: Arc::clone(&self.shared_buffer),
                        runtime: Arc::clone(&self.runtime),
                        current_plugins: Arc::clone(&self.current_plugins),
                        current_output_channels: Arc::clone(&self.current_output_channels),
                    };

                    // Handle each client in a separate thread
                    std::thread::spawn(move || {
                        daemon.handle_client(stream);
                    });
                }
                Err(e) => {
                    log::error!("Failed to accept connection: {}", e);
                }
            }
        }

        // Cleanup
        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_file(LEGACY_SOCKET_PATH);

        // Wait for HAL config watcher to finish
        #[cfg(target_os = "macos")]
        {
            let _ = hal_config_watcher.join();
        }

        Ok(())
    }
}

// =============================================================================
// HAL Config Watcher (macOS only)
// =============================================================================

/// Supported sample rates for HAL driver
#[cfg(target_os = "macos")]
const SUPPORTED_SAMPLE_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

/// Spawn a background thread that polls shared memory for HAL-initiated config changes
#[cfg(target_os = "macos")]
fn spawn_hal_config_watcher(
    audio_manager: Arc<Mutex<AudioEngineManager>>,
    running: Arc<Mutex<bool>>,
    current_plugins: Arc<Mutex<Vec<PluginConfig>>>,
    current_output_channels: Arc<Mutex<usize>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::time::Duration;

        let poll_interval = Duration::from_millis(50);

        log::info!("HAL config watcher thread started");

        loop {
            if !*running.lock() {
                break;
            }

            // Try to open shared memory
            if let Ok(mut buffer) = driver_hal::SharedAudioBuffer::open_default() {
                // Atomically check for pending config change from HAL
                // Read config_changed first (which has an Acquire barrier), then
                // check source. The order matters for memory visibility.
                let changed = buffer.config_changed();
                if changed {
                    let source = buffer.config_source();
                    // Only handle HAL-initiated changes (source=1)
                    if source == 1 {
                        handle_hal_config_change(
                            &mut buffer,
                            &audio_manager,
                            &current_plugins,
                            &current_output_channels,
                        );
                    }
                }
            }

            std::thread::sleep(poll_interval);
        }

        log::info!("HAL config watcher thread stopped");
    })
}

/// Handle a HAL-initiated config change
///
/// # Thread Safety
/// This function takes `&mut SharedAudioBuffer` because `acknowledge_config_change`
/// writes non-atomic fields. The mutable borrow is safe because:
/// 1. Only the config watcher thread calls this function
/// 2. The SharedAudioBuffer is opened fresh in each iteration, not shared
/// 3. Synchronization with HAL driver is via atomic flags + memory fences
#[cfg(target_os = "macos")]
fn handle_hal_config_change(
    buffer: &mut driver_hal::SharedAudioBuffer,
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    current_plugins: &Arc<Mutex<Vec<PluginConfig>>>,
    current_output_channels: &Arc<Mutex<usize>>,
) {
    let requested_rate = buffer.requested_sample_rate();
    let requested_frames = buffer.requested_buffer_frames();

    log::info!(
        "HAL config change request: sample_rate={}, buffer_frames={}",
        requested_rate,
        requested_frames
    );

    // Validate requested values
    if requested_rate == 0 {
        log::warn!("Invalid config request: sample_rate=0, ignoring");
        buffer.acknowledge_config_change(48000, requested_frames, 3, 2); // Error code 2 = invalid rate
        return;
    }
    if requested_frames == 0 || requested_frames > 65536 {
        log::warn!("Invalid config request: buffer_frames={}, out of range", requested_frames);
        buffer.acknowledge_config_change(requested_rate, 512, 3, 3); // Error code 3 = invalid frames
        return;
    }

    // Check if we support this sample rate
    if SUPPORTED_SAMPLE_RATES.contains(&requested_rate) {
        // Accept the config and reconfigure pipeline
        match reconfigure_audio_pipeline(
            audio_manager,
            requested_rate,
            requested_frames,
            current_plugins,
            current_output_channels,
        ) {
            Ok(()) => {
                // Set engine_ready so HAL driver continues sending audio
                buffer.set_engine_ready(true);
                buffer.acknowledge_config_change(requested_rate, requested_frames, 1, 0);
                log::info!(
                    "Config accepted: {}Hz, {} frames, engine_ready=true",
                    requested_rate,
                    requested_frames
                );
            }
            Err(e) => {
                log::error!("Pipeline reconfiguration failed: {}", e);
                buffer.acknowledge_config_change(requested_rate, requested_frames, 3, 1);
            }
        }
    } else {
        // Negotiate to closest supported rate
        let actual_rate = SUPPORTED_SAMPLE_RATES
            .iter()
            .min_by_key(|&&r| (r as i32 - requested_rate as i32).abs())
            .copied()
            .unwrap_or(48000);

        log::info!(
            "Config negotiated: requested {}Hz, using {}Hz",
            requested_rate,
            actual_rate
        );

        match reconfigure_audio_pipeline(
            audio_manager,
            actual_rate,
            requested_frames,
            current_plugins,
            current_output_channels,
        ) {
            Ok(()) => {
                // Set engine_ready so HAL driver continues sending audio
                buffer.set_engine_ready(true);
                buffer.acknowledge_config_change(actual_rate, requested_frames, 2, 0);
                log::info!(
                    "Config negotiated: {}Hz, {} frames, engine_ready=true",
                    actual_rate,
                    requested_frames
                );
            }
            Err(e) => {
                log::error!("Pipeline reconfiguration with negotiated rate failed: {}", e);
                buffer.acknowledge_config_change(actual_rate, requested_frames, 3, 1);
            }
        }
    }
}

/// Reconfigure the audio pipeline with new sample rate and buffer size
///
/// # Arguments
/// * `audio_manager` - The audio engine manager
/// * `hal_sample_rate` - The HAL's sample rate in Hz (what apps are sending)
/// * `buffer_frames` - New buffer size in frames (currently unused, reserved for future use)
/// * `current_plugins` - The stored user plugin configuration
/// * `current_output_channels` - The stored output channel count
///
/// This function restarts the HAL playback pipeline, adding a resampler plugin
/// if the HAL sample rate differs from the engine's target rate (48kHz).
/// It preserves the user's plugin configuration (EQ, upmixer, etc.) across restarts.
#[cfg(target_os = "macos")]
fn reconfigure_audio_pipeline(
    audio_manager: &Arc<Mutex<AudioEngineManager>>,
    hal_sample_rate: u32,
    #[allow(unused_variables)]
    buffer_frames: u32,
    current_plugins: &Arc<Mutex<Vec<PluginConfig>>>,
    current_output_channels: &Arc<Mutex<usize>>,
) -> Result<(), String> {
    let mut manager = audio_manager.lock();

    // Check if we have an active HAL playback session
    let state = manager.get_state();
    if state == sotf_audio::manager::StreamingState::Idle {
        // No active playback - just acknowledge, HAL driver will update its state
        log::debug!("No active playback, acknowledging config change");
        return Ok(());
    }

    // For HAL playback, we need to restart with the new sample rate
    log::info!(
        "Reconfiguring HAL playback: HAL rate={}Hz, target=48000Hz",
        hal_sample_rate
    );

    // Stop current playback
    if let Err(e) = manager.stop() {
        log::warn!("Failed to stop current playback: {}", e);
    }

    // Get stored user plugins (clone to avoid holding lock)
    let user_plugins = current_plugins.lock().clone();
    let output_channels = *current_output_channels.lock();

    // Build plugin chain: resampler (if needed) + user plugins
    let target_rate = 48000u32;
    let mut plugins = user_plugins;

    if hal_sample_rate != 0 && hal_sample_rate != target_rate {
        log::info!(
            "HAL sample rate ({}Hz) differs from target ({}Hz), DecoderThread will handle resampling.",
            hal_sample_rate,
            target_rate
        );
    }

    // Find output device
    let output_device = find_fallback_output_device();
    log::info!(
        "Restarting HAL playback with {} plugins, {} output channels, device: {:?}",
        plugins.len(),
        output_channels,
        output_device
    );

    // Restart HAL playback with preserved output channel count
    match manager.start_hal_playback(output_device, plugins, output_channels) {
        Ok(_) => {
            log::info!("HAL playback restarted successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to restart HAL playback: {}", e);
            Err(format!("Failed to restart HAL playback: {}", e))
        }
    }
}

fn find_fallback_output_device() -> Option<String> {
    if let Ok(devices) = list_audio_devices() {
        // Filter out virtual devices
        let physical_device = devices.iter().find(|d| {
            let name = d.get("name").and_then(|n| n.as_str()).unwrap_or("");
            !name.contains("SotF") && 
            !name.contains("BlackHole") && 
            !name.contains("ZoomAudio") && 
            !name.contains("Loopback")
        });

        if let Some(device) = physical_device {
            return device.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
        }
    }
    None
}

fn list_audio_devices() -> Result<Vec<serde_json::Value>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let mut devices = Vec::new();

    // Get default output device
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

    // Get all output devices
    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            let name = device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string());

            // Skip if already added as default
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

            // Get device config if available
            if let Ok(config) = device.default_output_config() {
                device_info["channels"] = config.channels().into();
                device_info["sample_rate"] = config.sample_rate().into();
            }

            devices.push(device_info);
        }
    }

    if devices.is_empty() {
        Err("No audio devices found".to_string())
    } else {
        Ok(devices)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger from RUST_LOG environment variable
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // Setup signal handling for graceful shutdown
    let running = Arc::new(Mutex::new(true));
    let r = Arc::clone(&running);

    ctrlc::set_handler(move || {
        println!("\nReceived interrupt signal, shutting down...");
        *r.lock() = false;
    })?;

    println!("===============================================================================");
    println!("🎵 AutoEQ Audio Control Daemon");
    println!("===============================================================================");

    // Initialize HAL driver
    println!();
    let daemon = AudioDaemon::new();

    {
        let mut hal = daemon.hal_manager.lock();
        match hal.initialize() {
            Ok(_) => {
                let status = get_hal_status();
                println!("📊 HAL Status:");
                println!(
                    "   Platform supported: {}",
                    if status.platform_supported {
                        "✅ Yes"
                    } else {
                        "❌ No"
                    }
                );
                println!(
                    "   Buffer initialized: {}",
                    if status.buffer_initialized {
                        "✅ Yes"
                    } else {
                        "❌ No"
                    }
                );
                println!(
                    "   Driver installed:   {}",
                    if status.driver_installed {
                        "✅ Yes"
                    } else {
                        "⚠️  No (optional)"
                    }
                );
                println!(
                    "   Ready to use:       {}",
                    if status.is_ready() {
                        "✅ Yes"
                    } else {
                        "❌ No"
                    }
                );

                if status.is_ready() {
                    println!();
                    println!("💡 Audio flow (capture mode):");
                    println!("   macOS Apps → HAL Driver → SharedMemory → Daemon → cpal → Hardware");
                    println!();
                    println!("   Note: hal_output plugin is NOT needed for direct hardware output.");
                    println!("         The daemon reads from SharedMemory and outputs via cpal.");
                }
            }
            Err(e) => {
                log::warn!("Failed to initialize HAL: {}", e);
                log::warn!("HAL plugins will not be available");
            }
        }
    }

    // Show encryption status
    {
        let key_manager = daemon.key_manager.lock();
        let status = key_manager.status();
        println!();
        println!("🔐 Encryption Status:");
        println!(
            "   Enabled:     {}",
            if status.enabled {
                "✅ Yes"
            } else {
                "❌ No (use set_encryption to enable)"
            }
        );
        println!("   Fingerprint: {}", status.fingerprint);
        println!("   Key path:    {}", status.key_path);
    }

    println!();
    println!("===============================================================================");
    println!("🚀 Starting daemon...");
    println!("===============================================================================");

    // Auto-start HAL playback with empty plugin chain
    // Audio flow: HAL input (via decoder thread's HalInputReader) → processing → cpal output
    // This ensures audio flows immediately without waiting for toolbar configuration
    {
        println!("▶️  Auto-starting HAL playback (2ch)...");

        // Check if SharedMemory file exists
        let uid = unsafe { libc::getuid() };
        let shm_path = format!("/tmp/sotf-{}/audio.shm", uid);
        let shm_exists = std::path::Path::new(&shm_path).exists();
        println!("   SharedMemory path: {}", shm_path);
        println!("   SharedMemory exists: {}", shm_exists);

        if !shm_exists {
            println!("   ⚠️  WARNING: SharedMemory file does not exist!");
            println!("   ⚠️  The HAL driver may not be installed or hasn't started IO yet.");
            println!("   ⚠️  Try: sudo launchctl kickstart -k system/com.apple.audio.coreaudiod");
        }

        // Find a physical output device (not the HAL virtual device)
        let output_device = find_fallback_output_device();
        println!("   Output device: {:?}", output_device);

        if output_device.is_none() {
            println!("   ⚠️  WARNING: No physical output device found!");
        }

        // Empty plugin chain - decoder thread reads from HAL, cpal outputs to hardware
        let plugins: Vec<PluginConfig> = vec![];

        // Use the daemon's shared runtime instead of creating a new one
        let result = daemon.runtime.block_on(daemon.handle_load_plugins_with_channels(plugins, 2));
        if result.success {
            println!("   ✅ HAL playback started successfully");
        } else {
            println!("   ❌ HAL playback failed: {:?}", result.error);
        }
    }

    daemon.run()?;

    // Explicit HAL cleanup (not in Drop to avoid issues with Arc cloning)
    {
        let mut hal = daemon.hal_manager.lock();
        hal.shutdown();
    }

    println!();
    println!("✅ Daemon stopped cleanly");
    Ok(())
}
