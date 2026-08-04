use sotf_audio::decoder::AudioSource;
use sotf_audio::engine::{AudioEngineState, PluginConfig, PluginGraphConfig};
use sotf_audio_player::{LoudnessData, PlaybackState, Player, SignalPath, SpectrumData};
use sotf_plugins::CompressorData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// The engine reads needed by one UI refresh.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerSnapshotRequest {
    pub input_monitor_idx: Option<usize>,
    pub output_monitor_idx: Option<usize>,
    pub spectrum_idx: Option<usize>,
    pub compressor_idx: Option<usize>,
    pub include_external_diagnostics: bool,
}

/// Immutable state published by the player actor.
///
/// The snapshot is built on the actor thread. Consumers only ever clone its
/// `Arc`, so the UI never locks or calls into `Player`/the audio engine.
#[derive(Debug)]
pub struct PlayerSnapshot {
    pub sequence: u64,
    pub position_secs: f64,
    pub is_playing: bool,
    pub sample_rate: Option<u32>,
    pub signal_path: SignalPath,
    pub input_loudness_info: Option<Arc<LoudnessData>>,
    pub loudness_info: Option<Arc<LoudnessData>>,
    pub spectrum_info: Option<Arc<SpectrumData>>,
    pub compressor_info: Option<Arc<CompressorData>>,
    pub external_engine_state: Option<AudioEngineState>,
}

/// A snapshot plus events consumed exactly once by the UI.
pub struct PlayerSnapshotRead {
    pub snapshot: Arc<PlayerSnapshot>,
    pub playback_state: PlaybackState,
}

#[derive(Default)]
struct PendingPlaybackEvents {
    last_error: Option<String>,
    engine_restarted: bool,
    engine_fatal: bool,
    track_ended: bool,
    gapless_transition: Option<AudioSource>,
    stream_metadata: Option<sotf_audio::engine::StreamMetadata>,
}

impl PendingPlaybackEvents {
    fn merge(&mut self, playback_state: PlaybackState) {
        if playback_state.last_error.is_some() {
            self.last_error = playback_state.last_error;
        }
        self.engine_restarted |= playback_state.engine_restarted;
        self.engine_fatal |= playback_state.engine_fatal;
        self.track_ended |= playback_state.track_ended;
        if playback_state.gapless_transition.is_some() {
            self.gapless_transition = playback_state.gapless_transition;
        }
        if playback_state.stream_metadata.is_some() {
            self.stream_metadata = playback_state.stream_metadata;
        }
    }

    fn take(&mut self, snapshot: &PlayerSnapshot) -> PlaybackState {
        PlaybackState {
            position_secs: snapshot.position_secs,
            is_playing: snapshot.is_playing,
            sample_rate: snapshot.sample_rate,
            last_error: self.last_error.take(),
            engine_restarted: std::mem::take(&mut self.engine_restarted),
            engine_fatal: std::mem::take(&mut self.engine_fatal),
            track_ended: std::mem::take(&mut self.track_ended),
            gapless_transition: self.gapless_transition.take(),
            stream_metadata: self.stream_metadata.take(),
        }
    }
}

#[derive(Clone)]
pub struct PlayerHandle {
    sender: mpsc::Sender<PlayerCommand>,
    failures: Arc<parking_lot::Mutex<Vec<PlayerCommandFailure>>>,
    latest_snapshot: Arc<parking_lot::RwLock<Option<Arc<PlayerSnapshot>>>>,
    pending_events: Arc<parking_lot::Mutex<PendingPlaybackEvents>>,
    snapshot_requested: Arc<AtomicBool>,
}

impl PlayerHandle {
    pub fn new(player: Player) -> Self {
        let (sender, receiver) = mpsc::channel::<PlayerCommand>();

        if let Err(error) = std::thread::Builder::new()
            .name("sotf-gpui-player-command".to_string())
            .spawn(move || {
                let mut player = player;
                while let Ok(command) = receiver.recv() {
                    command(&mut player);
                }
            })
        {
            log::error!("Failed to spawn player command worker: {error}");
        }

        Self {
            sender,
            failures: Arc::new(parking_lot::Mutex::new(Vec::new())),
            latest_snapshot: Arc::new(parking_lot::RwLock::new(None)),
            pending_events: Arc::new(parking_lot::Mutex::new(PendingPlaybackEvents::default())),
            snapshot_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Queue a coalesced actor request to refresh the immutable UI snapshot.
    ///
    /// At most one snapshot request can be queued at a time. This prevents a
    /// slow engine call from allowing the 60 Hz UI timer to grow an unbounded
    /// backlog behind it.
    pub fn request_snapshot(
        &self,
        request: PlayerSnapshotRequest,
    ) -> Result<(), PlayerCommandError> {
        if self.snapshot_requested.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let latest_snapshot = Arc::clone(&self.latest_snapshot);
        let pending_events = Arc::clone(&self.pending_events);
        let snapshot_requested = Arc::clone(&self.snapshot_requested);
        let result = self.sender.send(Box::new(move |player| {
            let external_engine_state = request
                .include_external_diagnostics
                .then(|| player.get_engine_state());
            let playback_state = player.get_playback_state();
            let position_secs = playback_state.position_secs;
            let is_playing = playback_state.is_playing;
            let sample_rate = playback_state.sample_rate;
            pending_events.lock().merge(playback_state);

            let snapshot = PlayerSnapshot {
                sequence: latest_snapshot
                    .read()
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.sequence.wrapping_add(1)),
                position_secs,
                is_playing,
                sample_rate,
                signal_path: player.signal_path(),
                input_loudness_info: request
                    .input_monitor_idx
                    .and_then(|idx| player.get_cached_plugin_data(idx))
                    .and_then(|data| Arc::downcast::<LoudnessData>(data).ok()),
                loudness_info: request
                    .output_monitor_idx
                    .and_then(|idx| player.get_cached_plugin_data(idx))
                    .and_then(|data| Arc::downcast::<LoudnessData>(data).ok()),
                spectrum_info: request
                    .spectrum_idx
                    .and_then(|idx| player.get_cached_plugin_data(idx))
                    .and_then(|data| Arc::downcast::<SpectrumData>(data).ok()),
                compressor_info: request
                    .compressor_idx
                    .and_then(|idx| player.get_cached_plugin_data(idx))
                    .and_then(|data| Arc::downcast::<CompressorData>(data).ok()),
                external_engine_state,
            };
            *latest_snapshot.write() = Some(Arc::new(snapshot));
            snapshot_requested.store(false, Ordering::Release);
        }));

        if result.is_err() {
            self.snapshot_requested.store(false, Ordering::Release);
        }
        result.map_err(|_| PlayerCommandError {
            label: "request_snapshot",
        })
    }

    /// Read the latest actor-published state without touching the player.
    pub fn read_snapshot(&self) -> Option<PlayerSnapshotRead> {
        let snapshot = self.latest_snapshot.read().clone()?;
        let playback_state = self.pending_events.lock().take(&snapshot);
        Some(PlayerSnapshotRead {
            snapshot,
            playback_state,
        })
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

    pub fn toggle_playback(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_result("toggle_playback", |player| {
            if player.is_playing() {
                player.pause()
            } else {
                player.resume()
            }
        })
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
