// ============================================================================
// Parameter Automation
// ============================================================================

use super::parameters::ParameterId;
use std::collections::HashMap;

/// Automation mode for a parameter
///
/// Defines who controls parameter changes:
/// - **Host**: DAW automation writes parameter changes
/// - **Plugin**: Plugin generates its own parameter changes (LFOs, envelopes)
/// - **Mixed**: Both host and plugin can modify the parameter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutomationMode {
    /// Parameter is controlled by the host (DAW automation)
    #[default]
    Host,

    /// Parameter is controlled by the plugin internally
    Plugin,

    /// Parameter can be controlled by both host and plugin
    Mixed,
}

/// Curve type for parameter automation
///
/// Defines how parameter values interpolate between control points.
#[derive(Debug, Clone, PartialEq)]
pub enum AutomationCurve {
    /// Hold each value for a specified number of samples
    Step {
        /// Values to hold
        values: Vec<f32>,
        /// Number of samples to hold each value (0 = use frame size)
        samples_per_step: usize,
    },

    /// Linear interpolation between values
    Linear {
        /// Control points (value, position) pairs
        values: Vec<f32>,
    },

    /// Bezier curve interpolation
    Bezier {
        /// Control points with bezier handles
        points: Vec<BezierPoint>,
    },

    /// Exponential interpolation (good for frequency/gain parameters)
    Exponential {
        /// Control values
        values: Vec<f32>,
        /// Minimum value for safety (exponential can't go through 0)
        min_value: f32,
    },
}

/// A point in a Bezier automation curve
///
/// Contains a value and handles for curve shaping.
#[derive(Debug, Clone, PartialEq)]
pub struct BezierPoint {
    /// Position in samples (0 = current position)
    pub position: usize,

    /// Value at this point
    pub value: f32,

    /// Handle position for curve shaping (relative to point)
    pub handle_left: f32,

    /// Handle position for curve shaping (relative to point)
    pub handle_right: f32,
}

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

/// Smoother for parameter transitions
///
/// Used to prevent clicks and pops when parameter values change abruptly.
/// Provides various interpolation modes.
#[derive(Debug, Clone)]
pub struct ParameterSmoother {
    /// Current value
    current: f32,

    /// Target value
    target: f32,

    /// Smoothing coefficient (0.0 = no smoothing, 1.0 = infinite smoothing)
    coeff: f32,

    /// Smoothing mode
    mode: SmoothingMode,
}

/// Smoothing algorithm modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmoothingMode {
    /// Exponential decay (default)
    #[default]
    Exponential,

    /// Linear interpolation
    Linear,

    /// Critical damping (fastest approach without overshoot)
    CriticalDamping,
}

impl ParameterSmoother {
    /// Create a new smoother
    ///
    /// # Arguments
    /// * `initial_value` - Starting value
    /// * `time_ms` - Smoothing time in milliseconds
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(initial_value: f32, time_ms: f32, sample_rate: f32) -> Self {
        let coeff = if time_ms > 0.0 {
            1.0 - (-1.0 / (time_ms * sample_rate / 1000.0)).exp()
        } else {
            0.0
        };

        Self {
            current: initial_value,
            target: initial_value,
            coeff,
            mode: SmoothingMode::Exponential,
        }
    }

    /// Set the smoothing time
    ///
    /// # Arguments
    /// * `time_ms` - Smoothing time in milliseconds
    /// * `sample_rate` - Sample rate in Hz
    pub fn set_time(&mut self, time_ms: f32, sample_rate: f32) {
        self.coeff = if time_ms > 0.0 {
            1.0 - (-1.0 / (time_ms * sample_rate / 1000.0)).exp()
        } else {
            0.0
        };
    }

    /// Set the target value
    #[inline]
    pub fn set_target(&mut self, value: f32) {
        self.target = value;
    }

    /// Process one sample
    ///
    /// Returns the smoothed value.
    #[inline]
    pub fn process(&mut self) -> f32 {
        self.current = match self.mode {
            SmoothingMode::Exponential | SmoothingMode::CriticalDamping => {
                self.current + self.coeff * (self.target - self.current)
            }
            SmoothingMode::Linear => {
                let diff = self.target - self.current;
                if diff.abs() < self.coeff {
                    self.target
                } else {
                    self.current + diff.signum() * self.coeff
                }
            }
        };
        self.current
    }

    /// Get the current value
    #[inline]
    pub fn value(&self) -> f32 {
        self.current
    }

    /// Reset to a specific value
    #[inline]
    pub fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }
}

/// Utility functions for automation
pub mod automation_utils {
    use super::{AutomationCurve, BezierPoint};

    /// Evaluate an automation curve at a given position
    ///
    /// # Arguments
    /// * `curve` - The automation curve
    /// * `sample` - Current sample position
    /// * `num_frames` - Number of frames in the current block
    ///
    /// Returns the value at the given position.
    pub fn eval_curve(curve: &AutomationCurve, sample: usize, num_frames: usize) -> f32 {
        match curve {
            AutomationCurve::Step {
                values,
                samples_per_step,
            } => {
                let step = if *samples_per_step > 0 {
                    sample / *samples_per_step
                } else {
                    sample / num_frames
                };
                values
                    .get(step)
                    .copied()
                    .unwrap_or_else(|| values.last().copied().unwrap_or(0.0))
            }
            AutomationCurve::Linear { values } => {
                if values.is_empty() {
                    return 0.0;
                }
                if values.len() == 1 {
                    return values[0];
                }
                let num_segments = values.len() - 1;
                let segment = (sample * num_segments / num_frames).min(num_segments);
                let t = (sample * num_segments % num_frames) as f32 / num_frames as f32;
                let start = values[segment];
                let end = values[segment + 1];
                start + (end - start) * t
            }
            AutomationCurve::Bezier { points } => {
                if points.is_empty() {
                    return 0.0;
                }
                // Simple Bezier evaluation - find surrounding points
                let pos = sample * points.len() / num_frames;
                let point = points.get(pos).or(points.last()).unwrap();
                point.value
            }
            AutomationCurve::Exponential { values, min_value } => {
                if values.is_empty() {
                    return *min_value;
                }
                if values.len() == 1 {
                    return values[0].max(*min_value);
                }
                let num_segments = values.len() - 1;
                let segment = (sample * num_segments / num_frames).min(num_segments);
                let t = (sample * num_segments % num_frames) as f32 / num_frames as f32;
                let start = values[segment].max(*min_value).ln();
                let end = values[segment + 1].max(*min_value).ln();
                (start + (end - start) * t).exp()
            }
        }
    }

    /// Create a linear automation curve from start to end value
    pub fn linear_ramp(start_value: f32, end_value: f32, num_steps: usize) -> AutomationCurve {
        let values: Vec<f32> = (0..num_steps)
            .map(|i| start_value + (end_value - start_value) * i as f32 / (num_steps - 1) as f32)
            .collect();
        AutomationCurve::Linear { values }
    }

    /// Create a step automation curve
    pub fn step_curve(values: Vec<f32>, samples_per_step: usize) -> AutomationCurve {
        AutomationCurve::Step {
            values,
            samples_per_step,
        }
    }
}
