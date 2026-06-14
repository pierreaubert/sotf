use super::default::default_attack;
use super::default::default_fft_size_index;
use super::default::default_knee;
use super::default::default_mix;
use super::default::default_ratio;
use super::default::default_release;
use super::default::default_spectral_smoothing;
use super::default::default_threshold;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralCompressorPluginParams {
    #[serde(default = "default_fft_size_index")]
    pub fft_size_index: usize,
    #[serde(default = "default_threshold")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_release")]
    pub release_ms: f32,
    #[serde(default = "default_knee")]
    pub knee_db: f32,
    #[serde(default = "default_spectral_smoothing")]
    pub spectral_smoothing: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

impl Default for SpectralCompressorPluginParams {
    fn default() -> Self {
        Self {
            fft_size_index: default_fft_size_index(),
            threshold_db: default_threshold(),
            ratio: default_ratio(),
            attack_ms: default_attack(),
            release_ms: default_release(),
            knee_db: default_knee(),
            spectral_smoothing: default_spectral_smoothing(),
            mix: default_mix(),
        }
    }
}
