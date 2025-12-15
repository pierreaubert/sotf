//! Parallel ILU preconditioners
//!
//! Two parallel approaches for ILU triangular solves:
//!
//! 1. **Graph Coloring (Level Scheduling)**: Groups rows into independent sets (levels)
//!    that can be solved in parallel. Rows within the same level have no dependencies.
//!
//! 2. **Fine-Grained Fixed-Point Iteration**: Uses Jacobi-style iteration to
//!    approximate the triangular solve. Each iteration is embarrassingly parallel.

use crate::sparse::CsrMatrix;
use crate::traits::{ComplexField, Preconditioner};
use ndarray::Array1;
use num_traits::FromPrimitive;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

// ============================================================================
// Graph Coloring (Level Scheduling) ILU
// ============================================================================

/// Parallel ILU using Graph Coloring / Level Scheduling
///
/// This approach analyzes the sparsity pattern of L and U to find independent
/// rows that can be solved in parallel. Rows are grouped into "levels" where
/// all rows in a level depend only on rows from previous levels.
///
/// Good for matrices with limited fill-in and structured sparsity patterns.
#[derive(Debug, Clone)]
pub struct IluColoringPreconditioner<T: ComplexField> {
    /// Lower triangular factor values
    l_values: Vec<T>,
    l_col_indices: Vec<usize>,
    l_row_ptrs: Vec<usize>,
    /// Upper triangular factor values
    u_values: Vec<T>,
    u_col_indices: Vec<usize>,
    u_row_ptrs: Vec<usize>,
    /// Diagonal of U
    u_diag: Vec<T>,
    /// Forward solve levels: each level contains row indices that can be solved in parallel
    forward_levels: Vec<Vec<usize>>,
    /// Backward solve levels: each level contains row indices that can be solved in parallel
    backward_levels: Vec<Vec<usize>>,
    /// Matrix dimension
    n: usize,
}

impl<T: ComplexField> IluColoringPreconditioner<T> {
    /// Create ILU preconditioner with graph coloring from a CSR matrix
    pub fn from_csr(matrix: &CsrMatrix<T>) -> Self {
        let n = matrix.num_rows;

        // Perform standard ILU(0) factorization
        let mut values = matrix.values.clone();
        let col_indices = matrix.col_indices.clone();
        let row_ptrs = matrix.row_ptrs.clone();

        // ILU(0) factorization (same as sequential)
        for i in 0..n {
            for idx in row_ptrs[i]..row_ptrs[i + 1] {
                let k = col_indices[idx];
                if k >= i {
                    break;
                }

                let mut u_kk = T::zero();
                for k_idx in row_ptrs[k]..row_ptrs[k + 1] {
                    if col_indices[k_idx] == k {
                        u_kk = values[k_idx];
                        break;
                    }
                }

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

                    for k_j_idx in row_ptrs[k]..row_ptrs[k + 1] {
                        if col_indices[k_j_idx] == j {
                            values[j_idx] = values[j_idx] - l_ik * values[k_j_idx];
                            break;
                        }
                    }
                }
            }
        }

        // Extract L and U
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

        // Compute level scheduling for L (forward solve)
        let forward_levels = compute_forward_levels(n, &l_col_indices, &l_row_ptrs);

        // Compute level scheduling for U (backward solve)
        let backward_levels = compute_backward_levels(n, &u_col_indices, &u_row_ptrs);

        Self {
            l_values,
            l_col_indices,
            l_row_ptrs,
            u_values,
            u_col_indices,
            u_row_ptrs,
            u_diag,
            forward_levels,
            backward_levels,
            n,
        }
    }

    /// Get statistics about the level structure
    pub fn level_stats(&self) -> (usize, usize, f64, f64) {
        let fwd_levels = self.forward_levels.len();
        let bwd_levels = self.backward_levels.len();
        let avg_fwd = self.n as f64 / fwd_levels as f64;
        let avg_bwd = self.n as f64 / bwd_levels as f64;
        (fwd_levels, bwd_levels, avg_fwd, avg_bwd)
    }
}

/// Compute forward substitution levels (for lower triangular L)
fn compute_forward_levels(
    n: usize,
    l_col_indices: &[usize],
    l_row_ptrs: &[usize],
) -> Vec<Vec<usize>> {
    // Level of each row: level[i] = max(level[j] for all j where L[i,j] != 0) + 1
    let mut level = vec![0usize; n];

    for i in 0..n {
        let mut max_dep_level = 0;
        for &j in &l_col_indices[l_row_ptrs[i]..l_row_ptrs[i + 1]] {
            max_dep_level = max_dep_level.max(level[j] + 1);
        }
        level[i] = max_dep_level;
    }

    // Group rows by level
    let max_level = *level.iter().max().unwrap_or(&0);
    let mut levels = vec![Vec::new(); max_level + 1];
    for (i, &lvl) in level.iter().enumerate() {
        levels[lvl].push(i);
    }

    levels
}

/// Compute backward substitution levels (for upper triangular U)
fn compute_backward_levels(
    n: usize,
    u_col_indices: &[usize],
    u_row_ptrs: &[usize],
) -> Vec<Vec<usize>> {
    // For backward solve, we process in reverse order
    // Level of each row: level[i] = max(level[j] for all j > i where U[i,j] != 0) + 1
    let mut level = vec![0usize; n];

    for i in (0..n).rev() {
        let mut max_dep_level = 0;
        for &j in &u_col_indices[u_row_ptrs[i]..u_row_ptrs[i + 1]] {
            if j > i {
                max_dep_level = max_dep_level.max(level[j] + 1);
            }
        }
        level[i] = max_dep_level;
    }

    // Group rows by level
    let max_level = *level.iter().max().unwrap_or(&0);
    let mut levels = vec![Vec::new(); max_level + 1];
    for (i, &lvl) in level.iter().enumerate() {
        levels[lvl].push(i);
    }

    levels
}

impl<T: ComplexField + Send + Sync> Preconditioner<T> for IluColoringPreconditioner<T> {
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        #[cfg(feature = "rayon")]
        {
            if self.n >= 246 {
                return self.apply_parallel(r);
            }
        }
        self.apply_sequential(r)
    }
}

impl<T: ComplexField + Send + Sync> IluColoringPreconditioner<T> {
    fn apply_sequential(&self, r: &Array1<T>) -> Array1<T> {
        let mut y = r.clone();

        // Forward substitution
        for i in 0..self.n {
            for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                let j = self.l_col_indices[idx];
                let l_ij = self.l_values[idx];
                y[i] = y[i] - l_ij * y[j];
            }
        }

        // Backward substitution
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

    #[cfg(feature = "rayon")]
    fn apply_parallel(&self, r: &Array1<T>) -> Array1<T> {
        use std::cell::UnsafeCell;

        // Wrapper to allow shared mutable access (safe because we guarantee non-overlapping writes)
        struct UnsafeVec<U>(UnsafeCell<Vec<U>>);
        unsafe impl<U: Send> Sync for UnsafeVec<U> {}

        impl<U: Copy> UnsafeVec<U> {
            fn get(&self, i: usize) -> U {
                unsafe { (&(*self.0.get()))[i] }
            }
            fn set(&self, i: usize, val: U) {
                unsafe { (&mut (*self.0.get()))[i] = val }
            }
        }

        let y = UnsafeVec(UnsafeCell::new(r.to_vec()));

        // Forward substitution by levels
        for level in &self.forward_levels {
            if level.len() > 32 {
                // Parallel within level
                level.par_iter().for_each(|&i| {
                    let mut sum = T::zero();
                    for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                        let j = self.l_col_indices[idx];
                        let l_ij = self.l_values[idx];
                        // Safe: j < i always for lower triangular, all j in previous levels
                        sum += l_ij * y.get(j);
                    }
                    // Safe: each thread writes to unique index
                    y.set(i, y.get(i) - sum);
                });
            } else {
                // Sequential for small levels
                for &i in level {
                    let mut sum = T::zero();
                    for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                        let j = self.l_col_indices[idx];
                        let l_ij = self.l_values[idx];
                        sum += l_ij * y.get(j);
                    }
                    y.set(i, y.get(i) - sum);
                }
            }
        }

        // Move y into x for backward solve
        let x = UnsafeVec(UnsafeCell::new(y.0.into_inner()));

        // Backward substitution by levels
        for level in &self.backward_levels {
            if level.len() > 32 {
                let u_diag = &self.u_diag;
                level.par_iter().for_each(|&i| {
                    let mut sum = T::zero();
                    for idx in self.u_row_ptrs[i]..self.u_row_ptrs[i + 1] {
                        let j = self.u_col_indices[idx];
                        if j > i {
                            let u_ij = self.u_values[idx];
                            // Safe: j > i and all j in previous levels
                            sum += u_ij * x.get(j);
                        }
                    }
                    let xi = x.get(i) - sum;
                    let u_ii = u_diag[i];
                    if u_ii.norm() > T::Real::from_f64(1e-30).unwrap() {
                        x.set(i, xi * u_ii.inv());
                    } else {
                        x.set(i, xi);
                    }
                });
            } else {
                for &i in level {
                    let mut sum = T::zero();
                    for idx in self.u_row_ptrs[i]..self.u_row_ptrs[i + 1] {
                        let j = self.u_col_indices[idx];
                        if j > i {
                            let u_ij = self.u_values[idx];
                            sum += u_ij * x.get(j);
                        }
                    }
                    let xi = x.get(i) - sum;
                    let u_ii = self.u_diag[i];
                    if u_ii.norm() > T::Real::from_f64(1e-30).unwrap() {
                        x.set(i, xi * u_ii.inv());
                    } else {
                        x.set(i, xi);
                    }
                }
            }
        }

        Array1::from_vec(x.0.into_inner())
    }
}

// ============================================================================
// Fine-Grained Fixed-Point Iteration ILU
// ============================================================================

/// Parallel ILU using Fine-Grained Fixed-Point Iteration
///
/// Instead of exact triangular solves, this approach uses Jacobi-style
/// fixed-point iteration: x^{k+1} = D^{-1}(b - (L+U)x^k)
///
/// Each iteration is embarrassingly parallel. Typically 2-5 iterations
/// are enough for a good approximation.
///
/// This trades exact solves for parallelism and can be faster on
/// highly parallel hardware.
#[derive(Debug, Clone)]
pub struct IluFixedPointPreconditioner<T: ComplexField> {
    /// Lower triangular factor values
    l_values: Vec<T>,
    l_col_indices: Vec<usize>,
    l_row_ptrs: Vec<usize>,
    /// Upper triangular factor values (excluding diagonal)
    u_off_values: Vec<T>,
    u_off_col_indices: Vec<usize>,
    u_off_row_ptrs: Vec<usize>,
    /// Inverse diagonal of U (for fast multiplication)
    u_diag_inv: Vec<T>,
    /// Number of fixed-point iterations
    iterations: usize,
    /// Matrix dimension
    n: usize,
}

impl<T: ComplexField> IluFixedPointPreconditioner<T> {
    /// Create ILU preconditioner with fixed-point iteration
    ///
    /// # Arguments
    /// * `matrix` - The sparse matrix to precondition
    /// * `iterations` - Number of fixed-point iterations (typically 2-5)
    pub fn from_csr(matrix: &CsrMatrix<T>, iterations: usize) -> Self {
        let n = matrix.num_rows;

        // Perform standard ILU(0) factorization
        let mut values = matrix.values.clone();
        let col_indices = matrix.col_indices.clone();
        let row_ptrs = matrix.row_ptrs.clone();

        for i in 0..n {
            for idx in row_ptrs[i]..row_ptrs[i + 1] {
                let k = col_indices[idx];
                if k >= i {
                    break;
                }

                let mut u_kk = T::zero();
                for k_idx in row_ptrs[k]..row_ptrs[k + 1] {
                    if col_indices[k_idx] == k {
                        u_kk = values[k_idx];
                        break;
                    }
                }

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

                    for k_j_idx in row_ptrs[k]..row_ptrs[k + 1] {
                        if col_indices[k_j_idx] == j {
                            values[j_idx] = values[j_idx] - l_ik * values[k_j_idx];
                            break;
                        }
                    }
                }
            }
        }

        // Extract L, U (off-diagonal), and D^{-1}
        let mut l_values = Vec::new();
        let mut l_col_indices = Vec::new();
        let mut l_row_ptrs = vec![0];

        let mut u_off_values = Vec::new();
        let mut u_off_col_indices = Vec::new();
        let mut u_off_row_ptrs = vec![0];

        let mut u_diag_inv = vec![T::one(); n];

        for i in 0..n {
            for idx in row_ptrs[i]..row_ptrs[i + 1] {
                let j = col_indices[idx];
                let val = values[idx];

                if j < i {
                    // Lower triangular (L)
                    l_values.push(val);
                    l_col_indices.push(j);
                } else if j == i {
                    // Diagonal
                    if val.norm() > T::Real::from_f64(1e-30).unwrap() {
                        u_diag_inv[i] = val.inv();
                    }
                } else {
                    // Upper triangular off-diagonal
                    u_off_values.push(val);
                    u_off_col_indices.push(j);
                }
            }
            l_row_ptrs.push(l_values.len());
            u_off_row_ptrs.push(u_off_values.len());
        }

        Self {
            l_values,
            l_col_indices,
            l_row_ptrs,
            u_off_values,
            u_off_col_indices,
            u_off_row_ptrs,
            u_diag_inv,
            iterations,
            n,
        }
    }

    /// Create with default 3 iterations
    pub fn from_csr_default(matrix: &CsrMatrix<T>) -> Self {
        Self::from_csr(matrix, 3)
    }
}

impl<T: ComplexField + Send + Sync> Preconditioner<T> for IluFixedPointPreconditioner<T> {
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        #[cfg(feature = "rayon")]
        {
            if self.n >= 246 {
                return self.apply_parallel(r);
            }
        }
        self.apply_sequential(r)
    }
}

impl<T: ComplexField + Send + Sync> IluFixedPointPreconditioner<T> {
    fn apply_sequential(&self, r: &Array1<T>) -> Array1<T> {
        // Solve (L+D+U)x = r using fixed-point iteration
        // Split: (D)(I + D^{-1}(L+U))x = r
        // Iteration: x^{k+1} = D^{-1}(r - (L+U)x^k)

        // Initial guess: x = D^{-1} r
        let mut x: Vec<T> = r
            .iter()
            .zip(self.u_diag_inv.iter())
            .map(|(&ri, &di)| ri * di)
            .collect();

        for _ in 0..self.iterations {
            let mut x_new = vec![T::zero(); self.n];

            for i in 0..self.n {
                let mut sum = r[i];

                // Subtract L*x contribution
                for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                    let j = self.l_col_indices[idx];
                    sum -= self.l_values[idx] * x[j];
                }

                // Subtract U_off*x contribution
                for idx in self.u_off_row_ptrs[i]..self.u_off_row_ptrs[i + 1] {
                    let j = self.u_off_col_indices[idx];
                    sum -= self.u_off_values[idx] * x[j];
                }

                x_new[i] = sum * self.u_diag_inv[i];
            }

            x = x_new;
        }

        Array1::from_vec(x)
    }

    #[cfg(feature = "rayon")]
    fn apply_parallel(&self, r: &Array1<T>) -> Array1<T> {
        // Initial guess: x = D^{-1} r
        let mut x: Vec<T> = r
            .iter()
            .zip(self.u_diag_inv.iter())
            .map(|(&ri, &di)| ri * di)
            .collect();

        let r_slice = r.as_slice().expect("Array should be contiguous");

        for _ in 0..self.iterations {
            // Each row can be computed independently
            let x_new: Vec<T> = (0..self.n)
                .into_par_iter()
                .map(|i| {
                    let mut sum = r_slice[i];

                    // Subtract L*x contribution
                    for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                        let j = self.l_col_indices[idx];
                        sum -= self.l_values[idx] * x[j];
                    }

                    // Subtract U_off*x contribution
                    for idx in self.u_off_row_ptrs[i]..self.u_off_row_ptrs[i + 1] {
                        let j = self.u_off_col_indices[idx];
                        sum -= self.u_off_values[idx] * x[j];
                    }

                    sum * self.u_diag_inv[i]
                })
                .collect();

            x = x_new;
        }

        Array1::from_vec(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iterative::{GmresConfig, gmres_preconditioned};
    use num_complex::Complex64;

    fn create_test_matrix() -> CsrMatrix<Complex64> {
        let n = 10;
        let mut dense = ndarray::Array2::from_elem((n, n), Complex64::new(0.0, 0.0));

        for i in 0..n {
            dense[[i, i]] = Complex64::new(4.0, 0.0);
            if i > 0 {
                dense[[i, i - 1]] = Complex64::new(-1.0, 0.0);
            }
            if i < n - 1 {
                dense[[i, i + 1]] = Complex64::new(-1.0, 0.0);
            }
        }

        CsrMatrix::from_dense(&dense, 1e-15)
    }

    #[test]
    fn test_ilu_coloring_basic() {
        let matrix = create_test_matrix();
        let precond = IluColoringPreconditioner::from_csr(&matrix);

        let r = Array1::from_iter((0..10).map(|i| Complex64::new((i as f64).sin(), 0.0)));
        let result = precond.apply(&r);

        // Should produce a reasonable result
        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|x| x.norm() < 100.0));
    }

    #[test]
    fn test_ilu_fixedpoint_basic() {
        let matrix = create_test_matrix();
        let precond = IluFixedPointPreconditioner::from_csr(&matrix, 3);

        let r = Array1::from_iter((0..10).map(|i| Complex64::new((i as f64).sin(), 0.0)));
        let result = precond.apply(&r);

        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|x| x.norm() < 100.0));
    }

    #[test]
    fn test_ilu_coloring_with_gmres() {
        let matrix = create_test_matrix();
        let precond = IluColoringPreconditioner::from_csr(&matrix);

        let b = Array1::from_iter((0..10).map(|i| Complex64::new((i as f64).sin(), 0.0)));

        let config = GmresConfig {
            max_iterations: 50,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let sol = gmres_preconditioned(&matrix, &precond, &b, &config);
        assert!(sol.converged, "GMRES with ILU-coloring should converge");
    }

    #[test]
    fn test_ilu_fixedpoint_with_gmres() {
        let matrix = create_test_matrix();
        let precond = IluFixedPointPreconditioner::from_csr(&matrix, 3);

        let b = Array1::from_iter((0..10).map(|i| Complex64::new((i as f64).sin(), 0.0)));

        let config = GmresConfig {
            max_iterations: 50,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let sol = gmres_preconditioned(&matrix, &precond, &b, &config);
        assert!(sol.converged, "GMRES with ILU-fixedpoint should converge");
    }

    #[test]
    fn test_level_stats() {
        let matrix = create_test_matrix();
        let precond = IluColoringPreconditioner::from_csr(&matrix);

        let (fwd, bwd, avg_fwd, avg_bwd) = precond.level_stats();

        // Tridiagonal matrix should have limited parallelism
        assert!(fwd > 0);
        assert!(bwd > 0);
        assert!(avg_fwd > 0.0);
        assert!(avg_bwd > 0.0);
    }
}
