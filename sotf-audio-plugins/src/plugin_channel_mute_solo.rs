// ============================================================================
// Channel Mute/Solo Plugin - Mute or solo individual channels
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

/// State for a single channel
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelState {
    pub muted: bool,
    pub soloed: bool,
    #[serde(default)]
    pub dimmed: bool,
}

/// Configuration parameters for ChannelMuteSoloPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMuteSoloParams {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub channel_states: Vec<ChannelState>,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Channel mute/solo plugin
///
/// Allows muting or soloing individual channels in a multi-channel stream.
/// When any channel is soloed, all non-soloed channels are muted.
/// Otherwise, only explicitly muted channels are silenced.
///
/// The plugin can be bypassed by setting enabled=false, which passes audio
/// through unchanged with zero overhead.
pub struct ChannelMuteSoloPlugin {
    /// Number of channels
    channels: usize,
    /// Whether the plugin is enabled (if false, audio passes through unchanged)
    enabled: bool,
    /// Per-channel mute/solo state
    channel_states: Vec<ChannelState>,
    /// Parameter ID for enabled flag
    param_enabled: ParameterId,
    /// Parameter ID for channel states (JSON)
    param_channel_states: ParameterId,
    /// Parameter ID for full state (enabled + channel_states combined)
    param_full_state: ParameterId,
}

impl ChannelMuteSoloPlugin {
    /// Create a new channel mute/solo plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `enabled` - Whether the plugin should process audio (false = bypass)
    pub fn new(channels: usize, enabled: bool) -> Self {
        let channel_states = vec![ChannelState::default(); channels];
        Self {
            channels,
            enabled,
            channel_states,
            param_enabled: ParameterId::from("enabled"),
            param_channel_states: ParameterId::from("channel_states"),
            param_full_state: ParameterId::from("full_state"),
        }
    }

    /// Create a new channel mute/solo plugin from configuration parameters
    pub fn from_params(channels: usize, params: ChannelMuteSoloParams) -> Self {
        let mut plugin = Self::new(channels, params.enabled);

        // Use provided channel states if they match channel count
        if params.channel_states.len() == channels {
            plugin.channel_states = params.channel_states;
        } else if !params.channel_states.is_empty() {
            log::warn!(
                "Channel state count mismatch: got {}, expected {}. Using defaults.",
                params.channel_states.len(),
                channels
            );
        }

        plugin
    }

    /// Set whether the plugin is enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get whether the plugin is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the state for a specific channel
    pub fn set_channel_state(&mut self, channel: usize, muted: bool, soloed: bool, dimmed: bool) {
        if channel < self.channels {
            self.channel_states[channel] = ChannelState {
                muted,
                soloed,
                dimmed,
            };
        }
    }

    /// Set all channel states at once
    pub fn set_channel_states(&mut self, states: Vec<ChannelState>) {
        if states.len() == self.channels {
            self.channel_states = states;
        } else {
            log::warn!(
                "Cannot set channel states: count mismatch (got {}, expected {})",
                states.len(),
                self.channels
            );
        }
    }

    /// Get the state for a specific channel
    pub fn get_channel_state(&self, channel: usize) -> Option<&ChannelState> {
        self.channel_states.get(channel)
    }
}

impl InPlacePlugin for ChannelMuteSoloPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Channel Mute/Solo".to_string(),
            version: "1.0.0".to_string(),
            author: "SOTF".to_string(),
            description: "Mute or solo individual channels in a multi-channel stream".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enabled", "Enabled", self.enabled)
                .with_description("Enable/disable the plugin (false = bypass)"),
            // Note: channel_states are set via plugin configuration, not runtime parameters
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_enabled {
            if let Some(enabled) = value.as_bool() {
                self.set_enabled(enabled);
                Ok(())
            } else {
                Err("enabled parameter must be a boolean".to_string())
            }
        } else if id == self.param_channel_states {
            // Accept JSON string containing channel states
            if let Some(json_str) = value.as_string() {
                match serde_json::from_str::<Vec<ChannelState>>(json_str) {
                    Ok(states) => {
                        self.set_channel_states(states);
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to parse channel states JSON: {}", e)),
                }
            } else {
                Err("channel_states parameter must be a JSON string".to_string())
            }
        } else if id == self.param_full_state {
            // Accept JSON object with both enabled and channel_states
            if let Some(json_str) = value.as_string() {
                match serde_json::from_str::<ChannelMuteSoloParams>(json_str) {
                    Ok(params) => {
                        self.set_enabled(params.enabled);
                        self.set_channel_states(params.channel_states);
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to parse full_state JSON: {}", e)),
                }
            } else {
                Err("full_state parameter must be a JSON string".to_string())
            }
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_enabled {
            Some(ParameterValue::Bool(self.enabled))
        } else if id == &self.param_channel_states {
            // Return channel states as JSON string
            match serde_json::to_string(&self.channel_states) {
                Ok(json) => Some(ParameterValue::String(json)),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify buffer size matches channel count
        if !buffer.len().is_multiple_of(self.channels) {
            return Err(format!(
                "Buffer size {} is not a multiple of channel count {}",
                buffer.len(),
                self.channels
            ));
        }

        // If not enabled, pass through unchanged (zero overhead)
        if !self.enabled {
            return Ok(());
        }

        // Determine if any channel is soloed
        let has_solo = self.channel_states.iter().any(|s| s.soloed);

        let num_frames = context.num_frames;

        // Process each frame
        for frame_idx in 0..num_frames {
            for ch_idx in 0..self.channels {
                let sample_idx = frame_idx * self.channels + ch_idx;
                let state = &self.channel_states[ch_idx];

                // Determine if this channel should be muted
                let is_muted = if has_solo {
                    // Solo mode: mute all channels except soloed ones
                    !state.soloed
                } else {
                    // Normal mode: mute only explicitly muted channels
                    state.muted
                };

                // Apply mute (set to 0.0) or dim (-20dB = multiply by 0.1)
                if is_muted {
                    buffer[sample_idx] = 0.0;
                } else if state.dimmed {
                    // Dim: -20dB = multiply by 0.1
                    buffer[sample_idx] *= 0.1;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bypass() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, false); // disabled
        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Should be unchanged when disabled
        assert_eq!(buffer, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_mute_single_channel() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, true, false, false); // Mute channel 0

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // [L0, R0, L1, R1]
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Channel 0 (left) should be muted
        assert_eq!(buffer, vec![0.0, 2.0, 0.0, 4.0]);
    }

    #[test]
    fn test_solo_single_channel() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, false, true, false); // Solo channel 0

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // [L0, R0, L1, R1]
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Channel 1 (right) should be muted (not soloed)
        assert_eq!(buffer, vec![1.0, 0.0, 3.0, 0.0]);
    }

    #[test]
    fn test_solo_takes_priority_over_mute() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, true, true, false); // Both muted AND soloed
        plugin.set_channel_state(1, false, false, false);

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Channel 0 should be audible (solo overrides mute)
        // Channel 1 should be muted (not soloed)
        assert_eq!(buffer, vec![1.0, 0.0, 3.0, 0.0]);
    }

    #[test]
    fn test_multichannel() {
        let mut plugin = ChannelMuteSoloPlugin::new(4, true);
        plugin.set_channel_state(1, true, false, false); // Mute channel 1
        plugin.set_channel_state(2, true, false, false); // Mute channel 2

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 1 frame, 4 channels
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 1,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Channels 1 and 2 should be muted
        assert_eq!(buffer, vec![1.0, 0.0, 0.0, 4.0]);
    }

    #[test]
    fn test_dim_single_channel() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, false, false, true); // Dim channel 0

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // [L0, R0, L1, R1]
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Channel 0 (left) should be dimmed by -20dB (multiply by 0.1)
        assert_eq!(buffer, vec![0.1, 2.0, 0.3, 4.0]);
    }

    #[test]
    fn test_mute_takes_priority_over_dim() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, true, false, true); // Both muted AND dimmed

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // [L0, R0, L1, R1]
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Channel 0 should be muted (mute takes priority over dim)
        assert_eq!(buffer, vec![0.0, 2.0, 0.0, 4.0]);
    }

    #[test]
    fn test_from_params() {
        let params = ChannelMuteSoloParams {
            enabled: true,
            channel_states: vec![
                ChannelState {
                    muted: true,
                    soloed: false,
                    dimmed: false,
                },
                ChannelState {
                    muted: false,
                    soloed: false,
                    dimmed: false,
                },
            ],
        };

        let plugin = ChannelMuteSoloPlugin::from_params(2, params);
        assert!(plugin.is_enabled());
        assert!(plugin.get_channel_state(0).unwrap().muted);
        assert!(!plugin.get_channel_state(1).unwrap().muted);
    }
}
