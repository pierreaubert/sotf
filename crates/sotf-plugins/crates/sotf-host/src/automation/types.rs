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

/// Utility functions for automation
pub mod automation_utils {

    use super::super::{AutomationCurve, BezierPoint};

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
