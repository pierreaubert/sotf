// ============================================================================
// Parameter Smoothing
// ============================================================================

/// Simple one-pole smoothing filter for control parameters to prevent zipper noise.
#[derive(Debug, Clone, Copy)]
pub struct Smoother {
    target: f32,
    current: f32,
    coeff: f32,
}

#[allow(dead_code)]
impl Smoother {
    /// Create a new smoother
    /// time_ms: Smoothing time constant (e.g., 10ms - 50ms)
    pub fn new(value: f32, time_ms: f32, sample_rate: u32) -> Self {
        let coeff = Self::calculate_coeff(time_ms, sample_rate);
        Self {
            target: value,
            current: value,
            coeff,
        }
    }

    fn calculate_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            // Standard one-pole coeff: e^(-1 / (tau * fs))
            // time_ms is roughly time to reach ~63% of target
            (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
        }
    }

    pub fn set_time(&mut self, time_ms: f32, sample_rate: u32) {
        self.coeff = Self::calculate_coeff(time_ms, sample_rate);
    }

    /// Set new target value
    pub fn set_target(&mut self, value: f32) {
        self.target = value;
        // If smoothing is disabled (coeff = 0), jump immediately
        if self.coeff == 0.0 {
            self.current = value;
        }
    }

    /// Process one sample step (updates current value)
    #[inline]
    pub fn next(&mut self) -> f32 {
        if (self.current - self.target).abs() < 1e-5 {
            self.current = self.target;
        } else {
            self.current = self.target + self.coeff * (self.current - self.target);
        }
        self.current
    }

    /// Get current smoothed value
    #[inline]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Get target value
    #[inline]
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Process one sample step (updates current value) - per-sample smoothing
    /// Returns the smoothed value for this sample
    #[inline]
    pub fn process_sample(&mut self, sample: f32) -> f32 {
        // Apply smoothing to parameter changes, then process input with smoothed gain
        // This gives smooth parameter transitions AND smooth gain application
        if (self.current - self.target).abs() < 1e-5 {
            self.current = self.target;
        } else {
            self.current = self.target + self.coeff * (self.current - self.target);
        }
        sample * self.current
    }

    /// Reset to value immediately
    pub fn reset(&mut self, value: f32) {
        self.target = value;
        self.current = value;
    }
}
