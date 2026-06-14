use super::smoothing::smoothing_coeff;
use super::smoothing::smoothing_samples;
use super::types::SmoothingMode;

/// Smoother for parameter transitions
///
/// Used to prevent clicks and pops when parameter values change abruptly.
/// Provides various interpolation modes.
#[derive(Debug, Clone)]
pub struct ParameterSmoother {
    /// Current value
    pub(super) current: f32,

    /// Target value
    pub(super) target: f32,

    /// Smoothing coefficient (0.0 = no smoothing, 1.0 = infinite smoothing)
    pub(super) coeff: f32,

    /// Smoothing duration expressed in samples.
    pub(super) smoothing_samples: f32,

    /// Smoothing mode
    pub(super) mode: SmoothingMode,

    /// Per-sample increment used by linear smoothing.
    pub(super) linear_step: f32,

    /// Velocity term used by the critically damped mode.
    pub(super) critical_velocity: f32,

    /// Initial error at the moment the critical damping target was set.
    pub(super) critical_initial_error: f32,

    /// Number of processed samples since the last critical damping target update.
    pub(super) critical_elapsed_samples: u32,
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

    pub(super) fn reset_critical_damping_state(&mut self) {
        self.critical_initial_error = self.target - self.current;
        self.critical_elapsed_samples = 0;
    }

    pub(super) fn process_critical_damped(&mut self) -> f32 {
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
            self.target
        } else {
            self.current
        }
    }

    pub(super) fn recompute_linear_step(&mut self) {
        if self.mode == SmoothingMode::Linear && self.coeff != 0.0 {
            self.linear_step = (self.target - self.current) / self.smoothing_samples.max(1.0);
        } else {
            self.linear_step = 0.0;
        }
    }
}
