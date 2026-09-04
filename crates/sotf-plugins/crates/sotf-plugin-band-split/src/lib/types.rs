use super::default::default_crossover_type;
use super::default::default_frequency;
use super::default::default_num_bands;
use crate::params::CROSSOVER_TYPES;
use serde::{Deserialize, Serialize};
use sotf_host::define_choice_string_deserializer;

define_choice_string_deserializer!(deserialize_crossover_type, CROSSOVER_TYPES);

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

    #[serde(
        rename = "type",
        alias = "crossover_type",
        default = "default_crossover_type",
        deserialize_with = "deserialize_crossover_type"
    )]
    pub crossover_type: String,
}
