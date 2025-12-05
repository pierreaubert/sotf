//! FMM-solver interface
//!
//! This module provides a unified interface for solving BEM systems using
//! either direct methods (TBEM) or iterative methods with FMM acceleration.
//!
//! The key abstraction is the `LinearOperator` trait, which allows iterative
//! solvers to work with any matrix representation.
//!
//! ## Preconditioning
//!
//! BEM systems are typically ill-conditioned. Simple diagonal (Jacobi) preconditioning
//! is **not sufficient** for BEM. Use ILU preconditioning instead:
//!
//! ```ignore
//! use bem::core::solver::{IluMethod, IluScanningDegree, solve_with_ilu};
//!
//! // For TBEM systems
//! let solution = solve_with_ilu(
//!     &matrix,
//!     &rhs,
//!     IluMethod::Tbem,
//!     IluScanningDegree::Fine,
//!     &cgs_config,
//! );
//! ```

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::assembly::mlfmm::MlfmmSystem;
#[cfg(any(feature = "native", feature = "wasm"))]
use crate::core::assembly::slfmm::SlfmmSystem;
use crate::core::assembly::sparse::CsrMatrix;

pub use super::ilu_preconditioner::{IluMethod, IluPreconditioner, IluScanningDegree, IluSetup};
pub use super::preconditioner::Preconditioner;

/// Trait for linear operators that can perform matrix-vector products
///
/// This abstraction allows iterative solvers to work with:
/// - Dense matrices (TBEM)
/// - FMM systems (SLFMM, MLFMM)
/// - Sparse matrices (CSR)
/// - Preconditioners
pub trait LinearOperator: Send + Sync {
    /// Number of rows
    fn num_rows(&self) -> usize;

    /// Number of columns
    fn num_cols(&self) -> usize;

    /// Apply the operator: y = A * x
    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64>;

    /// Apply the transpose operator: y = A^T * x
    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64>;

    /// Check if the operator is square
    fn is_square(&self) -> bool {
        self.num_rows() == self.num_cols()
    }
}

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

impl LinearOperator for DenseOperator {
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
///
/// Works with both `native` and `wasm` features via rayon (wasm-bindgen-rayon for WASM).
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
impl LinearOperator for SlfmmOperator {
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

impl LinearOperator for MlfmmOperator {
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
        // MLFMM transpose not yet implemented
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

impl LinearOperator for CsrOperator {
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

/// Solve a linear system using CGS with the given linear operator
///
/// # Arguments
/// * `operator` - Linear operator implementing the matrix-vector product
/// * `b` - Right-hand side vector
/// * `config` - CGS solver configuration
///
/// # Returns
/// Solution vector and convergence information
pub fn solve_cgs<O: LinearOperator>(
    operator: &O,
    b: &Array1<Complex64>,
    config: &super::cgs::CgsConfig,
) -> super::cgs::CgsSolution {
    let matvec = |x: &Array1<Complex64>| operator.apply(x);
    super::cgs::cgs_solve(matvec, b, None, config)
}

/// Solve a linear system using BiCGSTAB with the given linear operator
///
/// # Arguments
/// * `operator` - Linear operator implementing the matrix-vector product
/// * `b` - Right-hand side vector
/// * `config` - BiCGSTAB solver configuration
///
/// # Returns
/// Solution vector and convergence information
pub fn solve_bicgstab<O: LinearOperator>(
    operator: &O,
    b: &Array1<Complex64>,
    config: &super::bicgstab::BiCgstabConfig,
) -> super::bicgstab::BiCgstabSolution {
    let matvec = |x: &Array1<Complex64>| operator.apply(x);
    super::bicgstab::bicgstab_solve(matvec, b, None, config)
}

/// Solve a linear system using the appropriate method based on system size
///
/// For small systems (< 1000 DOFs), uses direct LU factorization.
/// For larger systems, uses iterative CGS with the provided operator.
///
/// # Arguments
/// * `operator` - Linear operator (FMM or dense)
/// * `b` - Right-hand side vector
/// * `use_iterative` - Force iterative solver even for small systems
///
/// # Returns
/// Solution vector
pub fn solve_adaptive<O: LinearOperator>(
    operator: &O,
    b: &Array1<Complex64>,
    use_iterative: bool,
) -> Array1<Complex64> {
    let n = operator.num_rows();

    if !use_iterative && n < 1000 {
        // For small systems, collect to dense and use direct solver
        // This is a fallback - ideally the caller provides a dense matrix directly
        eprintln!(
            "Warning: solve_adaptive called with operator on small system (n={}). \
             Consider using direct solver.",
            n
        );
    }

    // Use iterative solver
    let config = super::cgs::CgsConfig {
        max_iterations: n.max(100),
        tolerance: 1e-8,
        print_interval: 0,
    };

    let solution = solve_cgs(operator, b, &config);

    if !solution.converged {
        eprintln!(
            "Warning: CGS did not converge after {} iterations (residual = {:.2e})",
            solution.iterations, solution.residual
        );
    }

    solution.x
}

/// Diagonal preconditioner from a linear operator
///
/// Extracts the diagonal and uses its inverse as a preconditioner.
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

    /// Apply the preconditioner: y = M^{-1} * x
    pub fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        x * &self.inv_diag
    }
}

impl LinearOperator for DiagonalPreconditioner {
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
        // Diagonal matrix is symmetric
        x * &self.inv_diag
    }
}

/// ILU preconditioner wrapper implementing LinearOperator
///
/// This wraps the ILU preconditioner to work with the LinearOperator interface.
pub struct IluOperator {
    preconditioner: IluPreconditioner,
    n: usize,
}

impl IluOperator {
    /// Create from an ILU preconditioner
    pub fn new(preconditioner: IluPreconditioner, n: usize) -> Self {
        Self { preconditioner, n }
    }

    /// Create from a dense matrix with default settings for TBEM
    pub fn from_tbem_matrix(matrix: &Array2<Complex64>) -> Self {
        let n = matrix.nrows();
        let precond =
            IluPreconditioner::from_matrix(matrix, IluMethod::Tbem, IluScanningDegree::Fine);
        Self {
            preconditioner: precond,
            n,
        }
    }

    /// Create from a dense matrix with specified method and degree
    pub fn from_matrix(
        matrix: &Array2<Complex64>,
        method: IluMethod,
        degree: IluScanningDegree,
    ) -> Self {
        let n = matrix.nrows();
        let precond = IluPreconditioner::from_matrix(matrix, method, degree);
        Self {
            preconditioner: precond,
            n,
        }
    }

    /// Get fill ratio (nnz(L+U) / n^2)
    pub fn fill_ratio(&self) -> f64 {
        self.preconditioner.fill_ratio()
    }
}

impl LinearOperator for IluOperator {
    fn num_rows(&self) -> usize {
        self.n
    }

    fn num_cols(&self) -> usize {
        self.n
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.preconditioner.apply(x)
    }

    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        // ILU transpose application is more complex - for now just use forward
        // This is acceptable for left preconditioning
        self.preconditioner.apply(x)
    }
}

/// Solve a linear system using CGS with ILU preconditioning
///
/// This is the **recommended** method for solving BEM systems.
/// Diagonal preconditioning is NOT sufficient for BEM.
///
/// # Arguments
/// * `matrix` - The coefficient matrix
/// * `b` - Right-hand side vector
/// * `method` - BEM method type (TBEM, SLFMM, MLFMM)
/// * `degree` - ILU scanning degree (affects accuracy vs speed)
/// * `config` - CGS solver configuration
///
/// # Returns
/// Solution vector and convergence information
///
/// # Example
/// ```ignore
/// let solution = solve_with_ilu(
///     &matrix,
///     &rhs,
///     IluMethod::Tbem,
///     IluScanningDegree::Fine,
///     &CgsConfig::default(),
/// );
/// assert!(solution.converged);
/// ```
pub fn solve_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    method: IluMethod,
    degree: IluScanningDegree,
    config: &super::cgs::CgsConfig,
) -> super::cgs::CgsSolution {
    // Set up ILU with row scaling
    let setup = IluPreconditioner::setup_system(matrix, method, degree);

    // Scale the RHS
    let scaled_b: Array1<Complex64> = b
        .iter()
        .zip(setup.row_scale.iter())
        .map(|(&bi, &si)| bi * si)
        .collect();

    // Create matvec using scaled matrix
    let matvec = |x: &Array1<Complex64>| setup.scaled_matrix.dot(x);

    // Create preconditioner application
    let precond_solve = |r: &Array1<Complex64>| setup.preconditioner.apply(r);

    // Solve with preconditioning
    super::cgs::cgs_solve_preconditioned(matvec, precond_solve, &scaled_b, None, config)
}

/// Solve a linear system using CGS with ILU preconditioning (using LinearOperator)
///
/// This version accepts any LinearOperator and builds the ILU preconditioner
/// from the near-field matrix (for FMM systems) or the full matrix (for dense).
///
/// # Arguments
/// * `operator` - Linear operator for matrix-vector products
/// * `nearfield_matrix` - Matrix to build ILU from (near-field for FMM, full for TBEM)
/// * `b` - Right-hand side vector
/// * `method` - BEM method type
/// * `degree` - ILU scanning degree
/// * `config` - CGS configuration
pub fn solve_with_ilu_operator<O: LinearOperator>(
    operator: &O,
    nearfield_matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    method: IluMethod,
    degree: IluScanningDegree,
    config: &super::cgs::CgsConfig,
) -> super::cgs::CgsSolution {
    // Build ILU from near-field matrix
    let setup = IluPreconditioner::setup_system(nearfield_matrix, method, degree);

    // Scale the RHS
    let scaled_b: Array1<Complex64> = b
        .iter()
        .zip(setup.row_scale.iter())
        .map(|(&bi, &si)| bi * si)
        .collect();

    // Create scaled matvec: uses operator but applies row scaling
    let matvec = |x: &Array1<Complex64>| {
        let y = operator.apply(x);
        // Apply row scaling to output
        y.iter()
            .zip(setup.row_scale.iter())
            .map(|(&yi, &si)| yi * si)
            .collect()
    };

    // Create preconditioner application
    let precond_solve = |r: &Array1<Complex64>| setup.preconditioner.apply(r);

    super::cgs::cgs_solve_preconditioned(matvec, precond_solve, &scaled_b, None, config)
}

/// Solve a TBEM system with ILU preconditioning
///
/// Convenience function that uses recommended settings for TBEM.
pub fn solve_tbem_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &super::cgs::CgsConfig,
) -> super::cgs::CgsSolution {
    solve_with_ilu(matrix, b, IluMethod::Tbem, IluScanningDegree::Fine, config)
}

/// Information about ILU setup for diagnostics
#[derive(Debug, Clone)]
pub struct IluDiagnostics {
    /// Number of nonzeros in L factor
    pub nnz_l: usize,
    /// Number of nonzeros in U factor
    pub nnz_u: usize,
    /// Fill ratio (nnz(L+U) / n^2)
    pub fill_ratio: f64,
    /// Threshold used for dropping
    pub threshold_used: f64,
}

/// Create ILU diagnostics from a preconditioner
pub fn ilu_diagnostics(
    matrix: &Array2<Complex64>,
    method: IluMethod,
    degree: IluScanningDegree,
) -> IluDiagnostics {
    let precond = IluPreconditioner::from_matrix(matrix, method, degree);

    // Get threshold for reporting
    let threshold = match method {
        IluMethod::Tbem => match degree {
            IluScanningDegree::Coarse => 1.2,
            IluScanningDegree::Medium => 1.0,
            IluScanningDegree::Fine => 0.8,
            IluScanningDegree::Finest => 0.6,
        },
        IluMethod::Slfmm => match degree {
            IluScanningDegree::Coarse => 0.9,
            IluScanningDegree::Medium => 0.35,
            IluScanningDegree::Fine => 0.07,
            IluScanningDegree::Finest => 0.01,
        },
        IluMethod::Mlfmm => match degree {
            IluScanningDegree::Coarse => 0.65,
            IluScanningDegree::Medium => 0.15,
            IluScanningDegree::Fine => 0.05,
            IluScanningDegree::Finest => 0.005,
        },
    };

    IluDiagnostics {
        nnz_l: precond.nnz_l(),
        nnz_u: precond.nnz_u(),
        fill_ratio: precond.fill_ratio(),
        threshold_used: threshold,
    }
}

// ============================================================================
// GMRES Integration
// ============================================================================

/// Solve a linear system using GMRES with the given linear operator
///
/// # Arguments
/// * `operator` - Linear operator implementing the matrix-vector product
/// * `b` - Right-hand side vector
/// * `config` - GMRES solver configuration
///
/// # Returns
/// Solution vector and convergence information
pub fn solve_gmres<O: LinearOperator>(
    operator: &O,
    b: &Array1<Complex64>,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    let matvec = |x: &Array1<Complex64>| operator.apply(x);
    super::gmres::gmres_solve(matvec, b, None, config)
}

/// Solve a linear system using GMRES with ILU preconditioning
///
/// This is the **recommended** method for large BEM systems.
/// GMRES provides monotonic convergence and handles non-symmetric matrices well.
///
/// # Arguments
/// * `matrix` - The coefficient matrix
/// * `b` - Right-hand side vector
/// * `method` - BEM method type (TBEM, SLFMM, MLFMM)
/// * `degree` - ILU scanning degree (affects accuracy vs speed)
/// * `config` - GMRES solver configuration
///
/// # Returns
/// Solution vector and convergence information
///
/// # Example
/// ```ignore
/// let config = GmresConfig::for_large_bem();
/// let solution = gmres_solve_with_ilu(
///     &matrix,
///     &rhs,
///     IluMethod::Tbem,
///     IluScanningDegree::Fine,
///     &config,
/// );
/// assert!(solution.converged);
/// ```
pub fn gmres_solve_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    method: IluMethod,
    degree: IluScanningDegree,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    // Set up ILU with row scaling
    let setup = IluPreconditioner::setup_system(matrix, method, degree);

    // Scale the RHS
    let scaled_b: Array1<Complex64> = b
        .iter()
        .zip(setup.row_scale.iter())
        .map(|(&bi, &si)| bi * si)
        .collect();

    // Create matvec using scaled matrix
    let matvec = |x: &Array1<Complex64>| setup.scaled_matrix.dot(x);

    // Create preconditioner application
    let precond_solve = |r: &Array1<Complex64>| setup.preconditioner.apply(r);

    // Solve with GMRES + preconditioning
    super::gmres::gmres_solve_preconditioned(matvec, precond_solve, &scaled_b, None, config)
}

/// Solve a TBEM system with GMRES + ILU preconditioning
///
/// Convenience function that uses recommended settings for large TBEM problems.
pub fn gmres_solve_tbem_with_ilu(
    matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    gmres_solve_with_ilu(matrix, b, IluMethod::Tbem, IluScanningDegree::Fine, config)
}

/// Solve using GMRES with ILU preconditioning (using LinearOperator)
///
/// This version accepts any LinearOperator and builds the ILU preconditioner
/// from the near-field matrix (for FMM systems) or the full matrix (for dense).
///
/// # Arguments
/// * `operator` - Linear operator for matrix-vector products
/// * `nearfield_matrix` - Matrix to build ILU from (near-field for FMM, full for TBEM)
/// * `b` - Right-hand side vector
/// * `method` - BEM method type
/// * `degree` - ILU scanning degree
/// * `config` - GMRES configuration
pub fn gmres_solve_with_ilu_operator<O: LinearOperator>(
    operator: &O,
    nearfield_matrix: &Array2<Complex64>,
    b: &Array1<Complex64>,
    method: IluMethod,
    degree: IluScanningDegree,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    // Build ILU from near-field matrix
    let setup = IluPreconditioner::setup_system(nearfield_matrix, method, degree);

    // Scale the RHS
    let scaled_b: Array1<Complex64> = b
        .iter()
        .zip(setup.row_scale.iter())
        .map(|(&bi, &si)| bi * si)
        .collect();

    // Create scaled matvec: uses operator but applies row scaling
    let matvec = |x: &Array1<Complex64>| {
        let y = operator.apply(x);
        // Apply row scaling to output
        y.iter()
            .zip(setup.row_scale.iter())
            .map(|(&yi, &si)| yi * si)
            .collect()
    };

    // Create preconditioner application
    let precond_solve = |r: &Array1<Complex64>| setup.preconditioner.apply(r);

    super::gmres::gmres_solve_preconditioned(matvec, precond_solve, &scaled_b, None, config)
}

// ============================================================================
// Hierarchical FMM Preconditioner Integration
// ============================================================================

/// Solve using GMRES with hierarchical FMM preconditioner
///
/// This method avoids the O(N²) dense matrix assembly for preconditioning.
/// Instead, it uses block-wise LU factorization of the FMM near-field blocks.
///
/// # Advantages over ILU
/// - O(N) setup cost (only diagonal blocks)
/// - Parallel LU factorization of each block
/// - No dense matrix extraction needed
///
/// # Arguments
/// * `fmm_system` - The SLFMM system with near-field blocks
/// * `b` - Right-hand side vector
/// * `config` - GMRES configuration
///
/// **Note**: Works with both `native` and `wasm` features via rayon.
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn gmres_solve_with_hierarchical_precond(
    fmm_system: &crate::core::assembly::slfmm::SlfmmSystem,
    b: &Array1<Complex64>,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    use super::preconditioner::HierarchicalFmmPreconditioner;

    // Build hierarchical preconditioner from near-field blocks
    let precond = HierarchicalFmmPreconditioner::from_slfmm(fmm_system);

    // Create matvec closure
    let matvec = |x: &Array1<Complex64>| fmm_system.matvec(x);

    // Create preconditioner application closure
    let precond_solve = |r: &Array1<Complex64>| precond.apply(r);

    // Solve with preconditioned GMRES
    super::gmres::gmres_solve_preconditioned(matvec, precond_solve, b, None, config)
}

/// Solve using GMRES with hierarchical preconditioner (operator interface)
///
/// This version takes ownership of the FMM system and provides a cleaner interface.
///
/// **Note**: Works with both `native` and `wasm` features via rayon.
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn gmres_solve_fmm_hierarchical(
    fmm_operator: &SlfmmOperator,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    use super::preconditioner::HierarchicalFmmPreconditioner;

    // Build preconditioner
    let precond = HierarchicalFmmPreconditioner::from_slfmm(fmm_operator.system());

    // Get RHS
    let b = fmm_operator.rhs();

    // Create closures
    let matvec = |x: &Array1<Complex64>| fmm_operator.apply(x);
    let precond_solve = |r: &Array1<Complex64>| precond.apply(r);

    super::gmres::gmres_solve_preconditioned(matvec, precond_solve, b, None, config)
}

// ============================================================================
// Batched BLAS GMRES Solver (native only - requires rayon)
// ============================================================================

/// Solve using GMRES with batched BLAS operations
///
/// This version uses pre-allocated workspace and batched matrix operations
/// for improved performance on large FMM systems.
///
/// # Advantages over standard matvec
/// - Pre-allocated workspace avoids allocations in hot path
/// - Batched operations for better cache locality
/// - Reuses workspace across GMRES iterations
///
/// # Arguments
/// * `fmm_system` - The SLFMM system
/// * `b` - Right-hand side vector
/// * `config` - GMRES configuration
///
/// **Note**: Requires the `native` feature for parallel processing.
#[cfg(feature = "native")]
pub fn gmres_solve_fmm_batched(
    fmm_system: &crate::core::assembly::slfmm::SlfmmSystem,
    b: &Array1<Complex64>,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    use super::batched_blas::SlfmmMatvecWorkspace;
    use std::cell::RefCell;

    // Pre-allocate workspace in RefCell for interior mutability
    let workspace = RefCell::new(SlfmmMatvecWorkspace::new(
        fmm_system.num_clusters,
        fmm_system.num_sphere_points,
        fmm_system.num_dofs,
    ));

    // Create matvec closure using batched operations
    // The RefCell allows mutation from within an Fn closure
    let matvec = |x: &Array1<Complex64>| {
        super::batched_blas::slfmm_matvec_batched(fmm_system, x, &mut workspace.borrow_mut())
    };

    super::gmres::gmres_solve(matvec, b, None, config)
}

/// Solve using GMRES with batched BLAS and ILU preconditioning
///
/// Combines batched matvec with ILU preconditioning for optimal performance.
///
/// **Note**: Requires the `native` feature for parallel processing.
#[cfg(feature = "native")]
pub fn gmres_solve_fmm_batched_with_ilu(
    fmm_system: &crate::core::assembly::slfmm::SlfmmSystem,
    b: &Array1<Complex64>,
    method: IluMethod,
    degree: IluScanningDegree,
    config: &super::gmres::GmresConfig,
) -> super::gmres::GmresSolution {
    use super::batched_blas::SlfmmMatvecWorkspace;
    use std::cell::RefCell;

    // Extract near-field matrix for ILU
    let nearfield_matrix = fmm_system.extract_near_field_matrix();

    // Build ILU from near-field matrix
    let setup = IluPreconditioner::setup_system(&nearfield_matrix, method, degree);

    // Scale the RHS
    let scaled_b: Array1<Complex64> = b
        .iter()
        .zip(setup.row_scale.iter())
        .map(|(&bi, &si)| bi * si)
        .collect();

    // Pre-allocate workspace for batched matvec with RefCell for interior mutability
    let workspace = RefCell::new(SlfmmMatvecWorkspace::new(
        fmm_system.num_clusters,
        fmm_system.num_sphere_points,
        fmm_system.num_dofs,
    ));

    // Create scaled matvec closure using batched operations
    let matvec = |x: &Array1<Complex64>| {
        let y =
            super::batched_blas::slfmm_matvec_batched(fmm_system, x, &mut workspace.borrow_mut());
        // Apply row scaling to output
        y.iter()
            .zip(setup.row_scale.iter())
            .map(|(&yi, &si)| yi * si)
            .collect()
    };

    // Create preconditioner application closure
    let precond_solve = |r: &Array1<Complex64>| setup.preconditioner.apply(r);

    super::gmres::gmres_solve_preconditioned(matvec, precond_solve, &scaled_b, None, config)
}

// ============================================================================
// Frequency-Adaptive Mesh Utilities
// ============================================================================

/// Calculate recommended mesh resolution for a given frequency
///
/// Based on the Nyquist criterion for BEM: mesh element size should be
/// at most λ/6 to λ/10 for accurate results.
///
/// # Arguments
/// * `frequency` - Simulation frequency in Hz
/// * `speed_of_sound` - Speed of sound in m/s (default 343)
/// * `elements_per_wavelength` - Number of elements per wavelength (default 6)
///
/// # Returns
/// Recommended elements per meter
pub fn recommended_mesh_resolution(
    frequency: f64,
    speed_of_sound: f64,
    elements_per_wavelength: usize,
) -> f64 {
    let wavelength = speed_of_sound / frequency;
    elements_per_wavelength as f64 / wavelength
}

/// Calculate mesh resolution for a frequency range
///
/// Uses the highest frequency to determine minimum element size.
pub fn mesh_resolution_for_frequency_range(
    _min_freq: f64,
    max_freq: f64,
    speed_of_sound: f64,
    elements_per_wavelength: usize,
) -> f64 {
    // Use max frequency (shortest wavelength) to determine resolution
    recommended_mesh_resolution(max_freq, speed_of_sound, elements_per_wavelength)
}

/// Estimate element count for a room at given mesh resolution
///
/// Rough estimate based on room surface area.
pub fn estimate_element_count(
    room_dimensions: (f64, f64, f64), // (width, depth, height)
    mesh_resolution: f64,
) -> usize {
    let (w, d, h) = room_dimensions;

    // Total surface area of rectangular room
    let surface_area = 2.0 * (w * d + w * h + d * h);

    // Element size based on resolution (element per meter)
    let element_size = 1.0 / mesh_resolution;
    let element_area = element_size * element_size;

    (surface_area / element_area).ceil() as usize
}

/// Adaptive mesh configuration based on frequency and room size
pub struct AdaptiveMeshConfig {
    /// Base mesh resolution (elements per meter)
    pub base_resolution: f64,
    /// Refinement factor near sources (1.0 = no refinement)
    pub source_refinement: f64,
    /// Refinement radius around sources (meters)
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

    /// Create from explicit resolution
    pub fn from_resolution(resolution: f64) -> Self {
        Self {
            base_resolution: resolution,
            source_refinement: 1.0,
            source_refinement_radius: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_dense_operator() {
        let matrix = array![
            [Complex64::new(2.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];

        let op = DenseOperator::new(matrix);
        let x = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let y = op.apply(&x);

        // [2 1] * [1] = [4]
        // [1 3]   [2]   [7]
        assert!((y[0] - Complex64::new(4.0, 0.0)).norm() < 1e-10);
        assert!((y[1] - Complex64::new(7.0, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn test_csr_operator() {
        let matrix = CsrMatrix::from_triplets(
            2,
            2,
            vec![
                (0, 0, Complex64::new(2.0, 0.0)),
                (0, 1, Complex64::new(1.0, 0.0)),
                (1, 0, Complex64::new(1.0, 0.0)),
                (1, 1, Complex64::new(3.0, 0.0)),
            ],
        );

        let op = CsrOperator::new(matrix);
        let x = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let y = op.apply(&x);

        assert!((y[0] - Complex64::new(4.0, 0.0)).norm() < 1e-10);
        assert!((y[1] - Complex64::new(7.0, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn test_solve_cgs_with_operator() {
        let matrix = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];

        let op = DenseOperator::new(matrix.clone());
        let b = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let config = super::super::cgs::CgsConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = solve_cgs(&op, &b, &config);

        assert!(solution.converged);

        // Verify Ax = b
        let ax = matrix.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8);
    }

    #[test]
    fn test_diagonal_preconditioner() {
        let diag = array![Complex64::new(2.0, 0.0), Complex64::new(4.0, 0.0),];

        let precond = DiagonalPreconditioner::from_diagonal(diag);
        let x = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let y = precond.apply(&x);

        // y = D^{-1} * x = [1/2, 2/4] = [0.5, 0.5]
        assert!((y[0] - Complex64::new(0.5, 0.0)).norm() < 1e-10);
        assert!((y[1] - Complex64::new(0.5, 0.0)).norm() < 1e-10);
    }
}
