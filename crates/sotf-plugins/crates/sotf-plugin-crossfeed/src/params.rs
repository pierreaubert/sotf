//! Crossfeed plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Serializable state (Params struct with serde defaults)
//! - Index↔field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

mod consts;
mod d;
#[cfg(test)]
mod tests;

pub use consts::*;

use d::d_autogain_enabled;
use d::d_autogain_max_gain_db;
use d::d_autogain_smoothing_ms;
use d::d_autogain_target_lufs;
use d::d_bauer_fcut_hz;
use d::d_bauer_feed_db;
use d::d_crossfeed_mode;
use d::d_crossfeed_preset;
use d::d_enabled;
use d::d_itd_delay_ms;
use d::d_mb_high_feed_db;
use d::d_mb_low_feed_db;
use d::d_mb_low_freq_hz;
use d::d_mb_mid_feed_db;
use d::d_mb_mid_high_freq_hz;
use d::d_meier_level;
use d::d_mix;

/// Crossfeed plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
///
/// Mode and preset are stored as usize indices into MODE_LABELS / PRESET_LABELS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_crossfeed_mode")]
    pub crossfeed_mode: usize,
    #[serde(default = "d_crossfeed_preset")]
    pub crossfeed_preset: usize,
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_bauer_fcut_hz")]
    pub bauer_fcut_hz: f64,
    #[serde(default = "d_bauer_feed_db")]
    pub bauer_feed_db: f64,
    #[serde(default = "d_meier_level")]
    pub meier_level: f64,
    #[serde(default = "d_mb_low_freq_hz")]
    pub mb_low_freq_hz: f64,
    #[serde(default = "d_mb_mid_high_freq_hz")]
    pub mb_mid_high_freq_hz: f64,
    #[serde(default = "d_mb_low_feed_db")]
    pub mb_low_feed_db: f64,
    #[serde(default = "d_mb_mid_feed_db")]
    pub mb_mid_feed_db: f64,
    #[serde(default = "d_mb_high_feed_db")]
    pub mb_high_feed_db: f64,
    #[serde(default = "d_itd_delay_ms")]
    pub itd_delay_ms: f64,
    #[serde(default = "d_autogain_enabled")]
    pub autogain_enabled: bool,
    #[serde(default = "d_autogain_target_lufs")]
    pub autogain_target_lufs: f64,
    #[serde(default = "d_autogain_max_gain_db")]
    pub autogain_max_gain_db: f64,
    #[serde(default = "d_autogain_smoothing_ms")]
    pub autogain_smoothing_ms: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            crossfeed_mode: d_crossfeed_mode(),
            crossfeed_preset: d_crossfeed_preset(),
            enabled: d_enabled(),
            mix: d_mix(),
            bauer_fcut_hz: d_bauer_fcut_hz(),
            bauer_feed_db: d_bauer_feed_db(),
            meier_level: d_meier_level(),
            mb_low_freq_hz: d_mb_low_freq_hz(),
            mb_mid_high_freq_hz: d_mb_mid_high_freq_hz(),
            mb_low_feed_db: d_mb_low_feed_db(),
            mb_mid_feed_db: d_mb_mid_feed_db(),
            mb_high_feed_db: d_mb_high_feed_db(),
            itd_delay_ms: d_itd_delay_ms(),
            autogain_enabled: d_autogain_enabled(),
            autogain_target_lufs: d_autogain_target_lufs(),
            autogain_max_gain_db: d_autogain_max_gain_db(),
            autogain_smoothing_ms: d_autogain_smoothing_ms(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "crossfeed";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.crossfeed_mode as f64),
            1 => Some(self.crossfeed_preset as f64),
            2 => Some(if self.enabled { 1.0 } else { 0.0 }),
            3 => Some(self.mix),
            4 => Some(self.bauer_fcut_hz),
            5 => Some(self.bauer_feed_db),
            6 => Some(self.meier_level),
            7 => Some(self.mb_low_freq_hz),
            8 => Some(self.mb_mid_high_freq_hz),
            9 => Some(self.mb_low_feed_db),
            10 => Some(self.mb_mid_feed_db),
            11 => Some(self.mb_high_feed_db),
            12 => Some(self.itd_delay_ms),
            13 => Some(if self.autogain_enabled { 1.0 } else { 0.0 }),
            14 => Some(self.autogain_target_lufs),
            15 => Some(self.autogain_max_gain_db),
            16 => Some(self.autogain_smoothing_ms),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.crossfeed_mode = value as usize,
            1 => self.crossfeed_preset = value as usize,
            2 => self.enabled = value > 0.5,
            3 => self.mix = value,
            4 => self.bauer_fcut_hz = value,
            5 => self.bauer_feed_db = value,
            6 => self.meier_level = value,
            7 => self.mb_low_freq_hz = value,
            8 => self.mb_mid_high_freq_hz = value,
            9 => self.mb_low_feed_db = value,
            10 => self.mb_mid_feed_db = value,
            11 => self.mb_high_feed_db = value,
            12 => self.itd_delay_ms = value,
            13 => self.autogain_enabled = value > 0.5,
            14 => self.autogain_target_lufs = value,
            15 => self.autogain_max_gain_db = value,
            16 => self.autogain_smoothing_ms = value,
            _ => {}
        }
    }
}
