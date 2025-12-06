//! Spherical Bessel and Hankel functions
//!
//! Implementations of spherical Bessel functions that are critical
//! for 3D wave equation solutions.
//!
//! ## Definitions
//!
//! Spherical Bessel function of first kind:
//! ```text
//! j_n(x) = √(π/2x) * J_{n+1/2}(x)
//! ```
//!
//! Spherical Bessel function of second kind (Neumann):
//! ```text
//! y_n(x) = √(π/2x) * Y_{n+1/2}(x)
//! ```
//!
//! Spherical Hankel function of first kind:
//! ```text
//! h_n^(1)(x) = j_n(x) + i * y_n(x)
//! ```

use num_complex::Complex64;

/// Compute spherical Bessel functions j_n(x) for n = 0, 1, ..., order-1
///
/// Uses Miller's downward recurrence for numerical stability, which is
/// essential when n > x. The recurrence relation is:
/// ```text
/// j_{n-1}(x) = (2n+1)/x * j_n(x) - j_{n+1}(x)
/// ```
///
/// Normalization uses j_0(x) = sin(x)/x.
///
/// # Arguments
/// * `order` - Number of terms (returns j_0 through j_{order-1})
/// * `x` - Argument (must be > 0)
///
/// # Example
/// ```
/// use math_wave::special::spherical_bessel_j_array;
/// let j = spherical_bessel_j_array(5, 1.0);
/// // j[0] = sin(1)/1 ≈ 0.8414709848
/// ```
pub fn spherical_bessel_j_array(order: usize, x: f64) -> Vec<f64> {
    assert!(order >= 1, "Order must be at least 1");

    let mut result = vec![0.0; order];

    // Handle very small x
    if x.abs() < 1e-15 {
        result[0] = 1.0;
        return result;
    }

    if x.abs() < 1e-10 {
        // Series expansion for small x
        result[0] = 1.0 - x * x / 6.0;
        if order > 1 {
            result[1] = x / 3.0;
        }
        for item in result.iter_mut().take(order).skip(2) {
            *item = 0.0;
        }
        return result;
    }

    // Miller's downward recurrence algorithm
    let start_n = order + (x.abs() as usize) + 20;

    let mut values = vec![0.0; start_n + 2];
    values[start_n + 1] = 0.0;
    values[start_n] = 1e-30; // Arbitrary small starting value

    // Downward recurrence: j_{n-1} = (2n+1)/x * j_n - j_{n+1}
    for k in (0..start_n).rev() {
        values[k] = (2 * k + 3) as f64 / x * values[k + 1] - values[k + 2];
    }

    // Normalize using j_0(x) = sin(x)/x
    let true_j0 = x.sin() / x;
    let scale = true_j0 / values[0];

    for n in 0..order {
        result[n] = values[n] * scale;
    }

    result
}

/// Single spherical Bessel function jₙ(x)
pub fn spherical_bessel_j(n: usize, x: f64) -> f64 {
    if x.abs() < 1e-10 {
        return if n == 0 { 1.0 } else { 0.0 };
    }

    match n {
        0 => x.sin() / x,
        1 => x.sin() / (x * x) - x.cos() / x,
        _ => {
            let arr = spherical_bessel_j_array(n + 1, x);
            arr[n]
        }
    }
}

/// Compute spherical Bessel functions y_n(x) (Neumann functions) for n = 0, 1, ..., order-1
///
/// Uses upward recurrence, which is stable for y_n:
/// ```text
/// y_{n+1}(x) = (2n+1)/x * y_n(x) - y_{n-1}(x)
/// ```
///
/// Starting values:
/// ```text
/// y_0(x) = -cos(x)/x
/// y_1(x) = -cos(x)/x² - sin(x)/x
/// ```
///
/// # Arguments
/// * `order` - Number of terms (returns y_0 through y_{order-1})
/// * `x` - Argument (must be > 0)
pub fn spherical_bessel_y_array(order: usize, x: f64) -> Vec<f64> {
    assert!(order >= 1, "Order must be at least 1");

    let mut result = vec![0.0; order];

    if x.abs() < 1e-15 {
        // y_n → -∞ as x → 0
        for item in result.iter_mut().take(order) {
            *item = f64::NEG_INFINITY;
        }
        return result;
    }

    let cos_x = x.cos();
    let sin_x = x.sin();

    // y_0(x) = -cos(x)/x
    result[0] = -cos_x / x;

    if order == 1 {
        return result;
    }

    // y_1(x) = -cos(x)/x² - sin(x)/x
    result[1] = -cos_x / (x * x) - sin_x / x;

    // Upward recurrence: y_{n+1} = (2n+1)/x * y_n - y_{n-1}
    for n in 2..order {
        result[n] = (2 * n - 1) as f64 / x * result[n - 1] - result[n - 2];
    }

    result
}

/// Single spherical Bessel function yₙ(x) (Neumann)
pub fn spherical_bessel_y(n: usize, x: f64) -> f64 {
    if x.abs() < 1e-10 {
        return f64::NEG_INFINITY;
    }

    match n {
        0 => -x.cos() / x,
        1 => -x.cos() / (x * x) - x.sin() / x,
        _ => {
            let arr = spherical_bessel_y_array(n + 1, x);
            arr[n]
        }
    }
}

/// Compute spherical Hankel functions of the first kind h_n^(1)(x)
///
/// The spherical Hankel function is:
/// ```text
/// h_n^(1)(x) = j_n(x) + i * y_n(x)
/// ```
///
/// # Arguments
/// * `order` - Number of terms
/// * `x` - Argument (must be > 0)
///
/// # Returns
/// Vector of Complex64 values h_n^(1)(x) for n = 0, ..., order-1
pub fn spherical_hankel_first_kind(order: usize, x: f64) -> Vec<Complex64> {
    assert!(order >= 1, "Order must be at least 1");
    assert!(x > 0.0, "Argument must be positive");

    let j = spherical_bessel_j_array(order, x);
    let y = spherical_bessel_y_array(order, x);

    j.iter()
        .zip(y.iter())
        .map(|(&jn, &yn)| Complex64::new(jn, yn))
        .collect()
}

/// Single spherical Hankel function hₙ⁽¹⁾(x)
pub fn spherical_hankel_1(n: usize, x: f64) -> Complex64 {
    Complex64::new(spherical_bessel_j(n, x), spherical_bessel_y(n, x))
}

/// Spherical Hankel function of the second kind hₙ⁽²⁾(x)
///
/// h_n^(2)(x) = j_n(x) - i * y_n(x)
pub fn spherical_hankel_2(n: usize, x: f64) -> Complex64 {
    Complex64::new(spherical_bessel_j(n, x), -spherical_bessel_y(n, x))
}

/// Compute derivative of spherical Bessel jₙ'(x)
///
/// Uses the recurrence relation:
/// ```text
/// j_n'(x) = j_{n-1}(x) - (n+1)/x * j_n(x)
/// ```
///
/// Also valid: j_n'(x) = n/x * j_n(x) - j_{n+1}(x)
pub fn spherical_bessel_j_derivative(n: usize, x: f64) -> f64 {
    if x.abs() < 1e-10 {
        return if n == 1 { 1.0 / 3.0 } else { 0.0 };
    }

    let j = spherical_bessel_j_array(n + 2, x);

    if n == 0 {
        // j_0'(x) = -j_1(x)
        -j[1]
    } else {
        j[n - 1] - (n + 1) as f64 / x * j[n]
    }
}

/// Compute derivative of spherical Bessel yₙ'(x)
pub fn spherical_bessel_y_derivative(n: usize, x: f64) -> f64 {
    if x.abs() < 1e-10 {
        return f64::NEG_INFINITY;
    }

    let y = spherical_bessel_y_array(n + 2, x);

    if n == 0 {
        // y_0'(x) = -y_1(x)
        -y[1]
    } else {
        y[n - 1] - (n + 1) as f64 / x * y[n]
    }
}

/// Derivative of spherical Hankel function hₙ⁽¹⁾'(x)
pub fn spherical_hankel_1_derivative(n: usize, x: f64) -> Complex64 {
    Complex64::new(
        spherical_bessel_j_derivative(n, x),
        spherical_bessel_y_derivative(n, x),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_spherical_bessel_j0() {
        // j_0(x) = sin(x)/x
        let j = spherical_bessel_j_array(1, 1.0);
        let expected = 1.0_f64.sin() / 1.0;
        assert!((j[0] - expected).abs() < EPSILON);

        let j = spherical_bessel_j_array(1, PI);
        let expected = PI.sin() / PI;
        assert!((j[0] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_spherical_bessel_j1() {
        // j_1(x) = sin(x)/x² - cos(x)/x
        let x = 2.0;
        let j = spherical_bessel_j_array(2, x);
        let expected = x.sin() / (x * x) - x.cos() / x;
        assert!((j[1] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_spherical_bessel_y0() {
        // y_0(x) = -cos(x)/x
        let x = 1.0;
        let y = spherical_bessel_y_array(1, x);
        let expected = -x.cos() / x;
        assert!((y[0] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_spherical_bessel_y1() {
        // y_1(x) = -cos(x)/x² - sin(x)/x
        let x = 2.0;
        let y = spherical_bessel_y_array(2, x);
        let expected = -x.cos() / (x * x) - x.sin() / x;
        assert!((y[1] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_spherical_hankel_consistency() {
        // h_n^(1) = j_n + i*y_n
        let x = 3.0;
        let order = 5;
        let j = spherical_bessel_j_array(order, x);
        let y = spherical_bessel_y_array(order, x);
        let h = spherical_hankel_first_kind(order, x);

        for n in 0..order {
            assert!(
                (h[n].re - j[n]).abs() < 1e-8,
                "Real part mismatch at n={}: {} vs {}",
                n,
                h[n].re,
                j[n]
            );
            assert!(
                (h[n].im - y[n]).abs() < 1e-8,
                "Imag part mismatch at n={}: {} vs {}",
                n,
                h[n].im,
                y[n]
            );
        }
    }

    #[test]
    fn test_hankel_asymptotic() {
        // For large x, h_0^(1)(x) → (-i) * exp(ix)/x = (sin(x) - i*cos(x))/x
        let x = 50.0;
        let h = spherical_hankel_first_kind(3, x);

        let expected_re = x.sin() / x;
        let expected_im = -x.cos() / x;

        assert!(
            (h[0].re - expected_re).abs() < 0.01,
            "Asymptotic real mismatch"
        );
        assert!(
            (h[0].im - expected_im).abs() < 0.01,
            "Asymptotic imag mismatch"
        );
    }

    #[test]
    fn test_bessel_derivative_j0() {
        // j_0'(x) = -j_1(x)
        let x = 2.0;
        let jp = spherical_bessel_j_derivative(0, x);
        let j = spherical_bessel_j_array(2, x);
        assert!((jp + j[1]).abs() < EPSILON);
    }

    #[test]
    fn test_recurrence_stability() {
        // Test that computation is stable for order > x
        let x = 5.0;
        let order = 20;
        let j = spherical_bessel_j_array(order, x);

        // All values should be finite
        for n in 0..order {
            assert!(j[n].is_finite(), "j_{} is not finite", n);
        }

        // Values should decrease for n >> x
        assert!(j[15].abs() < j[5].abs());
    }

    #[test]
    fn test_single_functions() {
        let x = 2.5;

        // Compare single function vs array
        assert!((spherical_bessel_j(3, x) - spherical_bessel_j_array(4, x)[3]).abs() < EPSILON);
        assert!((spherical_bessel_y(2, x) - spherical_bessel_y_array(3, x)[2]).abs() < EPSILON);
    }
}
