use super::default::default_crossover_type;
use super::default::default_frequency;
use super::default::default_num_bands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BandSplitPluginParams {
    /// Crossover frequencies. Length determines the number of bands (len + 1).
    /// For backwards compatibility, a single frequency creates 2 bands.
    #[serde(default)]
    pub frequencies: Vec<f64>,

    /// Legacy single-frequency field (used when `frequencies` is empty).
    #[serde(default = "default_frequency")]
    pub frequency: f64,

    /// Number of bands (2-4). Ignored when `frequencies` is provided with > 1 element.
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,

    #[serde(rename = "type", default = "default_crossover_type")]
    pub crossover_type: String,
}
