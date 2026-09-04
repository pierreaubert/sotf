use super::default::default_bit_depth;
use super::default::default_dither_type;
use super::default::default_noise_shaping;
use crate::params::{BIT_DEPTH_LABELS, DITHER_TYPE_LABELS};
use serde::{Deserialize, Serialize};
use sotf_host::define_choice_index_deserializer;

define_choice_index_deserializer!(deserialize_bit_depth, BIT_DEPTH_LABELS);
define_choice_index_deserializer!(deserialize_dither_type, DITHER_TYPE_LABELS);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DitherPluginParams {
    #[serde(default = "default_bit_depth", deserialize_with = "deserialize_bit_depth")]
    pub bit_depth: usize,
    #[serde(default = "default_noise_shaping")]
    pub noise_shaping: bool,
    #[serde(
        default = "default_dither_type",
        deserialize_with = "deserialize_dither_type"
    )]
    pub dither_type: usize,
}
