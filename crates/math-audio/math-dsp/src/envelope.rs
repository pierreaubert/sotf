// ============================================================================
// Dual-Release Envelope — Program-dependent release for dynamics plugins
// ============================================================================

/// A dual-release envelope follower that switches between fast and slow release
/// based on the behavior of the gain reduction signal.
///
/// When gain reduction increases quickly (transients), fast release is used.
/// When gain reduction is sustained, slow release is used.
/// This prevents pumping on sustained signals while still allowing fast recovery
/// from transients.
#[derive(Debug, Clone, Copy)]
pub struct DualRelease {
    fast_coeff: f32,
    slow_coeff: f32,
    /// The current blended release coefficient.
    current_coeff: f32,
    /// Smoothed measure of how "sustained" the gain reduction is.
    sustain_tracker: f32,
    /// Coefficient for the sustain tracker's own smoothing.
    sustain_coeff: f32,
    /// Previous gain reduction value for derivative estimation.
    prev_gr: f32,
}

impl DualRelease {
    /// Create a new dual-release envelope.
    ///
    /// * `fast_ms` — fast release time (e.g., 50ms)
    /// * `slow_ms` — slow release time (e.g., 500ms)
    /// * `sample_rate` — audio sample rate
    pub fn new(fast_ms: f32, slow_ms: f32, sample_rate: u32) -> Self {
        Self {
            fast_coeff: Self::time_to_coeff(fast_ms, sample_rate),
            slow_coeff: Self::time_to_coeff(slow_ms, sample_rate),
            current_coeff: Self::time_to_coeff(fast_ms, sample_rate),
            sustain_tracker: 0.0,
            sustain_coeff: Self::time_to_coeff(200.0, sample_rate), // 200ms integration
            prev_gr: 0.0,
        }
    }

    fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
        }
    }

    /// Process one sample of gain reduction and return the appropriate release coefficient.
    ///
    /// `gain_reduction_db` should be positive when gain is being reduced.
    #[inline]
    pub fn process(&mut self, gain_reduction_db: f32) -> f32 {
        // Estimate the "sustain-ness": when GR is steady, sustain→1; when changing fast, sustain→0
        let delta = (gain_reduction_db - self.prev_gr).abs();
        self.prev_gr = gain_reduction_db;

        // Fast changes → low sustain, steady → high sustain
        // Absolute threshold: if delta < 0.01 dB/sample, treat as sustained.
        // Typical compressor attack: 6 dB over 480 samples = 0.0125 dB/sample.
        let sustained = if delta < 0.01 { 1.0 } else { 0.0 };
        self.sustain_tracker = sustained + self.sustain_coeff * (self.sustain_tracker - sustained);

        // Blend between fast and slow based on sustain level
        let blend = self.sustain_tracker.clamp(0.0, 1.0);
        self.current_coeff = self.fast_coeff * (1.0 - blend) + self.slow_coeff * blend;

        self.current_coeff
    }

    /// Get the current blended release coefficient.
    #[inline]
    pub fn coeff(&self) -> f32 {
        self.current_coeff
    }

    /// Update the fast and slow release times.
    pub fn set_times(&mut self, fast_ms: f32, slow_ms: f32, sample_rate: u32) {
        self.fast_coeff = Self::time_to_coeff(fast_ms, sample_rate);
        self.slow_coeff = Self::time_to_coeff(slow_ms, sample_rate);
    }

    pub fn reset(&mut self) {
        self.sustain_tracker = 0.0;
        self.prev_gr = 0.0;
        self.current_coeff = self.fast_coeff;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_release_on_transient() {
        let mut dr = DualRelease::new(50.0, 500.0, 48000);
        // Sudden large change → should use fast release
        let coeff = dr.process(20.0);
        // Fast coeff at 50ms
        let expected_fast = (-1.0f32 / (50.0 * 0.001 * 48000.0)).exp();
        assert!((coeff - expected_fast).abs() < 0.01);
    }

    #[test]
    fn test_slow_release_on_sustained() {
        let mut dr = DualRelease::new(50.0, 500.0, 48000);
        // Feed steady GR for many samples
        for _ in 0..48000 {
            dr.process(10.0);
        }
        let coeff = dr.coeff();
        let expected_slow = (-1.0f32 / (500.0 * 0.001 * 48000.0)).exp();
        // Should be close to slow coeff after sustained signal
        assert!((coeff - expected_slow).abs() < 0.01);
    }

    #[test]
    fn test_reset() {
        let mut dr = DualRelease::new(50.0, 500.0, 48000);
        for _ in 0..10000 {
            dr.process(10.0);
        }
        dr.reset();
        assert_eq!(dr.sustain_tracker, 0.0);
        assert_eq!(dr.prev_gr, 0.0);
    }
}
