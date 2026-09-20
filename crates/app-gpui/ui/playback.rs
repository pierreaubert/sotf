#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
use sotf_media_controls::MediaControlEvent;

impl PlayerView {
    /// Resume on the committed output without starting audio when Preferences is applied.
    fn resume_on_selected_output(state: &mut AppState) -> bool {
        if let Some(original) = state.app.audio_device_state.pending_output_restart.clone() {
            let Some(source) = state.app.queue_state.current_track_source() else {
                return false;
            };
            let position = state.app.playback.position_secs;
            Self::play_track_at(state, source.clone(), Some(position));
            if !state.app.playback.is_playing {
                state.app.audio_device_state.selected_output_device_index = original.0;
                state.app.audio_device_state.current_output_device_name = original.1;
                state.app.audio_device_state.follow_system_default = original.2;
                state.app.audio_device_state.pending_output_restart = None;
                Self::play_track_at(state, source, Some(position));
                let text = crate::app::i18n::DesktopTranslations::for_language(
                    state.app.ui_state.language,
                );
                state.app.ui_state.toast_message =
                    Some(crate::app::ToastMessage::error(text.output_failed));
            }
            return state.app.playback.is_playing;
        }
        match state.player.resume() {
            Ok(()) => true,
            Err(error) => {
                log::warn!("Player resume failed: {error}");
                false
            }
        }
    }
    /// Handle an OS media control event (MPRIS play/pause/next/etc.).
    /// Called from the timer loop inside a `state.update()` closure.
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    fn handle_media_control_event(state: &mut AppState, event: &MediaControlEvent) {
        match event {
            MediaControlEvent::Play => {
                if state.app.playback.current_queue_index.is_none() {
                    if let Some(path) = state.app.start_queue() {
                        Self::play_track(state, path);
                    }
                } else {
                    state.app.playback.is_playing = Self::resume_on_selected_output(state);
                }
            }
            MediaControlEvent::Pause => {
                if let Err(e) = state.player.pause() {
                    log::warn!("Player pause failed: {e}");
                }
                state.app.playback.is_playing = false;
            }
            MediaControlEvent::Toggle => {
                if state.app.playback.is_playing {
                    if let Err(e) = state.player.pause() {
                        log::warn!("Player pause failed: {e}");
                    }
                    state.app.playback.is_playing = false;
                } else if state.app.playback.current_queue_index.is_none() {
                    if let Some(path) = state.app.start_queue() {
                        Self::play_track(state, path);
                    }
                } else {
                    state.app.playback.is_playing = Self::resume_on_selected_output(state);
                }
            }
            MediaControlEvent::Next => {
                if let Some(path) = state.app.next_track() {
                    Self::play_track(state, path);
                } else {
                    state.app.playback.is_playing = false;
                }
            }
            MediaControlEvent::Previous => {
                if let Some(path) = state.app.previous_track() {
                    Self::play_track(state, path);
                }
            }
            MediaControlEvent::Stop => {
                if let Err(e) = state.player.stop() {
                    log::warn!("Player stop failed: {e}");
                }
                state.app.playback.is_playing = false;
                state.app.playback.current_queue_index = None;
            }
            MediaControlEvent::SetPosition(pos) => {
                if let Err(e) = state.player.seek(pos.0.as_secs_f64()) {
                    log::warn!("Player seek failed: {e}");
                }
            }
            MediaControlEvent::SetVolume(vol) => {
                let clamped = vol.clamp(0.0, 1.0) as f32;
                state.app.playback.volume = clamped;
                if let Err(e) = state.player.set_volume(clamped) {
                    log::warn!("Player set_volume failed: {e}");
                }
            }
            MediaControlEvent::Seek(direction) => {
                let offset = match direction {
                    sotf_media_controls::SeekDirection::Forward => 10.0,
                    sotf_media_controls::SeekDirection::Backward => -10.0,
                };
                let new_pos = (state.app.playback.position_secs + offset).max(0.0);
                if let Err(e) = state.player.seek(new_pos) {
                    log::warn!("Player seek failed: {e}");
                }
            }
            MediaControlEvent::SeekBy(direction, duration) => {
                let secs = duration.as_secs_f64();
                let offset = match direction {
                    sotf_media_controls::SeekDirection::Forward => secs,
                    sotf_media_controls::SeekDirection::Backward => -secs,
                };
                let new_pos = (state.app.playback.position_secs + offset).max(0.0);
                if let Err(e) = state.player.seek(new_pos) {
                    log::warn!("Player seek failed: {e}");
                }
            }
            MediaControlEvent::Raise | MediaControlEvent::OpenUri(_) => {}
            MediaControlEvent::Quit => {
                std::process::exit(0);
            }
        }
    }

    pub(crate) fn toggle_playback(
        &mut self,
        _: &PlayPause,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            #[cfg(all(target_os = "macos", feature = "hal"))]
            if matches!(
                state.app.audio_device_state.playback_source,
                crate::app::types::PlaybackSource::HalDevice
            ) {
                if state.app.playback.is_playing {
                    match state.player.stop() {
                        Ok(()) => state.app.playback.is_playing = false,
                        Err(error) => {
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::error(error.to_string()));
                        }
                    }
                } else {
                    Self::start_hal_playback_state(state);
                }
                return;
            }
            if state.app.playback.is_playing {
                if let Err(e) = state.player.pause() {
                    log::warn!("Player pause failed: {e}");
                }
                state.app.playback.is_playing = false;
                state.app.record_playback_paused();
            } else if state.app.playback.current_queue_index.is_none() {
                // No track loaded — start the queue from the beginning
                if let Some(path) = state.app.start_queue() {
                    Self::play_track(state, path);
                }
            } else {
                state.app.playback.is_playing = Self::resume_on_selected_output(state);
                if state.app.playback.is_playing {
                    state.app.record_playback_resumed();
                }
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if let Err(e) = state.player.stop() {
                log::warn!("Player stop failed: {e}");
            }
            state.app.playback.is_playing = false;
            state.app.playback.current_queue_index = None;
            state.app.record_playback_stopped();
        });
        cx.notify();
    }

    pub(crate) fn next_track(&mut self, _: &NextTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.ui_state.input_mode) {
                return;
            }
            // Cancel any pending gapless queue before manual skip
            if let Err(e) = state.player.cancel_next() {
                log::warn!("Player cancel_next failed: {e}");
            }
            let from_index = state.app.playback.current_queue_index;
            if let Some(path) = state.app.next_track() {
                Self::play_track(state, path);

                if let Some(to_index) = state.app.playback.current_queue_index {
                    state.app.record_track_changed(
                        from_index,
                        to_index,
                        crate::app::state::TrackChangeTrigger::NextTrack,
                    );
                }
            } else {
                state.app.playback.is_playing = false;
            }
        });
        cx.notify();
    }

    pub(crate) fn prev_track(&mut self, _: &PrevTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.ui_state.input_mode) {
                return;
            }
            // Cancel any pending gapless queue before manual skip
            if let Err(e) = state.player.cancel_next() {
                log::warn!("Player cancel_next failed: {e}");
            }
            let from_index = state.app.playback.current_queue_index;
            if let Some(path) = state.app.previous_track() {
                Self::play_track(state, path);

                if let Some(to_index) = state.app.playback.current_queue_index {
                    state.app.record_track_changed(
                        from_index,
                        to_index,
                        crate::app::state::TrackChangeTrigger::PrevTrack,
                    );
                }
            } else {
                state.app.playback.is_playing = false;
            }
        });
        cx.notify();
    }
}
