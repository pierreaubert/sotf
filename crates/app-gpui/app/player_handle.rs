use sotf_audio::decoder::AudioSource;
use sotf_audio::engine::{PluginConfig, PluginGraphConfig};
use sotf_audio_player::Player;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

type PlayerCommand = Box<dyn FnOnce(&mut Player) + Send + 'static>;

#[derive(Debug, Clone)]
pub struct PlayerCommandFailure {
    pub label: &'static str,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct PlayerCommandError {
    label: &'static str,
}

impl std::fmt::Display for PlayerCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "player command worker is not available for {}",
            self.label
        )
    }
}

impl std::error::Error for PlayerCommandError {}

#[derive(Clone)]
pub struct PlayerHandle {
    player: Arc<parking_lot::Mutex<Player>>,
    sender: mpsc::Sender<PlayerCommand>,
    failures: Arc<parking_lot::Mutex<Vec<PlayerCommandFailure>>>,
}

impl PlayerHandle {
    pub fn new(player: Player) -> Self {
        let player = Arc::new(parking_lot::Mutex::new(player));
        let (sender, receiver) = mpsc::channel::<PlayerCommand>();
        let worker_player = Arc::clone(&player);

        if let Err(error) = std::thread::Builder::new()
            .name("sotf-gpui-player-command".to_string())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    let mut player = worker_player.lock();
                    command(&mut player);
                }
            })
        {
            log::error!("Failed to spawn player command worker: {error}");
        }

        Self {
            player,
            sender,
            failures: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    pub fn try_with_player<R>(&self, f: impl FnOnce(&mut Player) -> R) -> Option<R> {
        self.player.try_lock().map(|mut player| f(&mut player))
    }

    pub fn with_player_blocking<R>(&self, f: impl FnOnce(&mut Player) -> R) -> R {
        let mut player = self.player.lock();
        f(&mut player)
    }

    pub fn drain_failures(&self) -> Vec<PlayerCommandFailure> {
        self.failures.lock().drain(..).collect()
    }

    fn enqueue_result<F>(&self, label: &'static str, f: F) -> Result<(), PlayerCommandError>
    where
        F: FnOnce(&mut Player) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
    {
        let failures = Arc::clone(&self.failures);
        self.sender
            .send(Box::new(move |player| {
                if let Err(error) = f(player) {
                    let error = error.to_string();
                    log::warn!("Player {label} failed: {error}");
                    failures.lock().push(PlayerCommandFailure { label, error });
                }
            }))
            .map_err(|_| PlayerCommandError { label })
    }

    pub fn pause(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_result("pause", |player| player.pause())
    }

    pub fn resume(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_result("resume", |player| player.resume())
    }

    pub fn stop(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_result("stop", |player| player.stop())
    }

    pub fn seek(&self, position_secs: f64) -> Result<(), PlayerCommandError> {
        self.enqueue_result("seek", move |player| player.seek(position_secs))
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_volume", move |player| player.set_volume(volume))
    }

    pub fn set_mute(&self, muted: bool) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_mute", move |player| player.set_mute(muted))
    }

    pub fn cancel_next(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_result("cancel_next", |player| player.cancel_next())
    }

    pub fn queue_next(&self, path: PathBuf) -> Result<(), PlayerCommandError> {
        self.enqueue_result("queue_next", move |player| player.queue_next(path))
    }

    pub fn set_output_device(&self, device_name: String) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_output_device", move |player| {
            player.set_output_device(device_name)
        })
    }

    pub fn update_plugins(&self, plugins: Vec<PluginConfig>) -> Result<(), PlayerCommandError> {
        self.enqueue_result("update_plugins", move |player| {
            player.update_plugins(plugins)
        })
    }

    pub fn update_plugin_graph(
        &self,
        graph_config: PluginGraphConfig,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("update_plugin_graph", move |player| {
            player.update_plugin_graph(graph_config)
        })
    }

    pub fn set_plugin_parameter(
        &self,
        engine_index: usize,
        param_id: String,
        value: String,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_plugin_parameter", move |player| {
            player.set_plugin_parameter(engine_index, param_id, value)
        })
    }

    pub fn load_and_play_source(
        &self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("load_and_play_source", move |player| {
            player.load_and_play_source(source, plugins, output_channels, output_device)
        })
    }

    pub fn load_or_switch_source_at(
        &self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        position: Option<f64>,
        prefer_smooth_switch: bool,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("load_or_switch_source_at", move |player| {
            if prefer_smooth_switch && position.is_none() {
                match player.switch_to_source_at(
                    source.clone(),
                    plugins.clone(),
                    output_channels,
                    output_device.clone(),
                    position,
                ) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        log::warn!(
                            "[GPUI] Smooth track switch unavailable, falling back to restart: {}",
                            error
                        );
                        player.load_and_play_source_at(
                            source,
                            plugins,
                            output_channels,
                            output_device,
                            position,
                        )
                    }
                }
            } else {
                player.load_and_play_source_at(
                    source,
                    plugins,
                    output_channels,
                    output_device,
                    position,
                )
            }
        })
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub fn start_hal_playback_with_config(
        &self,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        sample_rate: u32,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("start_hal_playback_with_config", move |player| {
            player.start_hal_playback_with_config(
                plugins,
                output_channels,
                output_device,
                sample_rate,
            )
        })
    }
}
