impl PlayerView {
    fn adjust_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.volume = (state.app.volume + delta).clamp(0.0, 1.0);
            let _ = state.player.lock().set_volume(state.app.volume);
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


}
