//! Algebraic Multigrid (AMG) Preconditioner for BEM
//!
//! This module provides a BEM-specific wrapper around the generic AMG preconditioner
//! from the math-solvers crate. The wrapper handles conversion between BEM's Complex64-specific
//! CsrMatrix type and the generic solver infrastructure.
//!
//! ## Features
//!
//! - **Parallel coarsening**: Classical Ruge-Stüben (RS) and PMIS algorithms
//! - **Interpolation**: Standard, Extended, and Direct interpolation operators
//! - **Smoothers**: Jacobi (fully parallel), L1-Jacobi, Symmetric Gauss-Seidel
//! - **V-cycle**: Standard V(ν₁, ν₂) cycling with configurable pre/post smoothing
//!
//! ## Scalability
//!
//! The AMG preconditioner scales better than ILU across multiple cores because:
//! - Coarsening can be parallelized (PMIS is inherently parallel)
//! - Jacobi smoothing is embarrassingly parallel
//! - Each level's operations can be parallelized independently
//!
//! ## Usage
//!
//! ```ignore
//! use bem::core::solver::{AmgPreconditioner, AmgConfig};
//! use bem::core::assembly::sparse::CsrMatrix;
//!
//! let config = AmgConfig::default();
//! let precond = AmgPreconditioner::from_csr(&matrix, config);
//!
//! // Use with GMRES
//! let z = precond.apply(&residual);
//! ```
//!
//! ## References
//!
//! - Ruge, J.W. and Stüben, K. (1987). "Algebraic multigrid"
//! - De Sterck, H., et al. (2006). "Reducing complexity in parallel algebraic multigrid preconditioners"
//! - hypre documentation: https://hypre.readthedocs.io/en/latest/solvers-boomeramg.html

use ndarray::Array1;
use num_complex::Complex64;

use super::preconditioner::Preconditioner;
use crate::core::assembly::sparse::CsrMatrix as BemCsrMatrix;

// Re-export configuration types from math-solvers
pub use solvers::{
    AmgCoarsening, AmgConfig, AmgCycle, AmgDiagnostics, AmgInterpolation, AmgSmoother,
};

// Import the generic AMG preconditioner and its Preconditioner trait
use solvers::{
    AmgPreconditioner as GenericAmgPreconditioner, CsrMatrix as SolverCsrMatrix,
    Preconditioner as SolversPreconditioner,
};

/// Algebraic Multigrid Preconditioner for BEM
///
/// This is a wrapper around the generic AMG implementation from math-solvers,
/// adapted for BEM's Complex64-specific matrix types.
#[derive(Debug, Clone)]
pub struct AmgPreconditioner {
    /// The underlying generic AMG preconditioner
    inner: GenericAmgPreconditioner<Complex64>,
}

impl AmgPreconditioner {
    /// Create AMG preconditioner from a BEM CSR matrix
    pub fn from_csr(matrix: &BemCsrMatrix, config: AmgConfig) -> Self {
        // Convert BEM CsrMatrix to solver CsrMatrix
        let solver_matrix = Self::convert_to_solver_csr(matrix);
        let inner = GenericAmgPreconditioner::from_csr(&solver_matrix, config);
        Self { inner }
    }

    /// Create AMG preconditioner from a dense matrix
    pub fn from_dense(matrix: &ndarray::Array2<Complex64>, config: AmgConfig) -> Self {
        let csr = BemCsrMatrix::from_dense(matrix, 1e-15);
        Self::from_csr(&csr, config)
    }

    /// Convert BEM's CsrMatrix to solver's CsrMatrix
    fn convert_to_solver_csr(bem_matrix: &BemCsrMatrix) -> SolverCsrMatrix<Complex64> {
        // Both CsrMatrix types have identical internal structure,
        // so we can directly construct the solver matrix from BEM's data
        SolverCsrMatrix::from_raw_parts(
            bem_matrix.num_rows,
            bem_matrix.num_cols,
            bem_matrix.row_ptrs.clone(),
            bem_matrix.col_indices.clone(),
            bem_matrix.values.clone(),
        )
    }

    /// Get number of levels in hierarchy
    pub fn num_levels(&self) -> usize {
        self.inner.num_levels()
    }

    /// Get setup time in milliseconds
    pub fn setup_time_ms(&self) -> f64 {
        self.inner.setup_time_ms()
    }

    /// Get grid complexity (sum of DOFs / fine DOFs)
    pub fn grid_complexity(&self) -> f64 {
        self.inner.grid_complexity()
    }

    /// Get operator complexity (sum of nnz / fine nnz)
    pub fn operator_complexity(&self) -> f64 {
        self.inner.operator_complexity()
    }

    /// Get configuration
    pub fn config(&self) -> &AmgConfig {
        self.inner.config()
    }

    /// Get diagnostic information
    pub fn diagnostics(&self) -> AmgDiagnostics {
        self.inner.diagnostics()
    }
}

impl Preconditioner for AmgPreconditioner {
    /// Apply AMG V-cycle: solve M*z ≈ r
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        SolversPreconditioner::apply(&self.inner, r)
    }
}

impl AmgPreconditioner {
    /// Configuration optimized for BEM systems
    ///
    /// BEM matrices are typically denser and less sparse than FEM,
    /// requiring adjusted thresholds.
    ///
    /// Returns an AmgConfig suitable for BEM problems.
    pub fn config_for_bem() -> AmgConfig {
        AmgConfig {
            strong_threshold: 0.5,           // Higher for denser BEM matrices
            coarsening: AmgCoarsening::Pmis, // Better parallel scalability
            smoother: AmgSmoother::L1Jacobi, // More robust for BEM
            max_interp_elements: 6,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    /// Create a simple 1D Laplacian matrix for testing
    fn create_1d_laplacian(n: usize) -> BemCsrMatrix {
        let mut triplets: Vec<(usize, usize, Complex64)> = Vec::new();

        for i in 0..n {
            // Diagonal
            triplets.push((i, i, Complex64::new(2.0, 0.0)));

            // Off-diagonals
            if i > 0 {
                triplets.push((i, i - 1, Complex64::new(-1.0, 0.0)));
            }
            if i < n - 1 {
                triplets.push((i, i + 1, Complex64::new(-1.0, 0.0)));
            }
        }

        BemCsrMatrix::from_triplets(n, n, triplets)
    }

    #[test]
    fn test_amg_creation() {
        let matrix = create_1d_laplacian(100);
        let config = AmgConfig::default();

        let amg = AmgPreconditioner::from_csr(&matrix, config);

        // Should have created a hierarchy
        assert!(amg.num_levels() >= 2);
        assert!(amg.grid_complexity() >= 1.0);
        assert!(amg.operator_complexity() >= 1.0);
    }

    #[test]
    fn test_amg_apply() {
        let matrix = create_1d_laplacian(50);
        let config = AmgConfig::default();
        let amg = AmgPreconditioner::from_csr(&matrix, config);

        // Create a test residual
        let r = Array1::from_vec((0..50).map(|i| Complex64::new(i as f64, 0.0)).collect());

        let z = amg.apply(&r);

        // Result should have same size
        assert_eq!(z.len(), r.len());

        // Result should be different from input (preconditioner did something)
        let diff: f64 = (&z - &r).iter().map(|x| x.norm()).sum();
        assert!(diff > 1e-10, "Preconditioner should modify the vector");
    }

    #[test]
    fn test_amg_pmis_coarsening() {
        let matrix = create_1d_laplacian(100);
        let config = AmgConfig {
            coarsening: AmgCoarsening::Pmis,
            ..Default::default()
        };

        let amg = AmgPreconditioner::from_csr(&matrix, config);

        // Should still work with PMIS
        assert!(amg.num_levels() >= 2);
    }

    #[test]
    fn test_amg_for_bem_config() {
        let matrix = create_1d_laplacian(50);
        let config = AmgPreconditioner::config_for_bem();
        let amg = AmgPreconditioner::from_csr(&matrix, config);

        let r = Array1::from_vec((0..50).map(|i| Complex64::new(i as f64, 0.0)).collect());
        let z = amg.apply(&r);
        assert_eq!(z.len(), r.len());
    }

    #[test]
    fn test_diagnostics() {
        let matrix = create_1d_laplacian(100);
        let amg = AmgPreconditioner::from_csr(&matrix, AmgConfig::default());

        let diag = amg.diagnostics();

        assert!(diag.num_levels >= 2);
        assert_eq!(diag.level_dofs.len(), diag.num_levels);
        assert_eq!(diag.level_nnz.len(), diag.num_levels);
        assert!(diag.grid_complexity >= 1.0);
        assert!(diag.setup_time_ms >= 0.0);
    }

    #[test]
    fn test_amg_from_dense() {
        let n = 20;
        let mut dense = Array2::zeros((n, n));

        // Create tridiagonal matrix
        for i in 0..n {
            dense[[i, i]] = Complex64::new(2.0, 0.0);
            if i > 0 {
                dense[[i, i - 1]] = Complex64::new(-1.0, 0.0);
            }
            if i < n - 1 {
                dense[[i, i + 1]] = Complex64::new(-1.0, 0.0);
            }
        }

        let amg = AmgPreconditioner::from_dense(&dense, AmgConfig::default());
        assert!(amg.num_levels() >= 1);
    }
}
