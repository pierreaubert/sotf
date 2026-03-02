// ============================================================================
// PND Plugin Configuration
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{find_by_key as pk, pnd::PARAMS as PD};

pub fn default_correction_strength() -> f32 {
    pk(PD, "correction_strength").default_f64() as f32
}

pub fn default_analysis_window_ms() -> f32 {
    pk(PD, "analysis_window_ms").default_f64() as f32
}

pub fn default_drift_smoothing() -> f32 {
    pk(PD, "drift_smoothing").default_f64() as f32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PndPluginParams {
    #[serde(default = "default_correction_strength")]
    pub correction_strength: f32,

    #[serde(default = "default_analysis_window_ms")]
    pub analysis_window_ms: f32,

    #[serde(default = "default_drift_smoothing")]
    pub drift_smoothing: f32,
}

impl Default for PndPluginParams {
    fn default() -> Self {
        Self {
            correction_strength: default_correction_strength(),
            analysis_window_ms: default_analysis_window_ms(),
            drift_smoothing: default_drift_smoothing(),
        }
    }
}
