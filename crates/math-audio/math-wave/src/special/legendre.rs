//! Legendre polynomials and associated functions
//!
//! Implementations of Legendre polynomials used in spherical
//! harmonic expansions and FMM.

/// Compute Legendre polynomials P_n(x) for n = 0, 1, ..., order-1
///
/// Uses the stable three-term recurrence relation:
/// ```text
/// (n+1) P_{n+1}(x) = (2n+1) x P_n(x) - n P_{n-1}(x)
/// ```
///
/// Starting values:
/// ```text
/// P_0(x) = 1
/// P_1(x) = x
/// ```
///
/// # Arguments
/// * `order` - Number of terms (must be >= 2)
/// * `x` - Argument (typically cos(θ), so |x| <= 1)
///
/// # Example
/// ```
/// use math_audio_wave::special::legendre_polynomials;
/// let p = legendre_polynomials(5, 0.5);
/// assert!((p[0] - 1.0).abs() < 1e-10);
/// assert!((p[1] - 0.5).abs() < 1e-10);
/// ```
pub fn legendre_polynomials(order: usize, x: f64) -> Vec<f64> {
    assert!(order >= 1, "Order must be at least 1");

    let mut result = vec![0.0; order];

    result[0] = 1.0;

    if order == 1 {
        return result;
    }

    result[1] = x;

    // Recurrence: (n+1) P_{n+1} = (2n+1) x P_n - n P_{n-1}
    // Rearranged: P_{n} = ((2n-1) x P_{n-1} - (n-1) P_{n-2}) / n
    for n in 2..order {
        let n_f64 = n as f64;
        result[n] =
            ((2.0 * n_f64 - 1.0) * x * result[n - 1] - (n_f64 - 1.0) * result[n - 2]) / n_f64;
    }

    result
}

/// Single Legendre polynomial Pₙ(x)
pub fn legendre_p(n: usize, x: f64) -> f64 {
    match n {
        0 => 1.0,
        1 => x,
        _ => {
            let p = legendre_polynomials(n + 1, x);
            p[n]
        }
    }
}

/// Compute derivative of Legendre polynomials P'_n(x) for n = 0, 1, ..., order-1
///
/// Uses the recurrence relation:
/// ```text
/// (1 - x²) P'_n(x) = n (P_{n-1}(x) - x P_n(x))
/// ```
///
/// For |x| ≈ 1, uses:
/// ```text
/// P'_n(±1) = ±n(n+1)/2  (special case at endpoints)
/// ```
pub fn legendre_polynomials_derivative(order: usize, x: f64) -> Vec<f64> {
    let p = legendre_polynomials(order, x);
    let mut result = vec![0.0; order];

    result[0] = 0.0; // P'_0 = 0

    if order == 1 {
        return result;
    }

    let x2_minus_1 = x * x - 1.0;

    if x2_minus_1.abs() < 1e-14 {
        // Special case: x = ±1
        let sign: f64 = if x > 0.0 { 1.0 } else { -1.0 };
        for (n, result_n) in result.iter_mut().enumerate().take(order).skip(1) {
            let n_f64 = n as f64;
            *result_n = sign.powi(n as i32 + 1) * n_f64 * (n_f64 + 1.0) / 2.0;
        }
    } else {
        for (n, result_n) in result.iter_mut().enumerate().take(order).skip(1) {
            let n_f64 = n as f64;
            *result_n = n_f64 * (x * p[n] - p[n - 1]) / x2_minus_1;
        }
    }

    result
}

/// Derivative of single Legendre polynomial P'ₙ(x)
pub fn legendre_p_derivative(n: usize, x: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }

    let dp = legendre_polynomials_derivative(n + 1, x);
    dp[n]
}

/// Compute associated Legendre functions P_n^m(x) for fixed m
///
/// The associated Legendre functions satisfy:
/// ```text
/// P_n^m(x) = (-1)^m (1-x²)^{m/2} d^m/dx^m P_n(x)
/// ```
///
/// Uses recurrence:
/// ```text
/// P_{n+1}^m(x) = ((2n+1)x P_n^m - (n+m) P_{n-1}^m) / (n+1-m)
/// ```
///
/// # Arguments
/// * `n_max` - Maximum degree
/// * `m` - Order (0 <= m <= n_max)
/// * `x` - Argument
///
/// # Returns
/// Vector of P_m^m, P_{m+1}^m, ..., P_{n_max}^m
pub fn associated_legendre(n_max: usize, m: usize, x: f64) -> Vec<f64> {
    assert!(m <= n_max, "Order m must be <= degree n_max");

    let num_values = n_max - m + 1;
    let mut result = vec![0.0; num_values];

    // Starting value: P_m^m
    let sqrt_1_minus_x2 = (1.0 - x * x).max(0.0).sqrt();
    let mut pmm = 1.0;

    // P_m^m = (-1)^m (2m-1)!! (1-x²)^{m/2}
    for i in 1..=m {
        pmm *= -((2 * i - 1) as f64) * sqrt_1_minus_x2;
    }

    result[0] = pmm;

    if num_values == 1 {
        return result;
    }

    // P_{m+1}^m = x (2m+1) P_m^m
    let pm1m = x * (2 * m + 1) as f64 * pmm;
    result[1] = pm1m;

    // Recurrence for higher degrees
    let mut p_prev = pmm;
    let mut p_curr = pm1m;

    for n in (m + 2)..=n_max {
        let n_f64 = n as f64;
        let m_f64 = m as f64;
        let p_next =
            ((2.0 * n_f64 - 1.0) * x * p_curr - (n_f64 + m_f64 - 1.0) * p_prev) / (n_f64 - m_f64);
        result[n - m] = p_next;
        p_prev = p_curr;
        p_curr = p_next;
    }

    result
}

/// Single associated Legendre function Pₙᵐ(x)
pub fn associated_legendre_single(n: usize, m: usize, x: f64) -> f64 {
    if m > n {
        return 0.0;
    }

    let p = associated_legendre(n, m, x);
    p[n - m]
}

/// Normalized associated Legendre functions used in spherical harmonics
///
/// These are normalized such that the spherical harmonics have unit L² norm:
/// ```text
/// Ỹ_n^m = √((2n+1)(n-m)! / (4π(n+m)!)) P_n^m
/// ```
pub fn normalized_associated_legendre(n_max: usize, m: usize, x: f64) -> Vec<f64> {
    let p = associated_legendre(n_max, m, x);
    let mut result = vec![0.0; p.len()];

    for (i, &pnm) in p.iter().enumerate() {
        let n = m + i;
        let norm = normalization_factor(n, m);
        result[i] = norm * pnm;
    }

    result
}

/// Compute normalization factor for associated Legendre functions
fn normalization_factor(n: usize, m: usize) -> f64 {
    use std::f64::consts::PI;

    // √((2n+1)(n-m)! / (4π(n+m)!))
    let two_n_plus_1 = 2 * n + 1;

    // Compute (n-m)! / (n+m)! using logarithms for stability
    let mut log_ratio = 0.0;
    for k in (n - m + 1)..=(n + m) {
        log_ratio -= (k as f64).ln();
    }

    ((two_n_plus_1 as f64 / (4.0 * PI)) * log_ratio.exp()).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-12;

    #[test]
    fn test_legendre_p0() {
        // P_0(x) = 1 for all x
        let p = legendre_polynomials(3, 0.5);
        assert!((p[0] - 1.0).abs() < EPSILON);

        let p = legendre_polynomials(3, -0.7);
        assert!((p[0] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_legendre_p1() {
        // P_1(x) = x
        let x = 0.5;
        let p = legendre_polynomials(3, x);
        assert!((p[1] - x).abs() < EPSILON);
    }

    #[test]
    fn test_legendre_p2() {
        // P_2(x) = (3x² - 1)/2
        let x = 0.5;
        let p = legendre_polynomials(3, x);
        let expected = (3.0 * x * x - 1.0) / 2.0;
        assert!((p[2] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_legendre_p3() {
        // P_3(x) = (5x³ - 3x)/2
        let x = 0.6;
        let p = legendre_polynomials(4, x);
        let expected = (5.0 * x * x * x - 3.0 * x) / 2.0;
        assert!((p[3] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_legendre_at_one() {
        // P_n(1) = 1 for all n
        let p = legendre_polynomials(10, 1.0);
        for n in 0..10 {
            assert!((p[n] - 1.0).abs() < 1e-10, "P_{}(1) = {} != 1", n, p[n]);
        }
    }

    #[test]
    fn test_legendre_at_minus_one() {
        // P_n(-1) = (-1)^n
        let p = legendre_polynomials(10, -1.0);
        for n in 0..10 {
            let expected = if n % 2 == 0 { 1.0 } else { -1.0 };
            assert!(
                (p[n] - expected).abs() < 1e-10,
                "P_{}(-1) = {} != {}",
                n,
                p[n],
                expected
            );
        }
    }

    #[test]
    fn test_legendre_orthogonality() {
        // Numerical test of orthogonality: ∫_{-1}^1 P_m P_n dx = 2/(2n+1) δ_{mn}
        // Use Gauss-Legendre quadrature
        let n_points = 20;
        let (points, weights) = gauss_legendre_points(n_points);

        let order = 5;

        for m in 0..order {
            for n in 0..order {
                let mut integral = 0.0;
                for (i, &x) in points.iter().enumerate() {
                    let p = legendre_polynomials(order, x);
                    integral += weights[i] * p[m] * p[n];
                }

                let expected = if m == n {
                    2.0 / (2 * n + 1) as f64
                } else {
                    0.0
                };

                assert!(
                    (integral - expected).abs() < 1e-10,
                    "Orthogonality failed for ({}, {}): {} != {}",
                    m,
                    n,
                    integral,
                    expected
                );
            }
        }
    }

    #[test]
    fn test_associated_legendre_m0() {
        // P_n^0 = P_n
        let x = 0.5;
        let p = legendre_polynomials(5, x);
        let p_assoc = associated_legendre(4, 0, x);

        for n in 0..5 {
            assert!((p[n] - p_assoc[n]).abs() < EPSILON, "P_{}^0 mismatch", n);
        }
    }

    #[test]
    fn test_associated_legendre_p11() {
        // P_1^1 = -(1-x²)^{1/2}
        let x = 0.5;
        let p = associated_legendre(1, 1, x);
        let expected = -(1.0 - x * x).sqrt();
        assert!((p[0] - expected).abs() < EPSILON);
    }

    #[test]
    fn test_single_legendre() {
        let x = 0.7;
        for n in 0..5 {
            let p_arr = legendre_polynomials(n + 1, x);
            let p_single = legendre_p(n, x);
            assert!((p_arr[n] - p_single).abs() < EPSILON);
        }
    }

    // Helper function for orthogonality test
    fn gauss_legendre_points(n: usize) -> (Vec<f64>, Vec<f64>) {
        // Precomputed Gauss-Legendre for n=20
        let gl20_x = [
            -0.9931285991850949,
            -0.9639719272779138,
            -0.9122344282513259,
            -0.8391169718222188,
            -0.7463319064601508,
            -0.6360536807265150,
            -0.5108670019508271,
            -0.3737060887154195,
            -0.2277858511416451,
            -0.0765265211334973,
            0.0765265211334973,
            0.2277858511416451,
            0.3737060887154195,
            0.5108670019508271,
            0.6360536807265150,
            0.7463319064601508,
            0.8391169718222188,
            0.9122344282513259,
            0.9639719272779138,
            0.9931285991850949,
        ];

        let gl20_w = [
            0.0176140071391521,
            0.0406014298003869,
            0.0626720483341091,
            0.0832767415767048,
            0.1019301198172404,
            0.1181945319615184,
            0.1316886384491766,
            0.1420961093183820,
            0.1491729864726037,
            0.1527533871307258,
            0.1527533871307258,
            0.1491729864726037,
            0.1420961093183820,
            0.1316886384491766,
            0.1181945319615184,
            0.1019301198172404,
            0.0832767415767048,
            0.0626720483341091,
            0.0406014298003869,
            0.0176140071391521,
        ];

        let mut points = vec![0.0; n];
        let mut weights = vec![0.0; n];

        for i in 0..n.min(20) {
            points[i] = gl20_x[i];
            weights[i] = gl20_w[i];
        }

        (points, weights)
    }
}
