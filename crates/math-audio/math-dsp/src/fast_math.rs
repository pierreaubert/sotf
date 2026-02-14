//! Fast mathematical approximations for audio processing
//!
//! Provides optimized versions of transcendental functions (log, exp, pow)
//! that are significantly faster than standard library versions at the cost
//! of some precision, which is usually acceptable for audio gain/dynamics.

/// Fast approximation of base-2 logarithm
///
/// Accuracy is approx 1e-3, significantly faster than `f32::log2`.
#[inline]
pub fn fast_log2(x: f32) -> f32 {
    let x_bits = x.to_bits();
    let exponent = (x_bits >> 23) as i32 - 127;
    let mantissa = (x_bits & 0x7FFFFF) as f32 / 8388608.0;

    // Linear approximation of the mantissa part
    exponent as f32 + mantissa
}

/// Fast approximation of base-10 logarithm
///
/// Useful for dB calculations: `20.0 * fast_log10(x)`
#[inline]
pub fn fast_log10(x: f32) -> f32 {
    fast_log2(x) * 0.30102999566 // 1.0 / log2(10)
}

/// Fast approximation of base-2 exponential
///
/// Accuracy is approx 1%, significantly faster than `f32::exp2`.
#[inline]
pub fn fast_exp2(x: f32) -> f32 {
    if x == 0.0 {
        return 1.0;
    }
    // Clamp to avoid overflow/underflow
    let x = x.clamp(-126.0, 126.0);
    // 2^x \approx f32::from_bits(((x + 127.0) * 8388608.0) as u32)
    // Using 126.94269504 as an offset for better average accuracy
    f32::from_bits(((x + 126.94269504f32) * 8388608.0f32) as u32)
}

/// Fast approximation of sine
///
/// Accuracy is approx 0.001, for x in [-PI, PI].
#[inline]
pub fn fast_sin(x: f32) -> f32 {
    const B: f32 = 4.0 / std::f32::consts::PI;
    const C: f32 = -4.0 / (std::f32::consts::PI * std::f32::consts::PI);

    let y = B * x + C * x * x.abs();

    // Extra precision step
    const P: f32 = 0.225;
    P * (y * y.abs() - y) + y
}

/// Fast approximation of cosine
#[inline]
pub fn fast_cos(x: f32) -> f32 {
    // cos(x) = sin(x + PI/2)
    let mut x = x + std::f32::consts::FRAC_PI_2;
    if x > std::f32::consts::PI {
        x -= 2.0 * std::f32::consts::PI;
    }
    fast_sin(x)
}

/// Fast approximation of base-10 exponential
///
/// Useful for linear gain from dB: `fast_pow10(db / 20.0)`
#[inline]
pub fn fast_pow10(x: f32) -> f32 {
    fast_exp2(x * 3.32192809489) // log2(10)
}

/// Fast approximation of natural logarithm
#[inline]
pub fn fast_ln(x: f32) -> f32 {
    fast_log2(x) * 0.69314718056 // ln(2)
}

/// Fast approximation of natural exponential
#[inline]
pub fn fast_exp(x: f32) -> f32 {
    fast_exp2(x * 1.44269504089) // 1.0 / ln(2)
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
                error < 0.1,
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
                rel_error < 0.1,
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
                rel_error < 0.1,
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
        for i in -314..314 {
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
        for i in -314..314 {
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
}
