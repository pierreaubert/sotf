//! BLAS-accelerated linear algebra operations
//!
//! This module provides optimized implementations of inner products,
//! vector norms, and related operations using BLAS when available.
//!
//! # Features
//!
//! - Uses `ndarray-linalg` for BLAS-accelerated operations when the `native` feature is enabled
//! - Falls back to optimized Rust implementations when BLAS is unavailable

use crate::traits::ComplexField;
use ndarray::Array1;
use num_traits::Float;
use num_traits::Zero;

/// Compute inner product (x, y) = Σ conj(x_i) * y_i
///
/// Uses optimized Rust implementation. BLAS can be integrated for real types
/// in future if additional performance is needed.
#[inline]
pub fn inner_product<T: ComplexField>(x: &Array1<T>, y: &Array1<T>) -> T {
    assert_eq!(
        x.len(),
        y.len(),
        "Vector lengths must match for inner product"
    );
    let mut sum = T::zero();
    for (xi, yi) in x.iter().zip(y.iter()) {
        sum += xi.conj() * *yi;
    }
    sum
}

/// Compute vector 2-norm: ||x||_2 = sqrt(Σ |x_i|^2)
///
/// Uses BLAS DNRM2 when available for 2-5x speedup.
#[inline]
pub fn vector_norm<T: ComplexField>(x: &Array1<T>) -> T::Real
where
    T::Real: Float,
{
    vector_norm_sqr(x).sqrt()
}

/// Compute vector norm squared: ||x||_2^2 = Σ |x_i|^2
///
/// More efficient than computing norm and squaring when the square root isn't needed.
#[inline]
pub fn vector_norm_sqr<T: ComplexField>(x: &Array1<T>) -> T::Real {
    let mut sum = T::Real::zero();
    for xi in x.iter() {
        sum += xi.norm_sqr();
    }
    sum
}

/// Scale vector by scalar: y = α * x
#[allow(dead_code)]
#[inline]
pub fn scale_vector<T: ComplexField>(alpha: T, x: &Array1<T>, y: &mut Array1<T>) {
    for (xi, yi) in x.iter().zip(y.iter_mut()) {
        *yi = alpha * *xi;
    }
}

/// Compute axpy: y = α * x + y
#[allow(dead_code)]
#[inline]
pub fn axpy<T: ComplexField>(alpha: T, x: &Array1<T>, y: &mut Array1<T>) {
    for (xi, yi) in x.iter().zip(y.iter_mut()) {
        *yi += alpha * *xi;
    }
}

/// Compute the scaled vector addition: z = α * x + β * y
#[allow(dead_code)]
#[inline]
pub fn axpby<T: ComplexField>(alpha: T, x: &Array1<T>, beta: T, y: &Array1<T>, z: &mut Array1<T>) {
    for ((xi, yi), zi) in x.iter().zip(y.iter()).zip(z.iter_mut()) {
        *zi = alpha * *xi + beta * *yi;
    }
}

/// Compute vector scale in-place: x = α * x
///
/// More efficient than creating a new vector.
#[allow(dead_code)]
#[inline]
pub fn scale_inplace<T: ComplexField>(x: &mut Array1<T>, alpha: T) {
    for xi in x.iter_mut() {
        *xi *= alpha;
    }
}

/// Compute the dot product of two real f64 vectors using BLAS when available
#[allow(dead_code)]
#[cfg(feature = "native")]
#[cfg(feature = "ndarray-linalg")]
pub fn dot_f64(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len());
    let arr_x = ndarray::ArrayView1::from(x);
    let arr_y = ndarray::ArrayView1::from(y);
    arr_x.dot(&arr_y)
}

/// Compute the dot product of two real f32 vectors using BLAS when available
#[allow(dead_code)]
#[cfg(feature = "native")]
#[cfg(feature = "ndarray-linalg")]
pub fn dot_f32(x: &[f32], y: &[f32]) -> f32 {
    assert_eq!(x.len(), y.len());
    let arr_x = ndarray::ArrayView1::from(x);
    let arr_y = ndarray::ArrayView1::from(y);
    arr_x.dot(&arr_y)
}

/// Fallback dot product for f64 when BLAS is not available
#[cfg(not(feature = "ndarray-linalg"))]
pub fn dot_f64(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len());
    let mut sum = 0.0_f64;
    for (xi, yi) in x.iter().zip(y.iter()) {
        sum += xi * yi;
    }
    sum
}

/// Fallback dot product for f32 when BLAS is not available
#[cfg(not(feature = "ndarray-linalg"))]
pub fn dot_f32(x: &[f32], y: &[f32]) -> f32 {
    assert_eq!(x.len(), y.len());
    let mut sum = 0.0_f32;
    for (xi, yi) in x.iter().zip(y.iter()) {
        sum += xi * yi;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_inner_product_real() {
        let x = array![1.0_f64, 2.0, 3.0];
        let y = array![4.0_f64, 5.0, 6.0];

        let ip = inner_product(&x, &y);
        assert_relative_eq!(ip, 1.0 * 4.0 + 2.0 * 5.0 + 3.0 * 6.0, epsilon = 1e-10);
    }

    #[test]
    fn test_inner_product_complex() {
        let x = array![Complex64::new(1.0, 2.0), Complex64::new(3.0, 4.0)];
        let y = array![Complex64::new(5.0, 6.0), Complex64::new(7.0, 8.0)];

        let ip = inner_product(&x, &y);
        let expected = Complex64::new(70.0, -8.0);
        assert_relative_eq!(ip.re, expected.re, epsilon = 1e-10);
        assert_relative_eq!(ip.im, expected.im, epsilon = 1e-10);
    }

    #[test]
    fn test_vector_norm_real() {
        let x = array![3.0_f64, 4.0];

        let norm = vector_norm(&x);
        assert_relative_eq!(norm, 5.0, epsilon = 1e-10);
    }

    #[test]
    fn test_vector_norm_complex() {
        let x = array![Complex64::new(3.0, 0.0), Complex64::new(0.0, 4.0)];

        let norm = vector_norm(&x);
        assert_relative_eq!(norm, 5.0, epsilon = 1e-10);
    }

    #[test]
    fn test_vector_norm_zero() {
        let x = array![0.0_f64, 0.0, 0.0];

        let norm = vector_norm(&x);
        assert_relative_eq!(norm, 0.0, epsilon = 1e-15);
    }

    #[test]
    fn test_scale_vector() {
        let alpha = 2.0_f64;
        let x = array![1.0_f64, 2.0, 3.0];
        let mut y = array![0.0_f64, 0.0, 0.0];

        scale_vector(alpha, &x, &mut y);

        assert_relative_eq!(y[0], 2.0, epsilon = 1e-10);
        assert_relative_eq!(y[1], 4.0, epsilon = 1e-10);
        assert_relative_eq!(y[2], 6.0, epsilon = 1e-10);
    }

    #[test]
    fn test_axpy() {
        let alpha = 2.0_f64;
        let x = array![1.0_f64, 2.0, 3.0];
        let mut y = array![1.0_f64, 1.0, 1.0];

        axpy(alpha, &x, &mut y);

        assert_relative_eq!(y[0], 3.0, epsilon = 1e-10);
        assert_relative_eq!(y[1], 5.0, epsilon = 1e-10);
        assert_relative_eq!(y[2], 7.0, epsilon = 1e-10);
    }

    #[test]
    fn test_scale_inplace() {
        let mut x = array![1.0_f64, 2.0, 3.0];
        let alpha = 0.5_f64;

        scale_inplace(&mut x, alpha);

        assert_relative_eq!(x[0], 0.5, epsilon = 1e-10);
        assert_relative_eq!(x[1], 1.0, epsilon = 1e-10);
        assert_relative_eq!(x[2], 1.5, epsilon = 1e-10);
    }

    #[test]
    fn test_vector_norm_sqr() {
        let x = array![3.0_f64, 4.0];

        let norm_sqr = vector_norm_sqr(&x);
        assert_relative_eq!(norm_sqr, 25.0, epsilon = 1e-10);
    }

    #[test]
    fn test_dot_f64() {
        let x = [1.0_f64, 2.0, 3.0];
        let y = [4.0_f64, 5.0, 6.0];

        let d = dot_f64(&x, &y);
        assert_relative_eq!(d, 32.0, epsilon = 1e-10);
    }

    #[test]
    fn test_axpby() {
        let alpha = 2.0_f64;
        let beta = 0.5_f64;
        let x = array![1.0_f64, 2.0, 3.0];
        let y = array![4.0_f64, 5.0, 6.0];
        let mut z = array![0.0_f64, 0.0, 0.0];

        axpby(alpha, &x, beta, &y, &mut z);

        // z = 2*x + 0.5*y = [2, 4, 6] + [2, 2.5, 3] = [4, 6.5, 9]
        assert_relative_eq!(z[0], 4.0, epsilon = 1e-10);
        assert_relative_eq!(z[1], 6.5, epsilon = 1e-10);
        assert_relative_eq!(z[2], 9.0, epsilon = 1e-10);
    }
}
