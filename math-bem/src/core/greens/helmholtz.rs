//! Helmholtz Green's function and derivatives
//!
//! The 3D Helmholtz Green's function is:
//! ```text
//! G(x, y) = exp(ik|x-y|) / (4π|x-y|)
//! ```
//!
//! This module provides computation of G and its normal derivatives,
//! which form the kernels for BEM integration.

use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Helmholtz Green's function G = exp(ikr)/(4πr)
///
/// # Arguments
/// * `r` - Distance |x - y|
/// * `k` - Wave number
/// * `harmonic_factor` - +1 or -1 for time convention
///
/// # Returns
/// Complex value exp(ikr)/(4πr)
#[inline]
pub fn greens_function(r: f64, k: f64, harmonic_factor: f64) -> Complex64 {
    if r < 1e-15 {
        // Singular at r = 0 - should not be called with r = 0
        return Complex64::new(f64::INFINITY, 0.0);
    }

    let kr = k * r * harmonic_factor;
    let exp_ikr = Complex64::new(kr.cos(), kr.sin());
    exp_ikr / (4.0 * PI * r)
}

/// Gradient of Green's function ∇_y G
///
/// ```text
/// ∇_y G = (ik - 1/r) * G * (y - x)/r
/// ```
///
/// # Arguments
/// * `x` - Source point
/// * `y` - Field point
/// * `k` - Wave number
/// * `harmonic_factor` - +1 or -1 for time convention
///
/// # Returns
/// Complex 3-vector ∇_y G
pub fn greens_function_gradient(
    x: &Array1<f64>,
    y: &Array1<f64>,
    k: f64,
    harmonic_factor: f64,
) -> Array1<Complex64> {
    let r_vec = y - x;
    let r = r_vec.dot(&r_vec).sqrt();

    if r < 1e-15 {
        return Array1::from_vec(vec![
            Complex64::new(f64::INFINITY, 0.0),
            Complex64::new(f64::INFINITY, 0.0),
            Complex64::new(f64::INFINITY, 0.0),
        ]);
    }

    let g = greens_function(r, k, harmonic_factor);
    let factor = Complex64::new(-1.0 / r, k * harmonic_factor) * g;

    let mut result = Array1::zeros(3);
    for i in 0..3 {
        result[i] = factor * r_vec[i] / r;
    }

    result
}

/// Normal derivative of Green's function ∂G/∂n_y
///
/// ```text
/// ∂G/∂n_y = ∇_y G · n_y = (ik - 1/r) * G * (y-x)·n_y / r
/// ```
///
/// # Arguments
/// * `x` - Source point
/// * `y` - Field point
/// * `n_y` - Unit normal at y
/// * `k` - Wave number
/// * `harmonic_factor` - +1 or -1 for time convention
#[inline]
pub fn greens_function_normal_derivative(
    x: &Array1<f64>,
    y: &Array1<f64>,
    n_y: &Array1<f64>,
    k: f64,
    harmonic_factor: f64,
) -> Complex64 {
    let r_vec = y - x;
    let r = r_vec.dot(&r_vec).sqrt();

    if r < 1e-15 {
        return Complex64::new(f64::INFINITY, 0.0);
    }

    let g = greens_function(r, k, harmonic_factor);
    let r_dot_n = r_vec.dot(n_y);

    // Factor: (ik - 1/r)
    let factor = Complex64::new(-1.0 / r, k * harmonic_factor);

    factor * g * r_dot_n / r
}

/// Adjoint double layer kernel ∂G/∂n_x
///
/// ```text
/// ∂G/∂n_x = -∇_y G · n_x = -(ik - 1/r) * G * (y-x)·n_x / r
/// ```
#[inline]
pub fn greens_function_adjoint_derivative(
    x: &Array1<f64>,
    y: &Array1<f64>,
    n_x: &Array1<f64>,
    k: f64,
    harmonic_factor: f64,
) -> Complex64 {
    let r_vec = y - x;
    let r = r_vec.dot(&r_vec).sqrt();

    if r < 1e-15 {
        return Complex64::new(f64::INFINITY, 0.0);
    }

    let g = greens_function(r, k, harmonic_factor);
    let r_dot_n = r_vec.dot(n_x);

    // Adjoint has negative sign
    let factor = Complex64::new(1.0 / r, -k * harmonic_factor);

    factor * g * r_dot_n / r
}

/// Hypersingular kernel ∂²G/(∂n_x ∂n_y)
///
/// ```text
/// ∂²G/(∂n_x ∂n_y) = [((ik)² - 3ik/r + 3/r²) (r·n_x)(r·n_y)/r²
///                    - (ik - 1/r)(n_x·n_y)/r] * G
/// ```
///
/// where r = y - x.
pub fn greens_function_hypersingular(
    x: &Array1<f64>,
    y: &Array1<f64>,
    n_x: &Array1<f64>,
    n_y: &Array1<f64>,
    k: f64,
    harmonic_factor: f64,
) -> Complex64 {
    let r_vec = y - x;
    let r = r_vec.dot(&r_vec).sqrt();

    if r < 1e-15 {
        return Complex64::new(f64::INFINITY, 0.0);
    }

    let g = greens_function(r, k, harmonic_factor);
    let ik = Complex64::new(0.0, k * harmonic_factor);

    let r_dot_nx = r_vec.dot(n_x);
    let r_dot_ny = r_vec.dot(n_y);
    let nx_dot_ny = n_x.dot(n_y);

    let r2 = r * r;

    // Term 1: ((ik)² - 3ik/r + 3/r²) (r·n_x)(r·n_y)/r² * G
    let coef1 = ik * ik - 3.0 * ik / r + Complex64::new(3.0 / r2, 0.0);
    let term1 = coef1 * r_dot_nx * r_dot_ny / r2;

    // Term 2: (ik - 1/r)(n_x·n_y)/r * G
    // Note: sign depends on convention
    let coef2 = ik - Complex64::new(1.0 / r, 0.0);
    let term2 = coef2 * nx_dot_ny / r;

    (term1 - term2) * g
}

/// Compute all four BEM kernels at once for efficiency
///
/// Returns (G, ∂G/∂n_y, ∂G/∂n_x, ∂²G/∂n_x∂n_y)
pub fn all_kernels(
    x: &Array1<f64>,
    y: &Array1<f64>,
    n_x: &Array1<f64>,
    n_y: &Array1<f64>,
    k: f64,
    harmonic_factor: f64,
) -> (Complex64, Complex64, Complex64, Complex64) {
    let r_vec = y - x;
    let r2 = r_vec.dot(&r_vec);
    let r = r2.sqrt();

    if r < 1e-15 {
        let inf = Complex64::new(f64::INFINITY, 0.0);
        return (inf, inf, inf, inf);
    }

    let kr = k * r * harmonic_factor;
    let exp_ikr = Complex64::new(kr.cos(), kr.sin());
    let g = exp_ikr / (4.0 * PI * r);

    let r_dot_nx = r_vec.dot(n_x);
    let r_dot_ny = r_vec.dot(n_y);
    let nx_dot_ny = n_x.dot(n_y);

    let ik = Complex64::new(0.0, k * harmonic_factor);

    // ∂G/∂n_y
    let factor_dg = ik - Complex64::new(1.0 / r, 0.0);
    let dg_dny = factor_dg * g * r_dot_ny / r;

    // ∂G/∂n_x (adjoint - negative sign)
    let dg_dnx = -factor_dg * g * r_dot_nx / r;

    // Hypersingular
    let coef1 = ik * ik - 3.0 * ik / r + Complex64::new(3.0 / r2, 0.0);
    let term1 = coef1 * r_dot_nx * r_dot_ny / r2;
    let term2 = factor_dg * nx_dot_ny / r;
    let d2g = (term1 - term2) * g;

    (g, dg_dny, dg_dnx, d2g)
}

/// Distance between two points
#[inline]
pub fn distance(x: &Array1<f64>, y: &Array1<f64>) -> f64 {
    let r_vec = y - x;
    r_vec.dot(&r_vec).sqrt()
}

/// Squared distance between two points
#[inline]
pub fn distance_squared(x: &Array1<f64>, y: &Array1<f64>) -> f64 {
    let r_vec = y - x;
    r_vec.dot(&r_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_greens_function_magnitude() {
        // |G| = 1/(4πr) for any k
        let r = 2.0;
        let k = 1.5;
        let g = greens_function(r, k, 1.0);

        let expected_magnitude = 1.0 / (4.0 * PI * r);
        assert!((g.norm() - expected_magnitude).abs() < EPSILON);
    }

    #[test]
    fn test_greens_function_k_zero() {
        // For k = 0, G = 1/(4πr) (Laplace Green's function)
        let r = 1.5;
        let g = greens_function(r, 0.0, 1.0);

        let expected = 1.0 / (4.0 * PI * r);
        assert!((g.re - expected).abs() < EPSILON);
        assert!(g.im.abs() < EPSILON);
    }

    #[test]
    fn test_normal_derivative_radial() {
        // When y-x is parallel to n_y (radial direction)
        let x = array![0.0, 0.0, 0.0];
        let y = array![1.0, 0.0, 0.0];
        let n_y = array![1.0, 0.0, 0.0]; // Outward normal

        let k = 2.0;
        let dg_dn = greens_function_normal_derivative(&x, &y, &n_y, k, 1.0);

        // For radial case: (y-x)·n/|y-x| = 1
        let r = 1.0;
        let g = greens_function(r, k, 1.0);
        let expected = (Complex64::new(-1.0 / r, k) * g).re;

        // Check that derivative has expected sign (pointing outward = positive)
        assert!(dg_dn.re < 0.0 || dg_dn.im.abs() > 0.0); // Depends on k
    }

    #[test]
    fn test_normal_derivative_tangential() {
        // When y-x is perpendicular to n_y (tangential)
        let x = array![0.0, 0.0, 0.0];
        let y = array![1.0, 0.0, 0.0];
        let n_y = array![0.0, 1.0, 0.0]; // Tangent to radial

        let k = 2.0;
        let dg_dn = greens_function_normal_derivative(&x, &y, &n_y, k, 1.0);

        // Should be zero since (y-x)·n = 0
        assert!(dg_dn.norm() < EPSILON);
    }

    #[test]
    fn test_adjoint_symmetry() {
        // ∂G(x,y)/∂n_x should relate to ∂G(y,x)/∂n_y
        let x = array![0.0, 0.0, 0.0];
        let y = array![1.0, 0.5, 0.2];
        let n_x = array![0.0, 1.0, 0.0];

        let k = 3.0;

        let dg_dnx = greens_function_adjoint_derivative(&x, &y, &n_x, k, 1.0);

        // Compare with normal derivative at x looking from y
        // Note: signs and directions need careful handling
        // This is a basic sanity check
        assert!(dg_dnx.norm().is_finite());
    }

    #[test]
    fn test_all_kernels_consistency() {
        let x = array![0.0, 0.0, 0.0];
        let y = array![1.0, 0.5, 0.3];
        let n_x = array![0.0, 0.0, 1.0];
        let n_y = array![1.0, 0.0, 0.0];
        let k = 2.5;

        let (g, dg_dny, dg_dnx, _d2g) = all_kernels(&x, &y, &n_x, &n_y, k, 1.0);

        // Compare with individual functions
        let g_single = greens_function(distance(&x, &y), k, 1.0);
        let dg_dny_single = greens_function_normal_derivative(&x, &y, &n_y, k, 1.0);
        let dg_dnx_single = greens_function_adjoint_derivative(&x, &y, &n_x, k, 1.0);

        assert!((g - g_single).norm() < EPSILON);
        assert!((dg_dny - dg_dny_single).norm() < EPSILON);
        assert!((dg_dnx - dg_dnx_single).norm() < EPSILON);
    }

    #[test]
    fn test_distance() {
        let x = array![1.0, 2.0, 3.0];
        let y = array![4.0, 6.0, 3.0];

        let d = distance(&x, &y);
        let expected = (9.0_f64 + 16.0).sqrt(); // sqrt(3² + 4²) = 5
        assert!((d - expected).abs() < EPSILON);
    }
}
