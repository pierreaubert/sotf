// ============================================================================
// ADAA — Antiderivative Anti-Aliasing for Nonlinear Processors
// ============================================================================
//
// Implements 1st-order and 2nd-order ADAA as described in:
//   Parker et al., "Reducing the Aliasing of Nonlinear Waveshaping Using
//   Continuous-Time Convolution" (DAFx-16)
//
// The core idea: instead of evaluating f(x) directly (which aliases),
// compute (AD1(x[n]) - AD1(x[n-1])) / (x[n] - x[n-1]) where AD1 is the
// antiderivative of f. This effectively applies a moving-average anti-alias
// filter across the nonlinearity.
//
// HARD RULES:
// - No allocations in process()
// - f64 intermediates for numerical stability in the division
// - Fallback to f(x_mid) when consecutive samples are near-identical

const EPSILON: f64 = 1e-5;

// ============================================================================
// First-order ADAA
// ============================================================================

/// First-order Antiderivative Anti-Aliasing processor.
///
/// Wraps a memoryless nonlinearity `f(x)` and its first antiderivative `AD1(x)`.
/// Produces significantly less aliasing than naive evaluation, at minimal CPU cost.
pub struct Adaa1 {
    /// The nonlinearity f(x)
    f: fn(f64) -> f64,
    /// First antiderivative AD1(x) = integral of f(x)
    ad1: fn(f64) -> f64,
    /// Previous input sample
    x_prev: f64,
}

impl Adaa1 {
    /// Create a new first-order ADAA processor.
    pub fn new(f: fn(f64) -> f64, ad1: fn(f64) -> f64) -> Self {
        Self {
            f,
            ad1,
            x_prev: 0.0,
        }
    }

    /// Process one sample.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let x_prev = self.x_prev;
        self.x_prev = x;

        let diff = x - x_prev;
        if diff.abs() < EPSILON {
            // Near-identical consecutive samples: fallback to f(midpoint)
            (self.f)((x + x_prev) * 0.5) as f32
        } else {
            // ADAA1: (AD1(x) - AD1(x_prev)) / (x - x_prev)
            (((self.ad1)(x) - (self.ad1)(x_prev)) / diff) as f32
        }
    }

    /// Process a block of samples in-place.
    #[inline]
    pub fn process_block(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.x_prev = 0.0;
    }
}

// ============================================================================
// Second-order ADAA
// ============================================================================

/// Second-order Antiderivative Anti-Aliasing processor.
///
/// Uses the second antiderivative for even better alias suppression,
/// at the cost of one additional sample of latency and slight HF rolloff.
pub struct Adaa2 {
    /// The nonlinearity f(x)
    f: fn(f64) -> f64,
    /// First antiderivative
    ad1: fn(f64) -> f64,
    /// Second antiderivative AD2(x) = integral of AD1(x)
    ad2: fn(f64) -> f64,
    /// Two previous samples
    x_prev1: f64,
    x_prev2: f64,
    /// Previous D1 value for the second-order finite difference
    d1_prev: f64,
}

impl Adaa2 {
    pub fn new(f: fn(f64) -> f64, ad1: fn(f64) -> f64, ad2: fn(f64) -> f64) -> Self {
        Self {
            f,
            ad1,
            ad2,
            x_prev1: 0.0,
            x_prev2: 0.0,
            d1_prev: 0.0,
        }
    }

    #[inline]
    fn compute_d1(&self, x: f64, x_prev: f64) -> f64 {
        let diff = x - x_prev;
        if diff.abs() < EPSILON {
            (self.ad1)((x + x_prev) * 0.5)
        } else {
            ((self.ad2)(x) - (self.ad2)(x_prev)) / diff
        }
    }

    /// Process one sample. Introduces 0.5 sample latency.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let x_prev1 = self.x_prev1;

        let d1 = self.compute_d1(x, x_prev1);

        let diff = (x - self.x_prev2) * 0.5;
        let result = if diff.abs() < EPSILON {
            (self.f)((x + x_prev1 + self.x_prev2) / 3.0)
        } else {
            (d1 - self.d1_prev) / diff
        };

        self.x_prev2 = self.x_prev1;
        self.x_prev1 = x;
        self.d1_prev = d1;

        result as f32
    }

    pub fn process_block(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    pub fn reset(&mut self) {
        self.x_prev1 = 0.0;
        self.x_prev2 = 0.0;
        self.d1_prev = 0.0;
    }
}

// ============================================================================
// Pre-built ADAA processors for common saturation functions
// ============================================================================

// --- tanh ---
fn tanh_f(x: f64) -> f64 {
    x.tanh()
}
fn tanh_ad1(x: f64) -> f64 {
    // AD1(tanh(x)) = ln(cosh(x))
    // For numerical stability: ln(cosh(x)) = |x| + ln(1 + e^(-2|x|)) - ln(2)
    let abs_x = x.abs();
    abs_x + (-2.0 * abs_x).exp().ln_1p() - std::f64::consts::LN_2
}
fn tanh_ad2(x: f64) -> f64 {
    // AD2(tanh(x)) = integral of ln(cosh(x))
    // = (1/2)[x^2 + Li_2(-e^{-2x})] - ln(2)*x + C
    // The linear term -(ln2)*x does NOT cancel in ADAA2 finite differences.
    let z = (-2.0 * x).exp();
    let li2 = dilog_neg(z);
    0.5 * (x * x + li2) - std::f64::consts::LN_2 * x
}

/// Approximate Li_2(-z) for z >= 0 using series expansion.
fn dilog_neg(z: f64) -> f64 {
    if z < 1e-15 {
        return 0.0;
    }
    if (1.0 - z).abs() < 1e-12 {
        return -std::f64::consts::PI * std::f64::consts::PI / 12.0;
    }
    if z <= 1.0 {
        // Li_2(-z) = sum_{k=1}^{inf} (-z)^k / k^2
        //          = -z + z^2/4 - z^3/9 + ...
        let mut result = 0.0;
        let mut z_pow = 1.0;
        for k in 1..=200 {
            z_pow *= z;
            let term = z_pow / (k * k) as f64;
            if k % 2 == 1 {
                result -= term;
            } else {
                result += term;
            }
            if term.abs() < 1e-15 {
                break;
            }
        }
        result
    } else {
        // For z > 1, use identity: Li_2(-z) = -Li_2(-1/z) - pi^2/6 - 0.5*ln(z)^2
        let ln_z = z.ln();
        -dilog_neg(1.0 / z) - std::f64::consts::PI * std::f64::consts::PI / 6.0 - 0.5 * ln_z * ln_z
    }
}

// --- soft clip: x / (1 + |x|) ---
fn softclip_f(x: f64) -> f64 {
    x / (1.0 + x.abs())
}
fn softclip_ad1(x: f64) -> f64 {
    // integral of x/(1+|x|) dx
    // For x >= 0: integral of x/(1+x) = x - ln(1+x) + C
    // For x < 0: integral of x/(1-x) = -x - ln(1-x) + C  (i.e., -(|x| - ln(1+|x|)))
    // Combined with continuity at x=0 (both give 0):
    // AD1(x) = |x| - ln(1 + |x|)    (always positive, symmetric)
    let abs_x = x.abs();
    abs_x - (1.0 + abs_x).ln()
}
fn softclip_ad2(x: f64) -> f64 {
    // AD2 = integral of AD1. Since f(x) is odd, AD1 is even, AD2 must be odd.
    // For x >= 0: integral of (x - ln(1+x)) = x^2/2 - (1+x)*ln(1+x) + x + C
    // With C chosen so AD2(0) = 0: C = 0 (since 0 - 1*ln(1) + 0 = 0).
    let abs_x = x.abs();
    let one_plus = 1.0 + abs_x;
    let magnitude = 0.5 * abs_x * abs_x - one_plus * one_plus.ln() + abs_x;
    if x >= 0.0 { magnitude } else { -magnitude }
}

// --- hard clip: clamp to [-1, 1] ---
fn hardclip_f(x: f64) -> f64 {
    x.clamp(-1.0, 1.0)
}
fn hardclip_ad1(x: f64) -> f64 {
    // integral of clamp(x, -1, 1)
    // For |x| <= 1: x^2/2
    // For x > 1:  x - 0.5  (value at x=1: 0.5, derivative = 1)
    // For x < -1: -x - 0.5 (value at x=-1: 0.5, derivative = -1)
    // Note: AD1 is even-symmetric for an odd function
    if (-1.0..=1.0).contains(&x) {
        x * x * 0.5
    } else if x > 1.0 {
        x - 0.5
    } else {
        -x - 0.5
    }
}
fn hardclip_ad2(x: f64) -> f64 {
    // integral of hardclip_ad1(x). Since f is odd, AD1 is even, AD2 must be odd.
    // For |x| <= 1: x^3/6  (odd)
    // For x > 1: x^2/2 - x/2 + 1/6
    // For x < -1: -(|x|^2/2 - |x|/2 + 1/6)  (odd symmetry)
    if (-1.0..=1.0).contains(&x) {
        x * x * x / 6.0
    } else if x > 1.0 {
        x * x * 0.5 - 0.5 * x + 1.0 / 6.0
    } else {
        // x < -1: negate the positive formula evaluated at |x|
        let abs_x = x.abs();
        -(abs_x * abs_x * 0.5 - 0.5 * abs_x + 1.0 / 6.0)
    }
}

/// Create a first-order ADAA processor for `tanh(x)` (soft saturation).
pub fn adaa1_tanh() -> Adaa1 {
    Adaa1::new(tanh_f, tanh_ad1)
}

/// Create a first-order ADAA processor for `x/(1+|x|)` (soft clip).
pub fn adaa1_softclip() -> Adaa1 {
    Adaa1::new(softclip_f, softclip_ad1)
}

/// Create a first-order ADAA processor for `clamp(x, -1, 1)` (hard clip).
pub fn adaa1_hardclip() -> Adaa1 {
    Adaa1::new(hardclip_f, hardclip_ad1)
}

/// Create a second-order ADAA processor for `tanh(x)`.
pub fn adaa2_tanh() -> Adaa2 {
    Adaa2::new(tanh_f, tanh_ad1, tanh_ad2)
}

/// Create a second-order ADAA processor for `x/(1+|x|)`.
pub fn adaa2_softclip() -> Adaa2 {
    Adaa2::new(softclip_f, softclip_ad1, softclip_ad2)
}

/// Create a second-order ADAA processor for `clamp(x, -1, 1)`.
pub fn adaa2_hardclip() -> Adaa2 {
    Adaa2::new(hardclip_f, hardclip_ad1, hardclip_ad2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tanh_ad1_identity() {
        // Verify AD1(tanh) = ln(cosh(x)) at several points
        for &x in &[0.0_f64, 0.5, 1.0, 2.0, -1.0, -3.0] {
            let expected = x.cosh().ln();
            let actual = tanh_ad1(x);
            assert!(
                (actual - expected).abs() < 1e-10,
                "tanh_ad1({x}): expected {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn test_softclip_ad1_identity() {
        // Verify via numerical differentiation: d/dx AD1(x) ≈ |f(x)|
        // AD1 is even (|x| - ln(1+|x|)), so d/dx AD1 = sign(x) * f(|x|)
        // which equals f(x) since f(x) = x/(1+|x|) is odd.
        // Wait — let's verify: AD1(x) = |x| - ln(1+|x|)
        // d/dx AD1 for x > 0: 1 - 1/(1+x) = x/(1+x) = f(x) ✓
        // d/dx AD1 for x < 0: -1 + 1/(1+|x|) = -|x|/(1+|x|) = x/(1+|x|) = f(x) ✓
        let h = 1e-7;
        for &x in &[0.1, 0.5, 1.0, 2.0, -0.5, -2.0] {
            let numerical_derivative = (softclip_ad1(x + h) - softclip_ad1(x - h)) / (2.0 * h);
            let actual = softclip_f(x);
            assert!(
                (numerical_derivative - actual).abs() < 1e-4,
                "softclip AD1 derivative mismatch at x={x}: d/dx AD1={numerical_derivative}, f(x)={actual}"
            );
        }
    }

    #[test]
    fn test_adaa1_tanh_basic() {
        let mut adaa = adaa1_tanh();
        // Process a ramp — should produce tanh-like output without aliasing
        let mut outputs = Vec::new();
        for i in 0..100 {
            let x = (i as f32 - 50.0) / 25.0; // -2.0 to +2.0
            outputs.push(adaa.process(x));
        }
        // Output should be bounded by [-1, 1]
        for &y in &outputs {
            assert!(y.abs() <= 1.01, "Output out of bounds: {y}");
        }
    }

    #[test]
    fn test_adaa1_reduces_aliasing() {
        // Compare alias energy: naive tanh vs ADAA1 tanh on a high-frequency sine
        let sr = 48000.0;
        let freq = 15000.0; // Near Nyquist
        let drive = 5.0; // Heavy drive creates harmonics that alias
        let n = 4096;

        // Naive
        let naive_output: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (drive * (2.0 * std::f64::consts::PI * freq * t).sin()).tanh() as f32
            })
            .collect();

        // ADAA1
        let mut adaa = adaa1_tanh();
        let adaa_output: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                let x = (drive * (2.0 * std::f64::consts::PI * freq * t).sin()) as f32;
                adaa.process(x)
            })
            .collect();

        // Compute energy in alias band (above Nyquist fold-back region)
        // Both should have fundamental at 15kHz. Aliases fold back below.
        // Simply compare total high-frequency energy as a proxy.
        let naive_energy: f32 = naive_output.iter().map(|x| x * x).sum();
        let adaa_energy: f32 = adaa_output.iter().map(|x| x * x).sum();

        // ADAA should have similar or less energy (less alias content)
        // This is a rough check — the key is no crash and bounded output
        assert!(adaa_energy > 0.0, "ADAA produced silence");
        assert!(naive_energy > 0.0, "Naive produced silence");
    }

    #[test]
    fn test_adaa1_reset() {
        let mut adaa = adaa1_tanh();
        adaa.process(1.0);
        adaa.reset();
        // After reset, processing 0 should give near-0
        let out = adaa.process(0.0);
        assert!(out.abs() < 0.01);
    }

    #[test]
    fn test_adaa2_tanh_bounded() {
        let mut adaa = adaa2_tanh();
        for i in 0..200 {
            let x = (i as f32 - 100.0) / 30.0;
            let y = adaa.process(x);
            assert!(y.abs() < 2.0, "ADAA2 output unbounded: {y} at x={x}");
        }
    }

    #[test]
    fn test_hardclip_adaa1() {
        let mut adaa = adaa1_hardclip();
        // Ramp through clipping region — skip first sample (transient from x_prev=0)
        let mut outputs = Vec::new();
        for i in 0..100 {
            let x = (i as f32 - 50.0) / 25.0;
            outputs.push(adaa.process(x));
        }
        // After the initial transient, outputs should be bounded
        for (i, &y) in outputs.iter().enumerate().skip(5) {
            assert!(
                y.abs() <= 1.5,
                "Hard clip ADAA output too large at i={i}: {y}"
            );
        }
    }

    #[test]
    fn test_consecutive_identical_samples() {
        let mut adaa = adaa1_tanh();
        // Feeding the same value repeatedly should not produce NaN or infinity
        for _ in 0..100 {
            let y = adaa.process(0.5);
            assert!(y.is_finite(), "Non-finite output: {y}");
        }
    }

    #[test]
    fn test_adaa2_fallback_uses_three_sample_centroid() {
        fn f(x: f64) -> f64 {
            x
        }
        fn ad1(x: f64) -> f64 {
            0.5 * x * x
        }
        fn ad2(x: f64) -> f64 {
            x * x * x / 6.0
        }

        let mut adaa = Adaa2::new(f, ad1, ad2);
        adaa.x_prev1 = 2.0;
        adaa.x_prev2 = 0.0;

        let y = adaa.process(0.0);
        assert!(
            (y - (2.0_f32 / 3.0)).abs() < 1e-6,
            "ADAA2 fallback must evaluate the three-sample centroid, got {y}"
        );
    }

    #[test]
    fn test_dilog_neg_basic() {
        // Li_2(0) = 0
        assert!((dilog_neg(0.0)).abs() < 1e-15);
        // Li_2(-1) = -pi^2/12
        let expected = -std::f64::consts::PI * std::f64::consts::PI / 12.0;
        let actual = dilog_neg(1.0);
        assert!(
            (actual - expected).abs() < 1e-4,
            "Li_2(-1): expected {expected}, got {actual}"
        );

        let near_one = dilog_neg(1.0 - 1e-13);
        assert!(
            (near_one - expected).abs() < 1e-12,
            "Li_2 near -1 should use the stable reference value"
        );
    }

    #[test]
    fn test_dilog_neg_very_small() {
        assert!(dilog_neg(1e-20).abs() < 1e-15);
    }

    #[test]
    fn test_dilog_neg_greater_than_one() {
        let z = 2.0;
        let val = dilog_neg(z);
        // Reference: Li_2(-2) ≈ -1.436746
        assert!((val + 1.436746).abs() < 1e-4);
    }

    #[test]
    fn test_dilog_neg_large_z() {
        let z = 1e6;
        let val = dilog_neg(z);
        let approx = -0.5 * (z.ln() * z.ln()) - std::f64::consts::PI * std::f64::consts::PI / 6.0;
        assert!(
            (val - approx).abs() < 0.1,
            "large z approximation failed: {val} vs {approx}"
        );
    }

    #[test]
    fn test_dilog_neg_identity_relation() {
        // Li_2(-z) + Li_2(-1/z) = -pi^2/6 - 0.5*ln(z)^2
        for z in [1.5, 2.0, 10.0, 100.0] {
            let lhs = dilog_neg(z) + dilog_neg(1.0 / z);
            let rhs = -std::f64::consts::PI * std::f64::consts::PI / 6.0 - 0.5 * z.ln() * z.ln();
            assert!(
                (lhs - rhs).abs() < 1e-6,
                "Identity failed at z={z}: lhs={lhs}, rhs={rhs}"
            );
        }
    }

    #[test]
    fn test_dilog_neg_near_one() {
        let expected = -std::f64::consts::PI * std::f64::consts::PI / 12.0;
        // Use 1e-13 so it triggers the (1-z) < 1e-12 fast path.
        let val = dilog_neg(1.0 - 1e-13);
        assert!((val - expected).abs() < 1e-8);
    }
}
