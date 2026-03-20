impl PlayerView {
    pub(crate) fn switch_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(screen, "action", cx);
    }

    pub(crate) fn switch_screen_with_trigger(
        &mut self,
        screen: Screen,
        trigger: &str,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.set_screen(screen, trigger);
        });
        cx.notify();
    }

    fn switch_to_library(&mut self, _: &SwitchToLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::Library, "SwitchToLibrary", cx);
    }

    fn switch_to_queue(&mut self, _: &SwitchToQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::Queue, "SwitchToQueue", cx);
    }

    fn switch_to_plugins(&mut self, _: &SwitchToPlugins, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_screen(Screen::Settings, "SwitchToPlugins");
            state.app.ui_state.active_settings_tab = crate::app::SettingsTab::Misc;
        });
        cx.notify();
    }

    fn switch_to_studio(&mut self, _: &SwitchToStudio, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::Studio, "SwitchToStudio", cx);
    }

    fn switch_to_plugin_graph(
        &mut self,
        _: &SwitchToPluginGraph,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::PluginGraph, "SwitchToPluginGraph", cx);
    }

    fn switch_to_spinorama(
        &mut self,
        _: &SwitchToSpinorama,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Spinorama, "SwitchToSpinorama", cx);
    }

    fn switch_to_devices(&mut self, _: &SwitchToDevices, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_screen(Screen::Settings, "SwitchToDevices");
            state.app.ui_state.active_settings_tab = crate::app::SettingsTab::AudioDevice;
        });
        cx.notify();
    }

    fn switch_to_settings(&mut self, _: &SwitchToSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::Settings, "SwitchToSettings", cx);
    }

    fn switch_to_recording(
        &mut self,
        _: &SwitchToRecording,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Recording, "SwitchToRecording", cx);
    }

    fn switch_to_room_eq(&mut self, _: &SwitchToRoomEQ, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::RoomEq, "SwitchToRoomEQ", cx);
    }

    fn switch_to_headphone_eq(
        &mut self,
        _: &SwitchToHeadphoneEQ,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::HeadphoneEq, "SwitchToHeadphoneEQ", cx);
    }
}
