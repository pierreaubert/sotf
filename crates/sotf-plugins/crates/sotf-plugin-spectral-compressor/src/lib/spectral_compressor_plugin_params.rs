use crate::params::{
    default_attack_ms, default_fft_size_index, default_knee_db, default_mix, default_ratio,
    default_release_ms, default_spectral_smoothing, default_threshold_db,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectralCompressorPluginParams {
    #[serde(default = "default_fft_size_index")]
    pub fft_size_index: usize,
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_spectral_smoothing")]
    pub spectral_smoothing: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default)]
    pub target_mode: usize,
    #[serde(default)]
    pub delta_listen: bool,
    #[serde(default)]
    pub adaptive_threshold: bool,
    #[serde(default)]
    pub adaptive_offset_db: f32,
    #[serde(default)]
    pub channel_link: f32,
}

impl Default for SpectralCompressorPluginParams {
    fn default() -> Self {
        Self {
            fft_size_index: default_fft_size_index(),
            threshold_db: default_threshold_db(),
            ratio: default_ratio(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            knee_db: default_knee_db(),
            spectral_smoothing: default_spectral_smoothing(),
            mix: default_mix(),
            target_mode: 0,
            delta_listen: false,
            adaptive_threshold: false,
            adaptive_offset_db: 0.0,
            channel_link: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_empty_json_uses_param_specs_defaults() {
        let p: SpectralCompressorPluginParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.fft_size_index, default_fft_size_index());
        assert_eq!(p.threshold_db, default_threshold_db());
        assert_eq!(p.ratio, default_ratio());
        assert_eq!(p.attack_ms, default_attack_ms());
        assert_eq!(p.release_ms, default_release_ms());
        assert_eq!(p.knee_db, default_knee_db());
        assert_eq!(p.spectral_smoothing, default_spectral_smoothing());
        assert_eq!(p.mix, default_mix());
    }
}
