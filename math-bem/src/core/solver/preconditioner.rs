//! Preconditioners for iterative solvers
//!
//! Provides various preconditioning strategies for improving
//! convergence of CGS and BiCGSTAB solvers.

use ndarray::{Array1, Array2};
use num_complex::Complex64;

/// Preconditioner trait
pub trait Preconditioner {
    /// Apply the preconditioner: solve M*z = r for z
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64>;
}

/// Identity preconditioner (no preconditioning)
#[derive(Debug, Clone)]
pub struct IdentityPreconditioner;

impl Preconditioner for IdentityPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        r.clone()
    }
}

/// Diagonal (Jacobi) preconditioner
///
/// M = diag(A), so M⁻¹ * r = r ./ diag(A)
#[derive(Debug, Clone)]
pub struct DiagonalPreconditioner {
    /// Inverse of diagonal elements
    diag_inv: Array1<Complex64>,
}

impl DiagonalPreconditioner {
    /// Create from a matrix
    pub fn from_matrix(a: &Array2<Complex64>) -> Self {
        let n = a.nrows();
        let diag_inv = Array1::from_vec(
            (0..n)
                .map(|i| {
                    let d = a[[i, i]];
                    if d.norm() > 1e-15 {
                        Complex64::new(1.0, 0.0) / d
                    } else {
                        Complex64::new(1.0, 0.0)
                    }
                })
                .collect(),
        );
        Self { diag_inv }
    }

    /// Create from a diagonal vector
    pub fn from_diagonal(diag: &Array1<Complex64>) -> Self {
        let diag_inv = diag
            .iter()
            .map(|&d| {
                if d.norm() > 1e-15 {
                    Complex64::new(1.0, 0.0) / d
                } else {
                    Complex64::new(1.0, 0.0)
                }
            })
            .collect();
        Self { diag_inv }
    }
}

impl Preconditioner for DiagonalPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        &self.diag_inv * r
    }
}

/// Row scaling preconditioner
///
/// Scales each row by the inverse of its norm (from NC code)
#[derive(Debug, Clone)]
pub struct RowScalingPreconditioner {
    /// Scale factors for each row
    scale: Array1<Complex64>,
}

impl RowScalingPreconditioner {
    /// Create from a matrix
    pub fn from_matrix(a: &Array2<Complex64>) -> Self {
        let n = a.nrows();
        let scale = Array1::from_vec(
            (0..n)
                .map(|i| {
                    let row_norm: f64 = a.row(i).iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
                    if row_norm > 1e-15 {
                        Complex64::new(1.0 / row_norm, 0.0)
                    } else {
                        Complex64::new(1.0, 0.0)
                    }
                })
                .collect(),
        );
        Self { scale }
    }
}

impl Preconditioner for RowScalingPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        &self.scale * r
    }
}

/// Block diagonal preconditioner
///
/// For FMM systems where diagonal blocks correspond to clusters
#[derive(Debug, Clone)]
pub struct BlockDiagonalPreconditioner {
    /// Inverse of each diagonal block
    block_inv: Vec<Array2<Complex64>>,
    /// Block sizes
    block_sizes: Vec<usize>,
}

impl BlockDiagonalPreconditioner {
    /// Create from diagonal blocks
    pub fn from_blocks(blocks: Vec<Array2<Complex64>>) -> Self {
        use ndarray_linalg::Inverse;

        let block_sizes: Vec<usize> = blocks.iter().map(|b| b.nrows()).collect();
        let block_inv: Vec<Array2<Complex64>> = blocks
            .into_iter()
            .map(|b| {
                b.inv().unwrap_or_else(|_| {
                    let n = b.nrows();
                    Array2::eye(n)
                })
            })
            .collect();

        Self {
            block_inv,
            block_sizes,
        }
    }
}

impl Preconditioner for BlockDiagonalPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        let mut z = Array1::zeros(r.len());
        let mut offset = 0;

        for (block_inv, &block_size) in self.block_inv.iter().zip(self.block_sizes.iter()) {
            let r_block =
                Array1::from_vec(r.slice(ndarray::s![offset..offset + block_size]).to_vec());
            let z_block = block_inv.dot(&r_block);

            for (i, &val) in z_block.iter().enumerate() {
                z[offset + i] = val;
            }

            offset += block_size;
        }

        z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_preconditioner() {
        let r = Array1::from_vec(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
        ]);

        let precond = IdentityPreconditioner;
        let z = precond.apply(&r);

        assert_eq!(z, r);
    }

    #[test]
    fn test_diagonal_preconditioner() {
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(2.0, 0.0),
            ],
        )
        .unwrap();

        let precond = DiagonalPreconditioner::from_matrix(&a);

        let r = Array1::from_vec(vec![Complex64::new(4.0, 0.0), Complex64::new(4.0, 0.0)]);
        let z = precond.apply(&r);

        // z[0] = 4.0 / 4.0 = 1.0
        // z[1] = 4.0 / 2.0 = 2.0
        assert!((z[0] - Complex64::new(1.0, 0.0)).norm() < 1e-10);
        assert!((z[1] - Complex64::new(2.0, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn test_row_scaling_preconditioner() {
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(3.0, 0.0),
                Complex64::new(4.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(5.0, 0.0),
            ],
        )
        .unwrap();

        let precond = RowScalingPreconditioner::from_matrix(&a);

        // Row 0: norm = sqrt(9 + 16) = 5, scale = 0.2
        // Row 1: norm = sqrt(0 + 25) = 5, scale = 0.2
        let r = Array1::from_vec(vec![Complex64::new(5.0, 0.0), Complex64::new(5.0, 0.0)]);
        let z = precond.apply(&r);

        assert!((z[0] - Complex64::new(1.0, 0.0)).norm() < 1e-10);
        assert!((z[1] - Complex64::new(1.0, 0.0)).norm() < 1e-10);
    }
}
