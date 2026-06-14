use super::default::default_attack;
use super::default::default_bands_params;
use super::default::default_knee;
use super::default::default_link_channels;
use super::default::default_mix;
use super::default::default_num_bands;
use super::default::default_ratio;
use super::default::default_release;
use super::default::default_threshold;
use super::dyn_eq_band_params::DynEqBandParams;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEqPluginParams {
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_release")]
    pub release_ms: f32,
    #[serde(default = "default_knee")]
    pub knee: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_bands_params")]
    pub bands: Vec<DynEqBandParams>,
}

impl Default for DynamicEqPluginParams {
    fn default() -> Self {
        Self {
            num_bands: default_num_bands(),
            threshold: default_threshold(),
            ratio: default_ratio(),
            attack_ms: default_attack(),
            release_ms: default_release(),
            knee: default_knee(),
            link_channels: default_link_channels(),
            mix: default_mix(),
            bands: default_bands_params(),
        }
    }
}
