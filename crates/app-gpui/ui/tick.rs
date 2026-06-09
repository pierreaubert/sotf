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
    library_stats_computing: bool,
    remote_server_probe_revision: u64,
    remote_album_page_revision: u64,
}

/// Screens where rack analyzer data is visible. Library/Queue only show rack
/// data in expanded three-panel mode; compact mode renders a single content
/// screen and should not pay for compressor/level-meter refreshes.
pub fn screen_shows_rack_data(screen: Screen, layout_mode: crate::app::LayoutMode) -> bool {
    matches!(screen, Screen::Studio | Screen::PluginGraph)
        || (matches!(screen, Screen::NowPlaying | Screen::Library | Screen::Queue)
            && layout_mode == crate::app::LayoutMode::Expanded)
}

/// True when a clean engine stop has no queue context to auto-advance from,
/// so the UI must clear stale "playing" state immediately.
pub fn engine_stop_without_queue_should_clear(
    was_playing: bool,
    engine_is_playing: bool,
    has_queue_context: bool,
) -> bool {
    was_playing && !engine_is_playing && !has_queue_context
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
            library_stats_computing: state.app.library_stats_computing,
            remote_server_probe_revision: state.app.remote.server_probe_revision,
            remote_album_page_revision: state.app.remote.remote_album_page_revision,
        }
    }

    /// Sync playback position, duration, and analyzer data from the audio engine.
    ///
    /// Locks the player, reads the engine's `PlaybackState`, copies
    /// position/duration into app state, reads cached analyzer plugin data,
    /// and updates visible level meters after dropping the player lock.
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
            && (state.app.spectrum_visible || current_screen == Screen::Spectrum);
        let include_rack_data = screen_shows_rack_data(current_screen, layout_mode);
        let include_compressor = include_rack_data;
        let include_level_meters = include_rack_data;
        let can_update_autogain = state.app.plugin_state.pending_plugin_update.is_none();

        let player_handle = state.player.clone();
        let mut player = player_handle.lock();
        let playback_state = player.get_playback_state();

        let was_playing = state.app.playback.is_playing;
        state.app.playback.is_playing = playback_state.is_playing;
        state.app.playback.position_secs = playback_state.position_secs;
        state.app.playback.duration_secs = state.app.get_current_track_duration();

        {
            let graph = &state.app.plugin_state.graph;

            state.app.playback.input_loudness_info = graph
                .input_monitor_engine_index()
                .and_then(|idx| player.get_cached_plugin_data(idx))
                .and_then(|d| d.downcast_ref::<sotf_audio_player::LoudnessData>().cloned());

            state.app.playback.loudness_info = graph
                .output_monitor_engine_index()
                .and_then(|idx| player.get_cached_plugin_data(idx))
                .and_then(|d| d.downcast_ref::<sotf_audio_player::LoudnessData>().cloned());

            if include_spectrum {
                state.app.playback.spectrum_info = graph
                    .spectrum_engine_index()
                    .and_then(|idx| player.get_cached_plugin_data(idx))
                    .and_then(|d| d.downcast_ref::<sotf_audio_player::SpectrumData>().cloned());
            }

            if include_compressor {
                let idx = match *compressor_idx_cache {
                    Some(cached) => cached,
                    None => {
                        let found = graph.compressor_engine_index();
                        *compressor_idx_cache = Some(found);
                        found
                    }
                };
                state.app.playback.compressor_info = idx
                    .and_then(|i| player.get_cached_plugin_data(i))
                    .and_then(|d| d.downcast_ref::<sotf_plugins::CompressorData>().cloned());
            } else {
                state.app.playback.compressor_info = None;
            }
        }

        if can_update_autogain
            && let Some((engine_index, next_gain_db)) =
                Self::sync_chain_autogain(state, frame_count)
            && let Err(e) = player.set_plugin_parameter(
                engine_index,
                "gain_db".to_string(),
                format!("{next_gain_db:.3}"),
            )
        {
            log::warn!("Failed to update chain AutoGain: {}", e);
        }

        drop(player);

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

        if !state.app.plugin_state.chain_autogain {
            return None;
        }

        let last_frame = state.app.plugin_state.chain_autogain_last_frame;
        if last_frame != 0 && frame_count.wrapping_sub(last_frame) < UPDATE_INTERVAL_FRAMES {
            return None;
        }

        let input = state.app.playback.input_loudness_info.as_ref()?;
        let output = state.app.playback.loudness_info.as_ref()?;

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
        state.app.plugin_state.chain_autogain_last_frame = frame_count;

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
        } else if (playback_state.track_ended || (was_playing && !playback_state.is_playing))
            && state.app.playback.current_queue_index.is_some()
        {
            state.app.stop_track_tracking();
            if let Some(path) = state.app.next_track() {
                Self::play_track_auto_advance(state, path);
            } else {
                state.app.playback.is_playing = false;
            }
        } else if engine_stop_without_queue_should_clear(
            was_playing,
            playback_state.is_playing,
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
                let _ = state.player.lock().queue_next(next_track.path.clone());
            }
        }
    }

    /// Run periodic background housekeeping: startup DB check, scan updates,
    /// toast updates, library cache validation, and pending stats pickup.
    fn tick_background_tasks(state: &mut AppState) {
        state.app.check_library_on_startup();

        state.app.scan_ctrl.update_all();
        state.app.update_library_scan();
        state.app.update_federation_scan();
        state.app.update_cast_discovery();
        state.app.update_remote_server_discovery();
        state.app.update_remote_server_probe();
        state.app.update_remote_event_stream();
        state.app.update_remote_album_queue_command();
        if state.app.ui_state.current_screen == Screen::Home
            && state.app.remote.server_store.selected_server_id.is_some()
            && state.app.remote.current_album_page.is_none()
            && !state.app.remote.refresh_requests.visible_album_page
        {
            state.app.remote.refresh_requests.visible_album_page = true;
        }
        state.app.update_remote_cache_refresh();
        state.app.update_toast();

        state.app.library_state.ensure_cache_valid();

        let pending = state.app.pending_library_stats.lock().take();
        if let Some(stats) = pending {
            state.app.library_stats = stats;
            state.app.library_stats_computing = false;
        }
    }
}
