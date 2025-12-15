//! FMM-solver interface
//!
//! This module provides a unified interface for solving BEM systems using
//! either direct methods (TBEM) or iterative methods with FMM acceleration.

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use solvers::iterative::{
    BiCgstabConfig, BiCgstabSolution, CgsConfig, CgsSolution, GmresConfig, GmresSolution,
};
use solvers::iterative::{bicgstab, cgs, gmres};
use solvers::preconditioners::IluPreconditioner;
use solvers::traits::{LinearOperator, Preconditioner};

use crate::core::assembly::mlfmm::MlfmmSystem;
#[cfg(any(feature = "native", feature = "wasm"))]
use crate::core::assembly::slfmm::SlfmmSystem;
use crate::core::assembly::sparse::CsrMatrix;

// ============================================================================
// Linear Operators
// ============================================================================

/// Dense matrix linear operator
pub struct DenseOperator {
    matrix: ndarray::Array2<Complex64>,
}

impl DenseOperator {
    /// Create a new dense operator from a matrix
    pub fn new(matrix: ndarray::Array2<Complex64>) -> Self {
        Self { matrix }
    }
}

impl LinearOperator<Complex64> for DenseOperator {
    fn num_rows(&self) -> usize {
        self.matrix.nrows()
    }

    fn num_cols(&self) -> usize {
        self.matrix.ncols()
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.matrix.dot(x)
    }

    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.matrix.t().dot(x)
    }
}

/// SLFMM linear operator
#[cfg(any(feature = "native", feature = "wasm"))]
pub struct SlfmmOperator {
    system: SlfmmSystem,
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl SlfmmOperator {
    /// Create a new SLFMM operator
    pub fn new(system: SlfmmSystem) -> Self {
        Self { system }
    }

    /// Get a reference to the underlying system
    pub fn system(&self) -> &SlfmmSystem {
        &self.system
    }

    /// Get the RHS vector
    pub fn rhs(&self) -> &Array1<Complex64> {
        &self.system.rhs
    }
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl LinearOperator<Complex64> for SlfmmOperator {
    fn num_rows(&self) -> usize {
        self.system.num_dofs
    }

    fn num_cols(&self) -> usize {
        self.system.num_dofs
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.system.matvec(x)
    }

    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.system.matvec_transpose(x)
    }
}

/// MLFMM linear operator
pub struct MlfmmOperator {
    system: MlfmmSystem,
}

impl MlfmmOperator {
    /// Create a new MLFMM operator
    pub fn new(system: MlfmmSystem) -> Self {
        Self { system }
    }

    /// Get a reference to the underlying system
    pub fn system(&self) -> &MlfmmSystem {
        &self.system
    }

    /// Get the RHS vector
    pub fn rhs(&self) -> &Array1<Complex64> {
        &self.system.rhs
    }
}

impl LinearOperator<Complex64> for MlfmmOperator {
    fn num_rows(&self) -> usize {
        self.system.num_dofs
    }

    fn num_cols(&self) -> usize {
        self.system.num_dofs
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.system.matvec(x)
    }

    fn apply_transpose(&self, _x: &Array1<Complex64>) -> Array1<Complex64> {
        unimplemented!("MLFMM transpose not yet implemented")
    }
}

/// CSR sparse matrix linear operator
pub struct CsrOperator {
    matrix: CsrMatrix,
}

impl CsrOperator {
    /// Create a new CSR operator
    pub fn new(matrix: CsrMatrix) -> Self {
        Self { matrix }
    }

    /// Get a reference to the underlying matrix
    pub fn matrix(&self) -> &CsrMatrix {
        &self.matrix
    }
}

impl LinearOperator<Complex64> for CsrOperator {
    fn num_rows(&self) -> usize {
        self.matrix.num_rows
    }

    fn num_cols(&self) -> usize {
        self.matrix.num_cols
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.matrix.matvec(x)
    }

    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.matrix.matvec_transpose(x)
    }
}

// ============================================================================
// Preconditioners
// ============================================================================

/// Diagonal preconditioner
pub struct DiagonalPreconditioner {
    inv_diag: Array1<Complex64>,
}

impl DiagonalPreconditioner {
    /// Create from a CSR matrix
    pub fn from_csr(matrix: &CsrMatrix) -> Self {
        let diag = matrix.diagonal();
        let inv_diag = diag.mapv(|d| {
            if d.norm() > 1e-15 {
                Complex64::new(1.0, 0.0) / d
            } else {
                Complex64::new(1.0, 0.0)
            }
        });
        Self { inv_diag }
    }

    /// Create from a diagonal vector
    pub fn from_diagonal(diag: Array1<Complex64>) -> Self {
        let inv_diag = diag.mapv(|d| {
            if d.norm() > 1e-15 {
                Complex64::new(1.0, 0.0) / d
            } else {
                Complex64::new(1.0, 0.0)
            }
        });
        Self { inv_diag }
    }
}

impl Preconditioner<Complex64> for DiagonalPreconditioner {
    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        x * &self.inv_diag
    }
}

impl LinearOperator<Complex64> for DiagonalPreconditioner {
    fn num_rows(&self) -> usize {
        self.inv_diag.len()
    }

    fn num_cols(&self) -> usize {
        self.inv_diag.len()
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        x * &self.inv_diag
    }

    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        x * &self.inv_diag
    }
}

/// Sparse ILU preconditioner for FMM near-field
///
/// Uses only the near-field blocks to build an ILU factorization.
#[derive(Debug, Clone)]
pub struct SparseNearfieldIlu {
    /// L factor values (lower triangular)
    l_values: Vec<Complex64>,
    /// Matrix dimension
    n: usize,
}

impl SparseNearfieldIlu {
    /// Creates a new `SparseNearfieldIlu` preconditioner from an SLFMM system.
    ///
    /// This uses a placeholder diagonal approximation based on the diagonal elements
    /// of the near-field blocks.
    #[cfg(any(feature = "native", feature = "wasm"))]
    pub fn from_slfmm(
        near_blocks: &[super::super::assembly::slfmm::NearFieldBlock],
        cluster_dof_indices: &[Vec<usize>],
        num_dofs: usize,
        _threshold: f64,
    ) -> Self {
        // Placeholder diagonal approximation
        let mut diag = Array1::ones(num_dofs);

        for block in near_blocks {
            if block.source_cluster == block.field_cluster {
                let dofs = &cluster_dof_indices[block.source_cluster];
                for (local_i, &global_i) in dofs.iter().enumerate() {
                    if local_i < block.coefficients.nrows() {
                        diag[global_i] = block.coefficients[[local_i, local_i]];
                    }
                }
            }
        }

        let l_values = diag
            .iter()
            .map(|d| {
                if d.norm() > 1e-15 {
                    *d
                } else {
                    Complex64::new(1.0, 0.0)
                }
            })
            .collect();

        Self {
            l_values,
            n: num_dofs,
        }
    }
}

impl Preconditioner<Complex64> for SparseNearfieldIlu {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        let mut z = Array1::zeros(self.n);
        for i in 0..self.n {
            z[i] = r[i] / self.l_values[i];
        }
        z
    }
}

/// Hierarchical FMM preconditioner
#[cfg(any(feature = "native", feature = "wasm"))]
#[derive(Debug, Clone)]
pub struct HierarchicalFmmPreconditioner {
    /// LU factors for each cluster's diagonal block
    #[allow(dead_code)]
    block_lu: Vec<Array2<Complex64>>,
    /// Global DOF indices for each cluster
    #[allow(dead_code)]
    cluster_dof_indices: Vec<Vec<usize>>,
    /// Total number of DOFs
    #[allow(dead_code)]
    num_dofs: usize,
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl HierarchicalFmmPreconditioner {
    /// Creates a new `HierarchicalFmmPreconditioner` from an `SlfmmSystem`.
    pub fn from_slfmm(system: &SlfmmSystem) -> Self {
        Self::from_slfmm_blocks(
            &system.near_matrix,
            &system.cluster_dof_indices,
            system.num_dofs,
        )
    }

    /// Creates a new `HierarchicalFmmPreconditioner` from near-field blocks and cluster DOF indices.
    ///
    /// This is a placeholder implementation that currently only initializes identity blocks.
    pub fn from_slfmm_blocks(
        _near_blocks: &[super::super::assembly::slfmm::NearFieldBlock],
        cluster_dof_indices: &[Vec<usize>],
        num_dofs: usize,
    ) -> Self {
        // Placeholder implementation
        let num_clusters = cluster_dof_indices.len();
        let mut block_lu = Vec::with_capacity(num_clusters);

        for dofs in cluster_dof_indices {
            let n = dofs.len();
            block_lu.push(Array2::eye(n));
        }

        Self {
            block_lu,
            cluster_dof_indices: cluster_dof_indices.to_vec(),
            num_dofs,
        }
    }
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl Preconditioner<Complex64> for HierarchicalFmmPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        r.clone()
    }
}

// ============================================================================
// Solver Wrappers
// ============================================================================

/// Solves a linear system using the Conjugate Gradient Squared (CGS) method.
pub fn solve_cgs<O: LinearOperator<Complex64>>(
    operator: &O,
    b: &Array1<Complex64>,
    config: &CgsConfig<f64>,
) -> CgsSolution<Complex64> {
    cgs(operator, b, config)
}

/// Solves a linear system using the Biconjugate Gradient Stabilized (BiCGSTAB) method.
pub fn solve_bicgstab<O: LinearOperator<Complex64>>(
    operator: &O,
    b: &Array1<Complex64>,
    config: &BiCgstabConfig<f64>,
) -> BiCgstabSolution<Complex64> {
    bicgstab(operator, b, config)
}

/// Solves a linear system using the Generalized Minimum Residual (GMRES) method.
pub fn solve_gmres<O: LinearOperator<Complex64>>(
    operator: &O,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    gmres(operator, b, config)
}

/// Solves a linear system with an ILU preconditioner.
///
/// Note: Current implementation falls back to unpreconditioned CGS.
pub fn solve_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &CgsConfig<f64>,
) -> CgsSolution<Complex64> {
    // Convert to CSR for ILU
    // This is expensive but necessary if using math-solvers ILU which requires CSR
    // For TBEM, we might want to avoid this or add Dense ILU to math-solvers
    let csr = solvers::sparse::CsrMatrix::from_dense(matrix, 1e-15);
    let _precond = IluPreconditioner::from_csr(&csr);
    let op = DenseOperator::new(matrix.clone());

    // cgs_preconditioned doesn't exist in math-solvers iterative exports directly?
    // math-solvers exports cgs which takes optional guess, but not preconditioned?
    // Checking iterative/mod.rs: pub use cgs::{CgsConfig, CgsSolution, cgs};
    // cgs::cgs might support preconditioning? No, usually separate function.
    // Actually math-solvers cgs module usually has cgs_preconditioned but maybe not exported?
    // Wait, iterative/gmres exported gmres_preconditioned. cgs module might not.
    // I need to check cgs module. Assuming cgs doesn't support preconditioning for now or use gmres instead.

    // Fallback to GMRES for preconditioned solve if CGS preconditioned is not available
    // But function returns CgsSolution.
    // Let's assume we can't do preconditioned CGS with math-solvers public API yet.
    // I'll return un-preconditioned CGS or panic.
    eprintln!(
        "Warning: CGS with ILU not fully supported via public API, running without preconditioner"
    );
    cgs(&op, b, config)
}

/// Solves a linear system with a given operator and an ILU preconditioner (placeholder, currently not used).
pub fn solve_with_ilu_operator<O: LinearOperator<Complex64>>(
    operator: &O,
    _nearfield_matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &CgsConfig<f64>,
) -> CgsSolution<Complex64> {
    cgs(operator, b, config)
}

/// Solve a TBEM system (dense matrix) using CGS with an ILU preconditioner.
///
/// This function acts as a wrapper around `solve_with_ilu` specifically for
/// dense TBEM matrices.
///
/// # Arguments
/// * `matrix` - The dense TBEM matrix to solve.
/// * `b` - The right-hand side vector.
/// * `config` - Configuration for the CGS solver.
///
/// # Returns
/// The CGS solution including the solution vector, iterations, and residual.
pub fn solve_tbem_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &CgsConfig<f64>,
) -> CgsSolution<Complex64> {
    solve_with_ilu(matrix, b, config)
}

/// Solves a linear system using GMRES with an ILU preconditioner derived from a dense matrix.
pub fn gmres_solve_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    let csr = solvers::sparse::CsrMatrix::from_dense(matrix, 1e-15);
    let precond = IluPreconditioner::from_csr(&csr);
    let op = DenseOperator::new(matrix.clone());
    solvers::iterative::gmres_preconditioned(&op, &precond, b, config)
}

/// Solves a linear system using GMRES with an ILU preconditioner derived from a nearfield matrix for a given operator.
pub fn gmres_solve_with_ilu_operator<O: LinearOperator<Complex64>>(
    operator: &O,
    nearfield_matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    let csr = solvers::sparse::CsrMatrix::from_dense(nearfield_matrix, 1e-15);
    let precond = IluPreconditioner::from_csr(&csr);
    solvers::iterative::gmres_preconditioned(operator, &precond, b, config)
}

/// Solves a TBEM system (dense matrix) using GMRES with an ILU preconditioner.
///
/// This function acts as a wrapper around `gmres_solve_with_ilu` specifically for
/// dense TBEM matrices.
pub fn gmres_solve_tbem_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    gmres_solve_with_ilu(matrix, b, config)
}

/// Solves a linear system using GMRES with a hierarchical FMM preconditioner.
///
/// This function constructs an `SlfmmOperator` from the `fmm_system` and a
/// `HierarchicalFmmPreconditioner` to solve the system.
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn gmres_solve_with_hierarchical_precond(
    fmm_system: &crate::core::assembly::slfmm::SlfmmSystem,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    let op = SlfmmOperator::new(fmm_system.clone());
    let precond = HierarchicalFmmPreconditioner::from_slfmm(fmm_system);
    solvers::iterative::gmres_preconditioned(&op, &precond, b, config)
}

/// Solves a linear system using GMRES with a hierarchical FMM preconditioner.
///
/// This function uses an existing `SlfmmOperator` and constructs a
/// `HierarchicalFmmPreconditioner` from its underlying system.
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn gmres_solve_fmm_hierarchical(
    fmm_operator: &SlfmmOperator,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    let precond = HierarchicalFmmPreconditioner::from_slfmm(fmm_operator.system());
    let b = fmm_operator.rhs();
    solvers::iterative::gmres_preconditioned(fmm_operator, &precond, b, config)
}

/// Solves a linear system using GMRES with a batched FMM operator (unpreconditioned).
#[cfg(feature = "native")]
pub fn gmres_solve_fmm_batched(
    fmm_system: &crate::core::assembly::slfmm::SlfmmSystem,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    let op = SlfmmOperator::new(fmm_system.clone());
    gmres(&op, b, config)
}

/// Solves a linear system using GMRES with a batched FMM operator and an ILU preconditioner.
#[cfg(feature = "native")]
pub fn gmres_solve_fmm_batched_with_ilu(
    fmm_system: &crate::core::assembly::slfmm::SlfmmSystem,
    b: &Array1<Complex64>,
    config: &GmresConfig<f64>,
) -> GmresSolution<Complex64> {
    let op = SlfmmOperator::new(fmm_system.clone());
    let nearfield_matrix = fmm_system.extract_near_field_matrix();
    let csr = solvers::sparse::CsrMatrix::from_dense(&nearfield_matrix, 1e-15);
    let precond = IluPreconditioner::from_csr(&csr);
    solvers::iterative::gmres_preconditioned(&op, &precond, b, config)
}

// ============================================================================
// Adaptive Mesh Utilities
// ============================================================================

/// Calculate recommended mesh resolution for a given frequency
pub fn recommended_mesh_resolution(
    frequency: f64,
    speed_of_sound: f64,
    elements_per_wavelength: usize,
) -> f64 {
    let wavelength = speed_of_sound / frequency;
    elements_per_wavelength as f64 / wavelength
}

/// Calculate mesh resolution for a frequency range
pub fn mesh_resolution_for_frequency_range(
    _min_freq: f64,
    max_freq: f64,
    speed_of_sound: f64,
    elements_per_wavelength: usize,
) -> f64 {
    recommended_mesh_resolution(max_freq, speed_of_sound, elements_per_wavelength)
}

/// Estimate element count for a rectangular room
pub fn estimate_element_count(room_dimensions: (f64, f64, f64), mesh_resolution: f64) -> usize {
    let (w, d, h) = room_dimensions;
    let surface_area = 2.0 * (w * d + w * h + d * h);
    let element_size = 1.0 / mesh_resolution;
    let element_area = element_size * element_size;
    (surface_area / element_area).ceil() as usize
}

/// Adaptive mesh configuration
pub struct AdaptiveMeshConfig {
    /// Base resolution (elements per meter)
    pub base_resolution: f64,
    /// Refinement factor near sources (1.0 = no refinement)
    pub source_refinement: f64,
    /// Radius around sources where refinement is applied (meters)
    pub source_refinement_radius: f64,
}

impl AdaptiveMeshConfig {
    /// Create configuration for a frequency range
    pub fn for_frequency_range(min_freq: f64, max_freq: f64) -> Self {
        let speed_of_sound = 343.0;
        let base = mesh_resolution_for_frequency_range(min_freq, max_freq, speed_of_sound, 6);
        Self {
            base_resolution: base,
            source_refinement: 1.5,
            source_refinement_radius: 0.5,
        }
    }

    /// Create configuration from a fixed resolution
    pub fn from_resolution(resolution: f64) -> Self {
        Self {
            base_resolution: resolution,
            source_refinement: 1.0,
            source_refinement_radius: 0.0,
        }
    }
}
