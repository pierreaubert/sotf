use super::misc::default_num_bands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandMergePluginParams {
    #[serde(default = "default_num_bands")]
    pub bands: usize,
    /// Per-band gain in dB. Defaults to 0.0 (unity) for each band.
    #[serde(default)]
    pub band_gains_db: Vec<f32>,
    /// Per-band mute flags. Defaults to false (unmuted) for each band.
    #[serde(default)]
    pub band_mutes: Vec<bool>,
}
