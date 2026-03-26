// ============================================================================
// Parameter Automation
// ============================================================================

use crate::parameters::ParameterId;
use serde::{Deserialize, Serialize};

/// Automation mode for a parameter
///
/// Defines who controls parameter changes:
/// - **Host**: DAW automation writes parameter changes
/// - **Plugin**: Plugin generates its own parameter changes (LFOs, envelopes)
/// - **Mixed**: Both host and plugin can modify the parameter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    use super::AutomationCurve;

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

#[cfg(test)]
mod tests {
    use super::*;
    use super::automation_utils::*;

    #[test]
    fn test_linear_curve_midpoint() {
        // Linear curve from 0.0 to 1.0, evaluate at the midpoint
        let curve = AutomationCurve::Linear {
            values: vec![0.0, 1.0],
        };
        let num_frames = 1000;
        let mid = num_frames / 2;
        let val = eval_curve(&curve, mid, num_frames);
        assert!(
            (val - 0.5).abs() < 0.01,
            "Linear curve at midpoint should be ~0.5, got {}",
            val
        );
    }

    #[test]
    fn test_linear_curve_endpoints() {
        let curve = AutomationCurve::Linear {
            values: vec![2.0, 8.0],
        };
        let num_frames = 1000;
        let val_start = eval_curve(&curve, 0, num_frames);
        assert!(
            (val_start - 2.0).abs() < 0.01,
            "Linear curve at start should be ~2.0, got {}",
            val_start
        );
    }

    #[test]
    fn test_linear_curve_single_value() {
        let curve = AutomationCurve::Linear {
            values: vec![42.0],
        };
        let val = eval_curve(&curve, 500, 1000);
        assert_eq!(val, 42.0, "Single-value linear curve should return that value");
    }

    #[test]
    fn test_linear_curve_empty() {
        let curve = AutomationCurve::Linear {
            values: vec![],
        };
        let val = eval_curve(&curve, 500, 1000);
        assert_eq!(val, 0.0, "Empty linear curve should return 0.0");
    }

    #[test]
    fn test_step_curve() {
        let curve = AutomationCurve::Step {
            values: vec![1.0, 2.0, 3.0],
            samples_per_step: 100,
        };
        let val0 = eval_curve(&curve, 0, 1000);
        assert_eq!(val0, 1.0, "Step 0 should be 1.0");
        let val1 = eval_curve(&curve, 100, 1000);
        assert_eq!(val1, 2.0, "Step 1 should be 2.0");
        let val2 = eval_curve(&curve, 200, 1000);
        assert_eq!(val2, 3.0, "Step 2 should be 3.0");
        // Beyond the last step: should hold last value
        let val_beyond = eval_curve(&curve, 500, 1000);
        assert_eq!(val_beyond, 3.0, "Beyond last step should hold 3.0");
    }

    #[test]
    fn test_linear_ramp_helper() {
        let curve = linear_ramp(0.0, 10.0, 11);
        match &curve {
            AutomationCurve::Linear { values } => {
                assert_eq!(values.len(), 11);
                assert!((values[0] - 0.0).abs() < 1e-6);
                assert!((values[5] - 5.0).abs() < 1e-6);
                assert!((values[10] - 10.0).abs() < 1e-6);
            }
            _ => panic!("linear_ramp should produce AutomationCurve::Linear"),
        }
    }

    #[test]
    fn test_parameter_smoother_exponential() {
        let mut smoother = ParameterSmoother::new(0.0, 10.0, 48000.0);
        smoother.set_target(1.0);
        // After many samples, should converge to target
        for _ in 0..48000 {
            smoother.process();
        }
        assert!(
            (smoother.value() - 1.0).abs() < 0.001,
            "Smoother should converge to target, got {}",
            smoother.value()
        );
    }

    #[test]
    fn test_parameter_smoother_reset() {
        let mut smoother = ParameterSmoother::new(0.0, 10.0, 48000.0);
        smoother.set_target(1.0);
        for _ in 0..1000 {
            smoother.process();
        }
        smoother.reset(5.0);
        assert_eq!(smoother.value(), 5.0, "After reset, value should be 5.0");
    }

    #[test]
    fn test_linear_curve_multi_block_progression() {
        // A linear ramp from 0.0 to 1.0 with 11 values.
        // total_frames = 11 * block_size. Evaluating at successive positions
        // should produce a smooth ramp, not immediately jump to the last value.
        let curve = linear_ramp(0.0, 1.0, 11);
        let block_size = 512;
        let total_frames = 11 * block_size;

        let val_start = eval_curve(&curve, 0, total_frames);
        assert!(
            val_start.abs() < 0.01,
            "Start of ramp should be ~0.0, got {val_start}"
        );

        let val_mid = eval_curve(&curve, total_frames / 2, total_frames);
        assert!(
            (val_mid - 0.5).abs() < 0.1,
            "Midpoint of ramp should be ~0.5, got {val_mid}"
        );

        let val_end = eval_curve(&curve, total_frames - 1, total_frames);
        assert!(
            val_end > 0.9,
            "End of ramp should be ~1.0, got {val_end}"
        );

        // Verify monotonic increase across several positions
        let mut prev = 0.0f32;
        for i in 0..=10 {
            let pos = i * block_size;
            let val = eval_curve(&curve, pos, total_frames);
            assert!(
                val >= prev - 0.01,
                "Ramp should be monotonic: pos={pos}, val={val}, prev={prev}"
            );
            prev = val;
        }
    }
}
