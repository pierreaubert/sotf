/// First-order shelving filter for frequency-dependent decay in the FDN.
///
/// Each FDN delay line needs a different RT60 at bass vs treble frequencies.
/// This filter sits inside the feedback loop and applies per-sample gain that
/// varies with frequency, creating the desired frequency-dependent decay.
///
/// Design: given target gains at DC (g_dc) and Nyquist (g_ny), compute
/// first-order IIR coefficients that interpolate between them.

/// First-order IIR tone correction filter.
///
/// Transfer function: `H(z) = b0 + b1*z^-1 / (1 + a1*z^-1)`
///
/// Designed so that `|H(0)| = g_dc` and `|H(pi)| = g_ny`.
pub struct ToneFilter {
    b0: f32,
    b1: f32,
    a1: f32,
    /// Filter state
    x1: f32,
    y1: f32,
}

impl ToneFilter {
    /// Design a tone correction filter from target gains at DC and Nyquist.
    ///
    /// - `g_dc`: desired gain at 0 Hz (controls bass RT60)
    /// - `g_ny`: desired gain at Nyquist (controls treble RT60)
    ///
    /// Both must be in (0, 1) for a stable decaying FDN.
    pub fn new(g_dc: f32, g_ny: f32) -> Self {
        // First-order filter design from two magnitude constraints:
        //   |H(z=1)| = g_dc  → (b0 + b1) / (1 + a1) = g_dc
        //   |H(z=-1)| = g_ny → (b0 - b1) / (1 - a1) = g_ny
        //
        // Solving with a1 as free parameter using the Jot (1991) approach:
        // Set the pole to create a smooth transition between g_dc and g_ny.
        //
        // We use the formulation from Jot's FDN reverberator:
        //   a1 = (g_dc - g_ny) / (g_dc + g_ny)  (approximate, works well for |g_dc - g_ny| small)
        //   Then solve for b0, b1 from the two constraints.

        let sum = g_dc + g_ny;
        let diff = g_dc - g_ny;

        if sum.abs() < 1e-10 {
            // Both gains essentially zero — pass nothing
            return Self {
                b0: 0.0,
                b1: 0.0,
                a1: 0.0,
                x1: 0.0,
                y1: 0.0,
            };
        }

        // Pole position
        let a1 = -diff / sum;

        // From H(1) = g_dc: (b0 + b1) = g_dc * (1 + a1)
        // From H(-1) = g_ny: (b0 - b1) = g_ny * (1 - a1)
        let h_dc = g_dc * (1.0 + a1);
        let h_ny = g_ny * (1.0 - a1);

        let b0 = (h_dc + h_ny) * 0.5;
        let b1 = (h_dc - h_ny) * 0.5;

        Self {
            b0,
            b1,
            a1,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Create a unity (bypass) filter.
    pub fn unity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            a1: 0.0,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Process one sample.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = input;
        self.y1 = output;
        output
    }

    /// Update filter coefficients (e.g., when RT60 parameters change).
    pub fn set_gains(&mut self, g_dc: f32, g_ny: f32) {
        let sum = g_dc + g_ny;
        let diff = g_dc - g_ny;
        if sum.abs() < 1e-10 {
            self.b0 = 0.0;
            self.b1 = 0.0;
            self.a1 = 0.0;
            return;
        }
        self.a1 = -diff / sum;
        let h_dc = g_dc * (1.0 + self.a1);
        let h_ny = g_ny * (1.0 - self.a1);
        self.b0 = (h_dc + h_ny) * 0.5;
        self.b1 = (h_dc - h_ny) * 0.5;
    }

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unity_filter() {
        let mut f = ToneFilter::unity();
        for i in 0..100 {
            let input = (i as f32 * 0.1).sin();
            let output = f.process(input);
            assert!(
                (output - input).abs() < 1e-5,
                "Unity filter should pass through, got {output} vs {input}"
            );
        }
    }

    #[test]
    fn test_dc_gain() {
        let g_dc = 0.95;
        let g_ny = 0.8;
        let mut f = ToneFilter::new(g_dc, g_ny);

        // Feed DC signal (constant 1.0) and check steady-state gain
        let mut output = 0.0;
        for _ in 0..10000 {
            output = f.process(1.0);
        }
        assert!(
            (output - g_dc).abs() < 0.01,
            "DC gain should be ~{g_dc}, got {output}"
        );
    }

    #[test]
    fn test_nyquist_gain() {
        let g_dc = 0.95;
        let g_ny = 0.7;
        let mut f = ToneFilter::new(g_dc, g_ny);

        // Feed Nyquist signal (alternating +1, -1)
        let mut output = 0.0;
        for i in 0..10000 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            output = f.process(input);
        }
        assert!(
            (output.abs() - g_ny).abs() < 0.01,
            "Nyquist gain should be ~{g_ny}, got {}",
            output.abs()
        );
    }

    #[test]
    fn test_both_gains_equal() {
        let g = 0.9;
        let mut f = ToneFilter::new(g, g);

        // Should behave like a simple scalar multiply
        let mut output = 0.0;
        for _ in 0..1000 {
            output = f.process(1.0);
        }
        assert!(
            (output - g).abs() < 0.01,
            "Equal gains should give flat response at {g}, got {output}"
        );
    }

    #[test]
    fn test_zero_gains() {
        let mut f = ToneFilter::new(0.0, 0.0);
        for _ in 0..100 {
            let output = f.process(1.0);
            assert!(output.abs() < 1e-10);
        }
    }

    #[test]
    fn test_reset() {
        let mut f = ToneFilter::new(0.95, 0.8);
        for _ in 0..100 {
            f.process(1.0);
        }
        f.reset();
        // After reset, first sample should be b0 * input (no history)
        let out = f.process(1.0);
        assert!((out - f.b0).abs() < 1e-6);
    }
}
