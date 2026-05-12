//! Fast mathematical approximations for audio processing
//!
//! Provides optimized versions of transcendental functions (log, exp, pow)
//! that are significantly faster than standard library versions at the cost
//! of some precision, which is usually acceptable for audio gain/dynamics.
//!
//! Accuracy targets (chosen to keep roundtrip drift negligible over long sessions):
//! - `fast_log2`: max error ~0.001, mean bias ~1e-4
//! - `fast_exp2`: max relative error ~0.08%, mean bias ~1e-4
//! - `fast_sin`/`fast_cos`: max error ~0.001

/// Fast approximation of base-2 logarithm
///
/// Uses IEEE 754 bit extraction with a cubic mantissa correction.
/// Minimax-fitted on [0, 1] with constraint poly(0)=0, poly(1)=1.
/// Max error ~0.001 across the full f32 positive range.
#[inline]
pub fn fast_log2(x: f32) -> f32 {
    let x_bits = x.to_bits();
    let exponent = (x_bits >> 23) as i32 - 127;
    let m = (x_bits & 0x7FFFFF) as f32 / 8388608.0;

    // Cubic minimax: log2(1+m) ≈ a*m + b*m² + c*m³, a+b+c=1
    // Grid-searched to minimize max error on [0,1]
    exponent as f32 + m * (1.4230 + m * (-0.5825 + m * 0.1595))
}

/// Fast approximation of base-10 logarithm
///
/// Useful for dB calculations: `20.0 * fast_log10(x)`
#[inline]
pub fn fast_log10(x: f32) -> f32 {
    fast_log2(x) * std::f32::consts::LOG10_2
}

/// Fast approximation of base-2 exponential.
///
/// Splits x into integer and fractional parts, uses bit manipulation for the
/// integer part and a degree-4 Taylor polynomial for the fractional part.
/// Max relative error ~0.08%.
///
/// # Caveat
///
/// The input is silently clamped to `[-126, 126]` to prevent IEEE-754
/// overflow/underflow.  Values outside this range saturate to the nearest
/// boundary rather than returning `inf` or `0`.
#[inline]
pub fn fast_exp2(x: f32) -> f32 {
    // Clamp to avoid overflow/underflow
    let x = x.clamp(-126.0, 126.0);

    // Split into integer and fractional parts
    let xi = x.floor();
    let xf = x - xi;

    // 2^integer via bit manipulation
    let int_part = f32::from_bits(((xi as i32 + 127) as u32) << 23);

    // 2^fraction via degree-4 Taylor polynomial on [0, 1)
    // Coefficients: ln(2)^k / k! for k=1..4
    #[allow(clippy::approx_constant)]
    // Intentional: hand-tuned polynomial coefficient, not a usage of LN_2
    let frac_part =
        1.0 + xf * (0.693_147_2 + xf * (0.240_226_5 + xf * (0.055_504_1 + xf * 0.009_618_1)));

    int_part * frac_part
}

/// Fast approximation of sine
///
/// Uses a parabolic approximation with a precision step.
/// Input is reduced to [-π, π] so any value is accepted.
/// Max error ~0.001.
#[inline]
pub fn fast_sin(x: f32) -> f32 {
    use std::f32::consts::PI;

    // Range reduction to [-π, π]
    let x = x - (2.0 * PI) * ((x + PI) / (2.0 * PI)).floor();

    const B: f32 = 4.0 / PI;
    const C: f32 = -4.0 / (PI * PI);

    let y = B * x + C * x * x.abs();

    // Extra precision step
    const P: f32 = 0.225;
    P * (y * y.abs() - y) + y
}

/// Fast approximation of cosine
#[inline]
pub fn fast_cos(x: f32) -> f32 {
    fast_sin(x + std::f32::consts::FRAC_PI_2)
}

/// Fast approximation of atan2(y, x)
///
/// Max error ~0.001 rad (0.05 degrees).
/// Based on a rational approximation for atan(x) on [0, 1].
#[inline]
pub fn fast_atan2(y: f32, x: f32) -> f32 {
    use std::f32::consts::PI;

    if x == 0.0 {
        if y > 0.0 {
            return PI / 2.0;
        }
        if y < 0.0 {
            return -PI / 2.0;
        }
        return 0.0;
    }

    let abs_y = y.abs();
    let abs_x = x.abs();
    let (a, b) = if abs_x > abs_y {
        (abs_y, abs_x)
    } else {
        (abs_x, abs_y)
    };

    // Rational approximation for atan(z) where z = a/b is in [0, 1]
    // atan(z) ≈ z * (π/4 + (1-z) * (0.273 + 0.07 * z))
    // This is very fast and has max error ~0.0015 rad.
    let z = a / b;
    let mut atan = z * (std::f32::consts::FRAC_PI_4 + (1.0 - z) * (0.273 + 0.07 * z));

    if abs_y > abs_x {
        atan = (PI / 2.0) - atan;
    }
    if x < 0.0 {
        atan = PI - atan;
    }
    if y < 0.0 {
        atan = -atan;
    }
    atan
}

/// Fast approximation of base-10 exponential
///
/// Useful for linear gain from dB: `fast_pow10(db / 20.0)`
#[inline]
pub fn fast_pow10(x: f32) -> f32 {
    fast_exp2(x * std::f32::consts::LOG2_10)
}

/// Fast approximation of natural logarithm
#[inline]
pub fn fast_ln(x: f32) -> f32 {
    fast_log2(x) * std::f32::consts::LN_2
}

/// Fast approximation of natural exponential
#[inline]
pub fn fast_exp(x: f32) -> f32 {
    fast_exp2(x * std::f32::consts::LOG2_E)
}

/// Fast approximation of x^y
///
/// Uses log2/exp2 identity: x^y = 2^(y * log2(x))
#[inline]
pub fn fast_powf(x: f32, y: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    fast_exp2(y * fast_log2(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_log2() {
        for i in 1..100 {
            let x = i as f32 * 0.1;
            let actual = x.log2();
            let approx = fast_log2(x);
            let error = (actual - approx).abs();
            assert!(
                error < 0.002,
                "log2 error at {}: {} vs {} (err: {})",
                x,
                actual,
                approx,
                error
            );
        }
    }

    #[test]
    fn test_fast_exp2() {
        for i in -20..20 {
            let x = i as f32 * 0.1;
            let actual = x.exp2();
            let approx = fast_exp2(x);
            let rel_error = (actual - approx).abs() / actual;
            assert!(
                rel_error < 0.001,
                "exp2 error at {}: {} vs {} (rel err: {})",
                x,
                actual,
                approx,
                rel_error
            );
        }
    }

    #[test]
    fn test_fast_pow10() {
        for db in -60..12 {
            let x = db as f32 / 20.0;
            let actual = 10.0_f32.powf(x);
            let approx = fast_pow10(x);
            let rel_error = (actual - approx).abs() / actual;
            assert!(
                rel_error < 0.003,
                "pow10 error at {}dB: {} vs {} (rel err: {})",
                db,
                actual,
                approx,
                rel_error
            );
        }
    }

    #[test]
    fn test_fast_sin() {
        // Test well beyond [-PI, PI] to verify range reduction
        for i in -1000..1000 {
            let x = i as f32 * 0.01;
            let actual = x.sin();
            let approx = fast_sin(x);
            let error = (actual - approx).abs();
            assert!(
                error < 0.01,
                "sin error at {}: {} vs {} (err: {})",
                x,
                actual,
                approx,
                error
            );
        }
    }

    #[test]
    fn test_fast_cos() {
        // Test well beyond [-PI, PI] to verify range reduction
        for i in -1000..1000 {
            let x = i as f32 * 0.01;
            let actual = x.cos();
            let approx = fast_cos(x);
            let error = (actual - approx).abs();
            assert!(
                error < 0.01,
                "cos error at {}: {} vs {} (err: {})",
                x,
                actual,
                approx,
                error
            );
        }
    }

    #[test]
    fn test_fast_atan2() {
        for yi in -10..10 {
            for xi in -10..10 {
                let y = yi as f32 * 0.5;
                let x = xi as f32 * 0.5;
                if x == 0.0 && y == 0.0 {
                    continue;
                }
                let actual = y.atan2(x);
                let approx = fast_atan2(y, x);
                let error = (actual - approx).abs();
                assert!(
                    error < 0.01,
                    "atan2 error at ({}, {}): {} vs {} (err: {})",
                    y,
                    x,
                    actual,
                    approx,
                    error
                );
            }
        }
    }
}
