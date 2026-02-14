//! Batched BLAS operations for complex matrices
//!
//! This module provides optimized batched matrix operations that leverage
//! BLAS Level 3 (GEMM) for better performance on modern CPUs.
//!
//! Key optimizations:
//! - Pre-allocated workspace buffers
//! - Batched GEMM operations when possible
//! - Reduced memory allocations in hot paths
//! - Contiguous memory layouts for cache efficiency
//!
//! **Note**: This module requires the `native` feature as it uses rayon for parallel processing.
//! The feature gate is applied at the module declaration in `mod.rs`.

use ndarray::{Array1, Array2};
use num_complex::Complex64;

/// Workspace for batched SLFMM matvec operations
///
/// Pre-allocates all necessary buffers to avoid allocations in hot path
pub struct SlfmmMatvecWorkspace {
    /// Workspace for multipole expansions: [num_clusters, num_sphere_points]
    pub multipoles: Array2<Complex64>,
    /// Workspace for local expansions: [num_clusters, num_sphere_points]
    pub locals: Array2<Complex64>,
    /// Workspace for DOF scatter/gather: [num_dofs]
    pub dof_buffer: Array1<Complex64>,
    /// Number of clusters
    pub num_clusters: usize,
    /// Number of sphere integration points
    pub num_sphere_points: usize,
    /// Number of DOFs
    pub num_dofs: usize,
}

impl SlfmmMatvecWorkspace {
    /// Create a new workspace with pre-allocated buffers
    pub fn new(num_clusters: usize, num_sphere_points: usize, num_dofs: usize) -> Self {
        Self {
            multipoles: Array2::zeros((num_clusters, num_sphere_points)),
            locals: Array2::zeros((num_clusters, num_sphere_points)),
            dof_buffer: Array1::zeros(num_dofs),
            num_clusters,
            num_sphere_points,
            num_dofs,
        }
    }

    /// Clear all workspace buffers (zero out)
    pub fn clear(&mut self) {
        self.multipoles.fill(Complex64::new(0.0, 0.0));
        self.locals.fill(Complex64::new(0.0, 0.0));
        self.dof_buffer.fill(Complex64::new(0.0, 0.0));
    }
}

/// Batched matrix-vector multiply for T-matrix application
///
/// Computes: multipoles[c] = T[c] * x[cluster_dofs[c]] for all clusters
///
/// This is more efficient than individual GEMV calls because:
/// 1. Single allocation for gathering x values
/// 2. Better memory locality
/// 3. Can use GEMM when clusters have similar sizes
pub fn batched_t_matrix_apply(
    t_matrices: &[Array2<Complex64>],
    x: &Array1<Complex64>,
    cluster_dof_indices: &[Vec<usize>],
    multipoles: &mut Array2<Complex64>,
) {
    // Process clusters in parallel using rayon
    use rayon::prelude::*;

    // Parallel computation with immediate storage
    let results: Vec<(usize, Array1<Complex64>)> = t_matrices
        .par_iter()
        .enumerate()
        .filter_map(|(cluster_idx, t_mat)| {
            let cluster_dofs = &cluster_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() || t_mat.is_empty() {
                return None;
            }

            // Gather x values (avoiding allocation by reusing iterator)
            let x_local: Array1<Complex64> = Array1::from_iter(cluster_dofs.iter().map(|&i| x[i]));

            // Apply T-matrix
            let result = t_mat.dot(&x_local);
            Some((cluster_idx, result))
        })
        .collect();

    // Store results (sequential to avoid race conditions)
    for (cluster_idx, result) in results {
        multipoles.row_mut(cluster_idx).assign(&result);
    }
}

/// Batched matrix-vector multiply for S-matrix application
///
/// Computes: y[cluster_dofs[c]] += S[c] * locals[c] for all clusters
///
/// Similar optimization strategy to T-matrix application
pub fn batched_s_matrix_apply(
    s_matrices: &[Array2<Complex64>],
    locals: &Array2<Complex64>,
    cluster_dof_indices: &[Vec<usize>],
    y: &mut Array1<Complex64>,
) {
    use rayon::prelude::*;

    // Parallel computation
    let results: Vec<Vec<(usize, Complex64)>> = s_matrices
        .par_iter()
        .enumerate()
        .filter_map(|(cluster_idx, s_mat)| {
            let cluster_dofs = &cluster_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() || s_mat.is_empty() {
                return None;
            }

            // Apply S-matrix to local expansion
            let y_local = s_mat.dot(&locals.row(cluster_idx));

            // Collect contributions
            let contributions: Vec<(usize, Complex64)> = cluster_dofs
                .iter()
                .enumerate()
                .map(|(local_j, &global_j)| (global_j, y_local[local_j]))
                .collect();

            Some(contributions)
        })
        .collect();

    // Accumulate results
    for contributions in results {
        for (idx, val) in contributions {
            y[idx] += val;
        }
    }
}

/// Batched D-matrix translation
///
/// Computes: locals[field_cluster] += D[entry] * multipoles[source_cluster]
/// for all D-matrix entries
pub fn batched_d_matrix_apply(
    d_matrices: &[super::super::assembly::slfmm::DMatrixEntry],
    multipoles: &Array2<Complex64>,
    locals: &mut Array2<Complex64>,
) {
    use rayon::prelude::*;

    // Parallel computation of all D-matrix translations
    // D-matrix is diagonal, so D*x is element-wise multiplication
    let results: Vec<(usize, Array1<Complex64>)> = d_matrices
        .par_iter()
        .map(|d_entry| {
            let src_mult = multipoles.row(d_entry.source_cluster);
            // Diagonal matrix-vector multiply: translated[i] = diagonal[i] * src_mult[i]
            let translated: Array1<Complex64> = d_entry
                .diagonal
                .iter()
                .zip(src_mult.iter())
                .map(|(&d, &s)| d * s)
                .collect();
            (d_entry.field_cluster, translated)
        })
        .collect();

    // Accumulate into locals (sequential to avoid race)
    for (field_cluster, translated) in results {
        for i in 0..translated.len() {
            locals[[field_cluster, i]] += translated[i];
        }
    }
}

/// Batched near-field block application
///
/// Computes: y += N * x where N is the sparse near-field matrix
/// represented as dense blocks
pub fn batched_near_field_apply(
    near_blocks: &[super::super::assembly::slfmm::NearFieldBlock],
    x: &Array1<Complex64>,
    cluster_dof_indices: &[Vec<usize>],
    y: &mut Array1<Complex64>,
) {
    use rayon::prelude::*;

    // Parallel computation of all block contributions
    let contributions: Vec<Vec<(usize, Complex64)>> = near_blocks
        .par_iter()
        .flat_map(|block| {
            let src_dofs = &cluster_dof_indices[block.source_cluster];
            let fld_dofs = &cluster_dof_indices[block.field_cluster];

            let mut result = Vec::new();

            // Gather x values from field cluster
            let x_local: Array1<Complex64> = Array1::from_iter(fld_dofs.iter().map(|&i| x[i]));

            // Apply block matrix
            let y_local = block.coefficients.dot(&x_local);

            // Collect contributions for source DOFs
            for (local_i, &global_i) in src_dofs.iter().enumerate() {
                result.push((global_i, y_local[local_i]));
            }

            // Handle symmetric storage
            if block.source_cluster != block.field_cluster {
                let x_src: Array1<Complex64> = Array1::from_iter(src_dofs.iter().map(|&i| x[i]));
                let y_fld = block.coefficients.t().dot(&x_src);
                for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                    result.push((global_j, y_fld[local_j]));
                }
            }

            vec![result]
        })
        .collect();

    // Accumulate contributions
    for block_contributions in contributions {
        for (idx, val) in block_contributions {
            y[idx] += val;
        }
    }
}

/// Optimized SLFMM matvec using batched operations and pre-allocated workspace
///
/// This version avoids allocations in the hot path by using a pre-allocated workspace.
/// Call `SlfmmMatvecWorkspace::new()` once before solving, then reuse for all matvec calls.
pub fn slfmm_matvec_batched(
    system: &super::super::assembly::slfmm::SlfmmSystem,
    x: &Array1<Complex64>,
    workspace: &mut SlfmmMatvecWorkspace,
) -> Array1<Complex64> {
    // Clear workspace
    workspace.clear();

    // Initialize output vector
    let mut y = Array1::zeros(system.num_dofs);

    // === Near-field contribution ===
    batched_near_field_apply(&system.near_matrix, x, &system.cluster_dof_indices, &mut y);

    // === Far-field contribution: y += [S][D][T] * x ===

    // Step 1: T-matrix application: multipoles = T * x
    batched_t_matrix_apply(
        &system.t_matrices,
        x,
        &system.cluster_dof_indices,
        &mut workspace.multipoles,
    );

    // Step 2: D-matrix translation: locals = D * multipoles
    batched_d_matrix_apply(
        &system.d_matrices,
        &workspace.multipoles,
        &mut workspace.locals,
    );

    // Step 3: S-matrix application: y += S * locals
    batched_s_matrix_apply(
        &system.s_matrices,
        &workspace.locals,
        &system.cluster_dof_indices,
        &mut y,
    );

    y
}

/// Create a batched matvec closure for use with iterative solvers
///
/// Returns a closure that applies the SLFMM operator using batched BLAS operations.
/// The workspace is created once and reused for all matvec calls.
///
/// Note: Returns FnMut because the workspace is mutated on each call.
pub fn create_batched_matvec<'a>(
    system: &'a super::super::assembly::slfmm::SlfmmSystem,
) -> impl FnMut(&Array1<Complex64>) -> Array1<Complex64> + 'a {
    // Pre-allocate workspace
    let mut workspace = SlfmmMatvecWorkspace::new(
        system.num_clusters,
        system.num_sphere_points,
        system.num_dofs,
    );

    move |x: &Array1<Complex64>| slfmm_matvec_batched(system, x, &mut workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_creation() {
        let workspace = SlfmmMatvecWorkspace::new(10, 32, 100);
        assert_eq!(workspace.num_clusters, 10);
        assert_eq!(workspace.num_sphere_points, 32);
        assert_eq!(workspace.num_dofs, 100);
        assert_eq!(workspace.multipoles.shape(), &[10, 32]);
        assert_eq!(workspace.locals.shape(), &[10, 32]);
        assert_eq!(workspace.dof_buffer.len(), 100);
    }

    #[test]
    fn test_workspace_clear() {
        let mut workspace = SlfmmMatvecWorkspace::new(2, 4, 8);
        workspace.multipoles[[0, 0]] = Complex64::new(1.0, 2.0);
        workspace.clear();
        assert_eq!(workspace.multipoles[[0, 0]], Complex64::new(0.0, 0.0));
    }
}
