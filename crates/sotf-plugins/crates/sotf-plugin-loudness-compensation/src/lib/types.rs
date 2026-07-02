use super::channel_loudness_params::ChannelLoudnessParams;
use super::default::default_auto_gain_enabled;
use super::default::default_auto_gain_max_db;
use super::default::default_auto_gain_position;
use super::default::default_auto_gain_smoothing_ms;
use super::default::default_high_freq;
use super::default::default_high_gain;
use super::default::default_low_freq;
use super::default::default_low_gain;
use super::default::default_mid_enabled;
use super::default::default_mid_freq;
use super::default::default_mid_gain;
use super::default::default_mid_q;
use super::default::default_playback_level_db;
use super::default::default_playback_volume_db;
use super::default::default_reference_level_db;
use super::loudness_compensation_plugin::LoudnessCompensationPlugin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessCompensationPluginParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
    #[serde(default = "default_mid_enabled")]
    pub mid_enabled: bool,
    #[serde(default = "default_mid_freq")]
    pub mid_freq: f32,
    #[serde(default = "default_mid_gain")]
    pub mid_gain: f32,
    #[serde(default = "default_mid_q")]
    pub mid_q: f32,
    #[serde(default)]
    pub channel_params: Vec<ChannelLoudnessParams>,
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,
    /// Auto-gain position: "pre", "post" (default), or "disabled"
    #[serde(default = "default_auto_gain_position")]
    pub auto_gain_position: String,
    /// 0 = Manual (default), 1 = ISO 226, 2 = Auto
    #[serde(default)]
    pub mode: usize,
    #[serde(default = "default_playback_level_db")]
    pub playback_level_db: f32,
    #[serde(default = "default_reference_level_db")]
    pub reference_level_db: f32,
    /// Engine playback volume in dB (used in Auto mode)
    #[serde(default = "default_playback_volume_db")]
    pub playback_volume_db: f32,
}

impl Default for LoudnessCompensationPluginParams {
    fn default() -> Self {
        Self {
            low_freq: default_low_freq(),
            low_gain: default_low_gain(),
            high_freq: default_high_freq(),
            high_gain: default_high_gain(),
            mid_enabled: default_mid_enabled(),
            mid_freq: default_mid_freq(),
            mid_gain: default_mid_gain(),
            mid_q: default_mid_q(),
            channel_params: Vec::new(),
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
            auto_gain_position: default_auto_gain_position(),
            mode: 0,
            playback_level_db: default_playback_level_db(),
            reference_level_db: default_reference_level_db(),
            playback_volume_db: default_playback_volume_db(),
        }
    }
}

/// Type alias for backward compatibility.
pub type FletcherMunsonPlugin = LoudnessCompensationPlugin;

/// Type alias for backward compatibility.
pub type FletcherMunsonPluginParams = LoudnessCompensationPluginParams;
