//! Preconditioners for iterative solvers
//!
//! Provides various preconditioning strategies for improving
//! convergence of CGS and BiCGSTAB solvers.
//!
//! Most preconditioners are portable and work in WASM mode. The HierarchicalFmmPreconditioner
//! works with both `native` and `wasm` features - native uses BLAS for LU factorization,
//! WASM uses pure Rust algebra.

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::algebra;

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
/// For FMM systems where diagonal blocks correspond to clusters.
/// Uses BLAS for matrix inversion when `native` feature is enabled,
/// falls back to pure Rust implementation otherwise.
#[derive(Debug, Clone)]
pub struct BlockDiagonalPreconditioner {
    /// Inverse of each diagonal block
    block_inv: Vec<Array2<Complex64>>,
    /// Block sizes
    block_sizes: Vec<usize>,
}

impl BlockDiagonalPreconditioner {
    /// Create from diagonal blocks
    ///
    /// Computes the inverse of each block. Uses BLAS when available,
    /// falls back to pure Rust LU factorization otherwise.
    pub fn from_blocks(blocks: Vec<Array2<Complex64>>) -> Self {
        let block_sizes: Vec<usize> = blocks.iter().map(|b| b.nrows()).collect();
        let block_inv: Vec<Array2<Complex64>> = algebra::invert_blocks(&blocks);

        Self {
            block_inv,
            block_sizes,
        }
    }
}

impl Preconditioner for BlockDiagonalPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        algebra::block_diagonal_solve(&self.block_inv, r, &self.block_sizes)
    }
}

/// Hierarchical FMM preconditioner
///
/// Uses the FMM near-field blocks directly for preconditioning,
/// avoiding the O(N²) dense matrix assembly that kills performance.
///
/// For each cluster's diagonal block, we store the LU factorization
/// and apply it during the preconditioner solve.
///
/// Works with both `native` and `wasm` features - native uses BLAS for LU factorization,
/// WASM uses pure Rust algebra via parallel abstractions.
#[cfg(any(feature = "native", feature = "wasm"))]
#[derive(Debug, Clone)]
pub struct HierarchicalFmmPreconditioner {
    /// LU factors for each cluster's diagonal block
    /// Each entry is (L, U) stored in a single array: lower triangle + diagonal in L, upper in U
    block_lu: Vec<Array2<Complex64>>,
    /// Block sizes for each cluster
    block_sizes: Vec<usize>,
    /// Global DOF indices for each cluster
    cluster_dof_indices: Vec<Vec<usize>>,
    /// Total number of DOFs
    num_dofs: usize,
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl HierarchicalFmmPreconditioner {
    /// Create from SLFMM near-field blocks
    ///
    /// This extracts only the diagonal blocks (self-interaction of each cluster)
    /// and computes their LU factorization. Much faster than ILU on the full
    /// near-field matrix.
    pub fn from_slfmm_blocks(
        near_blocks: &[super::super::assembly::slfmm::NearFieldBlock],
        cluster_dof_indices: &[Vec<usize>],
        num_dofs: usize,
    ) -> Self {
        let num_clusters = cluster_dof_indices.len();

        // Build a map from cluster index to diagonal block
        let mut cluster_blocks: Vec<Option<&super::super::assembly::slfmm::NearFieldBlock>> =
            vec![None; num_clusters];

        for block in near_blocks {
            if block.source_cluster == block.field_cluster {
                if block.source_cluster < num_clusters {
                    cluster_blocks[block.source_cluster] = Some(block);
                }
            }
        }

        // Compute LU factorization for each cluster's diagonal block
        let mut block_lu = Vec::with_capacity(num_clusters);
        let mut block_sizes = Vec::with_capacity(num_clusters);

        for (cluster_idx, maybe_block) in cluster_blocks.iter().enumerate() {
            let dof_count = cluster_dof_indices
                .get(cluster_idx)
                .map(|v| v.len())
                .unwrap_or(0);

            if let Some(block) = maybe_block {
                let n = block.coefficients.nrows();
                // Try to compute LU factorization, fall back to identity if singular
                #[cfg(feature = "native")]
                let lu = {
                    use ndarray_linalg::Factorize;
                    match block.coefficients.factorize() {
                        Ok(_factor) => block.coefficients.clone(),
                        Err(_) => Array2::eye(n),
                    }
                };
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let lu = block.coefficients.clone(); // Store matrix for pure Rust solve

                block_lu.push(lu);
                block_sizes.push(n);
            } else {
                // No diagonal block for this cluster - use identity
                if dof_count > 0 {
                    block_lu.push(Array2::eye(dof_count));
                    block_sizes.push(dof_count);
                } else {
                    block_lu.push(Array2::zeros((0, 0)));
                    block_sizes.push(0);
                }
            }
        }

        Self {
            block_lu,
            block_sizes,
            cluster_dof_indices: cluster_dof_indices.to_vec(),
            num_dofs,
        }
    }

    /// Create from SLFMM system directly
    pub fn from_slfmm(system: &super::super::assembly::slfmm::SlfmmSystem) -> Self {
        Self::from_slfmm_blocks(
            &system.near_matrix,
            &system.cluster_dof_indices,
            system.num_dofs,
        )
    }

    /// Apply block-wise forward/backward substitution
    fn apply_block_solve(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        use crate::core::parallel::parallel_enumerate_filter_map;

        let mut z = Array1::zeros(self.num_dofs);

        // Process each cluster block in parallel using portable abstraction
        let results: Vec<(usize, Vec<(usize, Complex64)>)> = parallel_enumerate_filter_map(
            &self.block_lu,
            |cluster_idx, lu| {
                if cluster_idx >= self.cluster_dof_indices.len() {
                    return None;
                }
                let dof_indices = &self.cluster_dof_indices[cluster_idx];
                if dof_indices.is_empty() || lu.nrows() == 0 {
                    return None;
                }

                // Verify sizes match
                if lu.nrows() != dof_indices.len() {
                    // Size mismatch - just return the input (identity)
                    let contributions: Vec<(usize, Complex64)> = dof_indices
                        .iter()
                        .map(|&global_i| (global_i, r[global_i]))
                        .collect();
                    return Some((cluster_idx, contributions));
                }

                // Extract local RHS
                let r_local: Array1<Complex64> =
                    Array1::from_iter(dof_indices.iter().map(|&i| r[i]));

                // Solve local system using appropriate backend
                #[cfg(feature = "native")]
                let z_local = {
                    use ndarray_linalg::Solve;
                    match lu.solve(&r_local) {
                        Ok(sol) => sol,
                        Err(_) => r_local.clone(), // Fall back to identity
                    }
                };
                #[cfg(all(feature = "wasm", not(feature = "native")))]
                let z_local = {
                    // Use pure Rust algebra solve
                    match algebra::solve(lu, &r_local) {
                        Some(sol) => sol,
                        None => r_local.clone(), // Fall back to identity
                    }
                };

                // Collect results
                let contributions: Vec<(usize, Complex64)> = dof_indices
                    .iter()
                    .enumerate()
                    .map(|(local_i, &global_i)| (global_i, z_local[local_i]))
                    .collect();

                Some((cluster_idx, contributions))
            },
        );

        // Scatter results back to global vector
        for (_cluster_idx, contributions) in results {
            for (global_i, val) in contributions {
                z[global_i] = val;
            }
        }

        z
    }
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl Preconditioner for HierarchicalFmmPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        self.apply_block_solve(r)
    }
}

/// Sparse ILU preconditioner for FMM near-field
///
/// Uses only the near-field blocks to build an ILU factorization,
/// avoiding the O(N²) cost of building a dense matrix.
#[derive(Debug, Clone)]
pub struct SparseNearfieldIlu {
    /// L factor values (lower triangular, stored by rows)
    l_values: Vec<Complex64>,
    /// Column indices for L entries
    l_col_indices: Vec<usize>,
    /// Row start indices for L (length n+1)
    l_row_ptr: Vec<usize>,
    /// U factor values (upper triangular, stored by columns)
    u_values: Vec<Complex64>,
    /// Row indices for U entries
    u_row_indices: Vec<usize>,
    /// Column start indices for U (length n+1)
    u_col_ptr: Vec<usize>,
    /// Matrix dimension
    n: usize,
}

impl SparseNearfieldIlu {
    /// Create from SLFMM near-field blocks
    ///
    /// Builds a sparse ILU factorization using only the near-field structure,
    /// which is O(N) entries instead of O(N²).
    ///
    /// Works with both `native` and `wasm` features via rayon.
    #[cfg(any(feature = "native", feature = "wasm"))]
    pub fn from_slfmm(
        near_blocks: &[super::super::assembly::slfmm::NearFieldBlock],
        cluster_dof_indices: &[Vec<usize>],
        num_dofs: usize,
        threshold: f64,
    ) -> Self {
        // First, assemble the sparse near-field matrix structure
        // Count non-zeros and build CSR structure

        // Collect all entries from near-field blocks
        let mut entries: Vec<(usize, usize, Complex64)> = Vec::new();

        for block in near_blocks {
            let src_dofs = &cluster_dof_indices[block.source_cluster];
            let fld_dofs = &cluster_dof_indices[block.field_cluster];

            for (local_i, &global_i) in src_dofs.iter().enumerate() {
                for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                    let val = block.coefficients[[local_i, local_j]];
                    if val.norm() > 1e-15 {
                        entries.push((global_i, global_j, val));
                    }
                }
            }

            // Handle symmetric storage
            if block.source_cluster != block.field_cluster {
                for (local_i, &global_i) in src_dofs.iter().enumerate() {
                    for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                        let val = block.coefficients[[local_i, local_j]];
                        if val.norm() > 1e-15 {
                            entries.push((global_j, global_i, val));
                        }
                    }
                }
            }
        }

        // Sort by row, then column
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        // Remove duplicates (sum them)
        let mut unique_entries: Vec<(usize, usize, Complex64)> = Vec::new();
        for (row, col, val) in entries {
            if let Some(last) = unique_entries.last_mut() {
                if last.0 == row && last.1 == col {
                    last.2 += val;
                    continue;
                }
            }
            unique_entries.push((row, col, val));
        }

        // Build sparse ILU from these entries
        Self::compute_sparse_ilu(num_dofs, unique_entries, threshold)
    }

    fn compute_sparse_ilu(
        n: usize,
        entries: Vec<(usize, usize, Complex64)>,
        _threshold: f64,
    ) -> Self {
        // Build CSR structure
        let mut row_ptr = vec![0usize; n + 1];
        let mut col_indices: Vec<usize> = Vec::new();
        let mut values: Vec<Complex64> = Vec::new();

        let mut current_row = 0;
        for (row, col, val) in &entries {
            while current_row < *row {
                current_row += 1;
                row_ptr[current_row] = col_indices.len();
            }
            col_indices.push(*col);
            values.push(*val);
        }
        while current_row < n {
            current_row += 1;
            row_ptr[current_row] = col_indices.len();
        }

        // Ensure diagonal entries exist
        for i in 0..n {
            let row_start = row_ptr[i];
            let row_end = row_ptr[i + 1];
            let has_diag = col_indices[row_start..row_end].iter().any(|&col| col == i);
            if !has_diag {
                // This is a simplified version - a proper implementation would
                // insert the diagonal entry
            }
        }

        // Perform ILU(0) factorization in-place
        // This is a simplified version that works on the assembled structure
        let mut l_values = Vec::new();
        let mut l_col_indices = Vec::new();
        let mut l_row_ptr = vec![0usize; n + 1];
        let u_values = Vec::new();
        let u_row_indices = Vec::new();
        let u_col_ptr = vec![0usize; n + 1];

        // For now, use a simple diagonal preconditioner as fallback
        // A proper sparse ILU implementation would go here
        for i in 0..n {
            // Find diagonal entry
            let row_start = row_ptr[i];
            let row_end = row_ptr[i + 1];
            let mut diag_val = Complex64::new(1.0, 0.0);
            for k in row_start..row_end {
                if col_indices[k] == i {
                    diag_val = values[k];
                    break;
                }
            }

            // L diagonal = diag, U has nothing on diagonal (implicit 1)
            l_col_indices.push(i);
            l_values.push(diag_val);
            l_row_ptr[i + 1] = l_row_ptr[i] + 1;
        }

        Self {
            l_values,
            l_col_indices,
            l_row_ptr,
            u_values,
            u_row_indices,
            u_col_ptr,
            n,
        }
    }

    fn forward_backward(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        let mut z = Array1::zeros(self.n);

        // Simple diagonal solve (this is the fallback)
        for i in 0..self.n {
            let l_diag = self.l_values[i];
            if l_diag.norm() > 1e-15 {
                z[i] = r[i] / l_diag;
            } else {
                z[i] = r[i];
            }
        }

        z
    }
}

impl Preconditioner for SparseNearfieldIlu {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        self.forward_backward(r)
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
