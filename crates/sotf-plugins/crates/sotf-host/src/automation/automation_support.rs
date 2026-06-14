use super::types::AutomationCurve;
use super::types::AutomationMode;
use crate::parameters::ParameterId;

/// Trait for plugins that support parameter automation
///
/// This trait enables plugins to:
/// - Receive automation data from DAW hosts
/// - Generate internal parameter changes (LFOs, envelopes)
/// - Smooth parameter transitions to prevent clicks
///
/// # Example
/// ```rust,ignore
/// use sotf_plugins::{AutomationSupport, AutomationCurve, AutomationMode};
///
/// impl AutomationSupport for LfoPlugin {
///     fn set_automation_curve(&mut self, param_id: &ParameterId, curve: AutomationCurve) {
///         if param_id == &self.param_frequency {
///             match curve {
///                 AutomationCurve::Linear { values } => {
///                     // Set up frequency automation
///                 }
///                 _ => {}
///             }
///         }
///     }
/// }
/// ```
pub trait AutomationSupport {
    /// Get the automation mode for a specific parameter
    ///
    /// Returns the current automation mode, or `AutomationMode::Host` if not set.
    fn automation_mode(&self, param_id: &ParameterId) -> AutomationMode;

    /// Set the automation mode for a parameter
    fn set_automation_mode(&mut self, param_id: ParameterId, mode: AutomationMode);

    /// Set an automation curve for a parameter
    ///
    /// The curve defines how the parameter value changes over time.
    /// This is used for:
    /// - DAW automation playback
    /// - Plugin-generated parameter changes (LFO, envelope)
    /// - Smooth parameter transitions
    fn set_automation_curve(&mut self, param_id: ParameterId, curve: AutomationCurve);

    /// Get the current parameter value with automation applied
    ///
    /// # Arguments
    /// * `param_id` - The parameter ID
    /// * `sample` - Current sample position (for curve evaluation)
    ///
    /// Returns the parameter value after applying automation curves.
    fn get_automated_value(&self, param_id: &ParameterId, sample: usize) -> f32;

    /// Clear all automation for a parameter
    fn clear_automation(&mut self, param_id: &ParameterId);

    /// Clear all automation
    fn clear_all_automation(&mut self);

    /// Get all parameters with automation
    fn automated_parameters(&self) -> Vec<&ParameterId>;
}
