//! Audio Engine Control Daemon
//!
//! A Unix socket daemon that provides IPC control for the AudioStreamingManager.
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
use hal_manager::{HalManager, get_hal_status};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sotf_audio::PluginConfig;
use sotf_audio::manager::AudioStreamingManager;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::time::Duration;

const SOCKET_PATH: &str = "/tmp/autoeq_audio.sock";
const IDLE_TIMEOUT_SECS: u64 = 3;

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
    manager: Arc<Mutex<AudioStreamingManager>>,
    last_activity: Arc<Mutex<std::time::Instant>>,
    running: Arc<Mutex<bool>>,
    hal_manager: Arc<Mutex<HalManager>>,
}

impl AudioDaemon {
    fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(AudioStreamingManager::new())),
            last_activity: Arc::new(Mutex::new(std::time::Instant::now())),
            running: Arc::new(Mutex::new(true)),
            hal_manager: Arc::new(Mutex::new(HalManager::new())),
        }
    }

    async fn handle_command(&self, cmd: Command) -> Response {
        // Update activity timestamp
        *self.last_activity.lock() = std::time::Instant::now();

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
        match manager.start_playback(None, vec![], 2) {
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

    async fn handle_set_device(&self, _device: &str) -> Response {
        // Device selection is not exposed in current AudioStreamingManager API
        // Would need to be implemented via cpal device enumeration + selection
        Response::err("Device selection not yet implemented in streaming manager")
    }

    async fn handle_load_plugins(&self, plugins: Vec<PluginConfig>) -> Response {
        let mut manager = self.manager.lock();

        // Stop current playback if running
        let _ = manager.stop();

        // Extract output channel count from hal_output plugin
        let output_channels = plugins
            .iter()
            .find(|p| p.plugin_type == "hal_output")
            .and_then(|p| p.parameters.get("channels"))
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize;

        log::info!(
            "Loading HAL plugin chain: {} plugins, {} output channels",
            plugins.len(),
            output_channels
        );

        // Start HAL playback (no file source needed)
        match manager.start_hal_playback(None, plugins, output_channels) {
            Ok(_) => {
                log::info!("HAL plugin chain loaded successfully");
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
                "peak": loudness.peak,
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

    fn handle_client(&self, mut stream: UnixStream) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
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
                            // Use tokio runtime for async operations
                            tokio::runtime::Runtime::new()
                                .unwrap()
                                .block_on(self.handle_command(cmd))
                        }
                        Err(e) => Response::err(format!("Invalid command: {}", e)),
                    };

                    let json = serde_json::to_string(&response).unwrap();
                    if let Err(e) = writeln!(stream, "{}", json) {
                        eprintln!("Failed to write response: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read from client: {}", e);
                    break;
                }
            }
        }
    }

    fn monitor_idle(&self) {
        let last_activity = Arc::clone(&self.last_activity);
        let manager = Arc::clone(&self.manager);
        let running = Arc::clone(&self.running);

        std::thread::spawn(move || {
            while *running.lock() {
                std::thread::sleep(Duration::from_secs(1));

                let elapsed = last_activity.lock().elapsed();
                if elapsed > Duration::from_secs(IDLE_TIMEOUT_SECS) {
                    let mgr = manager.lock();
                    let state = mgr.get_state();

                    // Only stop if not playing
                    if matches!(
                        state,
                        sotf_audio::manager::StreamingState::Idle
                            | sotf_audio::manager::StreamingState::Ready
                    ) {
                        println!("Idle timeout reached, audio engine in low-power mode");
                        // Engine is already stopped, nothing to do
                    }
                }
            }
        });
    }

    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove stale socket if exists
        let _ = std::fs::remove_file(SOCKET_PATH);

        let listener = UnixListener::bind(SOCKET_PATH)?;
        println!("Audio daemon listening on {}", SOCKET_PATH);

        // Start idle monitor thread
        self.monitor_idle();

        // Accept connections
        for stream in listener.incoming() {
            if !*self.running.lock() {
                println!("Shutdown requested, exiting");
                break;
            }

            match stream {
                Ok(stream) => {
                    // Clone daemon for client thread
                    // Note: hal_manager uses Arc, so Drop won't shutdown until last Arc drops
                    // Actual shutdown is called explicitly in main()
                    let daemon = AudioDaemon {
                        manager: Arc::clone(&self.manager),
                        last_activity: Arc::clone(&self.last_activity),
                        running: Arc::clone(&self.running),
                        hal_manager: Arc::clone(&self.hal_manager),
                    };

                    // Handle each client in a separate thread
                    std::thread::spawn(move || {
                        daemon.handle_client(stream);
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }

        // Cleanup
        let _ = std::fs::remove_file(SOCKET_PATH);
        Ok(())
    }
}

fn list_audio_devices() -> Result<Vec<serde_json::Value>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let mut devices = Vec::new();

    // Get default output device
    if let Some(default_out) = host.default_output_device() {
        let name = default_out.name().unwrap_or_else(|_| "Unknown".to_string());
        devices.push(serde_json::json!({
            "name": name,
            "is_default": true,
        }));
    }

    // Get all output devices
    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());

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
                device_info["sample_rate"] = config.sample_rate().0.into();
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
        match hal.initialize_default() {
            Ok(_) => {
                let status = get_hal_status();
                println!("📊 HAL Status:");
                println!("   Platform supported: {}", if status.platform_supported { "✅ Yes" } else { "❌ No" });
                println!("   Buffer initialized: {}", if status.buffer_initialized { "✅ Yes" } else { "❌ No" });
                println!("   Driver installed:   {}", if status.driver_installed { "✅ Yes" } else { "⚠️  No (optional)" });
                println!("   Ready to use:       {}", if status.is_ready() { "✅ Yes" } else { "❌ No" });

                if status.is_ready() {
                    println!();
                    println!("💡 HAL plugins available:");
                    println!("   - hal_input:  Read audio from macOS apps");
                    println!("   - hal_output: Write processed audio back (loopback)");
                }
            }
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to initialize HAL: {}", e);
                eprintln!("   HAL plugins will not be available");
            }
        }
    }

    println!();
    println!("===============================================================================");
    println!("🚀 Starting daemon...");
    println!("===============================================================================");

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
