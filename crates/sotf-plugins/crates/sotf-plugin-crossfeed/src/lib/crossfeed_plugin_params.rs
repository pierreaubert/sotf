use super::default::default_autogain_max_gain;
use super::default::default_autogain_smoothing;
use super::default::default_autogain_target;
use super::default::default_bauer_fcut;
use super::default::default_bauer_feed;
use super::default::default_enabled;
use super::default::default_mb_high_feed;
use super::default::default_mb_low_feed;
use super::default::default_mb_low_freq;
use super::default::default_mb_mid_feed;
use super::default::default_mb_mid_high_freq;
use super::default::default_meier_level;
use super::default::default_mix;
use super::types::CrossfeedMode;
use super::types::CrossfeedPreset;
use crate::params::PARAMS as CF;
use serde::{Deserialize, Serialize};
use sotf_host::param_specs::find_by_key as pk;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossfeedPluginParams {
    /// Maximum callback size reserved during construction. This is a setup-time
    /// graph contract, not an automatable audio parameter.
    #[serde(default = "default_max_block_frames")]
    pub max_block_frames: usize,
    #[serde(default)]
    pub mode: CrossfeedMode,
    #[serde(default)]
    pub preset: CrossfeedPreset,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_mix")]
    pub mix: f32,

    // Bauer
    #[serde(default = "default_bauer_fcut")]
    pub bauer_fcut_hz: f32,
    #[serde(default = "default_bauer_feed")]
    pub bauer_feed_db: f32,

    // Meier
    #[serde(default = "default_meier_level")]
    pub meier_level: f32,

    // Multiband
    #[serde(default = "default_mb_low_freq")]
    pub mb_low_freq_hz: f32,
    #[serde(default = "default_mb_mid_high_freq")]
    pub mb_mid_high_freq_hz: f32,
    #[serde(default = "default_mb_low_feed")]
    pub mb_low_feed_db: f32,
    #[serde(default = "default_mb_mid_feed")]
    pub mb_mid_feed_db: f32,
    #[serde(default = "default_mb_high_feed")]
    pub mb_high_feed_db: f32,

    // ITD delay
    #[serde(default)]
    pub itd_delay_ms: f32,

    /// Head yaw angle in degrees (-90 to +90, 0 = centered).
    /// Dynamically adjusts ITD based on head rotation.
    /// ITD = head_radius * sin(yaw) / speed_of_sound * 1000 ms.
    #[serde(default)]
    pub head_yaw_deg: f32,

    // Auto gain
    #[serde(default)]
    pub autogain_enabled: bool,
    #[serde(default = "default_autogain_target")]
    pub autogain_target_lufs: f32,
    #[serde(default = "default_autogain_max_gain")]
    pub autogain_max_gain_db: f32,
    #[serde(default = "default_autogain_smoothing")]
    pub autogain_smoothing_ms: f32,
}

impl Default for CrossfeedPluginParams {
    fn default() -> Self {
        Self {
            max_block_frames: default_max_block_frames(),
            mode: CrossfeedMode::Mb,
            preset: CrossfeedPreset::Default,
            enabled: true,
            mix: 1.0,
            bauer_fcut_hz: pk(CF, "bauer_fcut_hz").default_f64() as f32,
            bauer_feed_db: pk(CF, "bauer_feed_db").default_f64() as f32,
            meier_level: pk(CF, "meier_level").default_f64() as f32,
            mb_low_freq_hz: pk(CF, "mb_low_freq_hz").default_f64() as f32,
            mb_mid_high_freq_hz: pk(CF, "mb_mid_high_freq_hz").default_f64() as f32,
            mb_low_feed_db: pk(CF, "mb_low_feed_db").default_f64() as f32,
            mb_mid_feed_db: pk(CF, "mb_mid_feed_db").default_f64() as f32,
            mb_high_feed_db: pk(CF, "mb_high_feed_db").default_f64() as f32,
            itd_delay_ms: 0.0,
            head_yaw_deg: 0.0,
            autogain_enabled: pk(CF, "autogain_enabled").default_bool(),
            autogain_target_lufs: pk(CF, "autogain_target_lufs").default_f64() as f32,
            autogain_max_gain_db: pk(CF, "autogain_max_gain_db").default_f64() as f32,
            autogain_smoothing_ms: pk(CF, "autogain_smoothing_ms").default_f64() as f32,
        }
    }
}

fn default_max_block_frames() -> usize {
    16_384
}

impl CrossfeedPluginParams {
    pub fn from_preset(preset: CrossfeedPreset) -> Self {
        let mut params = match preset {
            CrossfeedPreset::Off => Self {
                mode: CrossfeedMode::Off,
                ..Default::default()
            },
            CrossfeedPreset::Default => Self {
                mode: CrossfeedMode::Bauer,
                bauer_fcut_hz: 700.0,
                bauer_feed_db: 4.5,
                ..Default::default()
            },
            CrossfeedPreset::Cmoy => Self {
                mode: CrossfeedMode::Bauer,
                bauer_fcut_hz: 700.0,
                bauer_feed_db: 6.0,
                ..Default::default()
            },
            CrossfeedPreset::Meier => Self {
                mode: CrossfeedMode::Meier,
                meier_level: 30.0,
                ..Default::default()
            },
            CrossfeedPreset::Mb => Self {
                mode: CrossfeedMode::Mb,
                mb_low_freq_hz: 150.0,
                mb_mid_high_freq_hz: 5700.0,
                mb_low_feed_db: 0.0,
                mb_mid_feed_db: 6.0,
                mb_high_feed_db: 3.0,
                ..Default::default()
            },
            CrossfeedPreset::Hrtf => Self {
                mode: CrossfeedMode::Hrtf,
                ..Default::default()
            },
        };
        params.preset = preset;
        params
    }
}
