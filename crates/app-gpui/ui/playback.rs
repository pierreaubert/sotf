impl PlayerView {
    pub(crate) fn toggle_playback(
        &mut self,
        _: &PlayPause,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            if state.app.playback.is_playing {
                let _ = state.player.lock().pause();
                state.app.playback.is_playing = false;
                state.app.record_playback_paused();
            } else {
                let _ = state.player.lock().resume();
                state.app.playback.is_playing = true;
                state.app.record_playback_resumed();
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let _ = state.player.lock().stop();
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
