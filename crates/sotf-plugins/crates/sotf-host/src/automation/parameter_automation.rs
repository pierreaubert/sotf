use super::types::AutomationCurve;
use super::types::AutomationMode;
use crate::parameters::ParameterId;

/// Automation state for a single parameter
#[derive(Debug, Clone)]
pub struct ParameterAutomation {
    /// Parameter ID
    pub param_id: ParameterId,

    /// Current automation mode
    pub mode: AutomationMode,

    /// Current automation curve (if any)
    pub curve: Option<AutomationCurve>,

    /// Current position in the automation curve (in samples)
    pub position: usize,

    /// Base parameter value (before automation is applied)
    pub base_value: f32,

    /// Last value written by automation
    pub last_value: f32,
}

impl Default for ParameterAutomation {
    fn default() -> Self {
        Self {
            param_id: ParameterId::from(""),
            mode: AutomationMode::Host,
            curve: None,
            position: 0,
            base_value: 0.0,
            last_value: 0.0,
        }
    }
}
