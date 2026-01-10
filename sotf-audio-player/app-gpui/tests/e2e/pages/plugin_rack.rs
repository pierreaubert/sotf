use crate::driver::AppDriver;
use gpui::*;
use sotf_audio::plugins::{PluginType, PluginSettings};
use sotf_audio_player_gpui::components::plugins::editing::PluginEditingManager;

pub struct PluginRackPage<'a> {
    driver: &'a mut AppDriver<'a>,
}

impl<'a> PluginRackPage<'a> {
    pub fn new(driver: &'a mut AppDriver<'a>) -> Self {
        Self { driver }
    }

    /// Add a plugin to the rack by index or type (simulating Add Plugin action)
    /// Since we don't have a direct "Add specific plugin" action exposed easily without UI,
    /// we might need to manipulate state or trace the UI path.
    /// However, `AddPlugin` action exists?
    /// Let's check `actions.rs` in `sotf-audio-player/app-gpui`.
    /// 
    /// For now, we will use direct state manipulation to add plugins as it's cleaner for E2E logic 
    /// unless we explicitly want to test the *menu*.
    pub fn add_plugin(&mut self, plugin_type: PluginType) -> usize {
        let max_id_opt = self.driver.read_app(|app| {
             app.plugin_state.plugin_chain.plugins().iter().map(|p| p.id).max()
        });
        
        self.driver.update_app(move |app, _cx| {
            app.add_plugin(&plugin_type);
        });
        
        let new_id = self.driver.read_app(move |app| {
             app.plugin_state.plugin_chain.plugins().iter()
                 .map(|p| p.id)
                 .find(|&id| max_id_opt.map_or(true, |max| id > max))
                 .expect("Should have at least one plugin now")
        });
        
        new_id
    }
    
    pub fn get_plugin_count(&mut self) -> usize {
        self.driver.read_app(|app| app.plugin_state.plugin_chain.len())
    }

    pub fn get_plugin_type(&mut self, index: usize) -> Option<PluginType> {
        self.driver.read_app(move |app| {
             app.plugin_state.plugin_chain.get_plugin(index).map(|p| p.plugin_type())
        })
    }

    pub fn is_plugin_enabled(&mut self, index: usize) -> bool {
        self.driver.read_app(move |app| {
            app.plugin_state.plugin_chain.get_plugin(index).map(|p| p.enabled).unwrap_or(false)
        })
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.driver.update_app(move |app, _cx| {
            app.toggle_plugin(index);
        });
    }

    pub fn remove_plugin(&mut self, index: usize) {
        self.driver.update_app(move |app, _cx| {
            app.remove_plugin(index);
        });
    }

    pub fn get_output_channels(&mut self) -> usize {
        self.driver.read_app(|app| {
            app.plugin_state.plugin_chain.output_channels()
        })
    }

    pub fn find_plugin_index_by_id(&mut self, id: usize) -> Option<usize> {
        self.driver.read_app(move |app| {
            app.plugin_state.plugin_chain.plugins().iter().position(|p| p.id == id)
        })
    }
    
    pub fn plugin_exists(&mut self, id: usize) -> bool {
        self.driver.read_app(move |app| {
            app.plugin_state.plugin_chain.plugins().iter().any(|p| p.id == id)
        })
    }
    
    pub fn get_eq_channels(&mut self, index: usize) -> usize {
        self.driver.read_app(move |app| {
            if let Some(plugin) = app.plugin_state.plugin_chain.get_plugin(index) {
                match &plugin.settings {
                    PluginSettings::EQ { channels, .. } => *channels,
                    _ => 0,
                }
            } else {
                0
            }
        })
    }
    
    pub fn select_plugin(&mut self, index: usize) {
        self.driver.update_app(move |app, _cx| {
            app.plugin_state.selected_plugin_index = index;
        });
    }
}
