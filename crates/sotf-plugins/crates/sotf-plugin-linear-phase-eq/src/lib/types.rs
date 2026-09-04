use super::default::default_active;
use super::default::default_filter_type;
use super::default::default_fir_length_index;
use super::default::default_frequency;
use super::default::default_mix;
use super::default::default_num_filters;
use super::default::default_phase_mode_index;
use super::default::default_q;
use crate::params::{FIR_LENGTH_OPTIONS, PHASE_MODE_OPTIONS};
use serde::{Deserialize, Serialize};
use sotf_host::define_choice_index_deserializer;

define_choice_index_deserializer!(deserialize_fir_length_index, FIR_LENGTH_OPTIONS);
define_choice_index_deserializer!(deserialize_phase_mode_index, PHASE_MODE_OPTIONS);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearPhaseEqPluginParams {
    #[serde(default = "default_num_filters")]
    pub num_filters: usize,
    #[serde(
        default = "default_fir_length_index",
        alias = "fir_length",
        deserialize_with = "deserialize_fir_length_index"
    )]
    pub fir_length_index: usize,
    #[serde(
        default = "default_phase_mode_index",
        alias = "phase_mode",
        deserialize_with = "deserialize_phase_mode_index"
    )]
    pub phase_mode_index: usize,
    #[serde(default)]
    pub auto_gain: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default)]
    pub filters: Vec<BandConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandConfig {
    #[serde(default = "default_filter_type")]
    pub filter_type: String,
    #[serde(default = "default_frequency")]
    pub frequency: f64,
    #[serde(default = "default_q")]
    pub q: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default = "default_active")]
    pub active: bool,
}
