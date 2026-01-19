//! Diagonal (Jacobi) preconditioner
//!
//! Simple but effective preconditioner that scales by the diagonal of A.
//!
//! This preconditioner is embarrassingly parallel since it only involves
//! element-wise operations.

use crate::sparse::CsrMatrix;
use crate::traits::{ComplexField, Preconditioner};
use ndarray::Array1;
use num_traits::FromPrimitive;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Diagonal (Jacobi) preconditioner
///
/// M = diag(A), so M^(-1) scales each component by 1/A_ii
#[derive(Debug, Clone)]
pub struct DiagonalPreconditioner<T: ComplexField> {
    /// Inverse diagonal elements
    inv_diag: Array1<T>,
}

impl<T: ComplexField> DiagonalPreconditioner<T> {
    /// Create a diagonal preconditioner from a CSR matrix
    pub fn from_csr(matrix: &CsrMatrix<T>) -> Self {
        let diag = matrix.diagonal();
        let inv_diag = diag.mapv(|d| {
            if d.norm() > T::Real::from_f64(1e-30).unwrap() {
                d.inv()
            } else {
                T::one()
            }
        });
        Self { inv_diag }
    }

    /// Create from a diagonal vector directly
    pub fn from_diagonal(diag: &Array1<T>) -> Self {
        let inv_diag = diag.mapv(|d| {
            if d.norm() > T::Real::from_f64(1e-30).unwrap() {
                d.inv()
            } else {
                T::one()
            }
        });
        Self { inv_diag }
    }

    /// Create from inverse diagonal vector directly
    pub fn from_inverse_diagonal(inv_diag: Array1<T>) -> Self {
        Self { inv_diag }
    }
}

impl<T: ComplexField> Preconditioner<T> for DiagonalPreconditioner<T> {
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        #[cfg(feature = "rayon")]
        {
            if r.len() >= 1000 {
                return self.apply_parallel(r);
            }
        }
        self.apply_sequential(r)
    }
}

impl<T: ComplexField> DiagonalPreconditioner<T> {
    fn apply_sequential(&self, r: &Array1<T>) -> Array1<T> {
        r.iter()
            .zip(self.inv_diag.iter())
            .map(|(&ri, &di)| ri * di)
            .collect()
    }

    #[cfg(feature = "rayon")]
    fn apply_parallel(&self, r: &Array1<T>) -> Array1<T>
    where
        T: Send + Sync,
    {
        let r_slice = r.as_slice().expect("Array should be contiguous");
        let inv_slice = self
            .inv_diag
            .as_slice()
            .expect("Array should be contiguous");

        let results: Vec<T> = r_slice
            .par_iter()
            .zip(inv_slice.par_iter())
            .map(|(&ri, &di)| ri * di)
            .collect();

        Array1::from_vec(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_diagonal_preconditioner() {
        let diag = array![
            Complex64::new(2.0, 0.0),
            Complex64::new(4.0, 0.0),
            Complex64::new(1.0, 0.0)
        ];

        let precond = DiagonalPreconditioner::from_diagonal(&diag);

        let r = array![
            Complex64::new(2.0, 0.0),
            Complex64::new(8.0, 0.0),
            Complex64::new(3.0, 0.0)
        ];

        let result = precond.apply(&r);

        assert_relative_eq!(result[0].re, 1.0, epsilon = 1e-10);
        assert_relative_eq!(result[1].re, 2.0, epsilon = 1e-10);
        assert_relative_eq!(result[2].re, 3.0, epsilon = 1e-10);
    }

    #[test]
    fn test_diagonal_from_csr() {
        use crate::sparse::CsrMatrix;

        let dense = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
        ];

        let matrix = CsrMatrix::from_dense(&dense, 1e-15);
        let precond = DiagonalPreconditioner::from_csr(&matrix);

        let r = array![Complex64::new(4.0, 0.0), Complex64::new(4.0, 0.0)];
        let result = precond.apply(&r);

        assert_relative_eq!(result[0].re, 1.0, epsilon = 1e-10);
        assert_relative_eq!(result[1].re, 2.0, epsilon = 1e-10);
    }
}
