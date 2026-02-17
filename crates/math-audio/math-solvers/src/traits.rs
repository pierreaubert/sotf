//! Core traits for linear algebra operations
//!
//! This module defines the fundamental abstractions used throughout the solver library:
//! - [`ComplexField`]: Trait for scalar types (complex and real numbers)
//! - [`LinearOperator`]: Trait for matrix-like objects that can perform matrix-vector products
//! - [`Preconditioner`]: Trait for preconditioning operations

use ndarray::Array1;
use num_complex::{Complex32, Complex64};
use num_traits::{Float, NumAssign, One, Zero};
use num_traits::{FromPrimitive, ToPrimitive};
use std::fmt::Debug;
use std::ops::Neg;

// Note: `Array1` is used in the BLAS-dispatch default methods on `ComplexField`.
// The f64/f32 overrides use ndarray's `.dot()` and `.scaled_add()` which route
// through BLAS when the `native` feature (ndarray/blas) is enabled.

/// Trait for scalar types that can be used in linear algebra operations.
///
/// This trait abstracts over real and complex number types, providing
/// a unified interface for operations like conjugation, norm computation,
/// and conversion from real values.
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

    // ------------------------------------------------------------------
    // BLAS-dispatch methods
    //
    // Default implementations use generic Rust loops. The f64/f32 impls
    // override these to use ndarray operations backed by BLAS (when the
    // `native` feature is enabled).
    // ------------------------------------------------------------------

    /// Inner product: Σ conj(x_i) * y_i
    fn vec_dot(x: &Array1<Self>, y: &Array1<Self>) -> Self {
        let mut sum = Self::zero();
        for (xi, yi) in x.iter().zip(y.iter()) {
            sum += xi.conj() * *yi;
        }
        sum
    }

    /// Squared vector norm: Σ |x_i|²
    fn vec_norm_sqr(x: &Array1<Self>) -> Self::Real {
        let mut sum = Self::Real::zero();
        for xi in x.iter() {
            sum += xi.norm_sqr();
        }
        sum
    }

    /// AXPY: y += α * x
    fn vec_axpy(alpha: Self, x: &Array1<Self>, y: &mut Array1<Self>) {
        for (xi, yi) in x.iter().zip(y.iter_mut()) {
            *yi += alpha * *xi;
        }
    }

    /// In-place scale: x *= α
    fn vec_scale(x: &mut Array1<Self>, alpha: Self) {
        for xi in x.iter_mut() {
            *xi *= alpha;
        }
    }
}

impl ComplexField for Complex64 {
    type Real = f64;

    #[inline]
    fn conj(&self) -> Self {
        Complex64::conj(self)
    }

    #[inline]
    fn norm_sqr(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
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
    type Real = f32;

    #[inline]
    fn conj(&self) -> Self {
        Complex32::conj(self)
    }

    #[inline]
    fn norm_sqr(&self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
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
    type Real = f64;

    #[inline]
    fn conj(&self) -> Self {
        *self
    }

    #[inline]
    fn norm_sqr(&self) -> f64 {
        *self * *self
    }

    #[inline]
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

    // BLAS-accelerated overrides via ndarray (uses DDOT/DNRM2/DAXPY)

    #[inline]
    fn vec_dot(x: &Array1<Self>, y: &Array1<Self>) -> Self {
        x.dot(y)
    }

    #[inline]
    fn vec_norm_sqr(x: &Array1<Self>) -> Self {
        x.dot(x)
    }

    #[inline]
    fn vec_axpy(alpha: Self, x: &Array1<Self>, y: &mut Array1<Self>) {
        y.scaled_add(alpha, x);
    }

    #[inline]
    fn vec_scale(x: &mut Array1<Self>, alpha: Self) {
        x.mapv_inplace(|v| v * alpha);
    }
}

impl ComplexField for f32 {
    type Real = f32;

    #[inline]
    fn conj(&self) -> Self {
        *self
    }

    #[inline]
    fn norm_sqr(&self) -> f32 {
        *self * *self
    }

    #[inline]
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

    // BLAS-accelerated overrides via ndarray (uses SDOT/SNRM2/SAXPY)

    #[inline]
    fn vec_dot(x: &Array1<Self>, y: &Array1<Self>) -> Self {
        x.dot(y)
    }

    #[inline]
    fn vec_norm_sqr(x: &Array1<Self>) -> Self {
        x.dot(x)
    }

    #[inline]
    fn vec_axpy(alpha: Self, x: &Array1<Self>, y: &mut Array1<Self>) {
        y.scaled_add(alpha, x);
    }

    #[inline]
    fn vec_scale(x: &mut Array1<Self>, alpha: Self) {
        x.mapv_inplace(|v| v * alpha);
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
        let x_conj: Array1<T> = x.mapv(|v| v.conj());
        let y = self.apply_transpose(&x_conj);
        y.mapv(|v| v.conj())
    }

    /// Check if the operator is square
    fn is_square(&self) -> bool {
        self.num_rows() == self.num_cols()
    }
}

/// Status of an iterative solver
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverStatus {
    /// Solver converged to the desired tolerance
    Converged,
    /// Solver reached the maximum number of iterations without converging
    MaxIterationsReached,
    /// Solver encountered a breakdown (e.g., division by zero)
    Breakdown,
    /// Solver stagnated (no progress made)
    Stagnated,
    /// Solver diverged (residual is increasing)
    Diverged,
}

/// Error information from iterative solvers
#[derive(Debug, thiserror::Error)]
pub enum SolverError {
    #[error("Solver failed to converge: {status:?}")]
    ConvergenceError {
        status: SolverStatus,
        iterations: usize,
        residual: f64,
    },
    #[error("Linear operator dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
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

        let z_conj = z.conj();
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
        assert_relative_eq!(x.conj(), 3.0);
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
