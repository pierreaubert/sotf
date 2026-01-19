//! Core traits for linear algebra operations
//!
//! This module defines the fundamental abstractions used throughout the solver library:
//! - [`ComplexField`]: Trait for scalar types (complex and real numbers)
//! - [`LinearOperator`]: Trait for matrix-like objects that can perform matrix-vector products
//! - [`Preconditioner`]: Trait for preconditioning operations

use ndarray::Array1;
use num_complex::{Complex32, Complex64};
use num_traits::{Float, NumAssign, One, Zero};
use std::fmt::Debug;
use std::ops::Neg;

/// Trait for scalar types that can be used in linear algebra operations.
///
/// This trait abstracts over real and complex number types, providing
/// a unified interface for operations like conjugation, norm computation,
/// and conversion from real values.
///
/// # Implementations
///
/// Provided for:
/// - `Complex64` (default for most acoustic applications)
/// - `Complex32` (for memory-constrained applications)
/// - `f64` (for real-valued problems)
/// - `f32` (for real-valued, memory-constrained applications)
#[cfg(feature = "ndarray-linalg")]
pub trait ComplexField:
    NumAssign
    + Clone
    + Copy
    + Send
    + Sync
    + Debug
    + Zero
    + One
    + Neg<Output = Self>
    + ndarray_linalg::Lapack
    + 'static
{
    // type Real inherited from ndarray_linalg::Scalar via Lapack

    /// Complex conjugate
    #[cfg(not(feature = "ndarray-linalg"))]
    fn conj(&self) -> Self;

    /// Squared magnitude |z|²
    fn norm_sqr(&self) -> Self::Real;

    /// Magnitude |z|
    fn norm(&self) -> Self::Real {
        self.norm_sqr().sqrt()
    }

    /// Create from a real value
    #[cfg(not(feature = "ndarray-linalg"))]
    fn from_real(r: Self::Real) -> Self;

    /// Create from real and imaginary parts
    fn from_re_im(re: Self::Real, im: Self::Real) -> Self;

    /// Real part
    fn re(&self) -> Self::Real;

    /// Imaginary part
    fn im(&self) -> Self::Real;

    /// Check if this is approximately zero
    fn is_zero_approx(&self, tol: Self::Real) -> bool {
        self.norm_sqr() < tol * tol
    }

    /// Multiplicative inverse (1/z)
    fn inv(&self) -> Self;

    /// Square root
    fn sqrt(&self) -> Self;
}

#[cfg(not(feature = "ndarray-linalg"))]
pub trait ComplexField:
    NumAssign + Clone + Copy + Send + Sync + Debug + Zero + One + Neg<Output = Self> + 'static
{
    /// The real number type underlying this field
    type Real: Float + NumAssign + FromPrimitive + ToPrimitive + Send + Sync + Debug + 'static;

    /// Complex conjugate
    fn conj(&self) -> Self;

    /// Squared magnitude |z|²
    fn norm_sqr(&self) -> Self::Real;

    /// Magnitude |z|
    fn norm(&self) -> Self::Real {
        self.norm_sqr().sqrt()
    }

    /// Create from a real value
    fn from_real(r: Self::Real) -> Self;

    /// Create from real and imaginary parts
    fn from_re_im(re: Self::Real, im: Self::Real) -> Self;

    /// Real part
    fn re(&self) -> Self::Real;

    /// Imaginary part
    fn im(&self) -> Self::Real;

    /// Check if this is approximately zero
    fn is_zero_approx(&self, tol: Self::Real) -> bool {
        self.norm_sqr() < tol * tol
    }

    /// Multiplicative inverse (1/z)
    fn inv(&self) -> Self;

    /// Square root
    fn sqrt(&self) -> Self;
}

impl ComplexField for Complex64 {
    #[cfg(not(feature = "ndarray-linalg"))]
    type Real = f64;

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn conj(&self) -> Self {
        Complex64::conj(self)
    }

    #[inline]
    fn norm_sqr(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn from_real(r: f64) -> Self {
        Complex64::new(r, 0.0)
    }

    #[inline]
    fn from_re_im(re: f64, im: f64) -> Self {
        Complex64::new(re, im)
    }

    #[inline]
    fn re(&self) -> f64 {
        self.re
    }

    #[inline]
    fn im(&self) -> f64 {
        self.im
    }

    #[inline]
    fn inv(&self) -> Self {
        let denom = self.norm_sqr();
        Complex64::new(self.re / denom, -self.im / denom)
    }

    #[inline]
    fn sqrt(&self) -> Self {
        Complex64::sqrt(*self)
    }
}

impl ComplexField for Complex32 {
    #[cfg(not(feature = "ndarray-linalg"))]
    type Real = f32;

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn conj(&self) -> Self {
        Complex32::conj(self)
    }

    #[inline]
    fn norm_sqr(&self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn from_real(r: f32) -> Self {
        Complex32::new(r, 0.0)
    }

    #[inline]
    fn from_re_im(re: f32, im: f32) -> Self {
        Complex32::new(re, im)
    }

    #[inline]
    fn re(&self) -> f32 {
        self.re
    }

    #[inline]
    fn im(&self) -> f32 {
        self.im
    }

    #[inline]
    fn inv(&self) -> Self {
        let denom = self.norm_sqr();
        Complex32::new(self.re / denom, -self.im / denom)
    }

    #[inline]
    fn sqrt(&self) -> Self {
        Complex32::sqrt(*self)
    }
}

impl ComplexField for f64 {
    #[cfg(not(feature = "ndarray-linalg"))]
    type Real = f64;

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn conj(&self) -> Self {
        *self
    }

    #[inline]
    fn norm_sqr(&self) -> f64 {
        *self * *self
    }

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn from_real(r: f64) -> Self {
        r
    }

    #[inline]
    fn from_re_im(re: f64, _im: f64) -> Self {
        re
    }

    #[inline]
    fn re(&self) -> f64 {
        *self
    }

    #[inline]
    fn im(&self) -> f64 {
        0.0
    }

    #[inline]
    fn inv(&self) -> Self {
        1.0 / *self
    }

    #[inline]
    fn sqrt(&self) -> Self {
        f64::sqrt(*self)
    }
}

impl ComplexField for f32 {
    #[cfg(not(feature = "ndarray-linalg"))]
    type Real = f32;

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn conj(&self) -> Self {
        *self
    }

    #[inline]
    fn norm_sqr(&self) -> f32 {
        *self * *self
    }

    #[inline]
    #[cfg(not(feature = "ndarray-linalg"))]
    fn from_real(r: f32) -> Self {
        r
    }

    #[inline]
    fn from_re_im(re: f32, _im: f32) -> Self {
        re
    }

    #[inline]
    fn re(&self) -> f32 {
        *self
    }

    #[inline]
    fn im(&self) -> f32 {
        0.0
    }

    #[inline]
    fn inv(&self) -> Self {
        1.0 / *self
    }

    #[inline]
    fn sqrt(&self) -> Self {
        f32::sqrt(*self)
    }
}

/// Trait for linear operators (matrices) that can perform matrix-vector products.
///
/// This abstraction allows solvers to work with dense matrices, sparse matrices,
/// and matrix-free operators (e.g., FMM) interchangeably.
pub trait LinearOperator<T: ComplexField>: Send + Sync {
    /// Number of rows in the operator
    fn num_rows(&self) -> usize;

    /// Number of columns in the operator
    fn num_cols(&self) -> usize;

    /// Apply the operator: y = A * x
    fn apply(&self, x: &Array1<T>) -> Array1<T>;

    /// Apply the transpose: y = A^T * x
    fn apply_transpose(&self, x: &Array1<T>) -> Array1<T>;

    /// Apply the Hermitian (conjugate transpose): y = A^H * x
    fn apply_hermitian(&self, x: &Array1<T>) -> Array1<T> {
        // Default implementation: conjugate(A^T * conj(x))
        // Note: x.mapv(|v| v.conj()) uses scalar conjugation.
        // If ComplexField does not have conj(), this relies on Scalar::conj().
        // However, mapv takes a closure.
        let x_conj: Array1<T> = x.mapv(|v| {
            #[cfg(feature = "ndarray-linalg")]
            {
                ndarray_linalg::Scalar::conj(&v)
            }
            #[cfg(not(feature = "ndarray-linalg"))]
            {
                v.conj()
            }
        });

        let y = self.apply_transpose(&x_conj);

        y.mapv(|v| {
            #[cfg(feature = "ndarray-linalg")]
            {
                ndarray_linalg::Scalar::conj(&v)
            }
            #[cfg(not(feature = "ndarray-linalg"))]
            {
                v.conj()
            }
        })
    }

    /// Check if the operator is square
    fn is_square(&self) -> bool {
        self.num_rows() == self.num_cols()
    }
}

/// Trait for preconditioners used in iterative solvers.
///
/// A preconditioner M approximates A^(-1), so that M*A is better conditioned
/// than A alone. This accelerates convergence of iterative methods.
pub trait Preconditioner<T: ComplexField>: Send + Sync {
    /// Apply the preconditioner: y = M * r
    ///
    /// This should approximate solving A * y = r
    fn apply(&self, r: &Array1<T>) -> Array1<T>;
}

/// Identity preconditioner (no preconditioning)
#[derive(Clone, Debug, Default)]
pub struct IdentityPreconditioner;

impl<T: ComplexField> Preconditioner<T> for IdentityPreconditioner {
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        r.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_complex64_field() {
        let z = Complex64::new(3.0, 4.0);
        assert_relative_eq!(z.norm_sqr(), 25.0);
        assert_relative_eq!(z.norm(), 5.0);

        // When ndarray-linalg is on, conj is from Scalar.
        // We can't test "ComplexField::conj" but we can test method call syntax.
        #[cfg(not(feature = "ndarray-linalg"))]
        let z_conj = ComplexField::conj(&z);
        #[cfg(feature = "ndarray-linalg")]
        let z_conj = ndarray_linalg::Scalar::conj(&z);

        assert_relative_eq!(z_conj.re, 3.0);
        assert_relative_eq!(z_conj.im, -4.0);

        let z_inv = z.inv();
        let product = z * z_inv;
        assert_relative_eq!(product.re, 1.0, epsilon = 1e-10);
        assert_relative_eq!(product.im, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_f64_field() {
        let x: f64 = 3.0;
        assert_relative_eq!(x.norm_sqr(), 9.0);
        assert_relative_eq!(x.norm(), 3.0);

        #[cfg(not(feature = "ndarray-linalg"))]
        assert_relative_eq!(ComplexField::conj(&x), 3.0);
        #[cfg(feature = "ndarray-linalg")]
        assert_relative_eq!(ndarray_linalg::Scalar::conj(&x), 3.0);

        assert_relative_eq!(x.inv(), 1.0 / 3.0);
    }

    #[test]
    fn test_identity_preconditioner() {
        let precond = IdentityPreconditioner;
        let r = Array1::from_vec(vec![Complex64::new(1.0, 2.0), Complex64::new(3.0, 4.0)]);
        let y = precond.apply(&r);
        assert_eq!(r, y);
    }
}
