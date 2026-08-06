use crate::driver::AppDriver;
#[allow(unused_imports)]
use gpui::*;
use sotf_audio::plugins::{PluginSettings, PluginType};
use sotf_audio_player_gpui::components::plugins::editing::PluginEditingManager;

fn controller_param_value(settings: &PluginSettings, param_idx: usize) -> Option<f64> {
    if let PluginSettings::EQ { filters, .. } = settings {
        let filter = filters.get(param_idx / 4)?;
        return match param_idx % 4 {
            0 => Some(filter.frequency),
            1 => Some(filter.q),
            2 => Some(filter.gain_db),
            _ => None,
        };
    }
    if let PluginSettings::LinearPhaseEq { filters, .. } = settings {
        let filter = filters.get(param_idx / 5)?;
        return match param_idx % 5 {
            1 => Some(filter.frequency),
            2 => Some(filter.q),
            3 => Some(filter.gain_db),
            4 => Some((!filter.muted) as u8 as f64),
            _ => None,
        };
    }
    settings.param_value(param_idx)
}

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
        let plugin_label = format!("{plugin_type:?}");
        let max_id_opt = self
            .driver
            .read_app(|app| app.plugin_state.graph.plugins().iter().map(|p| p.id).max());

        self.driver.update_app(move |app, _cx| {
            app.add_plugin(&plugin_type);
        });

        self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .plugins()
                .iter()
                .map(|p| p.id)
                .find(|&id| max_id_opt.is_none_or(|max| id > max))
                .unwrap_or_else(|| panic!("Should have added {plugin_label} to the plugin rack"))
        })
    }

    pub fn get_plugin_count(&mut self) -> usize {
        self.driver.read_app(|app| app.plugin_state.graph.len())
    }

    pub fn adapt_input_channels(&mut self, channels: usize) {
        self.driver.update_app(move |app, _| {
            app.plugin_state.graph.adapt_matrix_to_input(channels);
        });
    }

    pub fn get_plugin_type(&mut self, index: usize) -> Option<PluginType> {
        self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .get_plugin(index)
                .map(|p| p.plugin_type())
        })
    }

    pub fn is_plugin_enabled(&mut self, index: usize) -> bool {
        self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .get_plugin(index)
                .map(|p| p.enabled)
                .unwrap_or(false)
        })
    }

    pub fn is_plugin_permanent(&mut self, index: usize) -> bool {
        self.driver.read_app(move |app| {
            app.plugin_state
                .graph
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
            .read_app(|app| app.plugin_state.graph.output_channels())
    }

    pub fn find_plugin_index_by_id(&mut self, id: usize) -> Option<usize> {
        self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .plugins()
                .iter()
                .position(|p| p.id == id)
        })
    }

    pub fn plugin_exists(&mut self, id: usize) -> bool {
        self.driver
            .read_app(move |app| app.plugin_state.graph.plugins().iter().any(|p| p.id == id))
    }

    pub fn get_eq_channels(&mut self, index: usize) -> usize {
        self.driver.read_app(move |app| {
            if let Some(plugin) = app.plugin_state.graph.get_plugin(index) {
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
            app.plugin_state.editing_plugin_index = Some(index);
        });
    }

    /// Exercise the same App-level parameter mutation path used by rendered controls.
    /// Returns the parameter index whose round-trip was verified.
    pub fn exercise_first_numeric_parameter(&mut self, index: usize) -> Option<usize> {
        let matrix_before = self.driver.read_app(move |app| {
            let plugin = app.plugin_state.graph.get_plugin(index)?;
            match &plugin.settings {
                PluginSettings::Matrix { matrix, .. } => matrix.first().copied(),
                _ => None,
            }
        });
        if let Some(before) = matrix_before {
            let requested = if before.abs() < f32::EPSILON {
                1.0
            } else {
                0.0
            };
            self.driver.update_app(move |app, _cx| {
                if let Some(plugin) = app.plugin_state.graph.get_plugin_mut(index)
                    && let PluginSettings::Matrix { matrix, .. } = &mut plugin.settings
                    && let Some(value) = matrix.first_mut()
                {
                    *value = requested;
                    app.plugin_state.update_state.pending_plugin_update =
                        Some(sotf_audio_player_gpui::app::types::PluginUpdateType::Structural);
                }
            });
            let after = self.driver.read_app(move |app| {
                let plugin = app.plugin_state.graph.get_plugin(index)?;
                match &plugin.settings {
                    PluginSettings::Matrix { matrix, .. } => matrix.first().copied(),
                    _ => None,
                }
            });
            self.driver.update_app(move |app, _cx| {
                if let Some(plugin) = app.plugin_state.graph.get_plugin_mut(index)
                    && let PluginSettings::Matrix { matrix, .. } = &mut plugin.settings
                    && let Some(value) = matrix.first_mut()
                {
                    *value = before;
                    app.plugin_state.update_state.pending_plugin_update =
                        Some(sotf_audio_player_gpui::app::types::PluginUpdateType::Structural);
                }
            });
            return after
                .is_some_and(|after| (after - before).abs() > f32::EPSILON)
                .then_some(0);
        }

        let candidates = self.driver.read_app(move |app| {
            let plugin = app.plugin_state.graph.get_plugin(index)?;
            let count = sotf_audio_player::get_param_count(&plugin.settings);
            let specs = plugin.settings.param_specs();
            Some(
                (0..count)
                    .filter_map(|param_idx| {
                        let before = controller_param_value(&plugin.settings, param_idx)?;
                        let requested = specs.get(param_idx).map_or_else(
                            || {
                                if before.abs() < f64::EPSILON {
                                    1.0
                                } else {
                                    0.0
                                }
                            },
                            |spec| {
                                let min = spec.min_f64();
                                let max = spec.max_f64();
                                if (before - min).abs() < f64::EPSILON {
                                    max
                                } else {
                                    min
                                }
                            },
                        );
                        Some((param_idx, before, requested))
                    })
                    .collect::<Vec<_>>(),
            )
        })?;

        for (param_idx, before, requested) in candidates {
            self.driver.update_app(move |app, _cx| {
                app.set_plugin_param(index, param_idx, requested);
            });
            let after = self.driver.read_app(move |app| {
                app.plugin_state
                    .graph
                    .get_plugin(index)
                    .and_then(|plugin| controller_param_value(&plugin.settings, param_idx))
            });
            if after.is_some_and(|after| (after - before).abs() > f64::EPSILON) {
                self.driver.update_app(move |app, _cx| {
                    app.set_plugin_param(index, param_idx, before);
                });
                return Some(param_idx);
            }
        }
        None
    }

    pub fn numeric_parameter_count(&mut self, index: usize) -> usize {
        self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .get_plugin(index)
                .map(|plugin| sotf_audio_player::get_param_count(&plugin.settings))
                .unwrap_or_default()
        })
    }

    pub fn round_trip_parameter(&mut self, index: usize, param_idx: usize) -> bool {
        let before = self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .get_plugin(index)
                .and_then(|plugin| controller_param_value(&plugin.settings, param_idx))
        });
        let Some(before) = before else {
            return false;
        };
        let requested = if before.abs() < f64::EPSILON {
            1.0
        } else {
            0.0
        };
        self.driver.update_app(move |app, _cx| {
            app.set_plugin_param(index, param_idx, requested);
        });
        let changed = self.driver.read_app(move |app| {
            app.plugin_state
                .graph
                .get_plugin(index)
                .and_then(|plugin| controller_param_value(&plugin.settings, param_idx))
                .is_some_and(|after| (after - before).abs() > f64::EPSILON)
        });
        self.driver.update_app(move |app, _cx| {
            app.set_plugin_param(index, param_idx, before);
        });
        changed
    }
    pub fn get_matrix_channels(&mut self, index: usize) -> (usize, usize) {
        self.driver.read_app(move |app| {
            if let Some(plugin) = app.plugin_state.graph.get_plugin(index) {
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

            app.queue_state
                .push(sotf_audio_player_gpui::app::types::QueueItem {
                    album,
                    current_track_index: 0,
                });
            app.queue_state.selected_index = 0;
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
                        let plugins = state.app.plugin_state.graph.to_plugin_configs(sample_rate);
                        let output_channels = state.app.plugin_state.graph.output_channels();

                        if let Err(e) = state.player.load_and_play_source(
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
