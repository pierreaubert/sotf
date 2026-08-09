use crate::app::types::PluginUpdateType;
use crate::app::{App as AppState, ChannelGroup, ChannelInfo};
use sotf_audio_player::PluginSettings;
use sotf_plugins::ChannelState;
use sotf_plugins::speaker_config::{
    MeterGroupSpec, get_meter_groups, get_meter_groups_by_channels, make_fallback_channel,
};

const PEAK_HOLD_DECAY_RATE: f64 = 0.95;
const PEAK_HOLD_DECAY_THRESHOLD: f64 = 0.0001;

fn update_peak_hold_values(held: &mut Vec<f64>, current: &[f64], reduce_motion: bool) {
    held.resize(current.len(), 0.0);

    if reduce_motion {
        held.copy_from_slice(current);
        return;
    }

    for (held_peak, &current_peak) in held.iter_mut().zip(current) {
        if current_peak > *held_peak {
            *held_peak = current_peak;
        } else {
            *held_peak *= PEAK_HOLD_DECAY_RATE;
            if *held_peak < PEAK_HOLD_DECAY_THRESHOLD {
                *held_peak = 0.0;
            }
        }
    }
}

pub trait LevelMeterManager {
    fn update_level_meter_groups(&mut self);
    fn update_level_meter_peak_hold(&mut self);
    fn clear_level_meter_mutes_and_solos(&mut self);
    fn toggle_level_meter_mute(&mut self);
    fn toggle_level_meter_solo(&mut self);
    fn toggle_level_meter_dim(&mut self);
    fn set_level_meter_mute(&mut self, group_idx: usize, muted: bool);
    fn set_level_meter_solo(&mut self, group_idx: usize, soloed: bool);
    fn set_level_meter_dim(&mut self, group_idx: usize, dimmed: bool);
    fn select_next_level_meter_group(&mut self);
    fn select_previous_level_meter_group(&mut self);
    fn select_next_level_meter_control(&mut self);
    fn select_previous_level_meter_control(&mut self);
    fn toggle_selected_level_meter_control(&mut self);
    fn update_matrix_plugin(&mut self);
}

impl LevelMeterManager for AppState {
    /// Update level meter groups based on current speaker configuration or channel count
    /// Creates a default stereo layout when no audio is playing
    /// Uses caching to avoid rebuilding every frame
    fn update_level_meter_groups(&mut self) {
        let num_channels = self
            .playback
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        // Default to stereo (2 channels) when no audio is playing
        // This ensures meters are always visible with -60 dB default
        let num_channels = if num_channels == 0 { 2 } else { num_channels };

        // Get current speaker config
        let current_speaker_config = self.plugin_state.graph.output_speaker_config();

        // Skip rebuilding if nothing has changed
        if num_channels == self.level_meters.last_channel_count
            && current_speaker_config == self.level_meters.last_speaker_config
            && !self.level_meters.groups.is_empty()
        {
            return;
        }

        // Update cache
        self.level_meters.last_channel_count = num_channels;
        self.level_meters.last_speaker_config = current_speaker_config.clone();

        // Capture previous states to preserve them
        let old_groups: Vec<(String, bool, bool, bool)> = self
            .level_meters
            .groups
            .iter()
            .map(|g| (g.name.clone(), g.muted, g.soloed, g.dimmed))
            .collect();

        // Helper to find previous state
        let get_previous_state = |name: &str| -> (bool, bool, bool) {
            old_groups
                .iter()
                .find(|(n, _, _, _)| n == name)
                .map(|(_, m, s, d)| (*m, *s, *d))
                .unwrap_or((false, false, false))
        };

        self.level_meters.groups.clear();

        // Try to get meter groups from the speaker config (via upmixer plugin)
        // This handles collisions like 5.1.4 vs 7.1.2 (both 10 channels)
        let meter_groups: Option<&[MeterGroupSpec]> = current_speaker_config
            .as_deref()
            .and_then(get_meter_groups)
            .or_else(|| get_meter_groups_by_channels(num_channels));

        if let Some(groups) = meter_groups {
            // Convert static specs to runtime groups
            for group_spec in groups {
                let (muted, soloed, dimmed) = get_previous_state(group_spec.name);
                self.level_meters.groups.push(ChannelGroup {
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
                    muted,
                    soloed,
                    dimmed,
                });
            }
        } else {
            // Fallback for unknown channel counts (mono, quad, or exotic configs)
            match num_channels {
                1 => {
                    // Mono
                    let (muted, soloed, dimmed) = get_previous_state("Mono");
                    self.level_meters.groups.push(ChannelGroup {
                        name: "Mono".to_string(),
                        channels: vec![ChannelInfo {
                            index: 0,
                            name: "M".to_string(),
                            display_name: vec!["M".to_string()],
                        }],
                        muted,
                        soloed,
                        dimmed,
                    });
                }
                4 => {
                    // Quad (FL, FR, SL, SR) - not a standard speaker config
                    let (muted_lr, soloed_lr, dimmed_lr) = get_previous_state("L/R");
                    self.level_meters.groups.push(ChannelGroup {
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
                        muted: muted_lr,
                        soloed: soloed_lr,
                        dimmed: dimmed_lr,
                    });

                    let (muted_sr, soloed_sr, dimmed_sr) = get_previous_state("Surrounds");
                    self.level_meters.groups.push(ChannelGroup {
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
                        muted: muted_sr,
                        soloed: soloed_sr,
                        dimmed: dimmed_sr,
                    });
                }
                _ => {
                    // Generic fallback - treat all channels as one group
                    let (muted, soloed, dimmed) = get_previous_state("All Channels");
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
                    self.level_meters.groups.push(ChannelGroup {
                        name: "All Channels".to_string(),
                        channels,
                        muted,
                        soloed,
                        dimmed,
                    });
                }
            }
        }

        // Update Matrix plugin channel states
        self.update_matrix_plugin();
    }

    /// Update peak hold values for level meters
    /// Peak hold captures the maximum value and decays over time
    fn update_level_meter_peak_hold(&mut self) {
        let now = std::time::Instant::now();

        // Get current channel peaks from loudness data (zero-copy slice reference)
        let current_peaks: &[f64] = self
            .playback
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.as_slice())
            .unwrap_or(&[]);

        update_peak_hold_values(
            &mut self.level_meters.peak_hold,
            current_peaks,
            self.ui_state.reduce_motion,
        );

        self.level_meters.peak_hold_last_update = Some(now);
    }

    /// Clear all mutes, solos, and dims in level meter groups
    fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meters.groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_matrix_plugin();
    }

    /// Set mute state for a specific group
    fn set_level_meter_mute(&mut self, group_idx: usize, muted: bool) {
        if let Some(group) = self.level_meters.groups.get_mut(group_idx) {
            group.muted = muted;
            self.update_matrix_plugin();
        }
    }

    /// Set solo state for a specific group (with exclusivity logic)
    fn set_level_meter_solo(&mut self, group_idx: usize, soloed: bool) {
        if group_idx >= self.level_meters.groups.len() {
            return;
        }

        if soloed {
            // Solo behavior: only one group can be soloed at a time
            for (idx, g) in self.level_meters.groups.iter_mut().enumerate() {
                if idx == group_idx {
                    g.soloed = true;
                    // When soloing, ensure it's unmuted? Logic says:
                    // "When soloing, set soloed=true on selected group... if g.soloed { g.muted = false; }"
                    g.muted = false;
                } else {
                    g.soloed = false;
                }
            }
        } else if let Some(group) = self.level_meters.groups.get_mut(group_idx) {
            group.soloed = false;
        }
        self.update_matrix_plugin();
    }

    /// Set dim state for a specific group
    fn set_level_meter_dim(&mut self, group_idx: usize, dimmed: bool) {
        if let Some(group) = self.level_meters.groups.get_mut(group_idx) {
            group.dimmed = dimmed;
            self.update_matrix_plugin();
        }
    }

    /// Toggle mute for the selected level meter group
    fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meters
            .groups
            .get(self.level_meters.selected_group)
        {
            self.set_level_meter_mute(self.level_meters.selected_group, !group.muted);
        }
    }

    /// Toggle solo for the selected level meter group
    fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meters
            .groups
            .get(self.level_meters.selected_group)
        {
            self.set_level_meter_solo(self.level_meters.selected_group, !group.soloed);
        }
    }

    /// Toggle dim for the selected level meter group
    fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meters
            .groups
            .get(self.level_meters.selected_group)
        {
            self.set_level_meter_dim(self.level_meters.selected_group, !group.dimmed);
        }
    }

    /// Update the Matrix plugin based on current level meter group states
    fn update_matrix_plugin(&mut self) {
        // Calculate total channel count
        let num_channels: usize = self
            .level_meters
            .groups
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

        for group in &self.level_meters.groups {
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

        // Find and update the LAST Matrix plugin (closest to output)
        for i in (0..self.plugin_state.graph.len()).rev() {
            if let Some(plugin) = self.plugin_state.graph.get_plugin_mut(i)
                && matches!(&plugin.settings, PluginSettings::Matrix { .. })
            {
                // Update settings in memory
                match &mut plugin.settings {
                    PluginSettings::Matrix {
                        channel_states: settings_states,
                        ..
                    } => {
                        *settings_states = channel_states.clone();
                    }
                    _ => unreachable!(),
                }

                // Dispatch update to audio engine
                self.plugin_state.update_state.pending_plugin_update =
                    Some(PluginUpdateType::Structural);
                return; // Only update the last matrix plugin found
            }
        }
    }

    /// Navigate to next level meter group
    fn select_next_level_meter_group(&mut self) {
        if !self.level_meters.groups.is_empty() {
            self.level_meters.selected_group =
                (self.level_meters.selected_group + 1) % self.level_meters.groups.len();
        }
    }

    /// Navigate to previous level meter group
    fn select_previous_level_meter_group(&mut self) {
        if !self.level_meters.groups.is_empty() {
            if self.level_meters.selected_group == 0 {
                self.level_meters.selected_group = self.level_meters.groups.len() - 1;
            } else {
                self.level_meters.selected_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    fn select_next_level_meter_control(&mut self) {
        self.level_meters.control_selection = (self.level_meters.control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    fn select_previous_level_meter_control(&mut self) {
        self.level_meters.control_selection = if self.level_meters.control_selection == 0 {
            2
        } else {
            self.level_meters.control_selection - 1
        };
    }

    /// Toggle the currently selected level meter control (mute/solo/dim)
    fn toggle_selected_level_meter_control(&mut self) {
        match self.level_meters.control_selection {
            0 => self.toggle_level_meter_mute(),
            1 => self.toggle_level_meter_solo(),
            2 => self.toggle_level_meter_dim(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::update_peak_hold_values;

    #[test]
    fn peak_hold_snaps_to_current_values_when_motion_is_reduced() {
        let mut held = vec![0.9, 0.1, 0.4];
        update_peak_hold_values(&mut held, &[0.2, 0.8], true);
        assert_eq!(held, vec![0.2, 0.8]);
    }

    #[test]
    fn peak_hold_retains_and_decays_peaks_when_motion_is_enabled() {
        let mut held = vec![0.8, 0.2];
        update_peak_hold_values(&mut held, &[0.1, 0.7], false);
        assert!((held[0] - 0.76).abs() < f64::EPSILON);
        assert_eq!(held[1], 0.7);
    }
}
