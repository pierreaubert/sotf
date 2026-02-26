// ============================================================================
// PND Plugin Configuration
// ============================================================================

use sotf_host::param_specs::pnd::*;
use serde::{Deserialize, Serialize};

pub fn default_correction_strength() -> f32 {
    CORRECTION_STRENGTH_DEFAULT
}

pub fn default_analysis_window_ms() -> f32 {
    ANALYSIS_WINDOW_MS_DEFAULT
}

pub fn default_drift_smoothing() -> f32 {
    DRIFT_SMOOTHING_DEFAULT
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
