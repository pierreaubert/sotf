// ============================================================================
// Measured Auto-Makeup Gain — Based on actual gain reduction, not heuristics
// ============================================================================

/// Measured auto-makeup gain that tracks the actual average gain reduction
/// and compensates for it, rather than using a fixed heuristic like
/// `threshold × 0.5 × slope`.
///
/// Uses an exponential moving average (EMA) of the applied gain reduction
/// to determine how much makeup gain to apply.
#[derive(Debug, Clone, Copy)]
pub struct MeasuredMakeup {
    /// Smoothed average gain reduction in dB (positive = reduction).
    avg_gr_db: f32,
    /// EMA coefficient for smoothing.
    coeff: f32,
}

impl MeasuredMakeup {
    /// Create a new measured auto-makeup.
    ///
    /// * `smoothing_ms` — EMA time constant (e.g., 1000ms for slow tracking)
    /// * `sample_rate` — audio sample rate
    pub fn new(smoothing_ms: f32, sample_rate: u32) -> Self {
        let coeff = if smoothing_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (smoothing_ms * 0.001 * sample_rate as f32)).exp()
        };
        Self {
            avg_gr_db: 0.0,
            coeff,
        }
    }

    /// Update with the current gain reduction in dB (positive = reduction).
    ///
    /// Call this once per sample (or once per frame with the frame's GR).
    #[inline]
    pub fn update(&mut self, gain_reduction_db: f32) {
        let gr = gain_reduction_db.max(0.0);
        self.avg_gr_db = gr + self.coeff * (self.avg_gr_db - gr);
    }

    /// Get the makeup gain in dB that compensates for average reduction.
    #[inline]
    pub fn makeup_db(&self) -> f32 {
        self.avg_gr_db
    }

    /// Get the makeup gain as a linear multiplier.
    #[inline]
    pub fn makeup_linear(&self) -> f32 {
        // 10^(avg_gr_db / 20)
        fast_pow10(self.avg_gr_db / 20.0)
    }

    pub fn reset(&mut self) {
        self.avg_gr_db = 0.0;
    }

    /// Update smoothing time (e.g., when sample rate changes).
    pub fn set_smoothing(&mut self, smoothing_ms: f32, sample_rate: u32) {
        self.coeff = if smoothing_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (smoothing_ms * 0.001 * sample_rate as f32)).exp()
        };
    }
}

/// Fast 10^x approximation for real-time audio.
#[inline]
fn fast_pow10(x: f32) -> f32 {
    // 10^x = e^(x * ln10)
    (x * std::f32::consts::LN_10).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_reduction() {
        let mut m = MeasuredMakeup::new(100.0, 48000);
        for _ in 0..48000 {
            m.update(0.0);
        }
        assert!(m.makeup_db().abs() < 0.01);
        assert!((m.makeup_linear() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_steady_reduction() {
        let mut m = MeasuredMakeup::new(100.0, 48000);
        // Feed 6dB reduction for a long time
        for _ in 0..480000 {
            m.update(6.0);
        }
        // Should converge to ~6 dB makeup
        assert!((m.makeup_db() - 6.0).abs() < 0.1);
        // Linear: 10^(6/20) ≈ 1.995
        assert!((m.makeup_linear() - 1.995).abs() < 0.05);
    }

    #[test]
    fn test_reset() {
        let mut m = MeasuredMakeup::new(100.0, 48000);
        for _ in 0..48000 {
            m.update(10.0);
        }
        m.reset();
        assert!(m.makeup_db().abs() < 0.01);
    }
}
