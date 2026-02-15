//! LU decomposition solver
//!
//! Provides LU factorization with partial pivoting for solving dense linear systems.
//! Uses BLAS/LAPACK when available (native feature), with a pure-Rust fallback.

use crate::traits::ComplexField;
use ndarray::{Array1, Array2};
use num_traits::FromPrimitive;
use thiserror::Error;

#[cfg(feature = "ndarray-linalg")]
use ndarray_linalg::Solve;

/// Errors that can occur during LU factorization
#[derive(Error, Debug)]
pub enum LuError {
    #[error("Matrix is singular or nearly singular")]
    SingularMatrix,
    #[error("Matrix dimensions mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
}

/// LU factorization result
///
/// Stores L and U factors along with pivot information
#[derive(Debug, Clone)]
pub struct LuFactorization<T: ComplexField> {
    /// Combined L and U matrices (L is unit lower triangular, stored below diagonal)
    pub lu: Array2<T>,
    /// Pivot indices
    pub pivots: Vec<usize>,
    /// Matrix dimension
    pub n: usize,
}

impl<T: ComplexField> LuFactorization<T> {
    /// Solve Ax = b using the pre-computed LU factorization
    pub fn solve(&self, b: &Array1<T>) -> Result<Array1<T>, LuError> {
        if b.len() != self.n {
            return Err(LuError::DimensionMismatch {
                expected: self.n,
                got: b.len(),
            });
        }

        let mut x = Array1::from_elem(self.n, T::zero());

        // Apply row permutations Pb
        for i in 0..self.n {
            x[i] = b[self.pivots[i]];
        }

        // Forward substitution: Ly = Pb
        for i in 0..self.n {
            #[allow(clippy::needless_range_loop)]
            for j in 0..i {
                let l_ij = self.lu[[i, j]];
                let x_j = x[j];
                x[i] -= l_ij * x_j;
            }
        }

        // Backward substitution: Ux = y
        for i in (0..self.n).rev() {
            #[allow(clippy::needless_range_loop)]
            for j in (i + 1)..self.n {
                let u_ij = self.lu[[i, j]];
                let x_j = x[j];
                x[i] -= u_ij * x_j;
            }
            let u_ii = self.lu[[i, i]];
            if u_ii.is_zero_approx(T::Real::from_f64(1e-20).unwrap()) {
                return Err(LuError::SingularMatrix);
            }
            x[i] *= u_ii.inv();
        }

        Ok(x)
    }
}

/// Compute LU factorization with partial pivoting (pure Rust implementation)
#[allow(dead_code)]
pub fn lu_factorize<T: ComplexField>(a: &Array2<T>) -> Result<LuFactorization<T>, LuError> {
    let n = a.nrows();
    if n != a.ncols() {
        return Err(LuError::DimensionMismatch {
            expected: n,
            got: a.ncols(),
        });
    }

    let mut lu = a.clone();
    let mut pivots: Vec<usize> = (0..n).collect();

    for k in 0..n {
        // Find pivot
        let mut max_val = lu[[k, k]].norm();
        let mut max_row = k;

        for i in (k + 1)..n {
            let val = lu[[i, k]].norm();
            if val > max_val {
                max_val = val;
                max_row = i;
            }
        }

        // Check for singularity
        if max_val < T::Real::from_f64(1e-20).unwrap() {
            return Err(LuError::SingularMatrix);
        }

        // Swap rows if needed
        if max_row != k {
            for j in 0..n {
                let tmp = lu[[k, j]];
                lu[[k, j]] = lu[[max_row, j]];
                lu[[max_row, j]] = tmp;
            }
            pivots.swap(k, max_row);
        }

        // Compute multipliers and eliminate
        let pivot = lu[[k, k]];
        for i in (k + 1)..n {
            let mult = lu[[i, k]] * pivot.inv();
            lu[[i, k]] = mult; // Store multiplier in L part

            for j in (k + 1)..n {
                let update = mult * lu[[k, j]];
                lu[[i, j]] -= update;
            }
        }
    }

    Ok(LuFactorization { lu, pivots, n })
}

/// Solve Ax = b using LU decomposition
///
/// This is a convenience function that combines factorization and solve.
#[cfg(feature = "ndarray-linalg")]
pub fn lu_solve<T: ComplexField + ndarray_linalg::Lapack>(a: &Array2<T>, b: &Array1<T>) -> Result<Array1<T>, LuError> {
    a.solve_into(b.clone()).map_err(|_| LuError::SingularMatrix)
}

/// Solve Ax = b using LU decomposition
///
/// This is a convenience function that combines factorization and solve.
#[cfg(not(feature = "ndarray-linalg"))]
pub fn lu_solve<T: ComplexField>(a: &Array2<T>, b: &Array1<T>) -> Result<Array1<T>, LuError> {
    let factorization = lu_factorize(a)?;
    factorization.solve(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_lu_solve_real() {
        let a = array![[4.0_f64, 1.0], [1.0, 3.0],];

        let b = array![1.0_f64, 2.0];

        let x = lu_solve(&a, &b).expect("LU solve should succeed");

        // Verify: Ax = b
        let ax = a.dot(&x);
        for i in 0..2 {
            assert_relative_eq!(ax[i], b[i], epsilon = 1e-10);
        }
    }

    #[test]
    fn test_lu_solve_complex() {
        let a = array![
            [Complex64::new(4.0, 1.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, -1.0)],
        ];

        let b = array![Complex64::new(1.0, 1.0), Complex64::new(2.0, -1.0)];

        let x = lu_solve(&a, &b).expect("LU solve should succeed");

        // Verify: Ax ≈ b
        let ax = a.dot(&x);
        for i in 0..2 {
            assert_relative_eq!((ax[i] - b[i]).norm(), 0.0, epsilon = 1e-10);
        }
    }

    #[test]
    fn test_lu_identity() {
        let n = 5;
        let a = Array2::from_diag(&Array1::from_elem(n, 1.0_f64));
        let b = Array1::from_iter((1..=n).map(|i| i as f64));

        let x = lu_solve(&a, &b).expect("LU solve should succeed");

        for i in 0..n {
            assert_relative_eq!(x[i], b[i], epsilon = 1e-10);
        }
    }

    #[test]
    fn test_lu_singular() {
        let a = array![[1.0_f64, 2.0], [2.0, 4.0],]; // Singular matrix

        let b = array![1.0_f64, 2.0];

        let result = lu_solve(&a, &b);
        assert!(result.is_err());
    }

    #[test]
    fn test_lu_factorize_and_solve() {
        let a = array![[4.0_f64, 1.0, 0.0], [1.0, 3.0, 1.0], [0.0, 1.0, 2.0],];

        let factorization = lu_factorize(&a).expect("Factorization should succeed");

        // Solve multiple RHS
        let b1 = array![1.0_f64, 2.0, 3.0];
        let x1 = factorization.solve(&b1).expect("Solve should succeed");

        let ax1 = a.dot(&x1);
        for i in 0..3 {
            assert_relative_eq!(ax1[i], b1[i], epsilon = 1e-10);
        }

        let b2 = array![4.0_f64, 5.0, 6.0];
        let x2 = factorization.solve(&b2).expect("Solve should succeed");

        let ax2 = a.dot(&x2);
        for i in 0..3 {
            assert_relative_eq!(ax2[i], b2[i], epsilon = 1e-10);
        }
    }
}
