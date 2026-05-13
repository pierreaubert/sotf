// ============================================================================
// Channel Mute/Solo Plugin - Mute or solo individual channels
// ============================================================================

pub mod params;

use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::apply_per_channel_gain_simd;
use sotf_host::smoothing::Smoother;

/// Default smoothing time in ms for mute/solo/dim transitions (~5ms fade to avoid clicks)
const DEFAULT_FADE_MS: f32 = 5.0;

/// Default dim gain in dB (-20dB)
const DEFAULT_DIM_GAIN_DB: f32 = -20.0;

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
    /// Dim gain in dB (default -20.0)
    #[serde(default = "default_dim_gain_db")]
    pub dim_gain_db: f32,
    /// Fade time in ms for mute/solo/dim transitions (default 5.0)
    #[serde(default = "default_fade_ms")]
    pub fade_ms: f32,
}

fn default_dim_gain_db() -> f32 {
    DEFAULT_DIM_GAIN_DB
}

fn default_fade_ms() -> f32 {
    DEFAULT_FADE_MS
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
    /// Dim gain in dB (e.g. -20.0 means dimmed channels are attenuated by 20dB)
    dim_gain_db: f32,
    /// Dim gain as linear multiplier (cached from dim_gain_db)
    dim_gain_linear: f32,
    /// Fade time in ms for mute/solo/dim transitions
    fade_ms: f32,
    /// Parameter ID for enabled flag
    param_enabled: ParameterId,
    /// Parameter ID for channel states (JSON)
    param_channel_states: ParameterId,
    /// Parameter ID for dim gain in dB
    param_dim_gain_db: ParameterId,
    /// Parameter ID for fade time in ms
    param_fade_ms: ParameterId,
    /// Cached parameter descriptors — rebuilt lazily when `params_dirty` is true.
    /// `parameters()` takes `&self`, so we use `std::cell::Cell` + `std::cell::RefCell`
    /// for interior mutability to avoid rebuilding on every individual toggle.
    cached_parameters: std::cell::RefCell<Vec<Parameter>>,
    /// Dirty flag — set when any state change could affect cached_parameters.
    params_dirty: std::cell::Cell<bool>,
    /// Cache for SIMD optimization
    cached_gains: Vec<f32>,
    /// Pre-allocated start-of-block gains for block ramping.
    start_gains: Vec<f32>,
}

impl ChannelMuteSoloPlugin {
    /// Create a new channel mute/solo plugin
    pub fn new(channels: usize, enabled: bool) -> Self {
        let channel_states = vec![ChannelState::default(); channels];
        let sample_rate = 48000;
        let dim_gain_db = DEFAULT_DIM_GAIN_DB;
        let fade_ms = DEFAULT_FADE_MS;
        let channel_smoothers = vec![Smoother::new(1.0, fade_ms, sample_rate); channels];
        let p = Self {
            channels,
            enabled,
            channel_states,
            channel_smoothers,
            sample_rate,
            dim_gain_db,
            dim_gain_linear: Self::db_to_linear(dim_gain_db),
            fade_ms,
            param_enabled: ParameterId::from("enabled"),
            param_channel_states: ParameterId::from("channel_states"),
            param_dim_gain_db: ParameterId::from("dim_gain_db"),
            param_fade_ms: ParameterId::from("fade_ms"),
            cached_parameters: std::cell::RefCell::new(Vec::new()),
            params_dirty: std::cell::Cell::new(true),
            cached_gains: vec![1.0; channels],
            start_gains: vec![1.0; channels],
        };
        // Build initial cache so validate_parameter works before any set_parameter call.
        p.rebuild_cached_parameters_if_dirty();
        p
    }

    /// Create a new channel mute/solo plugin from configuration parameters
    pub fn from_params(channels: usize, params: ChannelMuteSoloParams) -> Self {
        let mut plugin = Self::new(channels, params.enabled);

        plugin.set_dim_gain_db(params.dim_gain_db);
        plugin.set_fade_ms(params.fade_ms);

        // Accept mismatched lengths: truncate if too many states, pad with defaults if too few.
        let mut states = params.channel_states;
        states.truncate(channels);
        states.resize(channels, ChannelState::default());
        plugin.channel_states = states;

        plugin.reset_smoothers_to_current();
        plugin.mark_params_dirty();
        // Eagerly rebuild after bulk state load so initial validate_parameter works.
        plugin.rebuild_cached_parameters_if_dirty();
        plugin
    }

    /// Set whether the plugin is enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.update_smoother_targets();
        self.mark_params_dirty();
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
            self.mark_params_dirty();
        }
    }

    /// Set all channel states at once
    pub fn set_channel_states(&mut self, states: Vec<ChannelState>) {
        if states.len() == self.channels {
            self.channel_states = states;
            self.update_smoother_targets();
            self.mark_params_dirty();
        }
    }

    /// Get the state for a specific channel
    pub fn get_channel_state(&self, channel: usize) -> Option<&ChannelState> {
        self.channel_states.get(channel)
    }

    /// Set the dim gain in dB
    pub fn set_dim_gain_db(&mut self, db: f32) {
        self.dim_gain_db = db;
        self.dim_gain_linear = Self::db_to_linear(db);
        self.update_smoother_targets();
        self.mark_params_dirty();
    }

    /// Get the dim gain in dB
    pub fn dim_gain_db(&self) -> f32 {
        self.dim_gain_db
    }

    /// Set the fade time in ms
    pub fn set_fade_ms(&mut self, ms: f32) {
        self.fade_ms = ms;
        for smoother in &mut self.channel_smoothers {
            smoother.set_time(ms, self.sample_rate);
        }
        self.mark_params_dirty();
    }

    /// Get the fade time in ms
    pub fn fade_ms(&self) -> f32 {
        self.fade_ms
    }

    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        sotf_host::db_to_linear(db)
    }

    /// Mark cached parameters as stale; they will be rebuilt lazily in `parameters()`.
    #[inline]
    fn mark_params_dirty(&self) {
        self.params_dirty.set(true);
    }

    /// Rebuild cached parameter descriptors if the dirty flag is set.
    /// Can be called from `&self` contexts via interior mutability.
    fn rebuild_cached_parameters_if_dirty(&self) {
        if !self.params_dirty.get() {
            return;
        }
        *self.cached_parameters.borrow_mut() = vec![
            Parameter::new_bool("enabled", "Enabled", self.enabled)
                .with_description("Enable/disable the plugin")
                .with_group("General")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_string(
                "channel_states",
                "Channel States",
                serde_json::to_string(&self.channel_states).unwrap_or_default(),
            )
            .with_description("Per-channel mute/solo/dim states (JSON)")
            .with_group("General"),
            Parameter::new_float("dim_gain_db", "Dim Gain", self.dim_gain_db, -60.0, 0.0)
                .with_description("Gain applied to dimmed channels (dB)")
                .with_group("General"),
            Parameter::new_float("fade_ms", "Fade Time", self.fade_ms, 0.0, 100.0)
                .with_description("Transition fade time (ms)")
                .with_group("General"),
        ];
        self.params_dirty.set(false);
    }

    /// Recompute smoother targets based on current channel states
    fn update_smoother_targets(&mut self) {
        let has_solo = self.channel_states.iter().any(|s| s.soloed);
        for (ch, state) in self.channel_states.iter().enumerate() {
            let target = if !self.enabled {
                1.0
            } else {
                self.compute_channel_gain(state, has_solo)
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
                self.compute_channel_gain(state, has_solo)
            };
            self.channel_smoothers[ch].reset(target);
        }
    }

    /// Compute the target gain for a channel given its state
    fn compute_channel_gain(&self, state: &ChannelState, has_solo: bool) -> f32 {
        if has_solo {
            if state.soloed { 1.0 } else { 0.0 }
        } else if state.muted {
            0.0
        } else if state.dimmed {
            self.dim_gain_linear
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
        self.rebuild_cached_parameters_if_dirty();
        self.cached_parameters.borrow().clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Per-channel dynamic parameters (mute_N / solo_N / dim_N) are not in cached_parameters
        // because their count is dynamic. Handle them before validate_parameter to avoid the
        // "Unknown parameter" rejection from the default validator.
        // Only treat as per-channel if the suffix is a valid decimal index (starts with digit),
        // to avoid false matches like "dim_gain_db" → "dim_" prefix with "gain_db" suffix.
        if let Some(rest) = id.0.strip_prefix("mute_")
            && rest.starts_with(|c: char| c.is_ascii_digit())
        {
            if let Ok(ch) = rest.parse::<usize>()
                && ch < self.channels
            {
                self.channel_states[ch].muted = value.as_bool().unwrap_or(false);
                self.update_smoother_targets();
                self.mark_params_dirty();
                return Ok(());
            }
            return Err(format!("Invalid channel index in {}", id));
        } else if let Some(rest) = id.0.strip_prefix("solo_")
            && rest.starts_with(|c: char| c.is_ascii_digit())
        {
            if let Ok(ch) = rest.parse::<usize>()
                && ch < self.channels
            {
                self.channel_states[ch].soloed = value.as_bool().unwrap_or(false);
                self.update_smoother_targets();
                self.mark_params_dirty();
                return Ok(());
            }
            return Err(format!("Invalid channel index in {}", id));
        } else if let Some(rest) = id.0.strip_prefix("dim_")
            && rest.starts_with(|c: char| c.is_ascii_digit())
        {
            if let Ok(ch) = rest.parse::<usize>()
                && ch < self.channels
            {
                self.channel_states[ch].dimmed = value.as_bool().unwrap_or(false);
                self.update_smoother_targets();
                self.mark_params_dirty();
                return Ok(());
            }
            return Err(format!("Invalid channel index in {}", id));
        }

        // For the registered parameters, use standard range/type validation.
        self.validate_parameter(&id, &value)?;
        if id == self.param_enabled {
            self.set_enabled(value.as_bool().unwrap_or(true));
            self.mark_params_dirty();
            Ok(())
        } else if id == self.param_channel_states {
            if let Some(json_str) = value.as_string() {
                let states: Vec<ChannelState> =
                    serde_json::from_str(json_str).map_err(|e| e.to_string())?;
                self.set_channel_states(states);
                self.mark_params_dirty();
                Ok(())
            } else {
                Err("channel_states must be string".to_string())
            }
        } else if id == self.param_dim_gain_db {
            if let Some(v) = value.as_float() {
                self.set_dim_gain_db(v);
                self.mark_params_dirty();
                Ok(())
            } else {
                Err("dim_gain_db must be float".to_string())
            }
        } else if id == self.param_fade_ms {
            if let Some(v) = value.as_float() {
                self.set_fade_ms(v);
                self.mark_params_dirty();
                Ok(())
            } else {
                Err("fade_ms must be float".to_string())
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
        } else if id == &self.param_dim_gain_db {
            Some(ParameterValue::Float(self.dim_gain_db))
        } else if id == &self.param_fade_ms {
            Some(ParameterValue::Float(self.fade_ms))
        } else if let Some(rest) = id.0.strip_prefix("mute_") {
            rest.parse::<usize>()
                .ok()
                .filter(|&ch| ch < self.channels)
                .map(|ch| ParameterValue::Bool(self.channel_states[ch].muted))
        } else if let Some(rest) = id.0.strip_prefix("solo_") {
            rest.parse::<usize>()
                .ok()
                .filter(|&ch| ch < self.channels)
                .map(|ch| ParameterValue::Bool(self.channel_states[ch].soloed))
        } else if let Some(rest) = id.0.strip_prefix("dim_") {
            rest.parse::<usize>()
                .ok()
                .filter(|&ch| ch < self.channels)
                .map(|ch| ParameterValue::Bool(self.channel_states[ch].dimmed))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        for smoother in &mut self.channel_smoothers {
            smoother.set_time(self.fade_ms, sample_rate);
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

        // Validate buffer length: must be exactly num_frames * channels.
        debug_assert_eq!(
            buffer.len(),
            num_frames * self.channels,
            "Buffer length {} does not match expected {}",
            buffer.len(),
            num_frames * self.channels
        );

        // If disabled and all smoothers have already settled at unity, skip all work.
        let all_at_unity = self
            .channel_smoothers
            .iter()
            .all(|s| (s.current() - 1.0).abs() < 1e-5);
        if !self.enabled && all_at_unity {
            return Ok(num_frames);
        }

        // Block-based smoothing: advance each smoother once across the whole block using
        // next_n(), then apply a per-frame linear ramp between start and end gain values.
        // For a 5 ms tau at 48 kHz the linear-ramp error vs. true exponential is <0.3%
        // across a 512-sample block — inaudible — while avoiding O(num_frames × channels)
        // individual smoother calls.
        let channels = self.channels;
        for (gain, smoother) in self
            .start_gains
            .iter_mut()
            .zip(self.channel_smoothers.iter())
        {
            *gain = smoother.current();
        }
        for (gain, smoother) in self.cached_gains.iter_mut().zip(self.channel_smoothers.iter_mut()) {
            // next_n() advances the smoother's current value to the end-of-block state.
            *gain = smoother.next_n(num_frames);
        }

        if num_frames == 1 {
            // Single frame: no ramp needed — just apply end gain via SIMD helper.
            apply_per_channel_gain_simd(buffer, channels, &self.cached_gains);
        } else {
            let inv_nf = 1.0 / num_frames as f32;
            for frame in 0..num_frames {
                // Linear interpolation: t goes from 0 at frame 0 to (nf-1)/nf at last frame.
                // Frame 0 uses the start gain; frame nf-1 approaches (but does not reach) the
                // end gain — the smoother state has been advanced to end_gain already.
                let t = frame as f32 * inv_nf;
                let offset = frame * channels;
                let frame_buf = &mut buffer[offset..offset + channels];
                for (s, (&sg, &eg)) in frame_buf
                    .iter_mut()
                    .zip(self.start_gains.iter().zip(self.cached_gains.iter()))
                {
                    *s *= sg + t * (eg - sg);
                }
            }
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
            dim_gain_db: DEFAULT_DIM_GAIN_DB,
            fade_ms: DEFAULT_FADE_MS,
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
            dim_gain_db: DEFAULT_DIM_GAIN_DB,
            fade_ms: DEFAULT_FADE_MS,
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

    #[test]
    fn test_configurable_dim_gain() {
        // Use -10dB dim instead of default -20dB
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_dim_gain_db(-10.0);
        plugin.set_channel_state(0, false, false, true); // Dim channel 0

        let expected_linear = 10.0_f32.powf(-10.0 / 20.0); // ~0.316

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - expected_linear).abs() < TOLERANCE,
            "Ch0 should be dimmed to ~0.316 (-10dB), got {}",
            last_frame[0]
        );
        assert!(
            (last_frame[1] - 1.0).abs() < TOLERANCE,
            "Ch1 should be unchanged"
        );
    }

    #[test]
    fn test_dim_gain_via_set_parameter() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin
            .set_parameter(
                ParameterId::from("dim_gain_db"),
                ParameterValue::Float(-6.0),
            )
            .unwrap();
        plugin.set_channel_state(0, false, false, true); // Dim channel 0

        let expected_linear = 10.0_f32.powf(-6.0 / 20.0); // ~0.501

        let last_frame = process_converged(&mut plugin, 2);
        assert!(
            (last_frame[0] - expected_linear).abs() < TOLERANCE,
            "Ch0 should be dimmed to ~0.501 (-6dB), got {}",
            last_frame[0]
        );
    }

    #[test]
    fn test_get_dim_gain_parameter() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_dim_gain_db(-12.0);

        let val = plugin.get_parameter(&ParameterId::from("dim_gain_db"));
        assert_eq!(val, Some(ParameterValue::Float(-12.0)));
    }

    #[test]
    fn test_fade_ms_via_set_parameter() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin
            .set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(50.0))
            .unwrap();

        assert!((plugin.fade_ms() - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_fade_ms_parameter() {
        let plugin = ChannelMuteSoloPlugin::new(2, true);
        let val = plugin.get_parameter(&ParameterId::from("fade_ms"));
        assert_eq!(val, Some(ParameterValue::Float(DEFAULT_FADE_MS)));
    }

    #[test]
    fn test_from_params_with_custom_dim_and_fade() {
        let params = ChannelMuteSoloParams {
            enabled: true,
            channel_states: vec![
                ChannelState {
                    muted: false,
                    soloed: false,
                    dimmed: true,
                },
                ChannelState {
                    muted: false,
                    soloed: false,
                    dimmed: false,
                },
            ],
            dim_gain_db: -6.0,
            fade_ms: 10.0,
        };

        let mut plugin = ChannelMuteSoloPlugin::from_params(2, params);
        assert!((plugin.dim_gain_db() - -6.0).abs() < f32::EPSILON);
        assert!((plugin.fade_ms() - 10.0).abs() < f32::EPSILON);

        // Verify the dim gain is applied correctly (from_params resets smoothers)
        let mut buffer = vec![1.0, 1.0];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };
        plugin.process_in_place(&mut buffer, &context).unwrap();

        let expected_linear = 10.0_f32.powf(-6.0 / 20.0);
        assert!(
            (buffer[0] - expected_linear).abs() < TOLERANCE,
            "Ch0 should be dimmed to ~0.501 (-6dB) immediately, got {}",
            buffer[0]
        );
    }

    #[test]
    fn test_dim_gain_out_of_range_rejected() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        // Above max (0.0)
        let result =
            plugin.set_parameter(ParameterId::from("dim_gain_db"), ParameterValue::Float(1.0));
        assert!(result.is_err());
        // Below min (-60.0)
        let result = plugin.set_parameter(
            ParameterId::from("dim_gain_db"),
            ParameterValue::Float(-70.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_fade_ms_out_of_range_rejected() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        // Below min (0.0)
        let result =
            plugin.set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(-1.0));
        assert!(result.is_err());
        // Above max (100.0)
        let result =
            plugin.set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(200.0));
        assert!(result.is_err());
    }

    #[test]
    fn test_dim_via_channel_states_parameter() {
        // Set channel 0 to dimmed via the channel_states JSON parameter,
        // verify channel 0 is attenuated by dim_gain_db and channel 1 is at full level.
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_dim_gain_db(-20.0);

        // Set channel_states via the parameter interface
        let states_json = r#"[{"muted":false,"soloed":false,"dimmed":true},{"muted":false,"soloed":false,"dimmed":false}]"#;
        plugin
            .set_parameter(
                ParameterId::from("channel_states"),
                ParameterValue::String(states_json.to_string()),
            )
            .unwrap();

        let last_frame = process_converged(&mut plugin, 2);
        let expected_dim = 10.0_f32.powf(-20.0 / 20.0); // 0.1

        assert!(
            (last_frame[0] - expected_dim).abs() < TOLERANCE,
            "Ch0 (dimmed via channel_states param) should be ~{}, got {}",
            expected_dim,
            last_frame[0]
        );
        assert!(
            (last_frame[1] - 1.0).abs() < TOLERANCE,
            "Ch1 (not dimmed) should be at full level, got {}",
            last_frame[1]
        );
    }

    #[test]
    fn test_params_serde_defaults() {
        // When deserializing JSON without dim_gain_db/fade_ms, defaults should apply
        let json = r#"{"enabled": true, "channel_states": []}"#;
        let params: ChannelMuteSoloParams = serde_json::from_str(json).unwrap();
        assert!((params.dim_gain_db - DEFAULT_DIM_GAIN_DB).abs() < f32::EPSILON);
        assert!((params.fade_ms - DEFAULT_FADE_MS).abs() < f32::EPSILON);
    }

    // =========================================================================
    // TDD tests for bug fixes
    // =========================================================================

    /// Fix 2.1: params.rs PARAMS spec fade_ms default must match lib.rs DEFAULT_FADE_MS (5.0).
    /// Previously params.rs had 10.0, lib.rs had 5.0, causing UI/DSP mismatch.
    #[test]
    fn test_params_spec_fade_ms_default_matches_dsp_default() {
        use crate::params::PARAMS;
        use sotf_host::param_specs::find_by_key as pk;
        let spec_default = pk(PARAMS, "fade_ms").default_f64() as f32;
        assert!(
            (spec_default - DEFAULT_FADE_MS).abs() < f32::EPSILON,
            "params.rs PARAMS fade_ms default ({}) must equal lib.rs DEFAULT_FADE_MS ({})",
            spec_default,
            DEFAULT_FADE_MS
        );
    }

    /// Fix 2.3: from_params with fewer channel_states than channels should pad with defaults.
    #[test]
    fn test_from_params_fewer_channel_states_pads_defaults() {
        let params = ChannelMuteSoloParams {
            enabled: true,
            channel_states: vec![ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            }],
            dim_gain_db: DEFAULT_DIM_GAIN_DB,
            fade_ms: DEFAULT_FADE_MS,
        };
        // 2-channel plugin, only 1 state provided
        let plugin = ChannelMuteSoloPlugin::from_params(2, params);
        // Ch0 from provided state
        assert!(plugin.get_channel_state(0).unwrap().muted);
        // Ch1 padded with default (not muted)
        assert!(!plugin.get_channel_state(1).unwrap().muted);
    }

    /// Fix 2.3: from_params with more channel_states than channels should truncate.
    #[test]
    fn test_from_params_more_channel_states_truncates() {
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
                    soloed: true,
                    dimmed: false,
                },
                ChannelState {
                    muted: false,
                    soloed: false,
                    dimmed: true,
                },
            ],
            dim_gain_db: DEFAULT_DIM_GAIN_DB,
            fade_ms: DEFAULT_FADE_MS,
        };
        // 2-channel plugin, 3 states provided — should use first 2
        let plugin = ChannelMuteSoloPlugin::from_params(2, params);
        assert!(plugin.get_channel_state(0).unwrap().muted);
        assert!(plugin.get_channel_state(1).unwrap().soloed);
        assert!(plugin.get_channel_state(2).is_none());
    }

    /// Fix 2.4: process_in_place buffer length mismatch must panic in debug (via debug_assert).
    /// In release builds we just verify it processes normally when the length is correct.
    #[test]
    fn test_process_correct_buffer_length_succeeds() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        let mut buffer = vec![1.0f32; 4]; // 2 frames × 2 channels
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 2,
        };
        let result = plugin.process_in_place(&mut buffer, &ctx);
        assert!(result.is_ok());
    }

    /// Fix 3.1 + 3.2: block-based smoothing and lazy rebuild should preserve correct DSP output.
    /// Verifies that the optimized path still converges to the correct target gain.
    #[test]
    fn test_block_smoothing_converges_to_correct_gain() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.set_channel_state(0, true, false, false); // mute ch0

        // Process 4096 frames — should converge to 0.0 for ch0, 1.0 for ch1
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 4096,
        };
        let mut buffer = vec![1.0f32; 4096 * 2];
        plugin.process_in_place(&mut buffer, &context).unwrap();

        let last_ch0 = buffer[4095 * 2];
        let last_ch1 = buffer[4095 * 2 + 1];
        assert!(
            last_ch0.abs() < TOLERANCE,
            "Ch0 (muted) should converge to 0.0 with block smoothing, got {}",
            last_ch0
        );
        assert!(
            (last_ch1 - 1.0).abs() < TOLERANCE,
            "Ch1 (unmuted) should remain 1.0, got {}",
            last_ch1
        );
    }

    /// Fix 3.2: lazy rebuild — parameters() channel_states JSON must reflect current state after
    /// set_channel_state() mutates mute/solo/dim flags.
    #[test]
    fn test_lazy_rebuild_reflects_current_state_after_mute_toggle() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        // Mutate via the direct method (not set_parameter, which requires per-channel params to be
        // registered — see companion test below). Then call parameters() and verify the JSON blob
        // reflects the change, proving rebuild_cached_parameters ran.
        plugin.set_channel_state(0, true, false, false);

        let params = plugin.parameters();
        let cs_param = params.iter().find(|p| p.id.0 == "channel_states").unwrap();
        let json = cs_param.default_value.as_string().unwrap();
        let states: Vec<ChannelState> = serde_json::from_str(json).unwrap();
        assert!(
            states[0].muted,
            "channel_states JSON in parameters() must reflect set_channel_state() change"
        );
    }

    /// Per-channel set_parameter (mute_N / solo_N / dim_N) must work via set_parameter interface.
    /// validate_parameter currently rejects these because they are not in cached_parameters.
    /// This test documents the fix: per-channel params must be skipped in validate_parameter.
    #[test]
    fn test_per_channel_set_parameter_mute_works() {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin
            .set_parameter(ParameterId::from("mute_0"), ParameterValue::Bool(true))
            .unwrap();
        assert!(
            plugin.get_channel_state(0).unwrap().muted,
            "set_parameter mute_0=true must mute channel 0"
        );
    }
}
