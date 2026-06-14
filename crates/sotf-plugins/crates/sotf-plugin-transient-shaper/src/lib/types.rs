use super::default::default_attack;
use super::default::default_mix;
use super::default::default_output_gain;
use super::default::default_sensitivity;
use super::default::default_sustain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransientShaperPluginParams {
    /// -100.0 to +100.0 (percent)
    #[serde(default = "default_attack")]
    pub attack: f32,
    /// -100.0 to +100.0 (percent)
    #[serde(default = "default_sustain")]
    pub sustain: f32,
    #[serde(default = "default_sensitivity")]
    pub sensitivity_db: f32,
    #[serde(default = "default_output_gain")]
    pub output_gain_db: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

#[derive(Debug, Clone, Default)]
pub struct TransientShaperData {
    /// Peak transient level (positive = transient detected)
    pub transient_level: f32,
    /// Peak sustain level
    pub sustain_level: f32,
    /// Current gain applied (linear)
    pub gain: f32,
}
