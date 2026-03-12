use super::app_impl::App;
use super::types::{ChannelGroup, ChannelInfo, PendingParameterUpdate};
use sotf_plugins::speaker_config::{
    MeterGroupSpec, get_meter_groups, get_meter_groups_by_channels, make_fallback_channel,
};

impl App {
    /// Build channel groups from current speaker configuration or channel count
    /// Uses caching to avoid rebuilding every frame
    pub fn update_level_meter_groups(&mut self) {
        let num_channels = self
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        if num_channels == 0 {
            return;
        }

        // Get current speaker config
        let current_speaker_config = self.plugin_chain.output_speaker_config().map(String::from);

        // Skip rebuilding if nothing has changed
        if num_channels == self.level_meter_last_channel_count
            && current_speaker_config == self.level_meter_last_speaker_config
            && !self.level_meter_groups.is_empty()
        {
            return;
        }

        // Update cache
        self.level_meter_last_channel_count = num_channels;
        self.level_meter_last_speaker_config = current_speaker_config.clone();

        self.level_meter_groups.clear();

        // Try to get meter groups from the speaker config (via upmixer plugin)
        // This handles collisions like 5.1.4 vs 7.1.2 (both 10 channels)
        let meter_groups: Option<&[MeterGroupSpec]> = current_speaker_config
            .as_deref()
            .and_then(get_meter_groups)
            .or_else(|| get_meter_groups_by_channels(num_channels));

        if let Some(groups) = meter_groups {
            // Convert static specs to runtime groups
            for group_spec in groups {
                self.level_meter_groups.push(ChannelGroup {
                    name: group_spec.name.to_string(),
                    channels: group_spec
                        .channels
                        .iter()
                        .map(|ch| ChannelInfo {
                            index: ch.index,
                            name: ch.label.to_string(),
                            display_name: ch
                                .display_chars
                                .iter()
                                .map(|s| (*s).to_string())
                                .collect(),
                        })
                        .collect(),
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        } else {
            // Fallback for unknown channel counts (mono, quad, or exotic configs)
            match num_channels {
                1 => {
                    // Mono
                    self.level_meter_groups.push(ChannelGroup {
                        name: "Mono".to_string(),
                        channels: vec![ChannelInfo {
                            index: 0,
                            name: "M".to_string(),
                            display_name: vec!["M".to_string()],
                        }],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                4 => {
                    // Quad (FL, FR, SL, SR) - not a standard speaker config
                    self.level_meter_groups.push(ChannelGroup {
                        name: "L/R".to_string(),
                        channels: vec![
                            ChannelInfo {
                                index: 0,
                                name: "L".to_string(),
                                display_name: vec!["L".to_string()],
                            },
                            ChannelInfo {
                                index: 1,
                                name: "R".to_string(),
                                display_name: vec!["R".to_string()],
                            },
                        ],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                    self.level_meter_groups.push(ChannelGroup {
                        name: "Surrounds".to_string(),
                        channels: vec![
                            ChannelInfo {
                                index: 2,
                                name: "SL".to_string(),
                                display_name: vec!["S".to_string(), "L".to_string()],
                            },
                            ChannelInfo {
                                index: 3,
                                name: "SR".to_string(),
                                display_name: vec!["S".to_string(), "R".to_string()],
                            },
                        ],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                _ => {
                    // Generic fallback - treat all channels as one group
                    let channels: Vec<ChannelInfo> = (0..num_channels)
                        .map(|i| {
                            let spec = make_fallback_channel(i);
                            ChannelInfo {
                                index: spec.index,
                                name: spec.label.to_string(),
                                display_name: spec
                                    .display_chars
                                    .iter()
                                    .map(|s| (*s).to_string())
                                    .collect(),
                            }
                        })
                        .collect();
                    self.level_meter_groups.push(ChannelGroup {
                        name: "All Channels".to_string(),
                        channels,
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
            }
        }

        // Update Matrix plugin channel states for M/S/D controls
        self.update_matrix_channel_states();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    pub fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meter_groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_matrix_channel_states();
    }

    /// Toggle mute for the selected level meter group
    pub fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.muted = !group.muted;
            self.update_matrix_channel_states();
        }
    }

    /// Toggle solo for the selected level meter group
    pub fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            let is_currently_soloed = group.soloed;

            // Solo behavior: only one group can be soloed at a time
            // When soloing, set soloed=true on selected group, soloed=false on all others
            // When un-soloing, set soloed=false on selected group
            for (idx, g) in self.level_meter_groups.iter_mut().enumerate() {
                if idx == self.selected_level_meter_group {
                    g.soloed = !is_currently_soloed;
                } else {
                    g.soloed = false;
                }
            }

            self.update_matrix_channel_states();
        }
    }

    /// Toggle dim for the selected level meter group
    pub fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.dimmed = !group.dimmed;
            self.update_matrix_channel_states();
        }
    }

    /// Update the Matrix plugin's channel states based on current level meter group M/S/D
    fn update_matrix_channel_states(&mut self) {
        use sotf_audio_player::PluginSettings;
        use sotf_plugins::ChannelState;

        // Calculate total channel count
        let num_channels = self
            .level_meter_groups
            .iter()
            .map(|g| g.channels.len())
            .sum();

        if num_channels == 0 {
            return;
        }

        // Build per-channel states from groups
        let mut channel_states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false
            };
            num_channels
        ];

        for group in &self.level_meter_groups {
            for channel_info in &group.channels {
                if channel_info.index < num_channels {
                    channel_states[channel_info.index] = ChannelState {
                        muted: group.muted,
                        soloed: group.soloed,
                        dimmed: group.dimmed,
                    };
                }
            }
        }

        // Find and update the permanent Matrix plugin's channel_states in memory
        for i in 0..self.plugin_chain.len() {
            if let Some(plugin) = self.plugin_chain.get_plugin_mut(i)
                && plugin.is_permanent()
                && matches!(&plugin.settings, PluginSettings::Matrix { .. })
            {
                if let PluginSettings::Matrix {
                    channel_states: ref mut cs,
                    ..
                } = plugin.settings
                {
                    *cs = channel_states.clone();
                }
                break;
            }
        }

        // Queue zero-dropout parameter update via matrix_engine_index
        if let Some(engine_index) = self.plugin_chain.matrix_engine_index()
            && let Ok(json) = serde_json::to_string(&channel_states)
        {
            self.pending_param_update = Some(PendingParameterUpdate {
                plugin_index: engine_index,
                param_id: "channel_states".to_string(),
                value: json,
            });
        }
    }

    /// Navigate to next level meter group
    pub fn select_next_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            self.selected_level_meter_group =
                (self.selected_level_meter_group + 1) % self.level_meter_groups.len();
        }
    }

    /// Navigate to previous level meter group
    pub fn select_previous_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            if self.selected_level_meter_group == 0 {
                self.selected_level_meter_group = self.level_meter_groups.len() - 1;
            } else {
                self.selected_level_meter_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    pub fn select_next_level_meter_control(&mut self) {
        self.level_meter_control_selection = (self.level_meter_control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    pub fn select_previous_level_meter_control(&mut self) {
        self.level_meter_control_selection = if self.level_meter_control_selection == 0 {
            2
        } else {
            self.level_meter_control_selection - 1
        };
    }
}
