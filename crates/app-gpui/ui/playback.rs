#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
use souvlaki::MediaControlEvent;

impl PlayerView {
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
                    if let Err(e) = state.player.lock().resume() {
                        log::warn!("Player resume failed: {e}");
                    }
                    state.app.playback.is_playing = true;
                }
            }
            MediaControlEvent::Pause => {
                if let Err(e) = state.player.lock().pause() {
                    log::warn!("Player pause failed: {e}");
                }
                state.app.playback.is_playing = false;
            }
            MediaControlEvent::Toggle => {
                if state.app.playback.is_playing {
                    if let Err(e) = state.player.lock().pause() {
                        log::warn!("Player pause failed: {e}");
                    }
                    state.app.playback.is_playing = false;
                } else if state.app.playback.current_queue_index.is_none() {
                    if let Some(path) = state.app.start_queue() {
                        Self::play_track(state, path);
                    }
                } else {
                    if let Err(e) = state.player.lock().resume() {
                        log::warn!("Player resume failed: {e}");
                    }
                    state.app.playback.is_playing = true;
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
                if let Err(e) = state.player.lock().stop() {
                    log::warn!("Player stop failed: {e}");
                }
                state.app.playback.is_playing = false;
                state.app.playback.current_queue_index = None;
            }
            MediaControlEvent::SetPosition(pos) => {
                if let Err(e) = state.player.lock().seek(pos.0.as_secs_f64()) {
                    log::warn!("Player seek failed: {e}");
                }
            }
            MediaControlEvent::SetVolume(vol) => {
                let clamped = vol.clamp(0.0, 1.0) as f32;
                state.app.playback.volume = clamped;
                if let Err(e) = state.player.lock().set_volume(clamped) {
                    log::warn!("Player set_volume failed: {e}");
                }
            }
            MediaControlEvent::Seek(direction) => {
                let offset = match direction {
                    souvlaki::SeekDirection::Forward => 10.0,
                    souvlaki::SeekDirection::Backward => -10.0,
                };
                let new_pos = (state.app.playback.position_secs + offset).max(0.0);
                if let Err(e) = state.player.lock().seek(new_pos) {
                    log::warn!("Player seek failed: {e}");
                }
            }
            MediaControlEvent::SeekBy(direction, duration) => {
                let secs = duration.as_secs_f64();
                let offset = match direction {
                    souvlaki::SeekDirection::Forward => secs,
                    souvlaki::SeekDirection::Backward => -secs,
                };
                let new_pos = (state.app.playback.position_secs + offset).max(0.0);
                if let Err(e) = state.player.lock().seek(new_pos) {
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
            if state.app.playback.is_playing {
                if let Err(e) = state.player.lock().pause() {
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
                if let Err(e) = state.player.lock().resume() {
                    log::warn!("Player resume failed: {e}");
                }
                state.app.playback.is_playing = true;
                state.app.record_playback_resumed();
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if let Err(e) = state.player.lock().stop() {
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
            if let Err(e) = state.player.lock().cancel_next() {
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
            if let Err(e) = state.player.lock().cancel_next() {
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
