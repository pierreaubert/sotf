use crate::driver::AppDriver;
use sotf_audio_player::PluginSettings;
use sotf_audio_player_gpui::components::plugins::level_meters::LevelMeterManager;

pub struct LevelMeterPage<'a, 'b> {
    pub driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> LevelMeterPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn ensure_visible(&mut self) {
        // Navigate to Queue view where meters are typically visible
        self.driver
            .view
            .update(self.driver.cx, |_, _, cx| {
                cx.dispatch_action(&sotf_audio_player_gpui::app::actions::SwitchToQueue);
            })
            .unwrap();
        self.driver.run_until_parked();
    }

    pub fn toggle_mute(&mut self, group_idx: usize) {
        self.driver.update_app(|app, _| {
            // Mirror UI behavior: Read current state, invert, and set
            if let Some(group) = app.level_meter_groups.get(group_idx) {
                let new_state = !group.muted;
                app.set_level_meter_mute(group_idx, new_state);
            }
        });
        self.driver.run_until_parked();
    }

    pub fn toggle_solo(&mut self, group_idx: usize) {
        self.driver.update_app(|app, _| {
            if let Some(group) = app.level_meter_groups.get(group_idx) {
                let new_state = !group.soloed;
                app.set_level_meter_solo(group_idx, new_state);
            }
        });
        self.driver.run_until_parked();
    }

    pub fn toggle_dim(&mut self, group_idx: usize) {
        self.driver.update_app(|app, _| {
            if let Some(group) = app.level_meter_groups.get(group_idx) {
                let new_state = !group.dimmed;
                app.set_level_meter_dim(group_idx, new_state);
            }
        });
        self.driver.run_until_parked();
    }

    pub fn is_muted(&mut self, group_idx: usize) -> bool {
        self.driver.read_app(|app| {
            app.level_meter_groups
                .get(group_idx)
                .map(|g| g.muted)
                .unwrap_or(false)
        })
    }

    pub fn is_soloed(&mut self, group_idx: usize) -> bool {
        self.driver.read_app(|app| {
            app.level_meter_groups
                .get(group_idx)
                .map(|g| g.soloed)
                .unwrap_or(false)
        })
    }

    pub fn is_dimmed(&mut self, group_idx: usize) -> bool {
        self.driver.read_app(|app| {
            app.level_meter_groups
                .get(group_idx)
                .map(|g| g.dimmed)
                .unwrap_or(false)
        })
    }

    pub fn get_matrix_channel_mute_state(&mut self, channel_idx: usize) -> bool {
        self.driver.read_app(|app| {
            for plugin in app.plugin_state.chain.plugins() {
                if let PluginSettings::Matrix { channel_states, .. } = &plugin.settings {
                    return channel_states
                        .get(channel_idx)
                        .map(|s| s.muted)
                        .unwrap_or(false);
                }
            }
            false
        })
    }

    pub fn get_matrix_channel_solo_state(&mut self, channel_idx: usize) -> bool {
        self.driver.read_app(|app| {
            for plugin in app.plugin_state.chain.plugins() {
                if let PluginSettings::Matrix { channel_states, .. } = &plugin.settings {
                    return channel_states
                        .get(channel_idx)
                        .map(|s| s.soloed)
                        .unwrap_or(false);
                }
            }
            false
        })
    }
}
