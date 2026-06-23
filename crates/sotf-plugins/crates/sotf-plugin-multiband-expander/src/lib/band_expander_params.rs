use crate::params::default_active;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandExpanderParams {
    pub threshold_db: Option<f32>,
    pub ratio: Option<f32>,
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    pub knee_db: Option<f32>,
    pub range_db: Option<f32>,
    pub hysteresis_db: Option<f32>,
    pub hold_ms: Option<f32>,
    #[serde(default)]
    pub auto_makeup: bool,
    #[serde(default)]
    pub measured_auto_makeup: bool,
    #[serde(default = "default_active")]
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
}

impl Default for BandExpanderParams {
    fn default() -> Self {
        Self {
            threshold_db: None,
            ratio: None,
            attack_ms: None,
            release_ms: None,
            knee_db: None,
            range_db: None,
            hysteresis_db: None,
            hold_ms: None,
            auto_makeup: false,
            measured_auto_makeup: false,
            active: true,
            solo: false,
            bypass: false,
        }
    }
}
