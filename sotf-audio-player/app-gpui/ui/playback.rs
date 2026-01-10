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
            } else {
                let _ = state.player.lock().resume();
                state.app.playback.is_playing = true;
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let _ = state.player.lock().stop();
            state.app.playback.is_playing = false;
            state.app.playback.current_queue_index = None;
        });
        cx.notify();
    }

    pub(crate) fn next_track(&mut self, _: &NextTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.ui_state.input_mode) {
                return;
            }
            if let Some(path) = state.app.next_track() {
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_state.plugin_chain.to_plugin_configs(sample_rate);
                let output_channels = state.app.plugin_state.plugin_chain.output_channels();

                if let Err(e) = state.player.lock().load_and_play(
                    path,
                    plugins,
                    output_channels,
                    state.app.current_output_device_name.clone(),
                ) {
                    log::error!("Failed to play next track: {}", e);
                    state.app.playback.is_playing = false;
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
            if let Some(path) = state.app.previous_track() {
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_state.plugin_chain.to_plugin_configs(sample_rate);
                let output_channels = state.app.plugin_state.plugin_chain.output_channels();

                if let Err(e) = state.player.lock().load_and_play(
                    path,
                    plugins,
                    output_channels,
                    state.app.current_output_device_name.clone(),
                ) {
                    log::error!("Failed to play previous track: {}", e);
                    state.app.playback.is_playing = false;
                }
            } else {
                state.app.playback.is_playing = false;
            }
        });
        cx.notify();
    }

}
