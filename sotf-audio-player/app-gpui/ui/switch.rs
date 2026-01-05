impl PlayerView {
    pub(crate) fn switch_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen != screen {
                state.app.last_screen = state.app.current_screen;
                state.app.current_screen = screen;
            }
        });
        cx.notify();
    }

    fn switch_to_library(&mut self, _: &SwitchToLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Library, cx);
    }

    fn switch_to_queue(&mut self, _: &SwitchToQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Queue, cx);
    }

    fn switch_to_plugins(&mut self, _: &SwitchToPlugins, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = Screen::Settings;
            state.app.active_settings_tab = crate::app::SettingsTab::AudioDevice;
        });
        cx.notify();
    }

    fn switch_to_studio(&mut self, _: &SwitchToStudio, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Studio, cx);
    }

    fn switch_to_plugin_graph(
        &mut self,
        _: &SwitchToPluginGraph,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::PluginGraph, cx);
    }

    fn switch_to_spinorama(
        &mut self,
        _: &SwitchToSpinorama,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::Spinorama, cx);
    }

    fn switch_to_devices(&mut self, _: &SwitchToDevices, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = Screen::Settings;
            state.app.active_settings_tab = crate::app::SettingsTab::AudioDevice;
        });
        cx.notify();
    }

    fn switch_to_settings(&mut self, _: &SwitchToSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Settings, cx);
    }

    fn switch_to_recording(
        &mut self,
        _: &SwitchToRecording,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::Recording, cx);
    }

    fn switch_to_room_eq(&mut self, _: &SwitchToRoomEQ, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::RoomEq, cx);
    }

    fn switch_to_headphone_eq(
        &mut self,
        _: &SwitchToHeadphoneEQ,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::HeadphoneEq, cx);
    }
}
