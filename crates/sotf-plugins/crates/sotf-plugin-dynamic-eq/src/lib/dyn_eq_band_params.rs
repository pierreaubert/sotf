use crate::params::{
    default_active, default_band_ratio, default_band_threshold, default_frequency, default_gain,
    default_q, default_solo,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynEqBandParams {
    #[serde(default = "default_frequency")]
    pub frequency: f32,
    #[serde(default = "default_q")]
    pub q: f32,
    #[serde(default = "default_gain")]
    pub gain: f32,
    #[serde(default = "default_band_threshold")]
    pub band_threshold: f32,
    #[serde(default = "default_band_ratio")]
    pub band_ratio: f32,
    #[serde(default = "default_active")]
    pub active: bool,
    #[serde(default = "default_solo")]
    pub solo: bool,
}

impl Default for DynEqBandParams {
    fn default() -> Self {
        Self {
            frequency: default_frequency(),
            q: default_q(),
            gain: default_gain(),
            band_threshold: default_band_threshold(),
            band_ratio: default_band_ratio(),
            active: default_active(),
            solo: default_solo(),
        }
    }
}
