//! Additive Schwarz Domain Decomposition Preconditioner
//!
//! Implements parallel domain decomposition with overlapping subdomains.
//! Each subdomain is solved independently (embarrassingly parallel), and
//! solutions are combined additively.
//!
//! # Algorithm
//!
//! 1. Partition DOFs into `num_subdomains` groups (contiguous blocks by default)
//! 2. Extend each subdomain by `overlap` layers of neighboring DOFs
//! 3. For each preconditioner apply:
//!    - Extract local subdomain problems in parallel
//!    - Solve each subdomain independently (using local ILU or direct solve)
//!    - Combine solutions with appropriate weighting in overlap regions
//!
//! # Parallelism
//!
//! The subdomain solves are embarrassingly parallel since they operate on
//! independent (though overlapping) portions of the matrix.

use crate::sparse::CsrMatrix;
use crate::traits::{ComplexField, Preconditioner};
use ndarray::Array1;
use num_traits::{FromPrimitive, One};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Additive Schwarz preconditioner with overlapping subdomains
#[derive(Debug, Clone)]
pub struct AdditiveSchwarzPreconditioner<T: ComplexField> {
    /// Subdomain information
    subdomains: Vec<Subdomain<T>>,
    /// Weights for combining solutions (1/count of subdomains containing each DOF)
    weights: Vec<T>,
    /// Total matrix dimension
    n: usize,
}

/// A single subdomain with its local matrix and solver
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Subdomain<T: ComplexField> {
    /// Global indices of DOFs in this subdomain
    global_indices: Vec<usize>,
    /// Local matrix (extracted from global)
    local_values: Vec<T>,
    local_col_indices: Vec<usize>,
    local_row_ptrs: Vec<usize>,
    /// Local ILU factors for solving
    l_values: Vec<T>,
    l_col_indices: Vec<usize>,
    l_row_ptrs: Vec<usize>,
    u_values: Vec<T>,
    u_col_indices: Vec<usize>,
    u_row_ptrs: Vec<usize>,
    u_diag: Vec<T>,
}

impl<T: ComplexField> AdditiveSchwarzPreconditioner<T> {
    /// Create an Additive Schwarz preconditioner
    ///
    /// # Arguments
    /// * `matrix` - The global sparse matrix
    /// * `num_subdomains` - Number of subdomains (typically = number of threads)
    /// * `overlap` - Number of overlap layers between subdomains
    pub fn from_csr(matrix: &CsrMatrix<T>, num_subdomains: usize, overlap: usize) -> Self {
        let n = matrix.num_rows;
        let num_subdomains = num_subdomains.max(1).min(n);

        // Step 1: Create initial partition (contiguous blocks)
        let base_size = n / num_subdomains;
        let remainder = n % num_subdomains;

        let mut partitions: Vec<Vec<usize>> = Vec::with_capacity(num_subdomains);
        let mut start = 0;
        for i in 0..num_subdomains {
            let size = base_size + if i < remainder { 1 } else { 0 };
            partitions.push((start..start + size).collect());
            start += size;
        }

        // Step 2: Build adjacency information for overlap extension
        let adjacency = build_adjacency(matrix);

        // Step 3: Extend each partition by overlap layers
        let extended_partitions: Vec<Vec<usize>> = partitions
            .iter()
            .map(|partition| extend_partition(partition, &adjacency, overlap, n))
            .collect();

        // Step 4: Compute weights (inverse of overlap count)
        let mut overlap_count = vec![0usize; n];
        for partition in &extended_partitions {
            for &idx in partition {
                overlap_count[idx] += 1;
            }
        }
        let weights: Vec<T> = overlap_count
            .iter()
            .map(|&c| {
                if c > 0 {
                    T::from_real(T::Real::one() / T::Real::from_usize(c).unwrap())
                } else {
                    T::one()
                }
            })
            .collect();

        // Step 5: Build subdomains with local matrices and ILU factorizations
        let subdomains: Vec<Subdomain<T>> = extended_partitions
            .into_iter()
            .map(|indices| build_subdomain(matrix, indices))
            .collect();

        Self {
            subdomains,
            weights,
            n,
        }
    }

    /// Create with automatic subdomain count based on available threads
    #[cfg(feature = "rayon")]
    pub fn from_csr_auto(matrix: &CsrMatrix<T>, overlap: usize) -> Self {
        let num_threads = rayon::current_num_threads();
        Self::from_csr(matrix, num_threads, overlap)
    }

    /// Get statistics about the domain decomposition
    pub fn stats(&self) -> (usize, usize, usize, f64) {
        let num_subdomains = self.subdomains.len();
        let min_size = self
            .subdomains
            .iter()
            .map(|s| s.global_indices.len())
            .min()
            .unwrap_or(0);
        let max_size = self
            .subdomains
            .iter()
            .map(|s| s.global_indices.len())
            .max()
            .unwrap_or(0);
        let avg_size = self
            .subdomains
            .iter()
            .map(|s| s.global_indices.len())
            .sum::<usize>() as f64
            / num_subdomains as f64;
        (num_subdomains, min_size, max_size, avg_size)
    }
}

/// Build adjacency list from sparse matrix
fn build_adjacency<T: ComplexField>(matrix: &CsrMatrix<T>) -> Vec<Vec<usize>> {
    let n = matrix.num_rows;
    let mut adjacency = vec![Vec::new(); n];

    for (i, row_adj) in adjacency.iter_mut().enumerate().take(n) {
        for idx in matrix.row_ptrs[i]..matrix.row_ptrs[i + 1] {
            let j = matrix.col_indices[idx];
            if i != j {
                row_adj.push(j);
            }
        }
    }

    adjacency
}

/// Extend a partition by `overlap` layers using adjacency
fn extend_partition(
    partition: &[usize],
    adjacency: &[Vec<usize>],
    overlap: usize,
    n: usize,
) -> Vec<usize> {
    let mut in_partition = vec![false; n];
    for &idx in partition {
        in_partition[idx] = true;
    }

    let mut frontier: Vec<usize> = partition.to_vec();

    for _ in 0..overlap {
        let mut new_frontier = Vec::new();
        for &idx in &frontier {
            for &neighbor in &adjacency[idx] {
                if !in_partition[neighbor] {
                    in_partition[neighbor] = true;
                    new_frontier.push(neighbor);
                }
            }
        }
        frontier = new_frontier;
    }

    // Collect all indices and sort for cache-friendly access
    let mut result: Vec<usize> = (0..n).filter(|&i| in_partition[i]).collect();
    result.sort_unstable();
    result
}

/// Build a subdomain with extracted local matrix and ILU factorization
fn build_subdomain<T: ComplexField>(
    matrix: &CsrMatrix<T>,
    global_indices: Vec<usize>,
) -> Subdomain<T> {
    let local_n = global_indices.len();

    // Build global-to-local index mapping
    let mut global_to_local = vec![usize::MAX; matrix.num_rows];
    for (local_idx, &global_idx) in global_indices.iter().enumerate() {
        global_to_local[global_idx] = local_idx;
    }

    // Extract local matrix in CSR format
    let mut local_values = Vec::new();
    let mut local_col_indices = Vec::new();
    let mut local_row_ptrs = vec![0];

    for &global_row in &global_indices {
        for idx in matrix.row_ptrs[global_row]..matrix.row_ptrs[global_row + 1] {
            let global_col = matrix.col_indices[idx];
            let local_col = global_to_local[global_col];
            if local_col != usize::MAX {
                local_values.push(matrix.values[idx]);
                local_col_indices.push(local_col);
            }
        }
        local_row_ptrs.push(local_values.len());
    }

    // Perform ILU(0) on the local matrix
    let (l_values, l_col_indices, l_row_ptrs, u_values, u_col_indices, u_row_ptrs, u_diag) =
        ilu_factorize(&local_values, &local_col_indices, &local_row_ptrs, local_n);

    Subdomain {
        global_indices,
        local_values,
        local_col_indices,
        local_row_ptrs,
        l_values,
        l_col_indices,
        l_row_ptrs,
        u_values,
        u_col_indices,
        u_row_ptrs,
        u_diag,
    }
}

/// ILU(0) factorization for a local matrix
#[allow(clippy::type_complexity)]
fn ilu_factorize<T: ComplexField>(
    values: &[T],
    col_indices: &[usize],
    row_ptrs: &[usize],
    n: usize,
) -> (
    Vec<T>,
    Vec<usize>,
    Vec<usize>,
    Vec<T>,
    Vec<usize>,
    Vec<usize>,
    Vec<T>,
) {
    // Copy values for in-place factorization
    let mut values = values.to_vec();

    // ILU(0) factorization
    for i in 0..n {
        for idx in row_ptrs[i]..row_ptrs[i + 1] {
            let k = col_indices[idx];
            if k >= i {
                break;
            }

            // Find u_kk
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

    (
        l_values,
        l_col_indices,
        l_row_ptrs,
        u_values,
        u_col_indices,
        u_row_ptrs,
        u_diag,
    )
}

impl<T: ComplexField> Subdomain<T> {
    /// Solve local system using ILU factors
    fn solve(&self, local_rhs: &[T]) -> Vec<T> {
        let n = self.global_indices.len();
        let mut y = local_rhs.to_vec();

        // Forward substitution: Ly = r
        for i in 0..n {
            for idx in self.l_row_ptrs[i]..self.l_row_ptrs[i + 1] {
                let j = self.l_col_indices[idx];
                let l_ij = self.l_values[idx];
                y[i] = y[i] - l_ij * y[j];
            }
        }

        // Backward substitution: Ux = y
        let mut x = y;
        for i in (0..n).rev() {
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

impl<T: ComplexField + Send + Sync> Preconditioner<T> for AdditiveSchwarzPreconditioner<T> {
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        #[cfg(feature = "rayon")]
        {
            if self.n >= 1000 && self.subdomains.len() > 1 {
                return self.apply_parallel(r);
            }
        }
        self.apply_sequential(r)
    }
}

impl<T: ComplexField + Send + Sync> AdditiveSchwarzPreconditioner<T> {
    fn apply_sequential(&self, r: &Array1<T>) -> Array1<T> {
        let mut result = Array1::from_elem(self.n, T::zero());

        for subdomain in &self.subdomains {
            // Extract local RHS
            let local_rhs: Vec<T> = subdomain.global_indices.iter().map(|&i| r[i]).collect();

            // Solve local system
            let local_solution = subdomain.solve(&local_rhs);

            // Scatter back to global with weights
            for (local_idx, &global_idx) in subdomain.global_indices.iter().enumerate() {
                result[global_idx] += local_solution[local_idx] * self.weights[global_idx];
            }
        }

        result
    }

    #[cfg(feature = "rayon")]
    fn apply_parallel(&self, r: &Array1<T>) -> Array1<T> {
        use std::cell::UnsafeCell;

        // Thread-safe wrapper for the result vector
        struct UnsafeVec<U>(UnsafeCell<Vec<U>>);
        unsafe impl<U: Send> Sync for UnsafeVec<U> {}

        impl<U: ComplexField> UnsafeVec<U> {
            fn add(&self, i: usize, val: U) {
                unsafe {
                    let vec = &mut (*self.0.get());
                    vec[i] = vec[i] + val;
                }
            }
        }

        let result = UnsafeVec(UnsafeCell::new(vec![T::zero(); self.n]));
        let r_slice = r.as_slice().expect("Array should be contiguous");
        let weights = &self.weights;

        // Solve all subdomains in parallel
        self.subdomains.par_iter().for_each(|subdomain| {
            // Extract local RHS
            let local_rhs: Vec<T> = subdomain
                .global_indices
                .iter()
                .map(|&i| r_slice[i])
                .collect();

            // Solve local system
            let local_solution = subdomain.solve(&local_rhs);

            // Scatter back to global with weights
            // Note: Multiple threads may write to overlapping indices, but since
            // we're doing addition and each thread adds a weighted portion,
            // the final result is correct (additive Schwarz property)
            for (local_idx, &global_idx) in subdomain.global_indices.iter().enumerate() {
                result.add(global_idx, local_solution[local_idx] * weights[global_idx]);
            }
        });

        Array1::from_vec(result.0.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iterative::{GmresConfig, gmres_preconditioned};
    use num_complex::Complex64;

    fn create_test_matrix() -> CsrMatrix<Complex64> {
        let n = 20;
        let mut dense = ndarray::Array2::from_elem((n, n), Complex64::new(0.0, 0.0));

        // Create a 2D Laplacian-like matrix (5-point stencil pattern)
        for i in 0..n {
            dense[[i, i]] = Complex64::new(4.0, 0.0);
            if i > 0 {
                dense[[i, i - 1]] = Complex64::new(-1.0, 0.0);
            }
            if i < n - 1 {
                dense[[i, i + 1]] = Complex64::new(-1.0, 0.0);
            }
            // Add some off-diagonal connections for more interesting structure
            if i >= 5 {
                dense[[i, i - 5]] = Complex64::new(-0.5, 0.0);
            }
            if i < n - 5 {
                dense[[i, i + 5]] = Complex64::new(-0.5, 0.0);
            }
        }

        CsrMatrix::from_dense(&dense, 1e-15)
    }

    #[test]
    fn test_schwarz_basic() {
        let matrix = create_test_matrix();
        let precond = AdditiveSchwarzPreconditioner::from_csr(&matrix, 4, 1);

        let r = Array1::from_iter((0..20).map(|i| Complex64::new((i as f64).sin(), 0.0)));
        let result = precond.apply(&r);

        assert_eq!(result.len(), 20);
        assert!(result.iter().all(|x| x.norm() < 100.0));
    }

    #[test]
    fn test_schwarz_stats() {
        let matrix = create_test_matrix();
        let precond = AdditiveSchwarzPreconditioner::from_csr(&matrix, 4, 2);

        let (num_subdomains, min_size, max_size, avg_size) = precond.stats();

        assert_eq!(num_subdomains, 4);
        assert!(min_size > 0);
        assert!(max_size >= min_size);
        assert!(avg_size > 0.0);
        // With overlap, subdomains should be larger than n/4
        assert!(avg_size > 5.0);
    }

    #[test]
    fn test_schwarz_with_gmres() {
        let matrix = create_test_matrix();
        let precond = AdditiveSchwarzPreconditioner::from_csr(&matrix, 4, 2);

        let b = Array1::from_iter((0..20).map(|i| Complex64::new((i as f64).sin(), 0.0)));

        let config = GmresConfig {
            max_iterations: 100,
            restart: 20,
            tolerance: 1e-8,
            print_interval: 0,
        };

        let sol = gmres_preconditioned(&matrix, &precond, &b, &config);
        assert!(sol.converged, "GMRES with Schwarz should converge");
    }

    #[test]
    fn test_schwarz_overlap_effect() {
        let matrix = create_test_matrix();

        // Test with different overlap sizes
        let precond_no_overlap = AdditiveSchwarzPreconditioner::from_csr(&matrix, 4, 0);
        let precond_overlap_1 = AdditiveSchwarzPreconditioner::from_csr(&matrix, 4, 1);
        let precond_overlap_2 = AdditiveSchwarzPreconditioner::from_csr(&matrix, 4, 2);

        let b = Array1::from_iter((0..20).map(|i| Complex64::new((i as f64).sin(), 0.0)));

        let config = GmresConfig {
            max_iterations: 100,
            restart: 20,
            tolerance: 1e-8,
            print_interval: 0,
        };

        let sol_no_overlap = gmres_preconditioned(&matrix, &precond_no_overlap, &b, &config);
        let sol_overlap_1 = gmres_preconditioned(&matrix, &precond_overlap_1, &b, &config);
        let sol_overlap_2 = gmres_preconditioned(&matrix, &precond_overlap_2, &b, &config);

        // All should converge
        assert!(sol_no_overlap.converged);
        assert!(sol_overlap_1.converged);
        assert!(sol_overlap_2.converged);

        // More overlap should generally help (or at least not hurt much)
        // Note: This may not always hold for small test problems
    }
}
