use super::default::default_freq_dependent;
use super::default::default_haas_delay_ms;
use super::default::default_stereo_width;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonoToStereoPluginParams {
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_freq_dependent")]
    pub freq_dependent: bool,
    #[serde(default = "default_haas_delay_ms")]
    pub haas_delay_ms: f32,
}
