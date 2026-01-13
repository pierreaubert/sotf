impl PlayerView {
    fn adjust_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.playback.volume = (state.app.playback.volume + delta).clamp(0.0, 1.0);
            let _ = state.player.lock().set_volume(state.app.playback.volume);
        });
        cx.notify();
    }

    fn volume_up(&mut self, _: &VolumeUp, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(0.05, cx);
    }

    fn volume_down(&mut self, _: &VolumeDown, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(-0.05, cx);
    }

    fn volume_up_small(&mut self, _: &VolumeUpSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(0.01, cx);
    }

    fn volume_down_small(&mut self, _: &VolumeDownSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(-0.01, cx);
    }

    fn volume_up_large(&mut self, _: &VolumeUpLarge, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(0.10, cx);
    }

    fn volume_down_large(&mut self, _: &VolumeDownLarge, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(-0.10, cx);
    }

    fn volume_max(&mut self, _: &VolumeMax, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.playback.volume = 1.0;
            let _ = state.player.lock().set_volume(1.0);
        });
        cx.notify();
    }

    fn volume_min(&mut self, _: &VolumeMin, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.playback.volume = 0.0;
            let _ = state.player.lock().set_volume(0.0);
        });
        cx.notify();
    }

    fn toggle_mute(&mut self, _: &ToggleMute, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.playback.muted = !state.app.playback.muted;
            // When muted, set volume to 0; restore when unmuted
            let effective_volume = if state.app.playback.muted {
                0.0
            } else {
                state.app.playback.volume
            };
            let _ = state.player.lock().set_volume(effective_volume);
        });
        cx.notify();
    }


}
