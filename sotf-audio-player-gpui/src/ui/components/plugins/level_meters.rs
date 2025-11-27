//! Level meter methods.
//!
//! Contains methods for level meter display and control (mute/solo/dim).

use sotf_audio_player::PluginSettings;
use sotf_plugins::ChannelState;

use crate::app::{App, ChannelGroup, ChannelInfo};

impl App {
    /// Update level meter groups based on current channel count from loudness info
    /// Creates a default stereo layout when no audio is playing
    pub fn update_level_meter_groups(&mut self) {
        self.level_meter_groups.clear();

        let num_channels = self
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        // Default to stereo (2 channels) when no audio is playing
        // This ensures meters are always visible with -60 dB default
        let num_channels = if num_channels == 0 { 2 } else { num_channels };

        // Standard channel layouts based on channel count
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
            2 => {
                // Stereo (2.0) - L and R are separate groups
                self.level_meter_groups.push(ChannelGroup {
                    name: "Left".to_string(),
                    channels: vec![ChannelInfo {
                        index: 0,
                        name: "L".to_string(),
                        display_name: vec!["L".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Right".to_string(),
                    channels: vec![ChannelInfo {
                        index: 1,
                        name: "R".to_string(),
                        display_name: vec!["R".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            3 => {
                // 2.1 - L, R, and LFE are separate groups
                self.level_meter_groups.push(ChannelGroup {
                    name: "Left".to_string(),
                    channels: vec![ChannelInfo {
                        index: 0,
                        name: "L".to_string(),
                        display_name: vec!["L".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Right".to_string(),
                    channels: vec![ChannelInfo {
                        index: 1,
                        name: "R".to_string(),
                        display_name: vec!["R".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            4 => {
                // Quad (FL, FR, SL, SR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
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
            5 => {
                // 5.0 (FL, FR, FC, SL, SR) - Same as 5.1 without LFE
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 3,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 4,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            6 => {
                // 5.1 (FL, FR, FC, LFE, SL, SR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            8 => {
                // 7.1 (FL, FR, FC, LFE, SL, SR, BL, BR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            10 => {
                // 5.1.4 or 7.1.2 - Currently assuming 5.1.4
                // 5.1.4: FL, FR, FC, LFE, SL, SR, TFL, TFR, TBL, TBR
                // 7.1.2: FL, FR, FC, LFE, SL, SR, BL, BR, TML, TMR
                // TODO: Add configuration option to distinguish between these layouts
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 8,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            14 => {
                // 9.1.4 (FL, FR, FC, LFE, SL, SR, BL, BR, FWL, FWR, TFL, TFR, TBL, TBR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Sides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Backs".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Wides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 8,
                            name: "FWL".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "FWR".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 10,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 11,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 12,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 13,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            16 => {
                // 9.1.6 (FL, FR, FC, LFE, SL, SR, BL, BR, FWL, FWR, TFL, TFR, TML, TMR, TBL, TBR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Sides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Backs".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Wides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 8,
                            name: "FWL".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "FWR".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 10,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 11,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 12,
                            name: "TML".to_string(),
                            display_name: vec!["T".to_string(), "M".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 13,
                            name: "TMR".to_string(),
                            display_name: vec!["T".to_string(), "M".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 14,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 15,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            _ => {
                // Generic fallback - treat all channels as one group
                let mut channels = Vec::new();
                for i in 0..num_channels {
                    channels.push(ChannelInfo {
                        index: i,
                        name: format!("CH{}", i + 1),
                        display_name: vec![format!("CH{}", i + 1)],
                    });
                }
                self.level_meter_groups.push(ChannelGroup {
                    name: "All Channels".to_string(),
                    channels,
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        }

        // Update ChannelMuteSolo plugin to have correct number of channels
        self.update_channel_mute_solo_plugin();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    pub fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meter_groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_channel_mute_solo_plugin();
    }

    /// Toggle mute for the selected level meter group
    pub fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.muted = !group.muted;
            self.update_channel_mute_solo_plugin();
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

            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle dim for the selected level meter group
    pub fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.dimmed = !group.dimmed;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Update the ChannelMuteSolo plugin based on current level meter group states
    fn update_channel_mute_solo_plugin(&mut self) {
        // Calculate total channel count
        let num_channels: usize = self
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

        // Determine if any channel is muted, soloed, or dimmed
        let enabled = channel_states
            .iter()
            .any(|s| s.muted || s.soloed || s.dimmed);

        // Find and update the ChannelMuteSolo plugin
        for i in 0..self.plugin_chain.len() {
            if let Some(plugin) = self.plugin_chain.get_plugin_mut(i) {
                if matches!(&plugin.settings, PluginSettings::ChannelMuteSolo { .. }) {
                    // Update settings in memory
                    plugin.settings = PluginSettings::ChannelMuteSolo {
                        enabled,
                        channel_states: channel_states.clone(),
                    };
                    // Flag that plugins need updating
                    self.needs_plugin_update = true;
                    return;
                }
            }
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

    /// Toggle the currently selected level meter control (mute/solo/dim)
    pub fn toggle_selected_level_meter_control(&mut self) {
        match self.level_meter_control_selection {
            0 => self.toggle_level_meter_mute(),
            1 => self.toggle_level_meter_solo(),
            2 => self.toggle_level_meter_dim(),
            _ => {}
        }
    }
}
