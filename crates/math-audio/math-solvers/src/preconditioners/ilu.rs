//! ILU(0) preconditioner
//!
//! Incomplete LU factorization with no fill-in.
//! Effective for many sparse systems, especially those arising from discretization.

use crate::sparse::CsrMatrix;
use crate::traits::{ComplexField, Preconditioner};
use ndarray::Array1;
use num_traits::FromPrimitive;

/// ILU(0) preconditioner
///
/// Computes an incomplete LU factorization where the sparsity pattern
/// of L and U is the same as the original matrix.
#[derive(Debug, Clone)]
pub struct IluPreconditioner<T: ComplexField> {
    /// Lower triangular factor (stored in CSR format conceptually)
    l_values: Vec<T>,
    l_col_indices: Vec<usize>,
    l_row_ptrs: Vec<usize>,
    /// Upper triangular factor
    u_values: Vec<T>,
    u_col_indices: Vec<usize>,
    u_row_ptrs: Vec<usize>,
    /// Diagonal of U (for fast access)
    u_diag: Vec<T>,
    /// Matrix dimension
    n: usize,
    #[allow(dead_code)]
    /// Precomputed diagonal indices for fast lookup during factorization
    diag_indices: Vec<usize>,
}

impl<T: ComplexField> IluPreconditioner<T> {
    /// Create ILU(0) preconditioner from a CSR matrix
    pub fn from_csr(matrix: &CsrMatrix<T>) -> Self {
        let n = matrix.num_rows;

        let col_indices = &matrix.col_indices;
        let row_ptrs = &matrix.row_ptrs;

        let mut diag_indices = vec![usize::MAX; n];
        for i in 0..n {
            #[allow(clippy::needless_range_loop)]
            for idx in row_ptrs[i]..row_ptrs[i + 1] {
                if col_indices[idx] == i {
                    diag_indices[i] = idx;
                    break;
                }
            }
        }

        let mut values = matrix.values.clone();

        for i in 0..n {
            for idx in row_ptrs[i]..row_ptrs[i + 1] {
                let k = col_indices[idx];
                if k >= i {
                    break;
                }

                let u_kk_idx = diag_indices[k];
                if u_kk_idx == usize::MAX {
                    continue;
                }

                let u_kk = values[u_kk_idx];

                if u_kk.norm() < T::Real::from_f64(1e-30).unwrap() {
                    continue;
                }

                let l_ik = values[idx] * u_kk.inv();
                values[idx] = l_ik;

                for j_idx in row_ptrs[i]..row_ptrs[i + 1] {
                    let j = col_indices[j_idx];
                    if j <= k {
                        continue;
                    }

                    let u_kj_idx = diag_indices[k] + 1;
                    if u_kj_idx < row_ptrs[k + 1] && col_indices[u_kj_idx] == j {
                        values[j_idx] = values[j_idx] - l_ik * values[u_kj_idx];
                    } else {
                        for search_idx in (row_ptrs[k] + 1)..row_ptrs[k + 1] {
                            if col_indices[search_idx] == j {
                                values[j_idx] = values[j_idx] - l_ik * values[search_idx];
                                break;
                            }
                        }
                    }
                }
            }
        }

        let mut l_values = Vec::new();
        let mut l_col_indices = Vec::new();
        let mut l_row_ptrs = vec![0];

        let mut u_values = Vec::new();
        let mut u_col_indices = Vec::new();
        let mut u_row_ptrs = vec![0];
        let mut u_diag = vec![T::one(); n];

        for i in 0..n {
            for idx in row_ptrs[i]..row_ptrs[i + 1] {
                let j = col_indices[idx];
                let val = values[idx];

                if j < i {
                    l_values.push(val);
                    l_col_indices.push(j);
                } else {
                    u_values.push(val);
                    u_col_indices.push(j);
                    if j == i {
                        u_diag[i] = val;
                    }
                }
            }
            l_row_ptrs.push(l_values.len());
            u_row_ptrs.push(u_values.len());
        }

        Self {
            l_values,
            l_col_indices,
            l_row_ptrs,
            u_values,
            u_col_indices,
            u_row_ptrs,
            u_diag,
            n,
            diag_indices,
        }
    }
}

impl<T: ComplexField> Preconditioner<T> for IluPreconditioner<T> {
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        let mut y = r.clone();

        // Forward substitution: Ly = r (L has unit diagonal)
        for i in 0..self.n {
            for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                let j = self.l_col_indices[idx];
                let l_ij = self.l_values[idx];
                y[i] = y[i] - l_ij * y[j];
            }
        }

        // Backward substitution: Ux = y
        let mut x = y;
        for i in (0..self.n).rev() {
            for idx in self.u_row_ptrs[i]..self.u_row_ptrs[i + 1] {
                let j = self.u_col_indices[idx];
                if j > i {
                    let u_ij = self.u_values[idx];
                    x[i] = x[i] - u_ij * x[j];
                }
            }

            let u_ii = self.u_diag[i];
            if u_ii.norm() > T::Real::from_f64(1e-30).unwrap() {
                x[i] *= u_ii.inv();
            }
        }

        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iterative::{GmresConfig, gmres, gmres_preconditioned};
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_ilu_preconditioner() {
        // Simple tridiagonal matrix
        let dense = array![
            [
                Complex64::new(4.0, 0.0),
                Complex64::new(-1.0, 0.0),
                Complex64::new(0.0, 0.0)
            ],
            [
                Complex64::new(-1.0, 0.0),
                Complex64::new(4.0, 0.0),
                Complex64::new(-1.0, 0.0)
            ],
            [
                Complex64::new(0.0, 0.0),
                Complex64::new(-1.0, 0.0),
                Complex64::new(4.0, 0.0)
            ],
        ];

        let matrix = CsrMatrix::from_dense(&dense, 1e-15);
        let precond = IluPreconditioner::from_csr(&matrix);

        let r = array![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0)
        ];

        let result = precond.apply(&r);

        // Verify that M*result ≈ r (where M = L*U)
        // For ILU(0) on this matrix, M should be close to A
        let check = matrix.matvec(&result);
        for i in 0..3 {
            // Allow some error since ILU is approximate
            assert!(
                (check[i] - r[i]).norm() < 0.5,
                "ILU should approximately invert: got {:?} expected {:?}",
                check[i],
                r[i]
            );
        }
    }

    #[test]
    fn test_ilu_with_gmres() {
        // Larger system where preconditioning helps
        let n = 10;
        let mut dense = ndarray::Array2::from_elem((n, n), Complex64::new(0.0, 0.0));

        // Tridiagonal matrix
        for i in 0..n {
            dense[[i, i]] = Complex64::new(4.0, 0.0);
            if i > 0 {
                dense[[i, i - 1]] = Complex64::new(-1.0, 0.0);
            }
            if i < n - 1 {
                dense[[i, i + 1]] = Complex64::new(-1.0, 0.0);
            }
        }

        let matrix = CsrMatrix::from_dense(&dense, 1e-15);
        let precond = IluPreconditioner::from_csr(&matrix);

        let b = Array1::from_iter((0..n).map(|i| Complex64::new((i as f64).sin(), 0.0)));

        let config = GmresConfig {
            max_iterations: 50,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        // Solve without preconditioning
        let sol_no_precond = gmres(&matrix, &b, &config);

        // Solve with preconditioning
        let sol_precond = gmres_preconditioned(&matrix, &precond, &b, &config);

        // Both should converge
        assert!(sol_no_precond.converged);
        assert!(sol_precond.converged);

        // Preconditioned should need fewer iterations (usually)
        // For this well-conditioned problem, difference may be small
        assert!(
            sol_precond.iterations <= sol_no_precond.iterations + 5,
            "Preconditioning should not hurt: {} vs {}",
            sol_precond.iterations,
            sol_no_precond.iterations
        );
    }
}
