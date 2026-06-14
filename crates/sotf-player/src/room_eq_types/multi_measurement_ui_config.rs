use super::bootstrap_uncertainty_ui_config::BootstrapUncertaintyUiConfig;
use serde::{Deserialize, Serialize};

/// Multi-measurement optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiMeasurementUiConfig {
    pub enabled: bool,
    pub strategy: String,
    pub variance_lambda: f64,
    pub weights: Vec<f64>,
    /// Bootstrap-uncertainty configuration (used when strategy = "minimax_uncertainty").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_uncertainty: Option<BootstrapUncertaintyUiConfig>,
}

impl Default for MultiMeasurementUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "average".to_string(),
            variance_lambda: 1.0,
            weights: Vec::new(),
            bootstrap_uncertainty: None,
        }
    }
}
