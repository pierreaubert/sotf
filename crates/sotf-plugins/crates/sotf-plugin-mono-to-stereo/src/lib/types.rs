use super::default::{
    default_decor_high_hz, default_decor_low_hz, default_freq_dependent, default_haas_delay_ms,
    default_stereo_width,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonoToStereoPluginParams {
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_freq_dependent")]
    pub freq_dependent: bool,
    #[serde(default = "default_haas_delay_ms")]
    pub haas_delay_ms: f32,
    #[serde(default = "default_decor_low_hz")]
    pub decor_low_hz: f32,
    #[serde(default = "default_decor_high_hz")]
    pub decor_high_hz: f32,
}

impl Default for MonoToStereoPluginParams {
    fn default() -> Self {
        Self {
            stereo_width: default_stereo_width(),
            freq_dependent: default_freq_dependent(),
            haas_delay_ms: default_haas_delay_ms(),
            decor_low_hz: default_decor_low_hz(),
            decor_high_hz: default_decor_high_hz(),
        }
    }
}
