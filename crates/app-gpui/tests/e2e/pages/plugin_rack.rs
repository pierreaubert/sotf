use crate::driver::AppDriver;
#[allow(unused_imports)]
use gpui::*;
use sotf_audio::plugins::{PluginSettings, PluginType};
use sotf_audio_player_gpui::components::plugins::editing::PluginEditingManager;

pub struct PluginRackPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> PluginRackPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    /// Add a plugin to the rack by index or type (simulating Add Plugin action)
    /// Since we don't have a direct "Add specific plugin" action exposed easily without UI,
    /// we might need to manipulate state or trace the UI path.
    /// However, `AddPlugin` action exists?
    /// Let's check `actions.rs` in `player/app-gpui`.
    ///
    /// For now, we will use direct state manipulation to add plugins as it's cleaner for E2E logic
    /// unless we explicitly want to test the *menu*.
    pub fn add_plugin(&mut self, plugin_type: PluginType) -> usize {
        let max_id_opt = self
            .driver
            .read_app(|app| app.plugin_state.chain.plugins().iter().map(|p| p.id).max());

        self.driver.update_app(move |app, _cx| {
            app.add_plugin(&plugin_type);
        });

        self.driver.read_app(move |app| {
            app.plugin_state
                .chain
                .plugins()
                .iter()
                .map(|p| p.id)
                .find(|&id| max_id_opt.is_none_or(|max| id > max))
                .expect("Should have at least one plugin now")
        })
    }

    pub fn get_plugin_count(&mut self) -> usize {
        self.driver.read_app(|app| app.plugin_state.chain.len())
    }

    pub fn get_plugin_type(&mut self, index: usize) -> Option<PluginType> {
        self.driver.read_app(move |app| {
            app.plugin_state
                .chain
                .get_plugin(index)
                .map(|p| p.plugin_type())
        })
    }

    pub fn is_plugin_enabled(&mut self, index: usize) -> bool {
        self.driver.read_app(move |app| {
            app.plugin_state
                .chain
                .get_plugin(index)
                .map(|p| p.enabled)
                .unwrap_or(false)
        })
    }

    pub fn is_plugin_permanent(&mut self, index: usize) -> bool {
        self.driver.read_app(move |app| {
            app.plugin_state
                .chain
                .get_plugin(index)
                .map(|p| p.permanent)
                .unwrap_or(false)
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
        self.driver
            .read_app(|app| app.plugin_state.chain.output_channels())
    }

    pub fn find_plugin_index_by_id(&mut self, id: usize) -> Option<usize> {
        self.driver.read_app(move |app| {
            app.plugin_state
                .chain
                .plugins()
                .iter()
                .position(|p| p.id == id)
        })
    }

    pub fn plugin_exists(&mut self, id: usize) -> bool {
        self.driver
            .read_app(move |app| app.plugin_state.chain.plugins().iter().any(|p| p.id == id))
    }

    pub fn get_eq_channels(&mut self, index: usize) -> usize {
        self.driver.read_app(move |app| {
            if let Some(plugin) = app.plugin_state.chain.get_plugin(index) {
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
    pub fn get_matrix_channels(&mut self, index: usize) -> (usize, usize) {
        self.driver.read_app(move |app| {
            if let Some(plugin) = app.plugin_state.chain.get_plugin(index) {
                match &plugin.settings {
                    PluginSettings::Matrix {
                        input_channels,
                        output_channels,
                        ..
                    } => (*input_channels, *output_channels),
                    _ => (0, 0),
                }
            } else {
                (0, 0)
            }
        })
    }

    pub fn has_spectrum_info(&mut self) -> bool {
        self.driver
            .read_app(|app| app.playback.spectrum_info.is_some())
    }

    pub fn get_spectrum_magnitudes(&mut self) -> Vec<f32> {
        self.driver.read_app(|app| {
            app.playback
                .spectrum_info
                .as_ref()
                .map(|info| info.magnitudes.iter().cloned().collect())
                .unwrap_or_default()
        })
    }

    pub fn inject_test_track(&mut self) {
        self.driver.update_app(|app, _cx| {
            let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop(); // crates/
            path.pop(); // root/
            path.push("data_generated/test-audio/wav/white_noise/white_noise_ch2_sr44100_b16.wav");

            let track = sotf_audio_player::library::Track {
                path,
                title: Some("Test Track".to_string()),
                artist: Some("Test Artist".to_string()),
                track_number: Some(1),
                duration_secs: Some(180),
                channels: Some(2),
                sample_rate: Some(44100),
                bit_depth: Some(16),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: Some("Test Artist".to_string()),
                ensemble: None,
                edition: None,
                is_favorite: false,
                play_count: 0,
                source: None,
                uuid: None,
            };

            let album = sotf_audio_player::library::Album {
                id: None,
                title: "Test Album".to_string(),
                year: Some(2024),
                tracks: vec![track],
                album_art_path: None,
                album_art_thumbnail: None,
                play_count: 0,
                edition: None,
                dynamic_range: None,
                is_favorite: false,
                uuid: None,
            };

            app.queue
                .push(sotf_audio_player_gpui::app::types::QueueItem {
                    album,
                    current_track_index: 0,
                });
            app.selected_queue_index = 0;
        });
    }

    pub fn toggle_playback(&mut self) {
        use sotf_audio_player_gpui::app::actions::PlayPause;
        self.driver
            .view
            .update(self.driver.cx, |_, _, cx| {
                cx.dispatch_action(&PlayPause);
            })
            .unwrap();
    }

    /// Start playback from the currently selected queue item.
    /// Unlike toggle_playback (which only pauses/resumes), this actually
    /// loads and starts playing the track from the queue.
    pub fn start_playback_from_queue(&mut self) {
        self.driver
            .view
            .update(self.driver.cx, |view, _, cx| {
                view.state.update(cx, |state, _cx| {
                    // Get the source of the track to play
                    if let Some(source) = state.app.play_selected_queue_item() {
                        let sample_rate = 48000.0;
                        let plugins = state.app.plugin_state.chain.to_plugin_configs(sample_rate);
                        let output_channels = state.app.plugin_state.chain.output_channels();

                        if let Err(e) = state.player.lock().load_and_play_source(
                            source,
                            plugins,
                            output_channels,
                            state
                                .app
                                .audio_device_state
                                .current_output_device_name
                                .clone(),
                        ) {
                            log::error!("Failed to start playback: {}", e);
                            state.app.playback.is_playing = false;
                        } else {
                            state.app.playback.is_playing = true;
                        }
                    }
                });
                cx.notify();
            })
            .unwrap();
    }

    pub fn wait_for_spectrum(&mut self, duration: std::time::Duration) {
        let step = std::time::Duration::from_millis(100);
        let steps = (duration.as_millis() / step.as_millis()).max(1) as usize;

        for _ in 0..steps {
            self.driver.cx.executor().advance_clock(step);
            self.driver.run_until_parked();
            if self.has_spectrum_info() {
                return;
            }
        }
    }

    pub fn wait(&mut self, duration: std::time::Duration) {
        self.driver.cx.executor().advance_clock(duration);
        self.driver.run_until_parked();
    }

    pub fn run_until_parked(&mut self) {
        self.driver.run_until_parked();
    }

    /// Check if playback is currently playing
    pub fn is_playing(&mut self) -> bool {
        self.driver.read_app(|app| app.playback.is_playing)
    }
}
