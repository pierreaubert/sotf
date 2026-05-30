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

    /// Smoothing duration expressed in samples.
    smoothing_samples: f32,

    /// Smoothing mode
    mode: SmoothingMode,

    /// Per-sample increment used by linear smoothing.
    linear_step: f32,

    /// Velocity term used by the critically damped mode.
    critical_velocity: f32,

    /// Initial error at the moment the critical damping target was set.
    critical_initial_error: f32,

    /// Number of processed samples since the last critical damping target update.
    critical_elapsed_samples: u32,
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
        let smoothing_samples = smoothing_samples(time_ms, sample_rate);
        let coeff = smoothing_coeff(smoothing_samples);

        Self {
            current: initial_value,
            target: initial_value,
            coeff,
            smoothing_samples,
            mode: SmoothingMode::Exponential,
            linear_step: 0.0,
            critical_velocity: 0.0,
            critical_initial_error: 0.0,
            critical_elapsed_samples: 0,
        }
    }

    /// Set the smoothing time
    ///
    /// # Arguments
    /// * `time_ms` - Smoothing time in milliseconds
    /// * `sample_rate` - Sample rate in Hz
    pub fn set_time(&mut self, time_ms: f32, sample_rate: f32) {
        self.smoothing_samples = smoothing_samples(time_ms, sample_rate);
        self.coeff = smoothing_coeff(self.smoothing_samples);
        self.reset_critical_damping_state();
        self.recompute_linear_step();
    }

    /// Set the smoothing mode.
    pub fn set_mode(&mut self, mode: SmoothingMode) {
        if self.mode != mode {
            self.mode = mode;
            self.critical_velocity = 0.0;
            self.reset_critical_damping_state();
            self.recompute_linear_step();
        }
    }

    /// Set the target value
    #[inline]
    pub fn set_target(&mut self, value: f32) {
        self.target = value;
        self.critical_velocity = 0.0;
        self.reset_critical_damping_state();
        self.recompute_linear_step();
    }

    /// Process one sample
    ///
    /// Returns the smoothed value.
    #[inline]
    pub fn process(&mut self) -> f32 {
        self.current = match self.mode {
            SmoothingMode::Exponential => self.current + self.coeff * (self.target - self.current),
            SmoothingMode::Linear => {
                if self.coeff == 0.0 {
                    self.target
                } else if (self.target - self.current).abs() <= self.linear_step.abs() {
                    self.linear_step = 0.0;
                    self.target
                } else {
                    self.current + self.linear_step
                }
            }
            SmoothingMode::CriticalDamping => self.process_critical_damped(),
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
        self.linear_step = 0.0;
        self.critical_velocity = 0.0;
        self.reset_critical_damping_state();
    }

    fn reset_critical_damping_state(&mut self) {
        self.critical_initial_error = self.target - self.current;
        self.critical_elapsed_samples = 0;
    }

    fn process_critical_damped(&mut self) -> f32 {
        let damping_rate = if self.smoothing_samples > 0.0 {
            10.0 / self.smoothing_samples
        } else {
            0.0
        };

        if damping_rate == 0.0 || self.critical_initial_error == 0.0 {
            self.critical_initial_error = 0.0;
            self.critical_elapsed_samples = 0;
            return self.target;
        }

        self.critical_elapsed_samples = self.critical_elapsed_samples.saturating_add(1);
        let t = self.critical_elapsed_samples as f32 * damping_rate;
        let damping = (1.0 + t) * (-t).exp();
        let error = damping * self.critical_initial_error;
        let previous = self.current;
        let next = self.target - error;

        self.current = next;
        self.critical_velocity = next - previous;

        if (next - self.target).abs() < 1e-6 {
            self.critical_initial_error = 0.0;
            self.critical_elapsed_samples = 0;
            return self.target;
        } else {
            self.current
        }
    }

    fn recompute_linear_step(&mut self) {
        if self.mode == SmoothingMode::Linear && self.coeff != 0.0 {
            self.linear_step = (self.target - self.current) / self.smoothing_samples.max(1.0);
        } else {
            self.linear_step = 0.0;
        }
    }
}

fn smoothing_samples(time_ms: f32, sample_rate: f32) -> f32 {
    if time_ms > 0.0 && sample_rate > 0.0 {
        (time_ms * sample_rate / 1000.0).max(1.0)
    } else {
        0.0
    }
}

fn smoothing_coeff(samples: f32) -> f32 {
    if samples > 0.0 {
        1.0 - (-1.0 / samples).exp()
    } else {
        0.0
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
                let step_len = if *samples_per_step > 0 {
                    *samples_per_step
                } else {
                    num_frames.max(1)
                };
                let step = sample / step_len;
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
                if num_frames == 0 {
                    return values[0];
                }
                let num_segments = values.len() - 1;
                let scaled = sample.saturating_mul(num_segments);
                let segment = (scaled / num_frames).min(num_segments - 1);
                let t = (sample * num_segments % num_frames) as f32 / num_frames as f32;
                let start = values[segment];
                let end = values[segment + 1];
                start + (end - start) * t
            }
            AutomationCurve::Bezier { points } => eval_bezier(points, sample, num_frames),
            AutomationCurve::Exponential { values, min_value } => {
                if values.is_empty() {
                    return *min_value;
                }
                if values.len() == 1 {
                    return values[0].max(*min_value);
                }
                if num_frames == 0 {
                    return values[0].max(*min_value);
                }
                let num_segments = values.len() - 1;
                let scaled = sample.saturating_mul(num_segments);
                let segment = (scaled / num_frames).min(num_segments - 1);
                let t = (sample * num_segments % num_frames) as f32 / num_frames as f32;
                let start = values[segment].max(*min_value).ln();
                let end = values[segment + 1].max(*min_value).ln();
                (start + (end - start) * t).exp()
            }
        }
    }

    fn eval_bezier(points: &[BezierPoint], sample: usize, num_frames: usize) -> f32 {
        if points.is_empty() {
            return 0.0;
        }
        if points.len() == 1 {
            return points[0].value;
        }

        if sample <= points[0].position {
            return points[0].value;
        }

        for pair in points.windows(2) {
            let p0 = &pair[0];
            let p1 = &pair[1];
            if sample <= p1.position {
                let span = p1.position.saturating_sub(p0.position);
                let t = if span > 0 {
                    (sample.saturating_sub(p0.position) as f32 / span as f32).clamp(0.0, 1.0)
                } else {
                    let fallback_span = num_frames.max(1) as f32;
                    (sample as f32 / fallback_span).clamp(0.0, 1.0)
                };
                return cubic_bezier_value(
                    p0.value,
                    p0.value + p0.handle_right,
                    p1.value + p1.handle_left,
                    p1.value,
                    t,
                );
            }
        }

        points.last().map_or(0.0, |p| p.value)
    }

    #[inline]
    fn cubic_bezier_value(p0: f32, c0: f32, c1: f32, p1: f32, t: f32) -> f32 {
        let omt = 1.0 - t;
        omt * omt * omt * p0 + 3.0 * omt * omt * t * c0 + 3.0 * omt * t * t * c1 + t * t * t * p1
    }

    /// Create a linear automation curve from start to end value
    pub fn linear_ramp(start_value: f32, end_value: f32, num_steps: usize) -> AutomationCurve {
        if num_steps == 0 {
            return AutomationCurve::Linear { values: Vec::new() };
        }
        if num_steps == 1 {
            return AutomationCurve::Linear {
                values: vec![start_value],
            };
        }
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
    use super::automation_utils::*;
    use super::*;

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
        let curve = AutomationCurve::Linear { values: vec![42.0] };
        let val = eval_curve(&curve, 500, 1000);
        assert_eq!(
            val, 42.0,
            "Single-value linear curve should return that value"
        );
    }

    #[test]
    fn test_linear_curve_empty() {
        let curve = AutomationCurve::Linear { values: vec![] };
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
    fn test_linear_ramp_single_step_is_finite() {
        let curve = linear_ramp(3.5, 9.0, 1);
        match curve {
            AutomationCurve::Linear { values } => {
                assert_eq!(values, vec![3.5]);
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
    fn bezier_curve_uses_handles_between_points() {
        let curve = AutomationCurve::Bezier {
            points: vec![
                BezierPoint {
                    position: 0,
                    value: 0.0,
                    handle_left: 0.0,
                    handle_right: 1.0,
                },
                BezierPoint {
                    position: 100,
                    value: 1.0,
                    handle_left: -1.0,
                    handle_right: 0.0,
                },
            ],
        };

        let midpoint = eval_curve(&curve, 50, 100);
        assert!(
            (midpoint - 0.5).abs() < 0.01,
            "symmetric handles should place midpoint near 0.5, got {midpoint}"
        );

        let plain_linear_midpoint = 0.5;
        let early = eval_curve(&curve, 25, 100);
        assert!(
            early > plain_linear_midpoint * 0.5,
            "right handle should pull the curve upward before midpoint, got {early}"
        );
    }

    #[test]
    fn linear_smoother_reaches_target_in_configured_time_for_large_diffs() {
        let mut smoother = ParameterSmoother::new(0.0, 10.0, 48_000.0);
        smoother.set_mode(SmoothingMode::Linear);
        smoother.set_target(10.0);

        for _ in 0..480 {
            smoother.process();
        }

        assert!(
            (smoother.value() - 10.0).abs() < 1e-4,
            "linear smoothing should complete in 10ms independent of delta, got {}",
            smoother.value()
        );
    }

    #[test]
    fn critical_damping_is_distinct_and_does_not_overshoot() {
        let mut exp = ParameterSmoother::new(0.0, 10.0, 48_000.0);
        let mut crit = ParameterSmoother::new(0.0, 10.0, 48_000.0);
        crit.set_mode(SmoothingMode::CriticalDamping);
        exp.set_target(1.0);
        crit.set_target(1.0);

        let mut last = 0.0;
        for _ in 0..480 {
            exp.process();
            let v = crit.process();
            assert!(v >= last - 1e-6, "critical damping should be monotonic");
            assert!(v <= 1.0 + 1e-6, "critical damping must not overshoot");
            last = v;
        }

        assert!(
            (crit.value() - exp.value()).abs() > 0.01,
            "critical damping should not collapse to exponential smoothing"
        );
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
        assert!(val_end > 0.9, "End of ramp should be ~1.0, got {val_end}");

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

    #[test]
    fn test_linear_smoothing_uses_remaining_delta() {
        let mut smoother = ParameterSmoother::new(0.0, 1000.0, 1000.0);
        smoother.mode = SmoothingMode::Linear;
        smoother.set_target(1.0);

        let mut values = [0.0f32; 5];
        for value in &mut values {
            *value = smoother.process();
        }

        assert!(
            (values[0] - 0.001).abs() < 1e-6,
            "first step should be one-thousandth, got {}",
            values[0]
        );
        assert!(
            (values[4] - 0.005).abs() < 1e-6,
            "fifth step should be five-thousandths, got {}",
            values[4]
        );
    }

    #[test]
    fn test_critical_damping_no_overshoot() {
        let mut smoother = ParameterSmoother::new(0.0, 20.0, 48000.0);
        smoother.mode = SmoothingMode::CriticalDamping;
        smoother.set_target(1.0);

        let mut max = smoother.value();
        for _ in 0..500 {
            let v = smoother.process();
            if v > max {
                max = v;
            }
        }

        assert!(
            max <= 1.0001,
            "critical mode should not overshoot, max={max}"
        );
        assert!((smoother.value() - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_bezier_curve_supports_in_between_segments() {
        let curve = AutomationCurve::Bezier {
            points: vec![
                BezierPoint {
                    position: 0,
                    value: 0.0,
                    handle_left: 0.0,
                    handle_right: 1.0,
                },
                BezierPoint {
                    position: 50,
                    value: 1.0,
                    handle_left: -1.0,
                    handle_right: 0.0,
                },
                BezierPoint {
                    position: 100,
                    value: 0.0,
                    handle_left: -1.0,
                    handle_right: 0.0,
                },
            ],
        };

        assert_eq!(eval_curve(&curve, 0, 100), 0.0);
        let middle = eval_curve(&curve, 25, 100);
        assert!(
            (middle - 0.5).abs() < 1e-5,
            "middle value changed unexpectedly: {middle}"
        );
        assert_eq!(eval_curve(&curve, 100, 100), 0.0);
    }
}
