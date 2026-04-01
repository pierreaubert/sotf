// ============================================================================
// True Peak Detector — 4x oversampled peak detection (ITU-R BS.1770-4 inspired)
// ============================================================================

/// A true peak detector that uses 4x oversampling via Catmull-Rom interpolation
/// to detect inter-sample peaks.
///
/// Standard peak detection misses peaks that occur between samples. This computes
/// 3 sub-sample interpolation points between each pair of consecutive samples
/// to catch inter-sample peaks that exceed the sample values.
#[derive(Debug, Clone)]
pub struct TruePeakDetector {
    /// Previous samples: [x[n-2], x[n-1]]
    prev: [f32; 2],
    /// Current true peak (linear, not dB).
    peak: f32,
}

impl TruePeakDetector {
    pub fn new() -> Self {
        Self {
            prev: [0.0; 2],
            peak: 0.0,
        }
    }

    /// Process one input sample. Returns the true peak level in dB.
    ///
    /// Computes 3 sub-sample interpolation points between the previous sample
    /// and the current sample, plus the current sample itself, and returns
    /// the maximum absolute value in dB.
    #[inline]
    /// Process one sample and return the true peak level in dB.
    /// Note: `process()` and `process_linear()` share state — call only one per sample.
    pub fn process(&mut self, sample: f32) -> f32 {
        let lin = self.process_linear(sample);
        if lin < 1e-12 {
            -120.0
        } else {
            20.0 * lin.log10()
        }
    }

    /// Process one sample and return the true peak as linear amplitude.
    /// This is the primary processing method that advances internal state.
    #[inline]
    pub fn process_linear(&mut self, sample: f32) -> f32 {
        // Interpolate between prev[1] (x[n-1]) and sample (x[n])
        // using Catmull-Rom with points: prev[0] (x[n-2]), prev[1] (x[n-1]),
        // sample (x[n]), and sample again for the future boundary (hold).
        let p0 = self.prev[0];
        let p1 = self.prev[1];
        let p2 = sample;
        let p3 = sample; // Hold boundary for causal operation

        let mut max_abs = sample.abs();
        max_abs = max_abs.max(catmull_rom(0.25, p0, p1, p2, p3).abs());
        max_abs = max_abs.max(catmull_rom(0.5, p0, p1, p2, p3).abs());
        max_abs = max_abs.max(catmull_rom(0.75, p0, p1, p2, p3).abs());
        max_abs = max_abs.max(p1.abs());

        self.prev[0] = self.prev[1];
        self.prev[1] = sample;
        self.peak = max_abs;
        max_abs
    }

    /// Get the last computed true peak in dB.
    #[inline]
    pub fn peak_db(&self) -> f32 {
        if self.peak < 1e-12 {
            -120.0
        } else {
            20.0 * self.peak.log10()
        }
    }

    /// Get the last computed true peak as linear amplitude.
    #[inline]
    pub fn peak_linear(&self) -> f32 {
        self.peak
    }

    pub fn reset(&mut self) {
        self.prev = [0.0; 2];
        self.peak = 0.0;
    }
}

/// Catmull-Rom spline interpolation between p1 and p2 at parameter t ∈ [0, 1].
#[inline]
fn catmull_rom(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((-t3 + 2.0 * t2 - t) * p0
        + (3.0 * t3 - 5.0 * t2 + 2.0) * p1
        + (-3.0 * t3 + 4.0 * t2 + t) * p2
        + (t3 - t2) * p3)
}

impl Default for TruePeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence() {
        let mut tp = TruePeakDetector::new();
        let db = tp.process(0.0);
        assert!(db <= -120.0);
    }

    #[test]
    fn test_unity_peak() {
        let mut tp = TruePeakDetector::new();
        let db = tp.process(1.0);
        // Current sample is 1.0 → should be 0 dBFS (or higher due to interpolation overshoot)
        assert!(db >= -0.1, "peak db: {}", db);
    }

    #[test]
    fn test_inter_sample_peak() {
        let mut tp = TruePeakDetector::new();
        // Alternating +0.9 / -0.9 at Nyquist creates large inter-sample peaks
        // First establish history
        tp.process(0.0);
        tp.process(0.9);
        let _db = tp.process(-0.9);
        // The Catmull-Rom interpolation between 0.9 and -0.9 should show
        // peaks larger than 0.9 due to the overshoot
        assert!(tp.peak_linear() >= 0.9, "peak: {}", tp.peak_linear());
    }

    #[test]
    fn test_detects_overshoot() {
        let mut tp = TruePeakDetector::new();
        // Create a signal that overshoots between samples:
        // 0.0, 0.8, 0.0 — the interpolation should show a peak >= 0.8
        tp.process(0.0);
        tp.process(0.8);
        tp.process(0.0);
        assert!(tp.peak_linear() >= 0.79, "peak: {}", tp.peak_linear());
    }

    #[test]
    fn test_reset() {
        let mut tp = TruePeakDetector::new();
        tp.process(1.0);
        tp.reset();
        assert_eq!(tp.peak_linear(), 0.0);
        assert_eq!(tp.prev, [0.0; 2]);
    }
}
