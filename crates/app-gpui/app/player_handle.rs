use sotf_audio::decoder::AudioSource;
use sotf_audio::engine::{AudioEngineState, PluginConfig, PluginGraphConfig};
use sotf_audio::manager::StreamingState;
use sotf_audio_player::{LoudnessData, PlaybackState, Player, SignalPath, SpectrumData};
use sotf_plugins::CompressorData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

type PlayerCommand = Box<dyn FnOnce(&mut Player) + Send + 'static>;

#[derive(Debug, Clone)]
pub struct PlayerCommandFailure {
    pub label: &'static str,
    pub error: String,
}

/// Completion of one specific actor command, independent of later commands.
/// `None` means queued or executing; `Some` means the Player operation returned.
#[derive(Debug, Clone)]
pub struct PlayerCommandReceipt {
    result: Arc<parking_lot::Mutex<Option<Result<(), PlayerCommandFailure>>>>,
}

impl PlayerCommandReceipt {
    /// Synthetic completion for isolated UI rejection-path verification.
    #[cfg(feature = "dev-api")]
    pub(crate) fn qa_rejected() -> Self {
        Self {
            result: Arc::new(parking_lot::Mutex::new(Some(Err(PlayerCommandFailure {
                label: "QA plugin update",
                error: "Synthetic plugin update rejection".into(),
            })))),
        }
    }

    /// Identity of one accepted actor command, independent of its result.
    #[must_use]
    pub fn is_same_command(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.result, &other.result)
    }

    #[must_use]
    pub fn poll(&self) -> Option<Result<(), PlayerCommandFailure>> {
        self.result.lock().clone()
    }
}

/// Compatibility name for callers tracking transport operations.
pub type TransportReceipt = PlayerCommandReceipt;

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
    /// Currently visible rack plugin, used by generic live visualizations.
    pub rack_plugin_idx: Option<usize>,
    /// Stable node identity and UI/engine revision at the time of the request.
    pub rack_plugin_identity: Option<(usize, u64, u64)>,
    pub include_external_diagnostics: bool,
}

/// Immutable state published by the player actor.
///
/// The snapshot is built on the actor thread. Consumers only ever clone its
/// `Arc`, so the UI never locks or calls into `Player`/the audio engine.
pub struct PlayerSnapshot {
    pub sequence: u64,
    transport_epoch: u64,
    pub position_secs: f64,
    pub is_playing: bool,
    pub streaming_state: StreamingState,
    pub sample_rate: Option<u32>,
    pub signal_path: SignalPath,
    pub input_loudness_info: Option<Arc<LoudnessData>>,
    pub loudness_info: Option<Arc<LoudnessData>>,
    pub spectrum_info: Option<Arc<SpectrumData>>,
    pub compressor_info: Option<Arc<CompressorData>>,
    pub rack_plugin_data: Option<(usize, Arc<dyn std::any::Any + Send + Sync>)>,
    pub rack_plugin_identity: Option<(usize, u64, u64)>,
    pub external_engine_state: Option<AudioEngineState>,
}

/// A snapshot plus events consumed exactly once by the UI.
#[derive(Debug)]
pub struct PlayerSnapshotRead {
    pub snapshot: Arc<PlayerSnapshot>,
    pub playback_state: PlaybackState,
}

/// Reconcile engine-owned profile changes only from the current rack request.
/// A queued command or graph replacement invalidates older actor snapshots.
#[must_use]
pub fn profile_request_matches(
    snapshot_identity: Option<(usize, u64, u64)>,
    current_identity: Option<(usize, u64, u64)>,
    updates_idle: bool,
) -> bool {
    updates_idle && current_identity.is_some() && snapshot_identity == current_identity
}

#[derive(Default)]
struct PendingPlaybackEvents {
    transport_epoch: u64,
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
            streaming_state: snapshot.streaming_state,
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
    transport_epoch: Arc<TransportSnapshotEpoch>,
    last_transport: Arc<parking_lot::Mutex<Option<TransportReceipt>>>,
}

/// Snapshots are usable only after every accepted transport command has run.
/// Counting completions (rather than storing the last command ID) also covers
/// concurrent senders whose channel insertion order differs from ID allocation.
#[derive(Default)]
struct TransportSnapshotEpoch {
    requested: AtomicU64,
    completed: AtomicU64,
}

impl TransportSnapshotEpoch {
    fn accepts(&self, epoch: u64) -> bool {
        epoch == self.requested.load(Ordering::Acquire)
    }
}

impl PlayerHandle {
    #[cfg(feature = "dev-api")]
    #[must_use]
    pub fn qa_transport_epoch(&self) -> u64 {
        self.transport_epoch.requested.load(Ordering::Acquire)
    }

    #[cfg(feature = "dev-api")]
    pub fn qa_disconnect_worker_for_test(&mut self) {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        self.sender = sender;
    }
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
            transport_epoch: Arc::new(TransportSnapshotEpoch::default()),
            last_transport: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    /// Queue a coalesced actor request to refresh the immutable UI snapshot.
    ///
    /// At most one snapshot request can be queued at a time. This prevents a
    /// slow engine call from allowing the 60 Hz UI timer to grow an unbounded
    /// backlog behind it.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
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
        let transport_epoch = Arc::clone(&self.transport_epoch);
        let result = self.sender.send(Box::new(move |player| {
            let external_engine_state = request
                .include_external_diagnostics
                .then(|| player.get_engine_state());
            let playback_state = player.get_playback_state();
            let position_secs = playback_state.position_secs;
            let is_playing = playback_state.is_playing;
            let streaming_state = playback_state.streaming_state;
            let sample_rate = playback_state.sample_rate;
            let epoch = transport_epoch.completed.load(Ordering::Acquire);
            {
                let mut events = pending_events.lock();
                if events.transport_epoch != epoch {
                    *events = PendingPlaybackEvents {
                        transport_epoch: epoch,
                        ..Default::default()
                    };
                }
                events.merge(playback_state);
            };

            let snapshot = PlayerSnapshot {
                transport_epoch: epoch,
                sequence: latest_snapshot
                    .read()
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.sequence.wrapping_add(1)),
                position_secs,
                is_playing,
                streaming_state,
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
                rack_plugin_data: request
                    .rack_plugin_idx
                    .and_then(|idx| player.get_cached_plugin_data(idx).map(|data| (idx, data))),
                rack_plugin_identity: request.rack_plugin_identity,
                external_engine_state,
            };
            *latest_snapshot.write() = Some(Arc::new(snapshot));
            snapshot_requested.store(false, Ordering::Release);
        }));

        if result.is_err() {
            self.snapshot_requested.store(false, Ordering::Release);
        }
        result.map_err(|_error| PlayerCommandError {
            label: "request_snapshot",
        })
    }

    /// Read the latest actor-published state without touching the player.
    #[must_use]
    pub fn read_snapshot(&self) -> Option<PlayerSnapshotRead> {
        let snapshot = self.latest_snapshot.read().clone()?;
        if !self.transport_epoch.accepts(snapshot.transport_epoch) {
            return None;
        }
        let mut events = self.pending_events.lock();
        if events.transport_epoch != snapshot.transport_epoch {
            return None;
        }
        let playback_state = events.take(&snapshot);
        Some(PlayerSnapshotRead {
            snapshot,
            playback_state,
        })
    }

    #[must_use]
    pub fn drain_failures(&self) -> Vec<PlayerCommandFailure> {
        self.failures.lock().drain(..).collect()
    }

    /// QA inspection must not drain the events owned by the UI tick.
    #[cfg(feature = "dev-api")]
    pub(crate) fn transport_diagnostics(&self) -> serde_json::Value {
        let snapshot = self.latest_snapshot.read();
        serde_json::json!({
            "requested_epoch": self.transport_epoch.requested.load(Ordering::Acquire),
            "completed_epoch": self.transport_epoch.completed.load(Ordering::Acquire),
            "snapshot_pending": self.snapshot_requested.load(Ordering::Acquire),
            "sequence": snapshot.as_ref().map(|snapshot| snapshot.sequence),
            "snapshot_epoch": snapshot.as_ref().map(|snapshot| snapshot.transport_epoch),
            "position_secs": snapshot.as_ref().map(|snapshot| snapshot.position_secs),
            "is_playing": snapshot.as_ref().map(|snapshot| snapshot.is_playing),
        })
    }

    /// QA-only view of the most recently published engine snapshot.
    ///
    /// This deliberately reads the actor-published immutable snapshot instead
    /// of requesting engine state from the UI/query thread.
    #[cfg(feature = "dev-api")]
    pub(crate) fn engine_diagnostics(&self) -> serde_json::Value {
        let snapshot = self.latest_snapshot.read();
        let Some(snapshot) = snapshot.as_ref() else {
            return serde_json::Value::Null;
        };
        let Some(engine) = snapshot.external_engine_state.as_ref() else {
            return serde_json::Value::Null;
        };

        serde_json::json!({
            "snapshot_sequence": snapshot.sequence,
            "playback_epoch": engine.playback_epoch,
            "playback_state": format!("{:?}", engine.playback_state),
            "playback_frames_received": engine.playback_frames_received,
            "playback_frames_written": engine.playback_frames_written,
            "output_peak_linear": engine.output_peak_linear,
            "sample_rate": engine.sample_rate,
        })
    }

    #[cfg(feature = "dev-api")]
    pub(crate) fn profile_action_diagnostics(&self) -> serde_json::Value {
        let latest = self.latest_snapshot.read().clone();
        let Some(snapshot) = latest.as_ref() else {
            return serde_json::Value::Null;
        };
        let Some((engine_index, data)) = snapshot.rack_plugin_data.as_ref() else {
            return serde_json::Value::Null;
        };
        Self::qa_profile_action_status_from_data(
            self.transport_epoch.requested.load(Ordering::Acquire),
            snapshot.transport_epoch,
            snapshot.sequence,
            *engine_index,
            data,
        )
    }

    /// QA status from an already published actor snapshot. It has no access to
    /// pending playback events, so inspection cannot consume UI-owned events.
    #[cfg(feature = "dev-api")]
    pub fn qa_profile_action_status_from_data(
        requested_epoch: u64,
        snapshot_epoch: u64,
        sequence: u64,
        engine_index: usize,
        data: &Arc<dyn std::any::Any + Send + Sync>,
    ) -> serde_json::Value {
        if requested_epoch != snapshot_epoch {
            return serde_json::Value::Null;
        }
        if let Some(hiss) =
            data.downcast_ref::<sotf_plugins::plugin_hiss_reducer::snapshot::ProfileSnapshot>()
        {
            let (capturing, progress) = hiss.capture_state();
            let status = hiss.try_status().ok();
            return serde_json::json!({
                "engine_index": engine_index,
                "snapshot_sequence": sequence,
                "transport_epoch": snapshot_epoch,
                "kind": "hiss_reducer",
                "capturing": capturing,
                "progress": progress,
                "profile_present": status.as_ref().map(|s| s.present),
                "using_profile": status.as_ref().map(|s| s.use_flag),
                "generation": status.as_ref().map(|s| s.generation),
            });
        }
        if let Some(denoiser) = data.downcast_ref::<sotf_plugins::DenoiserData>() {
            return serde_json::json!({
                "engine_index": engine_index,
                "snapshot_sequence": sequence,
                "transport_epoch": snapshot_epoch,
                "kind": "denoiser",
                "capturing": denoiser.is_learning_noise,
                "progress": denoiser.learning_progress,
                "profile_present": denoiser.has_captured_profile,
                "using_profile": denoiser.using_captured_profile,
                "requested_use_profile": denoiser.requested_use_profile,
            });
        }
        serde_json::Value::Null
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
            .map_err(|_error| PlayerCommandError { label })
    }

    /// Observe one command without treating it as a transport change or replacing
    /// the latest transport receipt. Errors remain available to the global drain.
    fn enqueue_with_receipt<F>(
        &self,
        label: &'static str,
        f: F,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError>
    where
        F: FnOnce(&mut Player) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
    {
        let receipt = PlayerCommandReceipt {
            result: Arc::new(parking_lot::Mutex::new(None)),
        };
        let completion = Arc::clone(&receipt.result);
        self.enqueue_result(label, move |player| {
            let result = f(player);
            *completion.lock() =
                Some(
                    result
                        .as_ref()
                        .copied()
                        .map_err(|error| PlayerCommandFailure {
                            label,
                            error: error.to_string(),
                        }),
                );
            result
        })?;
        Ok(receipt)
    }

    /// Preserve an expected offline result in the QA receipt without routing
    /// it to the application's error toast. Other failures still use the
    /// normal command-failure drain.
    #[cfg(feature = "dev-api")]
    fn enqueue_qa_receipt<F>(
        &self,
        label: &'static str,
        f: F,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError>
    where
        F: FnOnce(&mut Player) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
    {
        let receipt = PlayerCommandReceipt {
            result: Arc::new(parking_lot::Mutex::new(None)),
        };
        let completion = Arc::clone(&receipt.result);
        self.enqueue_result(label, move |player| {
            let result = f(player);
            let deferred = result
                .as_ref()
                .is_err_and(|error| error.to_string() == "No engine running");
            *completion.lock() =
                Some(
                    result
                        .as_ref()
                        .copied()
                        .map_err(|error| PlayerCommandFailure {
                            label,
                            error: error.to_string(),
                        }),
                );
            if deferred { Ok(()) } else { result }
        })?;
        Ok(receipt)
    }

    fn enqueue_transport<F>(&self, label: &'static str, f: F) -> Result<(), PlayerCommandError>
    where
        F: FnOnce(&mut Player) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
    {
        self.enqueue_transport_with_receipt(label, f).map(|_| ())
    }

    fn enqueue_transport_with_receipt<F>(
        &self,
        label: &'static str,
        f: F,
    ) -> Result<TransportReceipt, PlayerCommandError>
    where
        F: FnOnce(&mut Player) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
    {
        // Serialize submission and publication so cloned handles cannot expose
        // an older receipt as the latest command after a newer submission.
        let mut last_transport = self.last_transport.lock();
        let receipt = TransportReceipt {
            result: Arc::new(parking_lot::Mutex::new(None)),
        };
        let completion = Arc::clone(&receipt.result);
        self.transport_epoch
            .requested
            .fetch_add(1, Ordering::AcqRel);
        let epoch = Arc::clone(&self.transport_epoch);
        let result = self.enqueue_result(label, move |player| {
            let result = f(player);
            *completion.lock() =
                Some(
                    result
                        .as_ref()
                        .copied()
                        .map_err(|error| PlayerCommandFailure {
                            label,
                            error: error.to_string(),
                        }),
                );
            epoch.completed.fetch_add(1, Ordering::Release);
            result
        });
        if result.is_err() {
            self.transport_epoch
                .requested
                .fetch_sub(1, Ordering::AcqRel);
        } else {
            *last_transport = Some(receipt.clone());
        }
        result.map(|()| receipt)
    }

    /// Inspect the most recently accepted transport command across all clones.
    /// Another submitter may replace it between submission and lookup. For a
    /// transaction, use a submission method that returns its receipt directly.
    /// Completion does not by itself make an older playback snapshot fresh.
    #[must_use]
    pub fn last_transport_receipt(&self) -> Option<TransportReceipt> {
        self.last_transport.lock().clone()
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn pause(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_transport("pause", |player| player.pause())
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn resume(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_transport("resume", |player| player.resume())
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn toggle_playback(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_transport("toggle_playback", |player| {
            if player.is_playing() {
                player.pause()
            } else {
                player.resume()
            }
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn stop(&self) -> Result<(), PlayerCommandError> {
        self.stop_with_receipt().map(|_| ())
    }

    /// Submit Stop and return its completion independently of other submitters.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn stop_with_receipt(&self) -> Result<TransportReceipt, PlayerCommandError> {
        self.enqueue_transport_with_receipt("stop", sotf_audio_player::Player::stop)
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn seek(&self, position_secs: f64) -> Result<(), PlayerCommandError> {
        self.enqueue_transport("seek", move |player| player.seek(position_secs))
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn set_volume(&self, volume: f32) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_volume", move |player| player.set_volume(volume))
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn set_mute(&self, muted: bool) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_mute", move |player| player.set_mute(muted))
    }

    /// Queue processing bypass on the player actor without rebuilding the rack.
    ///
    /// # Errors
    /// Returns an error if the actor cannot accept the command.
    pub fn set_bypass(&self, bypassed: bool) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_bypass", move |player| player.set_bypass(bypassed))
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn cancel_next(&self) -> Result<(), PlayerCommandError> {
        self.enqueue_result("cancel_next", |player| player.cancel_next())
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn queue_next(&self, path: PathBuf) -> Result<(), PlayerCommandError> {
        self.enqueue_result("queue_next", move |player| player.queue_next(path))
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn set_output_device(&self, device_name: String) -> Result<(), PlayerCommandError> {
        self.enqueue_transport("set_output_device", move |player| {
            player.set_output_device(device_name)
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn update_plugins(&self, plugins: Vec<PluginConfig>) -> Result<(), PlayerCommandError> {
        self.enqueue_result("update_plugins", move |player| {
            player.update_plugins(plugins)
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn update_plugin_graph(
        &self,
        graph_config: PluginGraphConfig,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("update_plugin_graph", move |player| {
            player.update_plugin_graph(graph_config)
        })
    }

    /// Completes after Player/Manager returns, not when the actor accepts the work.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn update_plugins_with_receipt(
        &self,
        plugins: Vec<PluginConfig>,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_with_receipt("update_plugins", move |player| {
            player.update_plugins(plugins)
        })
    }

    /// Configure this actor's development output before starting playback.
    #[cfg(feature = "dev-api")]
    pub(crate) fn set_test_output_enabled(
        &self,
        enabled: bool,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_with_receipt("set_test_output_enabled", move |player| {
            player.set_test_output_enabled(enabled)
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn update_plugin_graph_with_receipt(
        &self,
        graph_config: PluginGraphConfig,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_with_receipt("update_plugin_graph", move |player| {
            player.update_plugin_graph(graph_config)
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
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

    /// Momentary actions have no saved setting to defer to the next engine.
    /// Report an absent engine as a real command failure in release builds.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn set_plugin_action(
        &self,
        engine_index: usize,
        key: String,
        value: String,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_result("set_plugin_action", move |player| {
            player.set_plugin_parameter_strict(engine_index, key, value)
        })
    }

    /// QA observes the same strict action and user-visible failure path.
    #[cfg(feature = "dev-api")]
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn set_plugin_action_with_receipt(
        &self,
        engine_index: usize,
        key: String,
        value: String,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_with_receipt("set_plugin_action", move |player| {
            player.set_plugin_parameter_strict(engine_index, key, value)
        })
    }

    /// Dev-only acknowledgement of the worker/processing result for one
    /// parameter command. The realtime callback is unchanged.
    #[cfg(feature = "dev-api")]
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn set_plugin_parameter_with_receipt(
        &self,
        engine_index: usize,
        param_id: String,
        value: String,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_qa_receipt("set_plugin_parameter_qa", move |player| {
            player.set_plugin_parameter_strict(engine_index, param_id, value)
        })
    }

    #[cfg(feature = "dev-api")]
    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn update_plugins_qa_receipt(
        &self,
        plugins: Vec<PluginConfig>,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_qa_receipt("update_plugins_qa", move |player| {
            player.update_plugins_strict(plugins)
        })
    }

    #[cfg(feature = "dev-api")]
    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn update_plugin_graph_qa_receipt(
        &self,
        graph_config: PluginGraphConfig,
    ) -> Result<PlayerCommandReceipt, PlayerCommandError> {
        self.enqueue_qa_receipt("update_plugin_graph_qa", move |player| {
            player.update_plugin_graph_strict(graph_config)
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn load_and_play_source(
        &self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
    ) -> Result<(), PlayerCommandError> {
        self.enqueue_transport("load_and_play_source", move |player| {
            player.load_and_play_source(source, plugins, output_channels, output_device)
        })
    }

    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn load_or_switch_source_at(
        &self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        position: Option<f64>,
        prefer_smooth_switch: bool,
    ) -> Result<(), PlayerCommandError> {
        self.load_or_switch_source_at_with_receipt(
            source,
            plugins,
            output_channels,
            output_device,
            position,
            prefer_smooth_switch,
            None,
        )
        .map(|_| ())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "The transport command accepts the source, output, position, and graph atomically"
    )]
    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn load_or_switch_source_at_with_receipt(
        &self,
        source: AudioSource,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        position: Option<f64>,
        prefer_smooth_switch: bool,
        graph: Option<PluginGraphConfig>,
    ) -> Result<TransportReceipt, PlayerCommandError> {
        self.enqueue_transport_with_receipt("load_or_switch_source_at", move |player| {
            player.set_playback_graph(graph);
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
                            "[GPUI] Smooth track switch unavailable, falling back to restart: {error}"
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
    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn start_hal_playback_with_config(
        &self,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        sample_rate: u32,
    ) -> Result<(), PlayerCommandError> {
        self.start_hal_playback_with_config_receipt(
            plugins,
            output_channels,
            output_device,
            sample_rate,
        )
        .map(|_| ())
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn restart_hal_playback_with_config_receipt(
        &self,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        sample_rate: u32,
    ) -> Result<TransportReceipt, PlayerCommandError> {
        self.enqueue_transport_with_receipt("restart_hal_playback_with_config", move |player| {
            player.stop()?;
            player.start_hal_playback_with_config(
                plugins,
                output_channels,
                output_device,
                sample_rate,
            )
        })
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    /// Queues a playback worker command.
    ///
    /// # Errors
    /// Returns an error if the playback worker command channel is disconnected.
    pub fn start_hal_playback_with_config_receipt(
        &self,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
        output_device: Option<String>,
        sample_rate: u32,
    ) -> Result<TransportReceipt, PlayerCommandError> {
        self.enqueue_transport_with_receipt("start_hal_playback_with_config", move |player| {
            player.start_hal_playback_with_config(
                plugins,
                output_channels,
                output_device,
                sample_rate,
            )
        })
    }
}

impl std::fmt::Debug for PlayerSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerSnapshot")
            .field("sequence", &self.sequence)
            .field("is_playing", &self.is_playing)
            .field("position_secs", &self.position_secs)
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for PlayerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerHandle")
            .field(
                "snapshot_requested",
                &self
                    .snapshot_requested
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn flush(handle: &PlayerHandle) {
        let (sent, received) = mpsc::channel();
        handle
            .enqueue_result("test-barrier", move |_| {
                sent.send(()).unwrap();
                Ok(())
            })
            .unwrap();
        received.recv_timeout(Duration::from_secs(3)).unwrap();
    }

    fn initial_handle() -> PlayerHandle {
        let handle = PlayerHandle::new(Player::new());
        handle
            .request_snapshot(PlayerSnapshotRequest::default())
            .unwrap();
        flush(&handle);
        assert!(handle.read_snapshot().is_some());
        handle
    }

    #[cfg(feature = "dev-api")]
    #[test]
    fn profile_diagnostics_reject_stale_transport_and_do_not_consume_events() {
        let handle = initial_handle();
        {
            let mut slot = handle.latest_snapshot.write();
            let snapshot = Arc::get_mut(slot.as_mut().unwrap()).unwrap();
            snapshot.rack_plugin_data = Some((
                3,
                Arc::new(sotf_plugins::plugin_hiss_reducer::snapshot::ProfileSnapshot::new(2)),
            ));
        }
        handle.pending_events.lock().track_ended = true;
        let current = handle.profile_action_diagnostics();
        assert_eq!(current["kind"], "hiss_reducer");
        assert_eq!(current["engine_index"], 3);
        assert_eq!(current["profile_present"], false);
        assert!(handle.pending_events.lock().track_ended);
        handle
            .enqueue_transport("profile-status-epoch", |_| Ok(()))
            .unwrap();
        flush(&handle);
        assert!(handle.profile_action_diagnostics().is_null());
        assert!(handle.pending_events.lock().track_ended);
    }

    #[test]
    fn accepted_transport_rejects_idle_snapshot_until_actor_refreshes() {
        let handle = initial_handle();
        handle.pending_events.lock().track_ended = true;
        let (release, wait) = mpsc::channel();
        handle
            .enqueue_transport("test-source-load", move |_| {
                wait.recv_timeout(Duration::from_secs(3)).unwrap();
                Ok(())
            })
            .unwrap();
        let receipt = handle.last_transport_receipt().unwrap();
        assert!(receipt.poll().is_none());
        // The cached Idle and end event precede the accepted source request.
        assert!(handle.read_snapshot().is_none());
        release.send(()).unwrap();
        flush(&handle);
        assert!(matches!(receipt.poll(), Some(Ok(()))));
        // Command completion alone cannot make the old cached snapshot fresh.
        assert!(handle.read_snapshot().is_none());
        handle
            .request_snapshot(PlayerSnapshotRequest::default())
            .unwrap();
        flush(&handle);
        let fresh = handle.read_snapshot().unwrap();
        assert!(!fresh.playback_state.track_ended);
        assert_eq!(fresh.snapshot.transport_epoch, 1);
    }

    #[test]
    fn consecutive_transport_commands_require_a_snapshot_after_both() {
        let handle = initial_handle();
        handle.enqueue_transport("first", |_| Ok(())).unwrap();
        handle.enqueue_transport("second", |_| Ok(())).unwrap();
        flush(&handle);
        assert!(handle.read_snapshot().is_none());
        handle
            .request_snapshot(PlayerSnapshotRequest::default())
            .unwrap();
        flush(&handle);
        assert_eq!(handle.read_snapshot().unwrap().snapshot.transport_epoch, 2);
    }

    #[test]
    fn rejected_transport_does_not_invalidate_the_last_snapshot() {
        let mut handle = initial_handle();
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        handle.sender = sender;
        assert!(handle.enqueue_transport("rejected", |_| Ok(())).is_err());
        assert!(handle.last_transport_receipt().is_none());
        assert!(handle.read_snapshot().is_some());
    }

    #[test]
    fn transport_receipts_preserve_each_commands_failure_or_success() {
        let handle = initial_handle();
        handle
            .enqueue_transport("failed-source", |_| {
                Err(std::io::Error::other("source unavailable").into())
            })
            .unwrap();
        let failed = handle.last_transport_receipt().unwrap();
        handle
            .enqueue_transport("retry-source", |_| Ok(()))
            .unwrap();
        let succeeded = handle.last_transport_receipt().unwrap();
        flush(&handle);

        let failure = failed.poll().unwrap().unwrap_err();
        assert_eq!(failure.label, "failed-source");
        assert_eq!(failure.error, "source unavailable");
        assert!(matches!(succeeded.poll(), Some(Ok(()))));
        // Polling must not consume the result: a pending UI transaction may
        // inspect a completed Stop while its following Start is still pending.
        assert_eq!(failed.poll().unwrap().unwrap_err().error, failure.error);
        assert!(matches!(succeeded.poll(), Some(Ok(()))));
    }

    #[test]
    fn rejected_transport_preserves_the_previous_receipt() {
        let mut handle = initial_handle();
        handle.enqueue_transport("accepted", |_| Ok(())).unwrap();
        let accepted = handle.last_transport_receipt().unwrap();
        flush(&handle);
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        handle.sender = sender;

        assert!(handle.enqueue_transport("rejected", |_| Ok(())).is_err());
        let latest = handle.last_transport_receipt().unwrap();
        assert!(Arc::ptr_eq(&accepted.result, &latest.result));
        assert!(matches!(latest.poll(), Some(Ok(()))));
    }

    #[test]
    fn submitted_receipt_is_not_replaced_by_a_cloned_handles_command() {
        let handle = initial_handle();
        let first = handle
            .enqueue_transport_with_receipt("first-source", |_| {
                Err(std::io::Error::other("first source failed").into())
            })
            .unwrap();
        let other = handle.clone();
        let second = other
            .enqueue_transport_with_receipt("second-source", |_| Ok(()))
            .unwrap();
        flush(&handle);

        assert_eq!(first.poll().unwrap().unwrap_err().label, "first-source");
        assert!(matches!(second.poll(), Some(Ok(()))));
        assert!(!Arc::ptr_eq(&first.result, &second.result));
    }

    #[test]
    fn plugin_receipt_waits_for_execution_without_replacing_transport_receipt() {
        let handle = initial_handle();
        handle
            .enqueue_transport("existing-transport", |_| Ok(()))
            .unwrap();
        flush(&handle);
        let transport = handle.last_transport_receipt().unwrap();
        let (release, blocked) = mpsc::channel();
        handle
            .sender
            .send(Box::new(move |_| {
                blocked.recv_timeout(Duration::from_secs(3)).unwrap();
            }))
            .unwrap();
        let receipt = handle.update_plugins_with_receipt(Vec::new()).unwrap();
        assert!(receipt.poll().is_none());
        assert!(Arc::ptr_eq(
            &transport.result,
            &handle.last_transport_receipt().unwrap().result
        ));
        release.send(()).unwrap();
        flush(&handle);
        assert!(matches!(receipt.poll(), Some(Ok(()))));
    }

    #[test]
    fn command_receipt_retains_failure_independently_of_later_success() {
        let handle = initial_handle();
        let failed = handle
            .enqueue_with_receipt("failed-plugin-update", |_| {
                Err("invalid processing graph".into())
            })
            .unwrap();
        let succeeded = handle
            .enqueue_with_receipt("plugin-retry", |_| Ok(()))
            .unwrap();
        flush(&handle);
        let failure = failed.poll().unwrap().unwrap_err();
        assert_eq!(failure.label, "failed-plugin-update");
        assert_eq!(failure.error, "invalid processing graph");
        assert!(matches!(succeeded.poll(), Some(Ok(()))));
        let failures = handle.drain_failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].error, failure.error);
        assert_eq!(failed.poll().unwrap().unwrap_err().error, failure.error);
    }

    #[test]
    fn rejected_plugin_submission_does_not_return_a_pending_receipt() {
        let mut handle = initial_handle();
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        handle.sender = sender;
        assert!(handle.update_plugins_with_receipt(Vec::new()).is_err());
    }
}
