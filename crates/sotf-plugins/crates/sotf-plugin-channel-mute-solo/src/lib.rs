// ============================================================================
// Channel Mute/Solo Plugin - Mute or solo individual channels
// ============================================================================

pub mod param_specs;

use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::apply_per_channel_gain_simd;
use sotf_host::smoothing::Smoother;

/// Smoothing time in ms for mute/solo/dim transitions (~5ms fade to avoid clicks)
const FADE_SMOOTH_MS: f32 = 5.0;

// ============================================================================
// Configuration
// ============================================================================

/// State for a single channel
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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
pub struct ChannelMuteSoloPlugin {
    /// Number of channels
    channels: usize,
    /// Whether the plugin is enabled (if false, audio passes through unchanged)
    enabled: bool,
    /// Per-channel mute/solo state
    channel_states: Vec<ChannelState>,
    /// Per-channel gain smoothers for click-free mute/solo/dim transitions
    channel_smoothers: Vec<Smoother>,
    /// Sample rate for smoother initialization
    sample_rate: u32,
    /// Parameter ID for enabled flag
    param_enabled: ParameterId,
    /// Parameter ID for channel states (JSON)
    param_channel_states: ParameterId,
    // Cache for SIMD optimization
    cached_gains: Vec<f32>,
}

impl ChannelMuteSoloPlugin {
    /// Create a new channel mute/solo plugin
    pub fn new(channels: usize, enabled: bool) -> Self {
        let channel_states = vec![ChannelState::default(); channels];
        let sample_rate = 48000;
        let channel_smoothers = vec![Smoother::new(1.0, FADE_SMOOTH_MS, sample_rate); channels];
        Self {
            channels,
            enabled,
            channel_states,
            channel_smoothers,
            sample_rate,
            param_enabled: ParameterId::from("enabled"),
            param_channel_states: ParameterId::from("channel_states"),
            cached_gains: vec![1.0; channels],
        }
    }

    /// Create a new channel mute/solo plugin from configuration parameters
    pub fn from_params(channels: usize, params: ChannelMuteSoloParams) -> Self {
        let mut plugin = Self::new(channels, params.enabled);

        if params.channel_states.len() == channels {
            plugin.channel_states = params.channel_states;
        }

        plugin.reset_smoothers_to_current();
        plugin
    }

    /// Set whether the plugin is enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.update_smoother_targets();
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
            self.update_smoother_targets();
        }
    }

    /// Set all channel states at once
    pub fn set_channel_states(&mut self, states: Vec<ChannelState>) {
        if states.len() == self.channels {
            self.channel_states = states;
            self.update_smoother_targets();
        }
    }

    /// Get the state for a specific channel
    pub fn get_channel_state(&self, channel: usize) -> Option<&ChannelState> {
        self.channel_states.get(channel)
    }

    /// Recompute smoother targets based on current channel states
    fn update_smoother_targets(&mut self) {
        let has_solo = self.channel_states.iter().any(|s| s.soloed);
        for (ch, state) in self.channel_states.iter().enumerate() {
            let target = if !self.enabled {
                1.0
            } else {
                Self::compute_channel_gain(state, has_solo)
            };
            self.channel_smoothers[ch].set_target(target);
        }
    }

    /// Reset smoothers to current state immediately
    fn reset_smoothers_to_current(&mut self) {
        let has_solo = self.channel_states.iter().any(|s| s.soloed);
        for (ch, state) in self.channel_states.iter().enumerate() {
            let target = if !self.enabled {
                1.0
            } else {
                Self::compute_channel_gain(state, has_solo)
            };
            self.channel_smoothers[ch].reset(target);
        }
    }

    /// Compute the target gain for a channel given its state
    fn compute_channel_gain(state: &ChannelState, has_solo: bool) -> f32 {
        if has_solo {
            if state.soloed { 1.0 } else { 0.0 }
        } else if state.muted {
            0.0
        } else if state.dimmed {
            0.1 // -20dB
        } else {
            1.0
        }
    }
}

impl InPlacePlugin for ChannelMuteSoloPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Channel Mute/Solo", "1.1.0", "SotF")
            .with_description("Mute or solo individual channels (Optimized & Smoothed)")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enabled", "Enabled", self.enabled)
                .with_description("Enable/disable the plugin")
                .with_group("General")
                .with_importance(ParameterImportance::Critical),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_enabled {
            self.set_enabled(value.as_bool().unwrap_or(true));
            Ok(())
        } else if id == self.param_channel_states {
            if let Some(json_str) = value.as_string() {
                let states: Vec<ChannelState> =
                    serde_json::from_str(json_str).map_err(|e| e.to_string())?;
                self.set_channel_states(states);
                Ok(())
            } else {
                Err("channel_states must be string".to_string())
            }
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_enabled {
            Some(ParameterValue::Bool(self.enabled))
        } else if id == &self.param_channel_states {
            serde_json::to_string(&self.channel_states)
                .ok()
                .map(ParameterValue::String)
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        for smoother in &mut self.channel_smoothers {
            smoother.set_time(FADE_SMOOTH_MS, sample_rate);
        }
        self.update_smoother_targets();
        Ok(())
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let num_frames = context.num_frames;

        // If enabled=false and ALL smoothers have reached target 1.0, bypass overhead
        let all_at_unity = self
            .channel_smoothers
            .iter()
            .all(|s| (s.current() - 1.0).abs() < 1e-5);
        if !self.enabled && all_at_unity {
            return Ok(num_frames);
        }

        // Optimized path: block-based smoothing and SIMD
        for frame in 0..num_frames {
            // Tick smoothers into cache
            for ch in 0..self.channels {
                self.cached_gains[ch] = self.channel_smoothers[ch].advance();
            }

            let offset = frame * self.channels;
            let frame_buffer = &mut buffer[offset..offset + self.channels];
            apply_per_channel_gain_simd(frame_buffer, self.channels, &self.cached_gains);
        }

        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    /// Number of frames to process for smoother convergence in tests
    const CONVERGE_FRAMES: usize = 2048;
    const TOLERANCE: f32 = 0.001;

    /// Helper: process enough frames for smoothers to converge, then check final frame
    fn process_converged(plugin: &mut ChannelMuteSoloPlugin, channels: usize) -> Vec<f32> {
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: CONVERGE_FRAMES,
        };
        // Fill with 1.0 so gain is the output value
        let mut buffer = vec![1.0; CONVERGE_FRAMES * channels];
        plugin.process_in_place(&mut buffer, &context).unwrap();
        // Return the last frame
        buffer[buffer.len() - channels..].to_vec()
    }

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

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - 0.0).abs() < TOLERANCE,
            "Ch0 should be muted"
        );
        assert!(
            (last_frame[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should be unchanged"
        );
    }

    #[test]
    fn test_solo_single_channel() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, false, true, false); // Solo channel 0

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - 1.0).abs() < TOLERANCE,
            "Ch0 (soloed) should be audible"
        );
        assert!(
            (last_frame[1] - 0.0).abs() < TOLERANCE,
            "Ch1 (not soloed) should be muted"
        );
    }

    #[test]
    fn test_solo_takes_priority_over_mute() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, true, true, false); // Both muted AND soloed
        plugin.set_channel_state(1, false, false, false);

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - 1.0).abs() < TOLERANCE,
            "Ch0 (soloed) should be audible"
        );
        assert!(
            (last_frame[1] - 0.0).abs() < TOLERANCE,
            "Ch1 (not soloed) should be muted"
        );
    }

    #[test]
    fn test_multichannel() {
        let mut plugin = ChannelMuteSoloPlugin::new(4, true);
        plugin.set_channel_state(1, true, false, false); // Mute channel 1
        plugin.set_channel_state(2, true, false, false); // Mute channel 2

        let last_frame = process_converged(&mut plugin, 4);
        assert!(
            (last_frame[0] - 1.0).abs() < TOLERANCE,
            "Ch0 should be unchanged"
        );
        assert!(
            (last_frame[1] - 0.0).abs() < TOLERANCE,
            "Ch1 should be muted"
        );
        assert!(
            (last_frame[2] - 0.0).abs() < TOLERANCE,
            "Ch2 should be muted"
        );
        assert!(
            (last_frame[3] - 1.0).abs() < TOLERANCE,
            "Ch3 should be unchanged"
        );
    }

    #[test]
    fn test_dim_single_channel() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, false, false, true); // Dim channel 0

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - 0.1).abs() < TOLERANCE,
            "Ch0 should be dimmed to 0.1"
        );
        assert!(
            (last_frame[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should be unchanged"
        );
    }

    #[test]
    fn test_mute_takes_priority_over_dim() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, true, false, true); // Both muted AND dimmed

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - 0.0).abs() < TOLERANCE,
            "Ch0 should be muted (not dimmed)"
        );
        assert!(
            (last_frame[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should be unchanged"
        );
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

    #[test]
    fn test_from_params_converged_immediately() {
        // from_params should reset smoothers so initial state is applied instantly
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

        let mut plugin = ChannelMuteSoloPlugin::from_params(2, params);
        // Even with just 1 frame, should be at target (smoothers were reset)
        let mut buffer = vec![1.0, 1.0];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };
        plugin.process_in_place(&mut buffer, &context).unwrap();

        assert!(
            (buffer[0] - 0.0).abs() < TOLERANCE,
            "Ch0 should be muted immediately"
        );
        assert!(
            (buffer[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should be unchanged"
        );
    }

    #[test]
    fn test_smooth_transition() {
        // Verify that muting fades rather than clicks
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.initialize(48000).unwrap();

        // First frame should be at gain 1.0 (all channels unmuted)
        let mut buffer = vec![1.0, 1.0];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };
        plugin.process_in_place(&mut buffer, &context).unwrap();
        assert!((buffer[0] - 1.0).abs() < TOLERANCE);

        // Now mute channel 0 — the first sample after shouldn't jump to 0.0
        plugin.set_channel_state(0, true, false, false);
        let mut buffer = vec![1.0, 1.0];
        plugin.process_in_place(&mut buffer, &context).unwrap();
        // Should be less than 1.0 but not yet 0.0 (fading)
        assert!(buffer[0] < 1.0, "Should start fading");
        assert!(buffer[0] > 0.0, "Should not jump to 0.0 instantly");
    }
}
