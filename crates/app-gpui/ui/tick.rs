/// Snapshot of observable tick state used to suppress redundant
/// `cx.notify()` calls. Every notify re-renders the whole view tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TickSnapshot {
    is_playing: bool,
    /// Position in centiseconds; sub-frame precision is unnecessary for
    /// UI refresh and avoids float-equality pitfalls.
    position_centiseconds: u64,
    duration_centiseconds: u64,
    current_screen: Screen,
    theme_id: crate::theme::ThemeId,
    queue_index: Option<usize>,
    has_compressor: bool,
    spectrum_present: bool,
    has_toast: bool,
    federation_scan_active: bool,
    scan_status_hidden: bool,
    library_scan_active: bool,
    library_scan_tracks: usize,
    library_scan_albums: usize,
    library_scan_total_files: usize,
    library_scan_elapsed_secs: u64,
    library_scan_eta_secs: Option<u64>,
    library_scan_rate_tenths: u32,
    library_scan_phase_len: usize,
    replay_gain_scan_active: bool,
    replay_gain_processed: usize,
    replay_gain_total: usize,
    replay_gain_album_done: usize,
    replay_gain_album_total: usize,
    waveform_scan_active: bool,
    waveform_processed: usize,
    waveform_total: usize,
    bliss_scan_active: bool,
    bliss_processed: usize,
    bliss_total: usize,
    library_stats_computing: bool,
    remote_server_probe_revision: u64,
    remote_album_page_revision: u64,
    signal_path_resampled: bool,
    signal_path_underruns: u64,
    signal_path_stream_errors: u64,
    signal_path_frames_dropped: u64,
    signal_path_clipping: Option<bool>,
}

/// Screens where rack analyzer data is visible. Library/Queue only show rack
/// data in expanded three-panel mode; compact mode renders a single content
/// screen and should not pay for compressor/level-meter refreshes.
pub fn screen_shows_rack_data(screen: Screen, layout_mode: crate::app::LayoutMode) -> bool {
    matches!(screen, Screen::Studio | Screen::PluginGraph)
        || (matches!(screen, Screen::NowPlaying | Screen::Library | Screen::Queue)
            && layout_mode == crate::app::LayoutMode::Expanded)
}

/// True when the engine stopped unexpectedly and the UI should auto-advance
/// the queue. A user-initiated pause surfaces as `StreamingState::Paused`
/// (and track reloads as `Loading`/`Ready`/`Seeking`), none of which may
/// advance; only a genuine stop (`Idle`/`Error`) or an end-of-stream flag
/// means the current track is over.
pub fn should_auto_advance_on_engine_stop(
    track_ended: bool,
    was_playing: bool,
    engine_is_playing: bool,
    streaming_state: StreamingState,
    has_queue_context: bool,
) -> bool {
    (track_ended
        || (was_playing
            && !engine_is_playing
            && matches!(
                streaming_state,
                StreamingState::Idle | StreamingState::Error
            )))
        && has_queue_context
}

/// Analyzer caches are only refreshed while audio flows. When the engine is
/// paused or sits idle at end-of-stream it stays alive and keeps serving the
/// last computed `LoudnessData`, which would freeze the meters mid-value.
/// Return a copy with the instantaneous level fields zeroed for any state
/// that isn't playing. The channel layout is preserved so meter groups don't
/// rebuild, and `integrated_lufs` is kept — program loudness doesn't change
/// just because playback paused.
pub fn silent_loudness(
    info: &Option<Arc<sotf_audio_player::LoudnessData>>,
) -> Option<Arc<sotf_audio_player::LoudnessData>> {
    info.as_ref().map(|data| {
        let mut silent = (**data).clone();
        silent.momentary_lufs = f64::NEG_INFINITY;
        silent.shortterm_lufs = f64::NEG_INFINITY;
        silent.peak = 0.0;
        silent.channel_peaks = Arc::new(vec![0.0; data.channel_peaks.len()]);
        silent.true_peaks_dbtp = Arc::new(vec![f64::NEG_INFINITY; data.true_peaks_dbtp.len()]);
        Arc::new(silent)
    })
}

/// True when a clean engine stop has no queue context to auto-advance from,
/// so the UI must clear stale "playing" state immediately. Pauses and
/// transitional states (`Paused`/`Loading`/`Ready`/`Seeking`) are not stops.
pub fn engine_stop_without_queue_should_clear(
    was_playing: bool,
    engine_is_playing: bool,
    streaming_state: StreamingState,
    has_queue_context: bool,
) -> bool {
    was_playing
        && !engine_is_playing
        && matches!(
            streaming_state,
            StreamingState::Idle | StreamingState::Error
        )
        && !has_queue_context
}

impl PlayerView {
    /// Build a `TickSnapshot` from the current state. Cheap (no allocation,
    /// just a few field reads). Used to suppress `cx.notify()` when no
    /// observable state changed in the tick.
    fn tick_snapshot(view: &PlayerView, cx: &mut Context<Self>) -> TickSnapshot {
        let state = view.state.read(cx);
        let position_centiseconds = (state.app.playback.position_secs.max(0.0) * 100.0) as u64;
        let duration_centiseconds = (state.app.playback.duration_secs.max(0.0) * 100.0) as u64;
        TickSnapshot {
            is_playing: state.app.playback.is_playing,
            position_centiseconds,
            duration_centiseconds,
            current_screen: state.app.ui_state.current_screen,
            theme_id: state.app.ui_state.theme_id,
            queue_index: state.app.playback.current_queue_index,
            has_compressor: state.app.playback.compressor_info.is_some(),
            spectrum_present: state.app.playback.spectrum_info.is_some(),
            has_toast: state.app.ui_state.toast_message.is_some(),
            federation_scan_active: state.app.federation.scan_progress.is_some(),
            scan_status_hidden: state.app.scan.status_hidden,
            library_scan_active: state.app.library_state.scan_in_progress,
            library_scan_tracks: state.app.library_state.scan_progress_tracks,
            library_scan_albums: state.app.library_state.scan_progress_albums,
            library_scan_total_files: state.app.scan.total_files,
            library_scan_elapsed_secs: state.app.scan.progress_elapsed_secs,
            library_scan_eta_secs: state.app.scan.progress_eta_secs,
            library_scan_rate_tenths: (state.app.scan.progress_tracks_per_sec.max(0.0) * 10.0)
                as u32,
            library_scan_phase_len: state.app.scan.progress_phase.len(),
            replay_gain_scan_active: state.app.scan.ctrl.replay_gain_manager.in_progress,
            replay_gain_processed: state.app.scan.ctrl.replay_gain_manager.processed,
            replay_gain_total: state.app.scan.ctrl.replay_gain_manager.total,
            replay_gain_album_done: state.app.scan.ctrl.replay_gain_manager.album_gain_done,
            replay_gain_album_total: state.app.scan.ctrl.replay_gain_manager.album_gain_total,
            waveform_scan_active: state.app.scan.ctrl.waveform_manager.in_progress,
            waveform_processed: state.app.scan.ctrl.waveform_manager.processed,
            waveform_total: state.app.scan.ctrl.waveform_manager.total,
            bliss_scan_active: state.app.scan.ctrl.bliss_manager.in_progress,
            bliss_processed: state.app.scan.ctrl.bliss_manager.processed,
            bliss_total: state.app.scan.ctrl.bliss_manager.total,
            library_stats_computing: state.app.library_view.stats_computing,
            remote_server_probe_revision: state.app.remote.server_probe_revision,
            remote_album_page_revision: state.app.remote.remote_album_page_revision,
            signal_path_resampled: state
                .app
                .playback
                .signal_path
                .as_ref()
                .is_some_and(|p| p.is_resampled()),
            signal_path_underruns: state
                .app
                .playback
                .signal_path
                .as_ref()
                .map_or(0, |p| p.health.underruns),
            signal_path_stream_errors: state
                .app
                .playback
                .signal_path
                .as_ref()
                .map_or(0, |p| p.health.stream_errors),
            signal_path_frames_dropped: state
                .app
                .playback
                .signal_path
                .as_ref()
                .map_or(0, |p| p.health.frames_dropped),
            signal_path_clipping: state
                .app
                .playback
                .signal_path
                .as_ref()
                .and_then(|p| p.health.clipping_detected),
        }
    }

    /// Sync playback position, duration, and analyzer data from the audio engine.
    ///
    /// Queues an actor snapshot request, reads the latest immutable snapshot,
    /// copies position/duration into app state, and updates visible level
    /// meters without touching the player or audio engine on the UI thread.
    ///
    /// Screen-gated: compressor downcast and level-meter recomputation are
    /// skipped when the current screen does not show them. Spectrum read
    /// remains gated by `spectrum_visible || Screen::Spectrum`. IN/OUT
    /// loudness reads stay unconditional because `sync_chain_autogain` and
    /// the footer transport bar both consume them.
    ///
    /// `compressor_idx_cache` is borrowed across ticks so the
    /// `compressor_engine_index()` lookup is amortised. It's invalidated by
    /// the tick wrapper whenever `apply_plugin_update` runs.
    ///
    /// Returns `(engine_playback_state, was_playing)` for downstream tick work.
    fn sync_playback_data(
        state: &mut AppState,
        frame_count: u64,
        compressor_idx_cache: &mut Option<Option<usize>>,
    ) -> (sotf_audio_player::PlaybackState, bool) {
        let current_screen = state.app.ui_state.current_screen;
        let layout_mode = state.app.ui_state.layout_mode;
        let should_update_spectrum = frame_count.is_multiple_of(2);
        let include_spectrum = should_update_spectrum
            && (state.app.layout.spectrum_visible || current_screen == Screen::Spectrum);
        let structural_update_pending = matches!(
            state
                .app
                .plugin_state
                .update_state
                .pending_plugin_update,
            Some(crate::app::types::PluginUpdateType::Structural)
        );
        // Engine indices describe the pre-update graph until the structural
        // update has been applied. Suppress live plugin data during that gap
        // so an index reused by a reordered node cannot display stale data.
        let include_rack_data =
            screen_shows_rack_data(current_screen, layout_mode) && !structural_update_pending;
        let include_compressor = include_rack_data;
        let include_level_meters = include_rack_data;
        let can_update_autogain = !structural_update_pending;
        let poll_external_diagnostics = frame_count.is_multiple_of(10);
        let include_external_diagnostics =
            poll_external_diagnostics && state.app.plugin_state.has_external_plugins();

        let was_playing = state.app.playback.is_playing;
        let (input_monitor_idx, output_monitor_idx, spectrum_idx, compressor_idx, rack_plugin_idx) = {
            let graph = &state.app.plugin_state.graph;
            let compressor_idx = if include_compressor {
                match *compressor_idx_cache {
                    Some(cached) => cached,
                    None => {
                        let found = graph.compressor_engine_index();
                        *compressor_idx_cache = Some(found);
                        found
                    }
                }
            } else {
                None
            };
            (
                graph.input_monitor_engine_index(),
                graph.output_monitor_engine_index(),
                if include_spectrum {
                    graph.spectrum_engine_index()
                } else {
                    None
                },
                compressor_idx,
                if include_rack_data {
                    graph.get_engine_index_by_linear_position(
                        state.app.plugin_state.selected_plugin_index,
                    )
                } else {
                    None
                },
            )
        };

        if let Err(error) = state.player.request_snapshot(
            crate::app::player_handle::PlayerSnapshotRequest {
                input_monitor_idx,
                output_monitor_idx,
                spectrum_idx,
                compressor_idx,
                rack_plugin_idx,
                include_external_diagnostics,
            },
        ) {
            log::debug!("Player snapshot request failed: {error}");
        }

        let Some(snapshot_read) = state.player.read_snapshot() else {
            // No snapshot yet: fabricate state consistent with the UI flags.
            // Never fabricate `Idle`/`Error` here — that would let the
            // auto-advance guard fire without any real engine input.
            let fallback_is_playing = state.app.playback.is_playing;
            return (
                sotf_audio_player::PlaybackState {
                    position_secs: state.app.playback.position_secs,
                    is_playing: fallback_is_playing,
                    streaming_state: if fallback_is_playing {
                        StreamingState::Playing
                    } else {
                        StreamingState::Paused
                    },
                    sample_rate: state.app.playback.sample_rate,
                    last_error: None,
                    engine_restarted: false,
                    engine_fatal: false,
                    track_ended: false,
                    gapless_transition: None,
                    stream_metadata: None,
                },
                was_playing,
            );
        };

        let snapshot = snapshot_read.snapshot;
        let playback_state = snapshot_read.playback_state;

        if let Some(engine_state) = snapshot.external_engine_state.as_ref() {
            state.app.plugin_state.sync_external_plugin_engine_diagnostics(
                engine_state.plugin_build_diagnostics.clone(),
                engine_state.isolated_external_plugin_worker_statuses.clone(),
            );
        } else if poll_external_diagnostics && !include_external_diagnostics {
            state
                .app
                .plugin_state
                .sync_external_plugin_engine_diagnostics(Vec::new(), Vec::new());
        }

        state.app.playback.signal_path = Some(snapshot.signal_path.clone());
        state.app.playback.is_playing = playback_state.is_playing;
        state.app.playback.position_secs = playback_state.position_secs;
        state.app.playback.duration_secs = state.app.get_current_track_duration();

        // Analyzer caches freeze when audio stops flowing (pause / end-of-
        // stream leave the engine alive); only full stop() drops the data.
        // Feed the meters zeroed levels whenever nothing is playing so they
        // fall to 0 instead of staying where they were.
        let meters_live = playback_state.is_playing;
        state.app.playback.input_loudness_info = if meters_live {
            snapshot.input_loudness_info.clone()
        } else {
            silent_loudness(&snapshot.input_loudness_info)
        };
        state.app.playback.loudness_info = if meters_live {
            snapshot.loudness_info.clone()
        } else {
            silent_loudness(&snapshot.loudness_info)
        };
        if include_spectrum {
            state.app.playback.spectrum_info = snapshot.spectrum_info.clone();
        }
        if include_compressor {
            state.app.playback.compressor_info = snapshot.compressor_info.clone();
        } else {
            state.app.playback.compressor_info = None;
        }
        state.app.playback.rack_plugin_data = if include_rack_data {
            match (rack_plugin_idx, snapshot.rack_plugin_data.as_ref()) {
                (Some(requested_idx), Some((snapshot_idx, data)))
                    if requested_idx == *snapshot_idx =>
                {
                    Some(data.clone())
                }
                _ => None,
            }
        } else {
            None
        };

        if can_update_autogain
            && let Some((engine_index, next_gain_db)) =
                Self::sync_chain_autogain(state, frame_count)
            && let Err(e) = state.player.set_plugin_parameter(
                engine_index,
                "gain_db".to_string(),
                format!("{next_gain_db:.3}"),
            )
        {
            log::warn!("Failed to update chain AutoGain: {}", e);
        }

        if include_level_meters {
            state.app.update_level_meter_groups();
            state.app.update_level_meter_peak_hold();
        }

        (playback_state, was_playing)
    }

    /// Keep the rack AutoGain trim aligned with the current IN/OUT loudness.
    fn sync_chain_autogain(state: &mut AppState, frame_count: u64) -> Option<(usize, f64)> {
        const UPDATE_INTERVAL_FRAMES: u64 = 5;
        const DEAD_BAND_DB: f64 = 0.25;
        const MAX_GAIN_DB: f64 = 24.0;

        if !state.app.plugin_state.chain_state.chain_autogain {
            return None;
        }

        let last_frame = state.app.plugin_state.chain_state.chain_autogain_last_frame;
        if last_frame != 0 && frame_count.wrapping_sub(last_frame) < UPDATE_INTERVAL_FRAMES {
            return None;
        }

        let input = state.app.playback.input_loudness_info.as_deref()?;
        let output = state.app.playback.loudness_info.as_deref()?;

        let input_lufs = input.momentary_lufs;
        let output_lufs = output.momentary_lufs;
        if !input_lufs.is_finite() || !output_lufs.is_finite() {
            return None;
        }

        let residual_db = input_lufs - output_lufs;
        if residual_db.abs() < DEAD_BAND_DB {
            return None;
        }

        let current_gain_db = state
            .app
            .plugin_state
            .graph
            .chain_auto_gain_db()
            .unwrap_or(0.0);
        let next_gain_db = (current_gain_db + residual_db).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
        if (next_gain_db - current_gain_db).abs() < 0.1 {
            return None;
        }

        state
            .app
            .plugin_state
            .graph
            .set_chain_auto_gain(Some(next_gain_db));
        state.app.plugin_state.chain_state.chain_autogain_last_frame = frame_count;

        state
            .app
            .plugin_state
            .graph
            .chain_auto_gain_engine_index()
            .map(|engine_index| (engine_index, next_gain_db))
    }

    /// Handle engine crash, errors, gapless transitions, and track auto-advance.
    ///
    /// Priority: fatal crash > playback error > engine restart > gapless transition > track end.
    fn handle_engine_state(
        state: &mut AppState,
        playback_state: &sotf_audio_player::PlaybackState,
        was_playing: bool,
    ) {
        if playback_state.engine_fatal {
            log::error!("[GPUI] Engine crashed fatally, cannot auto-restart");
            state.app.playback.is_playing = false;
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                "Audio engine crashed. Please play a new track to restart.",
            ));
        } else if let Some(ref err) = playback_state.last_error {
            log::error!("[GPUI] Playback error: {}", err);
            state.app.playback.is_playing = false;
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(format!(
                "Playback error: {}",
                err
            )));
        } else if playback_state.engine_restarted {
            log::info!("[GPUI] Engine auto-restarted after crash, resuming playback");
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::info(
                "Engine restarted, resuming playback",
            ));
        } else if playback_state.gapless_transition.is_some() {
            state.app.stop_track_tracking();
            let _ = state.app.next_track();
            if let Some(path) = state.app.get_current_track_path() {
                state.app.start_track_tracking(path);
            }
        } else if should_auto_advance_on_engine_stop(
            playback_state.track_ended,
            was_playing,
            playback_state.is_playing,
            playback_state.streaming_state,
            state.app.playback.current_queue_index.is_some(),
        ) {
            state.app.stop_track_tracking();
            if let Some(path) = state.app.next_track() {
                Self::play_track_auto_advance(state, path);
            } else {
                state.app.playback.is_playing = false;
            }
        } else if engine_stop_without_queue_should_clear(
            was_playing,
            playback_state.is_playing,
            playback_state.streaming_state,
            state.app.playback.current_queue_index.is_some(),
        ) {
            log::info!("[GPUI] Engine stopped without queue context; clearing playing state");
            state.app.playback.is_playing = false;
            state.app.stop_track_tracking();
        }
    }

    /// Queue the next file for gapless playback when near the end of the current track.
    ///
    /// Only queues when channel counts match (engine constraint for gapless transitions).
    fn handle_gapless_prequeue(
        state: &mut AppState,
        playback_state: &sotf_audio_player::PlaybackState,
    ) {
        if !playback_state.is_playing || state.app.playback.current_queue_index.is_none() {
            return;
        }

        let position = playback_state.position_secs;
        let duration = state.app.playback.duration_secs;
        let near_end = duration > 0.0 && position > 0.0 && (duration - position) < 10.0;

        if near_end && let Some(next_track) = state.app.queue_state.peek_next_track() {
            let next_ch = next_track.channels.unwrap_or(2) as usize;
            let current_ch = state
                .app
                .playback
                .current_queue_index
                .and_then(|idx| state.app.queue_state.get(idx))
                .and_then(|item| item.current_track())
                .and_then(|t| t.channels)
                .unwrap_or(2) as usize;

            if next_ch == current_ch {
                let _ = state.player.queue_next(next_track.path.clone());
            }
        }
    }

    /// Run periodic background housekeeping: startup DB check, scan updates,
    /// toast updates, library cache validation, and pending stats pickup.
    fn tick_background_tasks(state: &mut AppState) {
        state.app.check_library_on_startup();

        state.app.scan.ctrl.update_all();
        state.app.update_library_scan();
        state.app.update_federation_scan();
        state.app.update_cast_discovery();
        state.app.update_remote_server_discovery();
        state.app.update_remote_server_probe();
        state.app.update_remote_event_stream();
        state.app.update_remote_album_queue_command();
        let remote_browse_visible = matches!(
            state.app.ui_state.current_screen,
            Screen::Home | Screen::Library
        ) && state.app.remote.server_store.selected_server_id.is_some();
        if remote_browse_visible
            && state.app.remote.current_state.is_none()
            && !state.app.remote.refresh_requests.state
        {
            state.app.remote.refresh_requests.state = true;
        }
        if remote_browse_visible
            && !state.app.remote.refresh_requests.visible_album_page
            && state.app.remote_visible_album_page_needs_refresh()
        {
            state.app.remote.refresh_requests.visible_album_page = true;
        }
        state.app.update_remote_cache_refresh();
        state.app.update_toast();

        state.app.library_state.ensure_cache_valid();

        for failure in state.player.drain_failures() {
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(format!(
                "Audio command failed ({}): {}",
                failure.label, failure.error
            )));
        }

        let pending = state.app.library_view.pending_stats.lock().take();
        if let Some(stats) = pending {
            state.app.library_view.stats = stats;
            state.app.library_view.stats_computing = false;
        }
    }
}
