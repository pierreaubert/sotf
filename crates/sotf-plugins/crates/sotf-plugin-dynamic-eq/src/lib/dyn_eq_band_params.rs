use super::default::default_band_active;
use super::default::default_band_frequency;
use super::default::default_band_gain;
use super::default::default_band_q;
use super::default::default_band_ratio;
use super::default::default_band_threshold;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynEqBandParams {
    #[serde(default = "default_band_frequency")]
    pub frequency: f32,
    #[serde(default = "default_band_q")]
    pub q: f32,
    #[serde(default = "default_band_gain")]
    pub gain: f32,
    #[serde(default = "default_band_threshold")]
    pub band_threshold: f32,
    #[serde(default = "default_band_ratio")]
    pub band_ratio: f32,
    #[serde(default = "default_band_active")]
    pub active: bool,
    #[serde(default)]
    pub solo: bool,
}

impl Default for DynEqBandParams {
    fn default() -> Self {
        Self {
            frequency: default_band_frequency(),
            q: default_band_q(),
            gain: default_band_gain(),
            band_threshold: default_band_threshold(),
            band_ratio: default_band_ratio(),
            active: default_band_active(),
            solo: false,
        }
    }
}
